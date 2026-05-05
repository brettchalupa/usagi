//! `usagi refresh`: re-sync engine-managed files (`USAGI.md`,
//! `meta/usagi.lua`, `.luarc.json`) from the running engine version.
//!
//! Interactive by default — for each file that would change, prompts
//! `[y/N/a/q]` (yes / no / yes-to-all / quit). Pass `--yes` to
//! overwrite without prompting; `--dry-run` to preview without
//! writing. Never touches `main.lua` or `.gitignore` — those are
//! owned by the user once `usagi init` lays them down. To inspect
//! what would change before accepting, use `git diff` after running
//! with `--yes` (or revert via `git restore`).

use crate::init::{self, TemplateFile};
use crate::{Error, Result, msg};
use std::fs;
use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

pub fn run(path: &str, yes: bool, dry_run: bool) -> Result<()> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(Error::Cli(format!(
            "{} is not a directory; pass a project root.",
            dir.display()
        )));
    }
    if !yes && !dry_run && !std::io::stdin().is_terminal() {
        return Err(Error::Cli(
            "stdin is not a terminal; pass --yes to overwrite or --dry-run to preview.".into(),
        ));
    }

    let files: Vec<TemplateFile> = init::template_files()
        .into_iter()
        .filter(|t| t.engine_managed)
        .collect();

    let mut tally = Tally::default();
    let mut yes_to_all = yes;
    let stdin = std::io::stdin();
    let mut prompter = LinePrompter::new(stdin.lock());

    'outer: for tf in &files {
        let target = dir.join(tf.rel);
        match classify(&target, &tf.contents)? {
            Action::Identical => {
                msg::info!("{}: identical", tf.rel);
                tally.unchanged += 1;
            }
            Action::Missing => {
                if dry_run {
                    msg::info!("{}: would create", tf.rel);
                    tally.pending += 1;
                    continue;
                }
                let approve = if yes_to_all {
                    true
                } else {
                    match prompter.ask(&format!("create {}? [y/N/a/q]: ", tf.rel))? {
                        Decision::Yes => true,
                        Decision::No => false,
                        Decision::All => {
                            yes_to_all = true;
                            true
                        }
                        Decision::Quit => break 'outer,
                    }
                };
                if approve {
                    write_file(&target, &tf.contents)?;
                    msg::info!("created {}", tf.rel);
                    tally.created += 1;
                } else {
                    tally.skipped += 1;
                }
            }
            Action::Differs(_) => {
                if dry_run {
                    msg::info!("{}: would overwrite", tf.rel);
                    tally.pending += 1;
                    continue;
                }
                let approve = if yes_to_all {
                    true
                } else {
                    match prompter.ask(&format!("overwrite {}? [y/N/a/q]: ", tf.rel))? {
                        Decision::Yes => true,
                        Decision::No => false,
                        Decision::All => {
                            yes_to_all = true;
                            true
                        }
                        Decision::Quit => break 'outer,
                    }
                };
                if approve {
                    write_file(&target, &tf.contents)?;
                    msg::info!("wrote {}", tf.rel);
                    tally.written += 1;
                } else {
                    tally.skipped += 1;
                }
            }
        }
    }

    if dry_run {
        msg::info!(
            "dry-run: {} would change, {} unchanged",
            tally.pending,
            tally.unchanged
        );
    } else {
        msg::info!(
            "{} updated, {} created, {} unchanged, {} skipped",
            tally.written,
            tally.created,
            tally.unchanged,
            tally.skipped
        );
    }
    Ok(())
}

#[derive(Default)]
struct Tally {
    written: usize,
    created: usize,
    unchanged: usize,
    skipped: usize,
    pending: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Identical,
    Missing,
    Differs(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Decision {
    Yes,
    No,
    All,
    Quit,
}

fn classify(target: &Path, new_contents: &str) -> Result<Action> {
    if !target.exists() {
        return Ok(Action::Missing);
    }
    let existing = fs::read_to_string(target)
        .map_err(|e| Error::Cli(format!("reading {}: {e}", target.display())))?;
    if existing == new_contents {
        Ok(Action::Identical)
    } else {
        Ok(Action::Differs(existing))
    }
}

fn write_file(target: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| Error::Cli(format!("creating {}: {e}", parent.display())))?;
    }
    fs::write(target, contents)
        .map_err(|e| Error::Cli(format!("writing {}: {e}", target.display())))
}

/// Decision parser split off from the IO side so prompt logic is
/// unit-testable without mocking stdin.
fn parse_decision(line: &str) -> Option<Decision> {
    match line.trim() {
        "y" | "Y" | "yes" => Some(Decision::Yes),
        "" | "n" | "N" | "no" => Some(Decision::No),
        "a" | "A" | "all" => Some(Decision::All),
        "q" | "Q" | "quit" => Some(Decision::Quit),
        _ => None,
    }
}

struct LinePrompter<R: BufRead> {
    reader: R,
    buf: String,
}

impl<R: BufRead> LinePrompter<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            buf: String::new(),
        }
    }

    fn ask(&mut self, prompt: &str) -> Result<Decision> {
        loop {
            let mut out = std::io::stdout().lock();
            let _ = write!(out, "{prompt}");
            let _ = out.flush();
            drop(out);
            self.buf.clear();
            let n = self
                .reader
                .read_line(&mut self.buf)
                .map_err(|e| Error::Cli(format!("reading prompt response: {e}")))?;
            if n == 0 {
                // EOF on stdin — treat as quit so we don't loop forever.
                return Ok(Decision::Quit);
            }
            if let Some(d) = parse_decision(&self.buf) {
                return Ok(d);
            }
            // Unknown input: re-prompt.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn classifies_missing_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("nope.txt");
        assert_eq!(classify(&p, "anything").unwrap(), Action::Missing);
    }

    #[test]
    fn classifies_identical_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("same.txt");
        fs::write(&p, "hello\n").unwrap();
        assert_eq!(classify(&p, "hello\n").unwrap(), Action::Identical);
    }

    #[test]
    fn classifies_differing_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("diff.txt");
        fs::write(&p, "old\n").unwrap();
        match classify(&p, "new\n").unwrap() {
            Action::Differs(prev) => assert_eq!(prev, "old\n"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parse_decision_accepts_common_inputs() {
        assert_eq!(parse_decision("y\n"), Some(Decision::Yes));
        assert_eq!(parse_decision("Y"), Some(Decision::Yes));
        assert_eq!(parse_decision("yes"), Some(Decision::Yes));
        assert_eq!(parse_decision("\n"), Some(Decision::No));
        assert_eq!(parse_decision("n"), Some(Decision::No));
        assert_eq!(parse_decision("a"), Some(Decision::All));
        assert_eq!(parse_decision("q"), Some(Decision::Quit));
    }

    #[test]
    fn parse_decision_rejects_garbage() {
        assert_eq!(parse_decision("maybe"), None);
        assert_eq!(parse_decision("yy"), None);
    }

    #[test]
    fn errors_when_target_is_not_a_directory() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("not-a-dir");
        fs::write(&file, b"x").unwrap();
        let err = run(file.to_str().unwrap(), true, false).unwrap_err();
        assert!(format!("{err}").contains("not a directory"));
    }

    #[test]
    fn dry_run_writes_nothing_on_clean_project() {
        let dir = tempdir().unwrap();
        crate::init::run(dir.path().to_str().unwrap()).unwrap();
        let before: Vec<_> = init::template_files()
            .into_iter()
            .filter(|t| t.engine_managed)
            .map(|t| (t.rel, fs::read_to_string(dir.path().join(t.rel)).unwrap()))
            .collect();
        run(dir.path().to_str().unwrap(), false, true).unwrap();
        for (rel, prev) in before {
            let now = fs::read_to_string(dir.path().join(rel)).unwrap();
            assert_eq!(now, prev, "{rel} changed during dry-run");
        }
    }

    #[test]
    fn yes_overwrites_diverged_engine_files_but_leaves_user_files() {
        let dir = tempdir().unwrap();
        crate::init::run(dir.path().to_str().unwrap()).unwrap();
        // Diverge an engine-managed file and a user file.
        let usagi_md = dir.path().join("USAGI.md");
        let main_lua = dir.path().join("main.lua");
        fs::write(&usagi_md, "stale\n").unwrap();
        fs::write(&main_lua, "-- mine\n").unwrap();
        run(dir.path().to_str().unwrap(), true, false).unwrap();
        // Engine-managed file restored.
        assert_ne!(fs::read_to_string(&usagi_md).unwrap(), "stale\n");
        assert!(
            fs::read_to_string(&usagi_md)
                .unwrap()
                .starts_with("<!-- Generated by usagi")
        );
        // User file untouched.
        assert_eq!(fs::read_to_string(&main_lua).unwrap(), "-- mine\n");
    }

    #[test]
    fn yes_creates_missing_engine_files() {
        let dir = tempdir().unwrap();
        // Project with no engine files.
        fs::write(dir.path().join("main.lua"), "-- mine\n").unwrap();
        run(dir.path().to_str().unwrap(), true, false).unwrap();
        assert!(dir.path().join("USAGI.md").is_file());
        assert!(dir.path().join("meta/usagi.lua").is_file());
        assert!(dir.path().join(".luarc.json").is_file());
    }

    #[test]
    fn yes_does_not_create_user_managed_files() {
        let dir = tempdir().unwrap();
        run(dir.path().to_str().unwrap(), true, false).unwrap();
        assert!(!dir.path().join("main.lua").exists());
        assert!(!dir.path().join(".gitignore").exists());
    }
}
