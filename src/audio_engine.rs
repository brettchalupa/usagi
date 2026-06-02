//! Audio-thread callback mixer for synthesized sound. A `'static` processor
//! attached to raylib's master mix via `AttachAudioMixedProcessor` owns a
//! fixed voice array and sums them into the output buffer on the audio thread.
//!
//! The game thread only posts events (note-on/off, set-param, stop-all) through
//! a lock-free SPSC ring; the callback drains the ring per buffer, then renders.
//!
//! Threading: one producer (game thread, `engine().post`), one consumer (audio
//! thread, [`mix`]). The voice array is touched only by the consumer, so it
//! needs no locks. The `unsafe` rests on that invariant.
//!
//! [`mix`] does no alloc, takes no locks, never panics: fixed ring + voice
//! array, f32 math, soft-clip. A post to a full ring is dropped, never blocks.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use crate::modulator::{Envelope, ModShape};
use crate::synth::{LoopOsc, SynthSpec};

/// Max simultaneous voices; a further note-on steals the quietest.
pub const MAX_VOICES: usize = 16;

/// Game -> audio event ring capacity. Power of two for the wrap mask; 256
/// absorbs bursts (e.g. a chord) with margin.
const RING_CAP: usize = 256;
const RING_MASK: usize = RING_CAP - 1;

/// Envelope parameters (attack, hold, decay, sustain, release).
#[derive(Debug, Clone, Copy)]
pub struct EnvSpec {
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub decay_ms: f32,
    pub sustain: f32,
    pub release_ms: f32,
}

impl EnvSpec {
    /// Default AHD shape: attack 4ms, hold 0ms, decay 120ms (percussive blip),
    /// sustain 1.0, release 30ms.
    pub fn default_ahd() -> Self {
        Self {
            attack_ms: 4.0,
            hold_ms: 0.0,
            decay_ms: 120.0,
            sustain: 1.0,
            release_ms: 30.0,
        }
    }
}

/// Synth options from the Lua API, each field optional; resolved into a Patch.
#[derive(Debug, Clone, Copy)]
pub struct PatchOpts {
    pub wave: Option<i32>,
    pub freq_hz: Option<f32>,
    pub volume: Option<f32>,
    pub param: Option<f32>,
    pub shape: Option<i32>,
    pub attack_ms: Option<f32>,
    pub hold_ms: Option<f32>,
    pub decay_ms: Option<f32>,
    pub sustain: Option<f32>,
    pub release_ms: Option<f32>,
    pub slide_semitones: Option<f32>,
    pub slide_ms: Option<f32>,
}

impl PatchOpts {
    /// Resolves a PatchOpts into a fully-defaulted Patch. All unspecified fields
    /// use engine defaults; shape defaults to 0 (AHD); out-of-range values are clamped.
    pub fn resolve(self, id: u32) -> Patch {
        let env_defaults = EnvSpec::default_ahd();
        let env = EnvSpec {
            attack_ms: self.attack_ms.unwrap_or(env_defaults.attack_ms),
            hold_ms: self.hold_ms.unwrap_or(env_defaults.hold_ms),
            decay_ms: self.decay_ms.unwrap_or(env_defaults.decay_ms),
            sustain: self.sustain.unwrap_or(env_defaults.sustain),
            release_ms: self.release_ms.unwrap_or(env_defaults.release_ms),
        };

        let slide_semitones = self.slide_semitones.unwrap_or(0.0);
        let decay_ms_resolved = env.decay_ms;
        let slide_ms = self.slide_ms.unwrap_or(decay_ms_resolved);

        let wave_resolved = self.wave.unwrap_or(0);
        let freq_hz_resolved = self.freq_hz.unwrap_or(440.0);
        let volume_resolved = self.volume.unwrap_or(1.0);
        let param_resolved = self.param.unwrap_or(0.5);
        let shape_resolved = ModShape::from_i32(self.shape.unwrap_or(0));

        Patch {
            id,
            spec: crate::synth::SynthSpec::new(
                wave_resolved,
                freq_hz_resolved.round() as i32,
                0,
                param_resolved,
            ),
            freq_hz: freq_hz_resolved,
            volume: volume_resolved,
            shape: shape_resolved,
            env,
            slide_semitones,
            slide_ms,
        }
    }
}

/// A resolved note-on, `Copy` so it crosses the ring without allocation.
/// `id` ties a later note-off / set-param back to this voice.
#[derive(Debug, Clone, Copy)]
pub struct Patch {
    pub id: u32,
    pub spec: SynthSpec,
    /// Base frequency in Hz; the slide bends around it, set-param retargets it.
    pub freq_hz: f32,
    /// Amplitude `0.0..=1.0`, applied on top of the envelope.
    pub volume: f32,
    pub shape: ModShape,
    pub env: EnvSpec,
    /// Pitch bend in semitones, reached over `slide_ms` from note-on then held
    /// (+up/-down, 0 = none). Evaluated per-sample. The arcade jump/coin knob.
    pub slide_semitones: f32,
    /// Slide window in ms; defaults to decay_ms if not specified.
    pub slide_ms: f32,
}

/// An event from the game thread to the audio thread.
#[derive(Debug, Clone, Copy)]
pub enum Event {
    /// Start (or retrigger) the voice identified by `Patch::id`.
    NoteOn(Patch),
    /// Drop the gate on this voice (-> release). No-op if no live match.
    NoteOff { id: u32 },
    /// Live-update a voice; `None` fields stay put. `freq_hz` glides click-free
    /// (continuous phase), `volume` swells. Envelope/waveform stay baked at
    /// note-on. Retuning a still-sliding voice rebases the bend, but slide is
    /// used on one-shots and set_freq on un-slid sustained voices.
    SetParam {
        id: u32,
        freq_hz: Option<f32>,
        volume: Option<f32>,
    },
    /// Release every voice (e.g. on stop / scene change).
    StopAll,
}

/// One audio-thread-owned voice. Inactive voices are free to claim.
struct Voice {
    active: bool,
    id: u32,
    osc: LoopOsc,
    /// Amplitude envelope; drives the voice's gain and its lifetime.
    amp: Envelope,
    /// Pitch envelope: a ramp-and-hold whose `0..1` output scales
    /// `slide_semitones` into a per-sample bend. `0` depth means no bend.
    pitch: Envelope,
    /// Base (un-slid) frequency; the slide bends around it, set-param retargets.
    freq_hz: f32,
    /// Pitch slide depth in semitones (see [`Patch::slide_semitones`]); `0`
    /// when the patch requested no slide.
    slide_semitones: f32,
    volume: f32,
    /// Note still held. Note-off clears it (-> release); AHD/DRUM self-terminate.
    gate: bool,
    /// Monotonic claim order, for oldest-voice tie-break when stealing.
    seq: u64,
}

impl Voice {
    const fn silent() -> Self {
        Self {
            active: false,
            id: 0,
            osc: LoopOsc::silent(),
            amp: Envelope::silent(),
            pitch: Envelope::silent(),
            freq_hz: 0.0,
            slide_semitones: 0.0,
            volume: 0.0,
            gate: false,
            seq: 0,
        }
    }

    /// Builds a live voice from a note-on patch. `seq` is the engine's claim
    /// counter, for the oldest-voice steal tie-break. Owns the ms→samples
    /// slide conversion and the volume clamp.
    fn from_patch(p: &Patch, seq: u64) -> Self {
        // The slide is off unless both a window and a depth were requested;
        // zero the depth so `next_sample` skips the bend entirely.
        let has_slide = p.slide_ms > 0.0 && p.slide_semitones != 0.0;
        Self {
            active: true,
            id: p.id,
            osc: LoopOsc::new(&p.spec),
            amp: Envelope::new(
                p.shape,
                p.env.attack_ms,
                p.env.hold_ms,
                p.env.decay_ms,
                p.env.sustain,
                p.env.release_ms,
            ),
            pitch: Envelope::ramp_hold(p.slide_ms),
            freq_hz: p.freq_hz,
            slide_semitones: if has_slide { p.slide_semitones } else { 0.0 },
            volume: p.volume.clamp(0.0, 1.0),
            gate: true,
            seq,
        }
    }

    /// Renders the next enveloped, volume-scaled sample and advances the
    /// voice. Self-clears `active` once the envelope finishes, so the mixer
    /// never touches voice internals. Audio-thread only.
    fn next_sample(&mut self) -> f32 {
        let g = self.amp.tick(self.gate);
        // Pitch envelope (ticked ungated: the bend holds through note-off)
        // ramps `0..1`; scaled by depth it's the live semitone offset. Linear
        // in semitones = exponential in Hz = a steady glide.
        let eff_freq = if self.slide_semitones != 0.0 {
            let semis = self.slide_semitones * self.pitch.tick(true);
            self.freq_hz * 2.0f32.powf(semis / 12.0)
        } else {
            self.freq_hz
        };
        let s = self.osc.next_sample(eff_freq) * g * self.volume;
        if self.amp.is_done() {
            self.active = false;
        }
        s
    }

    /// Live-updates a sounding voice; `None` fields stay put.
    fn set_param(&mut self, freq_hz: Option<f32>, volume: Option<f32>) {
        if let Some(f) = freq_hz {
            self.freq_hz = f;
        }
        if let Some(v) = volume {
            self.volume = v.clamp(0.0, 1.0);
        }
    }

    /// Drops the gate, sending the envelope into release.
    fn release(&mut self) {
        self.gate = false;
    }

    /// Steal priority: the current envelope gain (quietest voice is stolen
    /// first). Volume is excluded so a held-but-turned-down voice isn't
    /// preferentially evicted.
    fn steal_score(&self) -> f32 {
        self.amp.current_gain()
    }
}

/// Lock-free SPSC ring of [`Event`]s. One producer (game thread), one
/// consumer (audio thread). `head` is the consumer cursor, `tail` the
/// producer cursor; both increase monotonically and are masked into the
/// backing array. Full when `tail - head == RING_CAP`.
struct EventRing {
    buf: [UnsafeCell<Event>; RING_CAP],
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl EventRing {
    const fn new() -> Self {
        Self {
            buf: [const { UnsafeCell::new(Event::StopAll) }; RING_CAP],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Producer side. Returns `false` (dropping the event) if the ring is full.
    ///
    /// # Safety
    /// Must be called from the single producer thread only.
    unsafe fn push(&self, ev: Event) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if tail.wrapping_sub(head) >= RING_CAP {
            return false; // full
        }
        // SAFETY: this slot is owned by the producer until the release
        // store below publishes it; the consumer won't read past `tail`.
        unsafe { *self.buf[tail & RING_MASK].get() = ev };
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        true
    }

    /// Consumer side. Returns the next event, or `None` if empty.
    ///
    /// # Safety
    /// Must be called from the single consumer thread only.
    unsafe fn pop(&self) -> Option<Event> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head == tail {
            return None; // empty
        }
        // SAFETY: `head < tail` so this slot was published by a release
        // store in `push`; we read it before advancing `head`.
        let ev = unsafe { *self.buf[head & RING_MASK].get() };
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Some(ev)
    }
}

/// The engine: event ring plus voice array. A single `'static` instance
/// ([`engine`]) backs the master processor, since the callback gets no user
/// pointer.
pub struct AudioEngine {
    ring: EventRing,
    /// Audio-thread-only; `UnsafeCell` since the callback mutates it through a
    /// shared `&'static`. Sound because only the audio thread touches it.
    voices: UnsafeCell<[Voice; MAX_VOICES]>,
    /// Voice claim order (audio-thread-only).
    seq: UnsafeCell<u64>,
    /// Master synth volume (pause-menu sfx level) as f32 bits. Written
    /// game-thread, read audio-thread; atomic so it can't tear.
    master_vol_bits: AtomicU32,
    /// Source of unique voice ids handed to the game thread.
    next_id: AtomicU32,
    /// Set once the processor is attached, to avoid double-attach.
    attached: AtomicBool,
    /// While true, [`render`] emits silence and freezes voice state, so a
    /// sustained voice resumes where it left off. Written game, read audio.
    paused: AtomicBool,
}

// SAFETY: the ring is a correct SPSC structure and `voices`/`seq` are touched
// only by the audio thread. See the module-level threading model.
unsafe impl Sync for AudioEngine {}

impl AudioEngine {
    const fn new() -> Self {
        Self {
            ring: EventRing::new(),
            voices: UnsafeCell::new([const { Voice::silent() }; MAX_VOICES]),
            seq: UnsafeCell::new(0),
            master_vol_bits: AtomicU32::new(0x3f80_0000), // 1.0_f32 bits
            next_id: AtomicU32::new(1),
            attached: AtomicBool::new(false),
            paused: AtomicBool::new(false),
        }
    }

    /// Pauses/resumes synth output; while paused, voice state is frozen.
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// Posts an event from the game thread. Returns `false` if the ring was
    /// full and the event dropped. Lock-free; safe to call every frame.
    pub fn post(&self, ev: Event) -> bool {
        // SAFETY: usagi posts from a single thread (the game loop).
        unsafe { self.ring.push(ev) }
    }

    /// Sets the master synth volume `0.0..=1.0` (pause-menu sfx level).
    pub fn set_master_volume(&self, v: f32) {
        self.master_vol_bits
            .store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    /// Returns a fresh, never-reused voice id for the game thread.
    pub fn next_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Renders `frames` stereo frames, adding the synth mix into the
    /// interleaved f32 `buf` (`frames * 2` samples). Audio-thread only.
    ///
    /// # Safety
    /// Must be called from the single consumer (audio) thread only.
    unsafe fn render(&self, buf: &mut [f32], frames: usize) {
        // SAFETY: consumer-thread-only access to the ring and voices.
        while let Some(ev) = unsafe { self.ring.pop() } {
            unsafe { self.apply(ev) };
        }

        // Paused: leave `buf` as-is (additive => silence) and freeze voices.
        if self.paused.load(Ordering::Relaxed) {
            return;
        }

        let master = f32::from_bits(self.master_vol_bits.load(Ordering::Relaxed));
        // Clamp so a mismatched host buffer can't panic the audio thread.
        let frames = frames.min(buf.len() / 2);
        let voices = unsafe { &mut *self.voices.get() };
        for frame in 0..frames {
            let mut acc = 0.0f32;
            for v in voices.iter_mut() {
                if v.active {
                    acc += v.next_sample();
                }
            }
            // tanh soft-clip: a dense chord can't wrap past full-scale, while
            // quiet signals pass near-transparently (unit slope at 0). A plain
            // clamp flat-topped peaks and buzzed with detuned voices.
            let s = (acc * master).tanh();
            buf[frame * 2] += s;
            buf[frame * 2 + 1] += s;
        }
    }

    /// Applies one drained event to the voice array. Audio-thread only.
    ///
    /// # Safety
    /// Must be called from the single consumer (audio) thread only.
    unsafe fn apply(&self, ev: Event) {
        let voices = unsafe { &mut *self.voices.get() };
        match ev {
            Event::NoteOn(p) => {
                let seq = unsafe { &mut *self.seq.get() };
                *seq = seq.wrapping_add(1);
                let slot = pick_slot(voices);
                voices[slot] = Voice::from_patch(&p, *seq);
            }
            Event::NoteOff { id } => {
                for v in voices.iter_mut() {
                    if v.active && v.id == id {
                        v.release();
                    }
                }
            }
            Event::SetParam {
                id,
                freq_hz,
                volume,
            } => {
                for v in voices.iter_mut() {
                    if v.active && v.id == id {
                        v.set_param(freq_hz, volume);
                    }
                }
            }
            Event::StopAll => {
                for v in voices.iter_mut() {
                    if v.active {
                        v.release();
                    }
                }
            }
        }
    }
}

/// Picks a slot for a new note: first inactive slot, else the quietest active
/// voice (lowest envelope gain), tie-broken by oldest `seq`.
fn pick_slot(voices: &[Voice; MAX_VOICES]) -> usize {
    if let Some(i) = voices.iter().position(|v| !v.active) {
        return i;
    }
    let mut best = 0;
    let mut best_gain = f32::INFINITY;
    let mut best_seq = u64::MAX;
    for (i, v) in voices.iter().enumerate() {
        let g = v.steal_score();
        if g < best_gain || (g == best_gain && v.seq < best_seq) {
            best = i;
            best_gain = g;
            best_seq = v.seq;
        }
    }
    best
}

/// The process-wide engine instance backing the master processor.
static ENGINE: AudioEngine = AudioEngine::new();

/// Accessor for the global engine (event posting from the game thread).
pub fn engine() -> &'static AudioEngine {
    &ENGINE
}

/// The master mixed processor. raylib invokes this on the audio thread with
/// the interleaved f32 stereo buffer (`frames * 2` samples); we add in place.
///
/// # Safety
/// Invoked by raylib on the audio thread only; `buffer` is valid for
/// `frames * 2` f32 samples for the duration of the call.
pub unsafe extern "C" fn mix(buffer: *mut std::ffi::c_void, frames: u32) {
    let buf = unsafe { std::slice::from_raw_parts_mut(buffer as *mut f32, frames as usize * 2) };
    // SAFETY: raylib calls this on the single audio thread (sole consumer).
    unsafe { ENGINE.render(buf, frames as usize) };
}

/// Attaches [`mix`] to the master mix, once. Idempotent.
///
/// # Safety
/// `audio` must outlive the attachment; detach before the device closes.
pub unsafe fn attach(audio: &sola_raylib::prelude::RaylibAudio) {
    if !ENGINE.attached.swap(true, Ordering::AcqRel) {
        unsafe { audio.attach_audio_mixed_processor(mix) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(id: u32, vol: f32) -> Patch {
        Patch {
            id,
            spec: SynthSpec::new(2, 440, 0, 0.5), // square; duration unused by the mixer
            freq_hz: 440.0,
            volume: vol,
            shape: ModShape::Adsr,
            env: EnvSpec {
                attack_ms: 1.0,
                hold_ms: 0.0,
                decay_ms: 1.0,
                sustain: 0.8,
                release_ms: 5.0,
            },
            slide_semitones: 0.0,
            slide_ms: 0.0,
        }
    }

    // Build a fresh engine (not the global) so tests don't share state.
    fn fresh() -> AudioEngine {
        AudioEngine::new()
    }

    #[test]
    fn patch_resolve_applies_defaults() {
        let opts = PatchOpts {
            wave: None,    // defaults to 0
            freq_hz: None, // defaults to 440
            volume: None,  // defaults to 1.0
            param: None,   // defaults to 0.5
            shape: None,   // defaults to 0 (AHD)
            attack_ms: None,
            hold_ms: None,
            decay_ms: None,
            sustain: None,
            release_ms: None,
            slide_semitones: None,
            slide_ms: None,
        };
        let p = opts.resolve(1);

        assert_eq!(p.id, 1);
        assert_eq!(p.freq_hz, 440.0);
        assert_eq!(p.volume, 1.0);
        assert_eq!(p.shape, ModShape::Ahd);
        assert_eq!(p.env.attack_ms, 4.0);
        assert_eq!(p.env.hold_ms, 0.0);
        assert_eq!(p.env.decay_ms, 120.0);
        assert_eq!(p.env.sustain, 1.0);
        assert_eq!(p.env.release_ms, 30.0);
        assert_eq!(p.slide_semitones, 0.0);
        assert_eq!(p.slide_ms, 120.0); // defaults to decay_ms
    }

    #[test]
    fn patch_resolve_overrides_defaults() {
        let opts = PatchOpts {
            wave: Some(2), // square
            freq_hz: Some(880.0),
            volume: Some(0.5),
            param: Some(0.3),
            shape: Some(1), // ADSR
            attack_ms: Some(10.0),
            hold_ms: Some(5.0),
            decay_ms: Some(200.0),
            sustain: Some(0.6),
            release_ms: Some(50.0),
            slide_semitones: Some(12.0),
            slide_ms: Some(150.0),
        };
        let p = opts.resolve(2);

        assert_eq!(p.env.attack_ms, 10.0);
        assert_eq!(p.env.hold_ms, 5.0);
        assert_eq!(p.env.decay_ms, 200.0);
        assert_eq!(p.env.sustain, 0.6);
        assert_eq!(p.env.release_ms, 50.0);
        assert_eq!(p.slide_semitones, 12.0);
        assert_eq!(p.slide_ms, 150.0);
    }

    #[test]
    fn patch_resolve_slide_ms_defaults_to_decay() {
        let opts = PatchOpts {
            wave: None,
            freq_hz: None,
            volume: None,
            param: None,
            shape: None,
            attack_ms: None,
            hold_ms: None,
            decay_ms: Some(250.0),
            sustain: None,
            release_ms: None,
            slide_semitones: Some(5.0),
            slide_ms: None, // not specified
        };
        let p = opts.resolve(3);

        assert_eq!(p.env.decay_ms, 250.0);
        assert_eq!(p.slide_ms, 250.0); // should default to decay
    }

    #[test]
    fn ring_push_pop_fifo_order() {
        let e = fresh();
        unsafe {
            assert!(e.ring.push(Event::NoteOff { id: 1 }));
            assert!(e.ring.push(Event::NoteOff { id: 2 }));
            assert!(matches!(e.ring.pop(), Some(Event::NoteOff { id: 1 })));
            assert!(matches!(e.ring.pop(), Some(Event::NoteOff { id: 2 })));
            assert!(e.ring.pop().is_none());
        }
    }

    #[test]
    fn ring_drops_when_full_without_blocking() {
        let e = fresh();
        unsafe {
            for i in 0..RING_CAP {
                assert!(e.ring.push(Event::NoteOff { id: i as u32 }), "slot {i}");
            }
            // One past capacity is dropped, not blocked.
            assert!(!e.ring.push(Event::NoteOff { id: 999 }));
            // Draining one frees exactly one slot.
            assert!(e.ring.pop().is_some());
            assert!(e.ring.push(Event::NoteOff { id: 1000 }));
        }
    }

    #[test]
    fn ring_wraps_around_many_times() {
        let e = fresh();
        unsafe {
            // Push/pop far more than RING_CAP to exercise index wrap.
            for i in 0..RING_CAP * 4 {
                assert!(e.ring.push(Event::NoteOff { id: i as u32 }));
                assert!(matches!(e.ring.pop(), Some(Event::NoteOff { id }) if id == i as u32));
            }
        }
    }

    #[test]
    fn note_on_claims_a_voice_and_renders_nonzero() {
        let e = fresh();
        assert!(e.post(Event::NoteOn(patch(1, 1.0))));
        let mut buf = vec![0.0f32; 64 * 2];
        unsafe { e.render(&mut buf, 64) };
        // A gated square at full volume produces signal.
        assert!(
            buf.iter().any(|&s| s.abs() > 0.01),
            "expected audible output"
        );
    }

    #[test]
    fn note_off_then_release_finishes_the_voice() {
        let e = fresh();
        e.post(Event::NoteOn(patch(7, 1.0)));
        let mut buf = vec![0.0f32; 32 * 2];
        unsafe { e.render(&mut buf, 32) }; // claim + start
        e.post(Event::NoteOff { id: 7 });
        // Release is 5ms (~220 samples); render well past it.
        let mut buf2 = vec![0.0f32; 512 * 2];
        unsafe { e.render(&mut buf2, 512) };
        // Voice should be reclaimed: a fresh note-on finds a free slot.
        let voices = unsafe { &*e.voices.get() };
        assert!(
            voices.iter().all(|v| !v.active),
            "all voices should be free"
        );
    }

    #[test]
    fn set_param_updates_a_live_voice_and_ignores_unknown_ids() {
        let e = fresh();
        e.post(Event::NoteOn(patch(5, 1.0)));
        let mut buf = vec![0.0f32; 8 * 2];
        unsafe { e.render(&mut buf, 8) }; // claim the voice
        e.post(Event::SetParam {
            id: 5,
            freq_hz: Some(880.0),
            volume: Some(0.25),
        });
        // Unknown id must not touch anything (and must not panic).
        e.post(Event::SetParam {
            id: 999,
            freq_hz: Some(110.0),
            volume: None,
        });
        unsafe { e.render(&mut buf, 8) }; // drains the SetParam events
        let voices = unsafe { &*e.voices.get() };
        let v = voices.iter().find(|v| v.active && v.id == 5).unwrap();
        assert_eq!(v.freq_hz, 880.0);
        assert_eq!(v.volume, 0.25);
    }

    #[test]
    fn stop_all_releases_every_voice() {
        let e = fresh();
        for id in 0..4 {
            e.post(Event::NoteOn(patch(id, 1.0)));
        }
        let mut buf = vec![0.0f32; 16 * 2];
        unsafe { e.render(&mut buf, 16) };
        e.post(Event::StopAll);
        unsafe { e.render(&mut buf, 16) };
        let voices = unsafe { &*e.voices.get() };
        assert!(voices.iter().filter(|v| v.active).all(|v| !v.gate));
    }

    #[test]
    fn voice_stealing_when_all_slots_busy() {
        let e = fresh();
        // Fill every slot with sustained (ADSR-gated) voices.
        for id in 0..MAX_VOICES as u32 {
            e.post(Event::NoteOn(patch(id, 1.0)));
        }
        let mut buf = vec![0.0f32; 8 * 2];
        unsafe { e.render(&mut buf, 8) };
        // One more note-on must steal a slot, not be dropped.
        e.post(Event::NoteOn(patch(999, 1.0)));
        unsafe { e.render(&mut buf, 8) };
        let voices = unsafe { &*e.voices.get() };
        assert!(
            voices.iter().any(|v| v.active && v.id == 999),
            "new voice should have stolen a slot"
        );
    }

    #[test]
    fn soft_clip_keeps_a_hot_chord_off_the_rail_yet_passes_quiet_signals() {
        // Hot case: three full-scale-ish sustained voices that summed past
        // unity and used to flat-top against the +/-1 clamp (audible buzz).
        let e = fresh();
        for (i, f) in [261.63f32, 329.63, 392.00].iter().enumerate() {
            e.post(Event::NoteOn(Patch {
                id: i as u32 + 1,
                spec: SynthSpec::new(0, *f as i32, 0, 0.5),
                freq_hz: *f,
                volume: 0.5,
                shape: ModShape::Adsr,
                env: EnvSpec {
                    attack_ms: 8.0,
                    hold_ms: 0.0,
                    decay_ms: 140.0,
                    sustain: 0.8,
                    release_ms: 120.0,
                },
                slide_semitones: 0.0,
                slide_ms: 0.0,
            }));
        }
        let n = 22_050;
        let mut buf = vec![0.0f32; n * 2];
        unsafe { e.render(&mut buf, n) };
        // No sample is pinned to the rail in the sustain region (was ~3%).
        let railed = buf
            .iter()
            .skip(8820 * 2)
            .filter(|s| s.abs() >= 0.999_99)
            .count();
        assert_eq!(railed, 0, "soft-clip should not flat-top against the rail");
        // Still strictly bounded (tanh asymptote).
        assert!(buf.iter().all(|s| s.abs() < 1.0));

        // Quiet single voice passes through near-transparently: a lone 0.3
        // voice peaks near 0.3, not visibly squashed.
        let e2 = fresh();
        e2.post(Event::NoteOn(Patch {
            id: 1,
            spec: SynthSpec::new(0, 220, 0, 0.5),
            freq_hz: 220.0,
            volume: 0.3,
            shape: ModShape::Adsr,
            env: EnvSpec {
                attack_ms: 1.0,
                hold_ms: 0.0,
                decay_ms: 1.0,
                sustain: 1.0,
                release_ms: 5.0,
            },
            slide_semitones: 0.0,
            slide_ms: 0.0,
        }));
        let mut q = vec![0.0f32; 4410 * 2];
        unsafe { e2.render(&mut q, 4410) };
        let peak = q.iter().skip(441 * 2).fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            peak > 0.27 && peak < 0.31,
            "quiet voice peak {peak} should be ~0.3"
        );
    }

    #[test]
    fn slide_bends_pitch_upward_over_its_window() {
        // A sine with a +24 semitone slide over 100ms should oscillate
        // faster at the end of the window than at the start. Count zero
        // crossings in an early vs late mono segment.
        let e = fresh();
        e.post(Event::NoteOn(Patch {
            id: 1,
            spec: SynthSpec::new(0, 220, 0, 0.5), // sine
            freq_hz: 220.0,
            volume: 1.0,
            shape: ModShape::Adsr,
            env: EnvSpec {
                attack_ms: 0.0,
                hold_ms: 0.0,
                decay_ms: 0.0,
                sustain: 1.0,
                release_ms: 5.0,
            },
            slide_semitones: 24.0, // +2 octaves -> ~4x frequency
            slide_ms: 100.0,
        }));
        let n = 4410; // 100ms at 44.1k
        let mut buf = vec![0.0f32; n * 2];
        unsafe { e.render(&mut buf, n) };
        let crossings = |range: std::ops::Range<usize>| {
            let mut c = 0;
            for i in (range.start + 1)..range.end {
                if (buf[i * 2] >= 0.0) != (buf[(i - 1) * 2] >= 0.0) {
                    c += 1;
                }
            }
            c
        };
        let early = crossings(0..441); // first 10ms (~base 220Hz)
        let late = crossings(3969..4410); // last 10ms (~880Hz)
        assert!(
            late > early * 2,
            "slide should raise pitch: early={early} late={late}"
        );
    }

    #[test]
    fn paused_render_is_silent_then_resumes_the_same_voice() {
        let e = fresh();
        e.post(Event::NoteOn(patch(1, 0.8))); // sustaining ADSR voice

        // Let it reach sustain, and confirm it's actually making sound.
        let mut warm = vec![0.0f32; 4410 * 2];
        unsafe { e.render(&mut warm, 4410) };
        let peak = warm.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.1, "voice should be audible before pause: {peak}");

        // Paused: render must add nothing (additive mix => exact silence).
        e.set_paused(true);
        let mut buf = vec![0.0f32; 4410 * 2];
        unsafe { e.render(&mut buf, 4410) };
        assert!(
            buf.iter().all(|s| *s == 0.0),
            "paused render must be silent"
        );

        // Resumed: the same sustained voice keeps sounding (state was frozen,
        // not torn down).
        e.set_paused(false);
        let mut after = vec![0.0f32; 4410 * 2];
        unsafe { e.render(&mut after, 4410) };
        let peak = after.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            peak > 0.1,
            "voice should resume audibly after pause: {peak}"
        );
    }

    #[test]
    fn render_adds_into_existing_buffer_contents() {
        let e = fresh();
        e.post(Event::NoteOn(patch(1, 1.0)));
        let mut buf = vec![0.25f32; 8 * 2]; // pretend other audio is present
        unsafe { e.render(&mut buf, 8) };
        // We add, never overwrite: every sample stayed >= the prior content
        // minus full-scale, and the buffer changed somewhere.
        assert!(buf.iter().any(|&s| (s - 0.25).abs() > 1e-6));
    }
}
