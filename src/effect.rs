//! Engine-level juice primitives: hitstop, screen shake, flash, slow_mo.
//! Owned by the session via `Rc<RefCell<Effects>>`; the Lua `effect.*`
//! API mutates the same cell. One `tick(dt)` per frame decays each
//! timer with real wall-clock dt (not affected by slow_mo).
//!
//! Stacking rule across all four: longer duration wins; for the
//! magnitude param (intensity / scale / color), the latest call wins.
//! That way `effect.screen_shake(0.1, 2)` followed by
//! `effect.screen_shake(0.5, 4)` gives 0.5s at intensity 4 (the union
//! of both calls), and spam-calling is safe.

pub struct Effects {
    hitstop_left: f32,
    shake_left: f32,
    shake_total: f32,
    shake_intensity: f32,
    flash_left: f32,
    flash_total: f32,
    flash_color_index: i32,
    slow_mo_left: f32,
    slow_mo_scale: f32,
    rng: Xorshift32,
}

impl Effects {
    pub fn new() -> Self {
        Self {
            hitstop_left: 0.0,
            shake_left: 0.0,
            shake_total: 0.0,
            shake_intensity: 0.0,
            flash_left: 0.0,
            flash_total: 0.0,
            flash_color_index: 0,
            slow_mo_left: 0.0,
            slow_mo_scale: 1.0,
            rng: Xorshift32::new(0xdeadbeef),
        }
    }

    /// Decays each timer by real wall-clock dt. Should run once per
    /// frame, gated on the engine pause menu being closed (so the
    /// pause overlay genuinely freezes everything).
    pub fn tick(&mut self, dt: f32) {
        self.hitstop_left = (self.hitstop_left - dt).max(0.0);
        self.shake_left = (self.shake_left - dt).max(0.0);
        self.flash_left = (self.flash_left - dt).max(0.0);
        self.slow_mo_left = (self.slow_mo_left - dt).max(0.0);
        if self.slow_mo_left == 0.0 {
            self.slow_mo_scale = 1.0;
        }
    }

    /// True while a hitstop is active. The session skips Lua `_update`
    /// for these frames; `_draw` and effect decay continue.
    pub fn frozen(&self) -> bool {
        self.hitstop_left > 0.0
    }

    /// Multiplier the session applies to dt before passing it to
    /// `_update`. 1.0 outside slow_mo.
    pub fn time_scale(&self) -> f32 {
        if self.slow_mo_left > 0.0 {
            self.slow_mo_scale
        } else {
            1.0
        }
    }

    /// (dx, dy) offset in *game pixels* applied to the RT-to-screen
    /// blit. Magnitude decays linearly to zero over the shake's
    /// duration; angle is randomized every call.
    pub fn shake_offset(&mut self) -> (f32, f32) {
        if self.shake_left <= 0.0 || self.shake_total <= 0.0 {
            return (0.0, 0.0);
        }
        let decay = self.shake_left / self.shake_total;
        let mag = self.shake_intensity * decay;
        let angle = self.rng.next_f32() * std::f32::consts::TAU;
        (angle.cos() * mag, angle.sin() * mag)
    }

    /// `(palette_index, alpha)` for the full-screen flash overlay drawn
    /// on top of `_draw`. Alpha decays linearly from 255 to 0 over the
    /// flash duration. None when no flash is active.
    pub fn flash_overlay(&self) -> Option<(i32, u8)> {
        if self.flash_left <= 0.0 || self.flash_total <= 0.0 {
            return None;
        }
        let decay = self.flash_left / self.flash_total;
        let alpha = (decay * 255.0).round().clamp(0.0, 255.0) as u8;
        Some((self.flash_color_index, alpha))
    }

    pub fn hitstop(&mut self, time: f32) {
        let t = time.max(0.0);
        if t > self.hitstop_left {
            self.hitstop_left = t;
        }
    }

    pub fn screen_shake(&mut self, time: f32, intensity: f32) {
        let t = time.max(0.0);
        if t > self.shake_left {
            self.shake_left = t;
            self.shake_total = t;
        }
        self.shake_intensity = intensity.max(0.0);
    }

    pub fn flash(&mut self, time: f32, color_index: i32) {
        let t = time.max(0.0);
        if t > self.flash_left {
            self.flash_left = t;
            self.flash_total = t;
        }
        self.flash_color_index = color_index;
    }

    pub fn slow_mo(&mut self, time: f32, scale: f32) {
        let t = time.max(0.0);
        if t > self.slow_mo_left {
            self.slow_mo_left = t;
        }
        self.slow_mo_scale = scale.max(0.0);
    }

    /// Clears every active timer (hitstop, shake, flash, slow_mo). Used
    /// when the game is reset so a long-running `effect.hitstop(100)`
    /// doesn't carry across `_init()` and freeze the fresh game.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for Effects {
    fn default() -> Self {
        Self::new()
    }
}

/// Tiny xorshift32 RNG. Used only for shake angle. Inlined to avoid
/// pulling in the `rand` crate for one f32 per frame during shake.
struct Xorshift32(u32);

impl Xorshift32 {
    fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_inert() {
        let e = Effects::new();
        assert!(!e.frozen());
        assert_eq!(e.time_scale(), 1.0);
        assert_eq!(e.flash_overlay(), None);
    }

    #[test]
    fn hitstop_freezes_until_tick_drains_it() {
        let mut e = Effects::new();
        e.hitstop(0.05);
        assert!(e.frozen());
        e.tick(0.02);
        assert!(e.frozen());
        e.tick(0.05);
        assert!(!e.frozen());
    }

    #[test]
    fn hitstop_takes_max_of_concurrent_calls() {
        let mut e = Effects::new();
        e.hitstop(0.1);
        e.hitstop(0.05);
        // Second call shorter; first call's duration should stand.
        e.tick(0.06);
        assert!(e.frozen());
    }

    #[test]
    fn shake_offset_zero_when_inactive() {
        let mut e = Effects::new();
        assert_eq!(e.shake_offset(), (0.0, 0.0));
    }

    #[test]
    fn shake_offset_bounded_by_intensity_and_decays() {
        let mut e = Effects::new();
        e.screen_shake(1.0, 4.0);
        let (x, y) = e.shake_offset();
        let mag = (x * x + y * y).sqrt();
        assert!(mag <= 4.0 + 1e-4, "expected |offset| <= 4, got {mag}");

        // Half the duration → half the magnitude bound.
        e.tick(0.5);
        let (x, y) = e.shake_offset();
        let mag = (x * x + y * y).sqrt();
        assert!(
            mag <= 2.0 + 1e-4,
            "expected decayed |offset| <= 2, got {mag}"
        );
    }

    #[test]
    fn shake_takes_max_duration_latest_intensity() {
        let mut e = Effects::new();
        e.screen_shake(0.1, 2.0);
        e.screen_shake(0.5, 4.0);
        // Stronger and longer → both update.
        e.tick(0.2);
        // 0.5 - 0.2 = 0.3 left, intensity 4: max mag = 4 * (0.3 / 0.5) = 2.4
        let (x, y) = e.shake_offset();
        let mag = (x * x + y * y).sqrt();
        assert!(mag <= 2.4 + 1e-4, "got {mag}");

        // Weaker, shorter call after: shorter time loses, latest intensity wins.
        let mut e = Effects::new();
        e.screen_shake(1.0, 4.0);
        e.screen_shake(0.1, 2.0);
        let (x, y) = e.shake_offset();
        let mag = (x * x + y * y).sqrt();
        assert!(mag <= 2.0 + 1e-4, "latest intensity should win, got {mag}");
    }

    #[test]
    fn flash_overlay_decays_linearly_then_clears() {
        let mut e = Effects::new();
        e.flash(0.4, 7);
        let (c, a) = e.flash_overlay().unwrap();
        assert_eq!(c, 7);
        assert_eq!(a, 255);

        e.tick(0.2);
        let (_, a) = e.flash_overlay().unwrap();
        // 0.2 / 0.4 = 0.5 → 128ish.
        assert!((120..=135).contains(&a), "got {a}");

        e.tick(0.3);
        assert_eq!(e.flash_overlay(), None);
    }

    #[test]
    fn flash_takes_max_duration_latest_color() {
        let mut e = Effects::new();
        e.flash(0.5, 8);
        e.flash(0.2, 11);
        let (c, _) = e.flash_overlay().unwrap();
        assert_eq!(c, 11, "latest color should win");
    }

    #[test]
    fn slow_mo_scales_dt_until_expiry() {
        let mut e = Effects::new();
        assert_eq!(e.time_scale(), 1.0);
        e.slow_mo(0.5, 0.25);
        assert_eq!(e.time_scale(), 0.25);
        e.tick(0.4);
        assert_eq!(e.time_scale(), 0.25);
        e.tick(0.2);
        assert_eq!(e.time_scale(), 1.0);
    }

    #[test]
    fn slow_mo_takes_max_duration_latest_scale() {
        let mut e = Effects::new();
        e.slow_mo(0.5, 0.5);
        e.slow_mo(0.1, 0.1);
        // Latest scale wins, longest duration wins.
        assert_eq!(e.time_scale(), 0.1);
        e.tick(0.3);
        assert_eq!(e.time_scale(), 0.1);
        e.tick(0.3);
        assert_eq!(e.time_scale(), 1.0);
    }

    #[test]
    fn negative_inputs_are_clamped_to_zero() {
        let mut e = Effects::new();
        e.hitstop(-1.0);
        e.screen_shake(-0.1, -2.0);
        e.flash(-0.5, 4);
        e.slow_mo(-1.0, -0.5);
        assert!(!e.frozen());
        assert_eq!(e.shake_offset(), (0.0, 0.0));
        assert_eq!(e.flash_overlay(), None);
        assert_eq!(e.time_scale(), 1.0);
    }

    #[test]
    fn zero_duration_is_a_no_op() {
        let mut e = Effects::new();
        e.hitstop(0.0);
        e.screen_shake(0.0, 4.0);
        assert!(!e.frozen());
        assert_eq!(e.shake_offset(), (0.0, 0.0));
    }

    /// `reset` zeros every timer so the next frame
    /// runs cleanly.
    #[test]
    fn reset_clears_every_active_timer() {
        let mut e = Effects::new();
        e.hitstop(100.0);
        e.screen_shake(5.0, 8.0);
        e.flash(2.0, 3);
        e.slow_mo(1.0, 0.25);
        assert!(e.frozen());
        e.reset();
        assert!(!e.frozen());
        assert_eq!(e.time_scale(), 1.0);
        assert_eq!(e.flash_overlay(), None);
        assert_eq!(e.shake_offset(), (0.0, 0.0));
    }
}
