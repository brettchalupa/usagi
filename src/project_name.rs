//! Display + filesystem-safe rendering of a game's project name.
//! `display()` is the verbatim user string for the window title, macOS
//! `.app/` directory, and Info.plist *Name keys. `slug()` is the ASCII
//! kebab-case form for archive filenames, exe names, and the bundle
//! binary. Source of truth is `_config().name`, with the project
//! directory name (passed by the caller) as fallback.

const FALLBACK_DISPLAY: &str = "Usagi";
const FALLBACK_SLUG: &str = "game";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectName {
    display: String,
    slug: String,
}

impl ProjectName {
    /// Order: explicit `_config().name` → path-derived hint → engine defaults.
    pub fn resolve(config_name: Option<&str>, fallback_hint: Option<&str>) -> Self {
        let configured = config_name.map(str::trim).filter(|s| !s.is_empty());
        let hint = fallback_hint.map(str::trim).filter(|s| !s.is_empty());

        let display = configured
            .or(hint)
            .map(String::from)
            .unwrap_or_else(|| FALLBACK_DISPLAY.to_string());

        // Slug from the display first; if that has nothing slug-able
        // (all non-ASCII, all punctuation), fall through to the hint,
        // then to FALLBACK_SLUG.
        let slug = slugify(&display)
            .or_else(|| hint.and_then(slugify))
            .unwrap_or_else(|| FALLBACK_SLUG.to_string());

        Self { display, slug }
    }

    pub fn display(&self) -> &str {
        &self.display
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }
}

/// Lowercase ASCII kebab-case. Non-alphanumeric and non-ASCII chars
/// are separators; runs collapse; ends are trimmed. None when nothing
/// alphanumeric survives.
fn slugify(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = true;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            for c in ch.to_lowercase() {
                out.push(c);
            }
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_config_name_over_fallback() {
        let p = ProjectName::resolve(Some("Sprite Example"), Some("spr"));
        assert_eq!(p.display(), "Sprite Example");
        assert_eq!(p.slug(), "sprite-example");
    }

    #[test]
    fn resolve_uses_fallback_when_config_name_missing() {
        let p = ProjectName::resolve(None, Some("spr"));
        assert_eq!(p.display(), "spr");
        assert_eq!(p.slug(), "spr");
    }

    #[test]
    fn resolve_treats_blank_config_name_as_unset() {
        let p = ProjectName::resolve(Some("   "), Some("spr"));
        assert_eq!(p.display(), "spr");
    }

    #[test]
    fn resolve_falls_back_to_engine_default_when_neither_source_present() {
        // No config name and no path hint → default display, slug derived
        // from that default. (FALLBACK_SLUG only kicks in when the display
        // can't be slugged at all, e.g. all non-ASCII.)
        let p = ProjectName::resolve(None, None);
        assert_eq!(p.display(), FALLBACK_DISPLAY);
        assert_eq!(p.slug(), "usagi");
    }

    #[test]
    fn slug_drops_punctuation_and_collapses_separators() {
        assert_eq!(
            slugify("Sprite  Example!").as_deref(),
            Some("sprite-example")
        );
        assert_eq!(slugify("My_Game-2").as_deref(), Some("my-game-2"));
        assert_eq!(
            slugify("--leading--trailing--").as_deref(),
            Some("leading-trailing")
        );
    }

    #[test]
    fn slug_drops_non_ascii() {
        assert_eq!(slugify("café").as_deref(), Some("caf"));
        assert_eq!(slugify("日本語").as_deref(), None);
    }

    #[test]
    fn slug_returns_none_for_empty_or_punctuation_only_input() {
        assert_eq!(slugify(""), None);
        assert_eq!(slugify("!!!"), None);
        assert_eq!(slugify("---"), None);
    }

    #[test]
    fn resolve_uses_fallback_slug_when_configured_name_has_no_slugable_chars() {
        // Display keeps the user's pretty (if unhelpful) name, but the
        // slug falls through to something the filesystem can use.
        let p = ProjectName::resolve(Some("日本語"), Some("spr"));
        assert_eq!(p.display(), "日本語");
        assert_eq!(p.slug(), "spr");
    }

    #[test]
    fn resolve_falls_back_to_game_when_nothing_slugable_anywhere() {
        let p = ProjectName::resolve(Some("!!!"), Some("???"));
        assert_eq!(p.slug(), FALLBACK_SLUG);
    }
}
