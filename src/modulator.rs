//! Amplitude modulators (envelopes) for the audio-thread callback mixer.
//!
//! M8-style envelope shapes evaluated **per sample inside the audio
//! callback** (see ADR 0002). A modulator is pure DSP: it owns no heap,
//! takes no locks, and never panics, so it is safe to tick on the audio
//! thread. One [`Envelope`] lives per voice; the callback calls
//! [`Envelope::tick`] once per output frame to get the voice's current
//! amplitude gain in `0.0..=1.0`, then multiplies the oscillator sample by
//! it.
//!
//! Three shapes, amplitude-only for now (M8 also modulates pitch/filter;
//! that is future work):
//!
//! - **AHD** — Attack, Hold, Decay to zero. A fully *timed* one-shot
//!   envelope: it runs to completion on its own, ignoring how long the
//!   note is held. The natural fit for fire-and-forget sfx (jump, shoot).
//! - **ADSR** — Attack, Decay to a Sustain level, hold while the note is
//!   gated, then Release. The note-off (gate drop) drives the release, so
//!   this is the shape for sustained / looping voices.
//! - **DRUM** — a percussive one-shot: near-instant attack to peak, then a
//!   curved (fast-then-slow) decay to zero. Like AHD but punchier; ignores
//!   gate length.
//!
//! All shapes also honor an early gate drop: releasing the note before the
//! envelope finishes cuts to the release stage from the current level, so
//! a stopped voice fades out instead of clicking.
//!
//! Times are authored in milliseconds (the Lua-facing unit) and converted
//! to whole sample counts at construction against [`SAMPLE_RATE`], so
//! `tick` does only integer compares and a couple of multiplies.

use crate::synth::SAMPLE_RATE;

/// Envelope shape selector. Integer reprs mirror the Lua-side constants
/// (`AHD=0`, `ADSR=1`, `DRUM=2`), matching the engine's enum idiom
/// (`gfx.COLOR_*`, `sfx.SINE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModShape {
    Ahd,
    Adsr,
    Drum,
}

impl ModShape {
    /// Maps the Lua-side integer constant to a variant. Out-of-range
    /// values fall back to `Ahd` (a safe self-terminating default),
    /// matching the engine's forgiving "unknown value no-ops" stance.
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => ModShape::Adsr,
            2 => ModShape::Drum,
            _ => ModShape::Ahd,
        }
    }
}

/// Which segment of the envelope a voice is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Attack,
    Hold,
    Decay,
    /// ADSR only: holds `sustain` until the gate drops.
    Sustain,
    Release,
    Done,
}

/// A per-voice amplitude envelope. Construct once at note-on, then call
/// [`tick`](Envelope::tick) once per output sample. `Copy` and entirely
/// stack-resident: a voice array can hold these inline with no allocation.
///
/// Stage durations are stored as whole sample counts; `0` means an instant
/// transition (no divide-by-zero in `tick`).
#[derive(Debug, Clone, Copy)]
pub struct Envelope {
    shape: ModShape,
    attack: u32,
    hold: u32,
    decay: u32,
    /// Sustain level in `0.0..=1.0` (ADSR only; ignored otherwise).
    sustain: f32,
    release: u32,

    stage: Stage,
    /// Samples elapsed in the current stage.
    t: u32,
    /// Gain returned by the previous tick. Used as the starting level for
    /// an early release so the fade-out begins exactly where the voice was.
    level: f32,
    /// Level captured when Release was entered, so release lerps from the
    /// actual current gain (which differs if release came mid-attack).
    release_from: f32,
}

impl Envelope {
    /// Builds an envelope from millisecond times and a sustain level.
    /// `sustain` is clamped to `0.0..=1.0`; it only matters for
    /// [`ModShape::Adsr`]. Times are clamped non-negative and rounded to
    /// whole samples.
    pub fn new(
        shape: ModShape,
        attack_ms: f32,
        hold_ms: f32,
        decay_ms: f32,
        sustain: f32,
        release_ms: f32,
    ) -> Self {
        Self {
            shape,
            attack: ms_to_samples(attack_ms),
            hold: ms_to_samples(hold_ms),
            decay: ms_to_samples(decay_ms),
            sustain: sustain.clamp(0.0, 1.0),
            release: ms_to_samples(release_ms),
            stage: Stage::Attack,
            t: 0,
            level: 0.0,
            release_from: 0.0,
        }
    }

    /// An already-finished envelope for an inactive voice slot. `const` so
    /// a fixed voice array can be initialized at compile time; overwritten
    /// by [`new`](Envelope::new) when the slot is claimed. Reports
    /// [`is_done`](Envelope::is_done) immediately.
    pub const fn silent() -> Self {
        Self {
            shape: ModShape::Ahd,
            attack: 0,
            hold: 0,
            decay: 0,
            sustain: 0.0,
            release: 0,
            stage: Stage::Done,
            t: 0,
            level: 0.0,
            release_from: 0.0,
        }
    }

    /// The gain emitted by the most recent [`tick`](Envelope::tick) (or
    /// `0.0` before the first). The mixer uses this to steal the quietest
    /// voice when all slots are busy.
    pub fn current_gain(&self) -> f32 {
        self.level
    }

    /// True once the envelope has fully run out. A voice whose envelope is
    /// finished contributes silence and can be reclaimed by the mixer.
    pub fn is_done(&self) -> bool {
        self.stage == Stage::Done
    }

    /// Advances one sample and returns the amplitude gain `0.0..=1.0` for
    /// this frame. `gate` is whether the note is still held: dropping it
    /// (note-off) moves the envelope into Release from wherever it is. AHD
    /// and DRUM self-terminate, so for them `gate` only matters as an early
    /// cut; ADSR waits in Sustain until `gate` goes false.
    pub fn tick(&mut self, gate: bool) -> f32 {
        // A gate drop anywhere before Release cuts to the release stage,
        // fading from the level we last emitted (no click).
        if !gate && !matches!(self.stage, Stage::Release | Stage::Done) {
            self.enter_release();
        }

        let gain = match self.stage {
            Stage::Attack => ramp_up(self.t, self.attack),
            Stage::Hold => 1.0,
            Stage::Decay => {
                // Decay falls toward the floor. Only ADSR rests at a
                // non-zero sustain; AHD/DRUM always decay to zero (their
                // `sustain` field is unused, so the Patch's ADSR-oriented
                // default must not leave them stuck at full amplitude and
                // then hard-cut to silence — that was an audible click).
                // DRUM curves the fall (squared) for a punchier tail.
                let floor = if self.shape == ModShape::Adsr {
                    self.sustain
                } else {
                    0.0
                };
                let lin = ramp_down(self.t, self.decay);
                let shaped = if self.shape == ModShape::Drum {
                    lin * lin
                } else {
                    lin
                };
                floor + (1.0 - floor) * shaped
            }
            Stage::Sustain => self.sustain,
            Stage::Release => {
                let lin = ramp_down(self.t, self.release);
                self.release_from * lin
            }
            Stage::Done => 0.0,
        };

        self.level = gain;
        self.advance();
        gain
    }

    /// Moves to the next stage when the current one's duration elapses.
    /// Called after the gain for the current `t` has been emitted.
    fn advance(&mut self) {
        self.t += 1;
        match self.stage {
            Stage::Attack => {
                if self.t >= self.attack {
                    self.goto(Stage::Hold);
                    // Zero-length hold falls straight through to decay.
                    if self.hold == 0 {
                        self.goto(Stage::Decay);
                    }
                }
            }
            Stage::Hold => {
                if self.t >= self.hold {
                    self.goto(Stage::Decay);
                }
            }
            Stage::Decay => {
                if self.t >= self.decay {
                    // ADSR rests at the sustain level until note-off;
                    // AHD/DRUM have decayed to zero and are finished.
                    if self.shape == ModShape::Adsr {
                        self.goto(Stage::Sustain);
                    } else {
                        self.goto(Stage::Done);
                    }
                }
            }
            // Sustain is left only by a gate drop (handled in `tick`).
            Stage::Sustain => {}
            Stage::Release => {
                if self.t >= self.release {
                    self.goto(Stage::Done);
                }
            }
            Stage::Done => {}
        }
    }

    fn goto(&mut self, stage: Stage) {
        self.stage = stage;
        self.t = 0;
    }

    /// Enters Release, capturing the current level so the fade starts from
    /// exactly where the voice is (whether that's full sustain or a partial
    /// attack). A zero-length release lands on Done immediately via the
    /// next `advance`.
    fn enter_release(&mut self) {
        self.release_from = self.level;
        self.goto(Stage::Release);
    }
}

/// Converts milliseconds to a whole sample count at the engine rate.
/// Negative inputs clamp to `0` (instant).
fn ms_to_samples(ms: f32) -> u32 {
    if ms <= 0.0 {
        0
    } else {
        (ms / 1000.0 * SAMPLE_RATE as f32).round() as u32
    }
}

/// Linear `0 -> 1` ramp over `len` samples. `len == 0` is instant (returns
/// `1.0`); the final sample of the ramp also returns `1.0`.
fn ramp_up(t: u32, len: u32) -> f32 {
    if len == 0 {
        1.0
    } else {
        (t as f32 / len as f32).min(1.0)
    }
}

/// Linear `1 -> 0` ramp over `len` samples. `len == 0` is instant (returns
/// `0.0`).
fn ramp_down(t: u32, len: u32) -> f32 {
    if len == 0 {
        0.0
    } else {
        (1.0 - t as f32 / len as f32).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(env: &mut Envelope, gate: bool, n: u32) -> f32 {
        let mut g = 0.0;
        for _ in 0..n {
            g = env.tick(gate);
        }
        g
    }

    #[test]
    fn shape_from_i32_maps_and_defaults() {
        assert_eq!(ModShape::from_i32(0), ModShape::Ahd);
        assert_eq!(ModShape::from_i32(1), ModShape::Adsr);
        assert_eq!(ModShape::from_i32(2), ModShape::Drum);
        assert_eq!(ModShape::from_i32(99), ModShape::Ahd);
    }

    #[test]
    fn ahd_rises_holds_then_decays_to_zero_ignoring_gate() {
        // 10ms attack, 10ms hold, 10ms decay @ 44100 = 441 samples each.
        let mut env = Envelope::new(ModShape::Ahd, 10.0, 10.0, 10.0, 0.0, 0.0);
        // Attack: first sample near 0, climbing.
        let first = env.tick(true);
        assert!(first < 0.1);
        // Reach the hold plateau (gate held, but AHD ignores it anyway).
        let peak = run(&mut env, true, 441);
        assert!((peak - 1.0).abs() < 0.02);
        // Through hold + decay it returns to zero and reports done.
        run(&mut env, true, 441 + 441 + 2);
        assert!(env.is_done());
        assert_eq!(env.tick(true), 0.0);
    }

    #[test]
    fn ahd_decays_to_zero_even_with_a_nonzero_sustain_field() {
        // Regression: AHD/DRUM ignore `sustain`, so a Patch carrying the
        // ADSR default (1.0) must not pin them at full amplitude and then
        // hard-cut to silence (an audible click). Decay must reach ~0.
        let mut env = Envelope::new(ModShape::Ahd, 1.0, 0.0, 20.0, 1.0, 0.0);
        run(&mut env, true, 44); // through the 1ms attack
        let near_start = env.tick(true);
        let near_end = run(&mut env, true, 1000); // past the 20ms (~882-sample) decay
        assert!(near_end < near_start, "decay must descend, not hold flat");
        assert!(near_end < 0.05, "decay must reach ~0, was {near_end}");
        assert!(env.is_done());
    }

    #[test]
    fn ahd_self_terminates_even_while_gated() {
        let mut env = Envelope::new(ModShape::Ahd, 1.0, 1.0, 1.0, 0.0, 0.0);
        // Hold the gate the whole time; AHD must still finish on its own.
        run(&mut env, true, 44_100);
        assert!(env.is_done());
    }

    #[test]
    fn adsr_holds_sustain_while_gated_then_releases() {
        // 5ms A, (no hold), 5ms D to 0.5 sustain, 5ms R.
        let mut env = Envelope::new(ModShape::Adsr, 5.0, 0.0, 5.0, 0.5, 5.0);
        // After attack+decay, gated -> parks at sustain indefinitely.
        let s = run(&mut env, true, 221 + 221 + 10);
        assert!((s - 0.5).abs() < 0.02);
        // Still gated far later -> still sustaining, not done.
        let s2 = run(&mut env, true, 44_100);
        assert!((s2 - 0.5).abs() < 0.02);
        assert!(!env.is_done());
        // Drop the gate -> release from sustain to zero.
        run(&mut env, false, 221 + 2);
        assert!(env.is_done());
    }

    #[test]
    fn early_gate_drop_releases_from_current_level_no_click() {
        // Long attack; drop the gate partway up so we release from a
        // partial level, not from 1.0.
        let mut env = Envelope::new(ModShape::Adsr, 100.0, 0.0, 10.0, 0.8, 10.0);
        let mid = run(&mut env, true, 2205); // ~50ms into a 100ms attack
        assert!(mid > 0.3 && mid < 0.7, "mid level was {mid}");
        // One ungated tick enters release; the gain should not jump up.
        let after = env.tick(false);
        assert!(after <= mid + 0.01, "release jumped: {mid} -> {after}");
        // And it winds down to done.
        run(&mut env, false, 441 + 2);
        assert!(env.is_done());
    }

    #[test]
    fn drum_decays_faster_at_the_start_than_linear() {
        // DRUM uses a squared decay: at the midpoint of decay it should be
        // below the 0.5 a linear decay would give (faster initial fall).
        let mut env = Envelope::new(ModShape::Drum, 0.0, 0.0, 100.0, 0.0, 0.0);
        let half = run(&mut env, true, 2205); // ~50ms into 100ms decay
        assert!(half < 0.45, "drum midpoint {half} not below linear 0.5");
        assert!(half > 0.0);
        run(&mut env, true, 2300);
        assert!(env.is_done());
    }

    #[test]
    fn zero_length_stages_do_not_divide_by_zero() {
        // All-instant envelope: attack 0 -> immediate peak, decay 0 ->
        // immediate done. Must not panic or NaN.
        let mut env = Envelope::new(ModShape::Ahd, 0.0, 0.0, 0.0, 0.0, 0.0);
        let g = env.tick(true);
        assert!(g.is_finite());
        run(&mut env, true, 4);
        assert!(env.is_done());
    }

    #[test]
    fn gain_stays_in_unit_range_across_a_full_adsr_life() {
        let mut env = Envelope::new(ModShape::Adsr, 7.0, 3.0, 11.0, 0.6, 9.0);
        for i in 0..4000 {
            let gate = i < 2000; // release halfway through
            let g = env.tick(gate);
            assert!((0.0..=1.0).contains(&g), "gain {g} out of range at {i}");
        }
        assert!(env.is_done());
    }
}
