//! Audio-thread callback mixer — the core of usagi's synthesized sound.
//! A single `'static` processor is attached to raylib's master
//! mix via `AttachAudioMixedProcessor`; it owns a fixed array of voices and
//! synthesizes + sums them straight into the output buffer on the audio
//! thread, every device buffer.
//!
//! The game thread never touches a voice. It only **posts events**
//! (note-on with a full patch, note-off, stop-all) through a lock-free
//! single-producer / single-consumer ring. The callback drains the ring at
//! the top of each buffer, mutates its voices, then renders. This is what
//! keeps one-shots low-latency and immune to frame-rate stalls.
//!
//! ## Threading model
//!
//! - **Producer:** exactly one thread posts events — the game/main thread
//!   (`engine().post(..)`). usagi drives Lua from a single thread, so this
//!   holds.
//! - **Consumer:** exactly one thread drains + renders — raylib's audio
//!   thread, inside [`mix`]. The voice array is touched *only* there.
//!
//! Because of that 1:1 split the ring needs no locks, and the voices need
//! no synchronization at all (one accessor thread). The `unsafe` rests on
//! that invariant, not on the borrow checker.
//!
//! ## Real-time discipline (the callback)
//!
//! [`mix`] does **no heap allocation, takes no locks, and never panics**:
//! fixed-size ring + fixed voice array, pure f32 math, soft-clip mix. An
//! event posted when the ring is full is dropped, never blocks the producer.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use crate::modulator::{Envelope, ModShape};
use crate::synth::{LoopOsc, SynthSpec};

/// Maximum simultaneous synth voices. Beyond this the mixer steals the
/// quietest voice. 16 is generous for chiptune-style sfx without making the
/// per-frame voice sweep costly.
pub const MAX_VOICES: usize = 16;

/// Capacity of the game -> audio event ring. A frame posts at most a
/// handful of events; 256 absorbs bursts (e.g. a chord) with margin. Must
/// be a power of two for the wrap mask.
const RING_CAP: usize = 256;
const RING_MASK: usize = RING_CAP - 1;

/// A fully-resolved note-on patch: everything the audio thread needs to
/// build and run a voice, all `Copy` so it crosses the ring without
/// allocation. `id` ties a later note-off / param change back to this voice.
#[derive(Debug, Clone, Copy)]
pub struct Patch {
    /// Unique voice id (from [`AudioEngine::next_id`]). A later
    /// [`Event::NoteOff`] addresses the voice by this id.
    pub id: u32,
    pub spec: SynthSpec,
    /// Starting frequency in Hz. Carried separately from `spec` so a glide
    /// (future param change) can move it without re-keying the spec.
    pub freq_hz: f32,
    /// Playback amplitude `0.0..=1.0`, applied on top of the envelope.
    pub volume: f32,
    pub shape: ModShape,
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub decay_ms: f32,
    pub sustain: f32,
    pub release_ms: f32,
    /// Pitch bend in semitones applied from note-on, reaching its full value
    /// after `slide_ms` and then held. 0 = no sweep; positive bends up,
    /// negative down. Evaluated per-sample in the callback (smooth, no 60fps
    /// stair-step) — this is the arcade jump/coin/laser knob.
    pub slide_semitones: f32,
    /// Window over which `slide_semitones` completes, in ms. Game-side the
    /// default is the patch's `decay`.
    pub slide_ms: f32,
}

/// An event posted from the game thread to the audio thread. `Copy` +
/// `'static`, no heap.
#[derive(Debug, Clone, Copy)]
pub enum Event {
    /// Start (or retrigger) the voice identified by `Patch::id`.
    NoteOn(Patch),
    /// Drop the gate on the voice with this id, moving it to release. A
    /// no-op if no live voice matches.
    NoteOff { id: u32 },
    /// Live-update fields of the voice with this id; `None` fields are left
    /// unchanged. `freq_hz` glides click-free (phase is continuous);
    /// `volume` swells. A no-op if no live voice matches. Envelope/waveform
    /// are not retargetable mid-voice (the envelope is a running state
    /// machine), so they stay baked at note-on. Note: `freq_hz` is the
    /// voice's *base* frequency, which any active pitch `slide` bends around
    /// — retuning a still-sliding voice rebases the bend. In practice slide
    /// is used on fire-and-forget one-shots and `set_freq` on un-slid
    /// sustained voices, so the two don't overlap.
    SetParam {
        id: u32,
        freq_hz: Option<f32>,
        volume: Option<f32>,
    },
    /// Release every voice (e.g. on stop / scene change).
    StopAll,
}

/// One audio-thread-owned voice. Inactive voices contribute nothing and are
/// free to claim.
struct Voice {
    active: bool,
    id: u32,
    osc: LoopOsc,
    env: Envelope,
    /// Base (un-slid) frequency. The per-sample pitch slide bends around
    /// this, and `SetParam` retargets it.
    freq_hz: f32,
    /// Pitch slide depth in semitones (see [`Patch::slide_semitones`]).
    slide_semitones: f32,
    /// Slide window in samples; the bend reaches full depth here, then holds.
    /// 0 disables the slide.
    slide_samples: f32,
    /// Samples elapsed since note-on, the slide's progress clock.
    age: f32,
    volume: f32,
    /// Whether the note is still held. Note-off clears it, sending the
    /// envelope into release; AHD/DRUM self-terminate regardless.
    gate: bool,
    /// Monotonic claim order, for oldest-voice tie-breaking when stealing.
    seq: u64,
}

impl Voice {
    const fn silent() -> Self {
        Self {
            active: false,
            id: 0,
            osc: LoopOsc::silent(),
            env: Envelope::silent(),
            freq_hz: 0.0,
            slide_semitones: 0.0,
            slide_samples: 0.0,
            age: 0.0,
            volume: 0.0,
            gate: false,
            seq: 0,
        }
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

    /// Producer side. Returns `false` (dropping the event) if the ring is
    /// full — the audio thread must never be blocked waiting for space.
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

/// The whole engine: the event ring plus the voice array. A single
/// `'static` instance ([`engine`]) backs the master processor, since
/// `AttachAudioMixedProcessor` passes no user pointer to the callback.
pub struct AudioEngine {
    ring: EventRing,
    /// Audio-thread-only. `UnsafeCell` because the callback mutates it
    /// through a shared `&'static AudioEngine`; sound only because exactly
    /// one thread (the audio thread) ever touches it.
    voices: UnsafeCell<[Voice; MAX_VOICES]>,
    /// Monotonic counter for voice claim order (audio-thread-only).
    seq: UnsafeCell<u64>,
    /// Master synth volume (pause-menu sfx level) as f32 bits. Read on the
    /// audio thread, written on the game thread — an atomic so the two don't
    /// tear. Applied on top of each voice's own volume in [`render`].
    master_vol_bits: AtomicU32,
    /// Source of unique voice ids handed to the game thread by
    /// [`next_id`](AudioEngine::next_id), so a one-shot can still be stopped
    /// early and two concurrent voices never collide.
    next_id: AtomicU32,
    /// Set once the processor is attached, so we don't double-attach.
    attached: AtomicBool,
    /// When true, [`render`] emits silence and freezes voice state (envelopes
    /// and slide age don't advance), so the engine-level pause overlay
    /// silences sustained synth voices and resumes them exactly where they
    /// left off. Written game-thread, read audio-thread.
    paused: AtomicBool,
}

// SAFETY: cross-thread sharing is sound by construction — the ring is a
// correct SPSC structure, and `voices`/`seq` are touched only by the audio
// thread. See the module-level threading model.
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

    /// Pauses (`true`) or resumes (`false`) synth output. While paused,
    /// [`render`] adds silence and leaves all voice state untouched, so a
    /// sustained voice picks up where it left off on resume. Game-thread side.
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// Posts an event from the game thread. Returns `false` if the ring was
    /// full and the event was dropped. Lock-free; safe to call every frame.
    pub fn post(&self, ev: Event) -> bool {
        // SAFETY: usagi posts from a single thread (the game loop).
        unsafe { self.ring.push(ev) }
    }

    /// Sets the master synth volume `0.0..=1.0` (pause-menu sfx level).
    /// Game-thread side; takes effect on the next audio buffer.
    pub fn set_master_volume(&self, v: f32) {
        self.master_vol_bits
            .store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    /// Returns a fresh, never-reused voice id for the game thread to pass in
    /// a [`Patch`] and later address with [`Event::NoteOff`].
    pub fn next_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Renders `frames` stereo frames, **adding** the synth mix into the
    /// interleaved f32 `buf` (`frames * 2` samples). Drains pending events
    /// first, then sweeps voices per frame. Audio-thread only.
    ///
    /// # Safety
    /// Must be called from the single consumer (audio) thread only.
    unsafe fn render(&self, buf: &mut [f32], frames: usize) {
        // Drain all queued events into voice state up front.
        // SAFETY: consumer-thread-only access to the ring and voices.
        while let Some(ev) = unsafe { self.ring.pop() } {
            unsafe { self.apply(ev) };
        }

        // Paused: leave `buf` as-is (additive mix => silence) and don't touch
        // voice state, so envelopes and slide age freeze and resume cleanly.
        if self.paused.load(Ordering::Relaxed) {
            return;
        }

        let master = f32::from_bits(self.master_vol_bits.load(Ordering::Relaxed));
        // Defensive: never index past the buffer. By contract `buf` is
        // `frames * 2` stereo samples, but clamping keeps a mismatched
        // host buffer from panicking the audio thread (RT-safety).
        let frames = frames.min(buf.len() / 2);
        let voices = unsafe { &mut *self.voices.get() };
        for frame in 0..frames {
            let mut acc = 0.0f32;
            for v in voices.iter_mut() {
                if !v.active {
                    continue;
                }
                let g = v.env.tick(v.gate);
                // Per-sample pitch slide: bend `slide_semitones` over the
                // first `slide_samples`, then hold. Linear in semitones =
                // exponential in Hz, so it sounds like a steady glide.
                let eff_freq = if v.slide_samples > 0.0 && v.slide_semitones != 0.0 {
                    let progress = (v.age / v.slide_samples).min(1.0);
                    v.freq_hz * 2.0f32.powf(v.slide_semitones / 12.0 * progress)
                } else {
                    v.freq_hz
                };
                acc += v.osc.next_sample(eff_freq) * g * v.volume;
                v.age += 1.0;
                if v.env.is_done() {
                    v.active = false;
                }
            }
            // Soft-clip (tanh) so a dense chord can't blow past full-scale
            // and wrap. tanh has unit slope at 0, so quiet signals pass
            // through ~transparently; it only bends as the sum nears the
            // rail and asymptotes to +/-1 (still a hard bound). A plain
            // clamp instead flat-topped peaks, and with detuned voices the
            // clip engaged periodically -> an audible cycling buzz.
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
                voices[slot] = Voice {
                    active: true,
                    id: p.id,
                    osc: LoopOsc::new(&p.spec),
                    env: Envelope::new(
                        p.shape,
                        p.attack_ms,
                        p.hold_ms,
                        p.decay_ms,
                        p.sustain,
                        p.release_ms,
                    ),
                    freq_hz: p.freq_hz,
                    slide_semitones: p.slide_semitones,
                    slide_samples: (p.slide_ms * 0.001 * crate::synth::SAMPLE_RATE as f32).max(0.0),
                    age: 0.0,
                    volume: p.volume.clamp(0.0, 1.0),
                    gate: true,
                    seq: *seq,
                };
            }
            Event::NoteOff { id } => {
                for v in voices.iter_mut() {
                    if v.active && v.id == id {
                        v.gate = false;
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
                        if let Some(f) = freq_hz {
                            v.freq_hz = f;
                        }
                        if let Some(vol) = volume {
                            v.volume = vol.clamp(0.0, 1.0);
                        }
                    }
                }
            }
            Event::StopAll => {
                for v in voices.iter_mut() {
                    if v.active {
                        v.gate = false;
                    }
                }
            }
        }
    }
}

/// Chooses a voice slot for a new note: the first inactive slot, else the
/// quietest active voice (lowest current envelope gain), tie-broken by the
/// oldest (smallest `seq`). Stealing the quietest minimizes the audible
/// disruption of running out of voices.
fn pick_slot(voices: &[Voice; MAX_VOICES]) -> usize {
    if let Some(i) = voices.iter().position(|v| !v.active) {
        return i;
    }
    let mut best = 0;
    let mut best_gain = f32::INFINITY;
    let mut best_seq = u64::MAX;
    for (i, v) in voices.iter().enumerate() {
        let g = v.env.current_gain();
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
/// the interleaved f32 stereo master buffer (`frames * 2` samples); we add
/// the synth mix in place.
///
/// # Safety
/// Invoked by raylib on the audio thread only; `buffer` is valid for
/// `frames * 2` f32 samples for the duration of the call.
pub unsafe extern "C" fn mix(buffer: *mut std::ffi::c_void, frames: u32) {
    let buf = unsafe { std::slice::from_raw_parts_mut(buffer as *mut f32, frames as usize * 2) };
    // SAFETY: raylib calls this on a single audio thread — the sole
    // consumer of the ring and the only toucher of the voice array.
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
            attack_ms: 1.0,
            hold_ms: 0.0,
            decay_ms: 1.0,
            sustain: 0.8,
            release_ms: 5.0,
            slide_semitones: 0.0,
            slide_ms: 0.0,
        }
    }

    // Build a fresh engine (not the global) so tests don't share state.
    fn fresh() -> AudioEngine {
        AudioEngine::new()
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
                attack_ms: 8.0,
                hold_ms: 0.0,
                decay_ms: 140.0,
                sustain: 0.8,
                release_ms: 120.0,
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
            attack_ms: 1.0,
            hold_ms: 0.0,
            decay_ms: 1.0,
            sustain: 1.0,
            release_ms: 5.0,
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
            attack_ms: 0.0,
            hold_ms: 0.0,
            decay_ms: 0.0,
            sustain: 1.0,
            release_ms: 5.0,
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
