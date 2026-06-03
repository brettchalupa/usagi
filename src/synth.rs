//! Oscillator primitives for synthesized sound. A [`SynthSpec`] (waveform +
//! shape param) plus a [`LoopOsc`] phase accumulator produce per-sample audio,
//! summed and enveloped by the callback mixer (`audio_engine`). Pure f32 math,
//! no filesystem or native toolchain, so it works on web unchanged.

/// Output sample rate, Hz. Matches raylib's device rate (no resampling).
pub const SAMPLE_RATE: u32 = 44_100;

/// Waveform selector. Integer reprs mirror the Lua `sfx.*` constants
/// (`SINE=0 .. TRIANGLE=4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Noise,
    Triangle,
}

impl Waveform {
    /// Single source of truth for (name, int, variant) tuples. Used for Lua API
    /// registration and `from_i32` derivation.
    pub const LUA_CONSTS: &'static [(&'static str, i32, Waveform)] = &[
        ("SINE", 0, Waveform::Sine),
        ("SAW", 1, Waveform::Saw),
        ("SQUARE", 2, Waveform::Square),
        ("NOISE", 3, Waveform::Noise),
        ("TRIANGLE", 4, Waveform::Triangle),
    ];

    /// Maps the Lua integer constant to a variant; out-of-range falls back to
    /// `Sine` rather than erroring (the engine's forgiving stance).
    pub fn from_i32(v: i32) -> Self {
        Self::LUA_CONSTS
            .iter()
            .find(|(_, n, _)| *n == v)
            .map(|(_, _, w)| *w)
            .unwrap_or(Waveform::Sine)
    }
}

/// A waveform + shape request for building a [`LoopOsc`]. `param` is the
/// per-waveform shape control (`0..1`): pulse width (square), phase offset
/// (saw/sine/triangle), softness (noise). `frequency_hz` seeds the noise RNG;
/// the live pitch is supplied per sample so a voice can glide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SynthSpec {
    pub wave: i32,
    pub frequency_hz: u32,
    pub duration_ms: u32,
    /// param in milli-units (`0..=1000`); integer so the spec stays `Hash`/`Eq`.
    pub param_milli: u16,
}

impl SynthSpec {
    pub fn new(wave: i32, frequency_hz: i32, duration_ms: i32, param: f32) -> Self {
        Self {
            wave,
            frequency_hz: frequency_hz.max(0) as u32,
            duration_ms: duration_ms.max(0) as u32,
            param_milli: (param.clamp(0.0, 1.0) * 1000.0).round() as u16,
        }
    }

    fn param(&self) -> f32 {
        self.param_milli as f32 / 1000.0
    }

    /// Deterministic non-zero RNG seed derived from the spec: distinct specs
    /// get distinct sequences, a given spec is reproducible.
    fn noise_seed(&self) -> u32 {
        let mut h: u32 = 0x811C_9DC5; // FNV-ish mix
        for v in [
            self.wave as u32,
            self.frequency_hz,
            self.duration_ms,
            self.param_milli as u32,
        ] {
            h = (h ^ v).wrapping_mul(0x0100_0193);
        }
        h | 1 // xorshift requires a non-zero state
    }
}

/// One waveform sample in `-1.0..=1.0` for phase `t` in `0.0..1.0`
/// (fraction of one cycle). `param` is the shape control.
fn sample(wave: Waveform, t: f32, param: f32, rng: &mut u32) -> f32 {
    use std::f32::consts::TAU;
    match wave {
        Waveform::Sine => (TAU * (t + param)).sin(),
        Waveform::Square => {
            // param = pulse width (duty cycle). 0.5 -> even square.
            if t < param.clamp(0.01, 0.99) {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Saw => {
            // Monotonic ramp -1 -> 1 with a sharp per-cycle reset; `param`
            // offsets the phase.
            let p = (t + param).fract();
            2.0 * p - 1.0
        }
        Waveform::Triangle => {
            // Symmetric triangle, softer than saw; `param` offsets the phase.
            let p = (t + param).fract();
            if p < 0.5 {
                4.0 * p - 1.0
            } else {
                3.0 - 4.0 * p
            }
        }
        Waveform::Noise => {
            // White noise via xorshift; `param` softness is low-passed in
            // `LoopOsc::next_sample` (which holds the filter state).
            *rng ^= *rng << 13;
            *rng ^= *rng >> 17;
            *rng ^= *rng << 5;
            (*rng as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }
}

/// A continuous, stateful oscillator. Its phase accumulator carries across
/// [`next_sample`](LoopOsc::next_sample) calls, so a held voice has no seam and
/// pitch glides don't click. Noise carries its `rng` state too.
pub struct LoopOsc {
    wave: Waveform,
    param: f32,
    /// Phase in cycles, wrapped to `0.0..1.0`.
    phase: f32,
    rng: u32,
    /// One-pole low-pass state for NOISE softness (last filtered output).
    lp: f32,
}

impl LoopOsc {
    /// A do-nothing oscillator for an inactive voice slot. `const` for
    /// compile-time array init; overwritten by [`new`](LoopOsc::new) on claim.
    pub const fn silent() -> Self {
        Self {
            wave: Waveform::Sine,
            param: 0.0,
            phase: 0.0,
            rng: 1, // xorshift needs non-zero state
            lp: 0.0,
        }
    }

    /// Builds a generator for `spec`'s waveform + shape. Frequency is
    /// supplied per sample (so it can glide), not stored here.
    pub fn new(spec: &SynthSpec) -> Self {
        Self {
            wave: Waveform::from_i32(spec.wave),
            param: spec.param(),
            phase: 0.0,
            rng: spec.noise_seed(),
            lp: 0.0,
        }
    }

    /// Generates one raw sample in `-1.0..=1.0` at `freq_hz` and advances the
    /// phase (the mixer applies envelope gain). `freq_hz <= 0` emits silence
    /// without advancing, so a paused voice doesn't drift.
    pub fn next_sample(&mut self, freq_hz: f32) -> f32 {
        if freq_hz <= 0.0 {
            return 0.0;
        }
        let step = freq_hz / SAMPLE_RATE as f32;
        let s = sample(self.wave, self.phase, self.param, &mut self.rng);
        self.phase += step;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
        }
        if self.wave == Waveform::Noise {
            // One-pole low-pass: `param` softness darkens the noise. param 0 ->
            // white (no smoothing); param 1 -> heavily smoothed.
            let coeff = (1.0 - self.param).clamp(0.05, 1.0);
            self.lp += coeff * (s - self.lp);
            return self.lp;
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_quantization_dedupes_near_identical_specs() {
        // param 0.5004 and 0.4996 both round to 500 milli -> equal specs.
        let a = SynthSpec::new(0, 440, 100, 0.5004);
        let b = SynthSpec::new(0, 440, 100, 0.4996);
        assert_eq!(a, b);
    }

    #[test]
    fn noise_seed_differs_across_specs_but_is_stable() {
        let a = SynthSpec::new(3, 440, 100, 0.5);
        let b = SynthSpec::new(3, 880, 100, 0.5);
        assert_ne!(a.noise_seed(), b.noise_seed());
        assert_eq!(
            a.noise_seed(),
            SynthSpec::new(3, 440, 100, 0.5).noise_seed()
        );
        assert_ne!(a.noise_seed(), 0); // xorshift needs non-zero state
    }

    #[test]
    fn square_param_changes_duty_cycle() {
        let mut rng = 1u32;
        // narrow pulse: most of the cycle is low.
        assert_eq!(sample(Waveform::Square, 0.05, 0.1, &mut rng), 1.0);
        assert_eq!(sample(Waveform::Square, 0.5, 0.1, &mut rng), -1.0);
    }

    #[test]
    fn triangle_peaks_and_troughs_symmetrically() {
        let mut rng = 1u32;
        assert!((sample(Waveform::Triangle, 0.0, 0.0, &mut rng) + 1.0).abs() < 1e-6); // trough
        assert!(sample(Waveform::Triangle, 0.25, 0.0, &mut rng).abs() < 1e-6); // zero rising
        assert!((sample(Waveform::Triangle, 0.5, 0.0, &mut rng) - 1.0).abs() < 1e-6); // peak
        assert!(sample(Waveform::Triangle, 0.75, 0.0, &mut rng).abs() < 1e-6); // zero falling
    }

    #[test]
    fn saw_is_a_monotonic_ramp_over_the_cycle() {
        // Regression: the old Saw skewed its peak by `param`, so the default
        // param=0.5 produced a symmetric triangle (rose then fell) and sounded
        // like a sine. A true sawtooth rises monotonically across the cycle
        // then resets, so consecutive samples within one cycle only increase.
        let mut rng = 1u32;
        let steps = 64;
        let mut prev = sample(Waveform::Saw, 0.0, 0.0, &mut rng);
        assert!((prev + 1.0).abs() < 1e-6, "ramp starts at -1");
        for i in 1..steps {
            let t = i as f32 / steps as f32;
            let s = sample(Waveform::Saw, t, 0.0, &mut rng);
            assert!(s > prev, "saw must rise monotonically: t={t} {s} <= {prev}");
            prev = s;
        }
        // Unlike the triangle, the saw is still climbing at the half-cycle
        // (a triangle would be at its peak / turning back down here).
        assert!(sample(Waveform::Saw, 0.5, 0.0, &mut rng).abs() < 1e-6); // ~0 at midpoint
        assert!(sample(Waveform::Saw, 0.99, 0.0, &mut rng) > 0.9); // near +1 before reset
    }

    #[test]
    fn next_sample_is_seamless_and_silent_at_zero_hz() {
        let spec = SynthSpec::new(0, 440, 0, 0.5);
        let mut osc = LoopOsc::new(&spec);
        for _ in 0..64 {
            osc.next_sample(440.0);
        }
        let phase_after = osc.phase;
        // Continuing advances phase (no reset to 0 -> no seam).
        for _ in 0..64 {
            osc.next_sample(440.0);
        }
        assert_ne!(osc.phase, phase_after);
        assert!((0.0..1.0).contains(&osc.phase));
        // 0 Hz -> silence, phase frozen.
        let frozen = osc.phase;
        assert_eq!(osc.next_sample(0.0), 0.0);
        assert_eq!(osc.phase, frozen);
    }
}
