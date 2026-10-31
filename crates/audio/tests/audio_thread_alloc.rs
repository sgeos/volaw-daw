//! The audio thread must not allocate or free heap memory.
//!
//! A counting global allocator records every allocation, reallocation and
//! deallocation made by the current thread while it is armed. Each test arms
//! it only around the work that the audio callback does, which is
//! `EngineRunner::render_f32` and the commands it drains, so test setup on the
//! same thread and other tests running in parallel are not counted.
//!
//! The allocator is global to this test binary only, which is why these tests
//! live in their own integration test rather than in the library.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, ThreadId};

use audio::{
    AudioEngine, AudioSource, CommandSink, EngineCommand, EngineRunner, Pedal, PedalChain,
    VoiceMode, Waveform, command_queue,
};

struct CountingAllocator;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static EVENTS: Cell<usize> = const { Cell::new(0) };
}

/// Count one heap event if this thread is armed. `try_with` keeps the
/// allocator usable while thread-local storage is being torn down.
fn record() {
    let armed = ARMED.try_with(Cell::get).unwrap_or(false);
    if armed {
        let _ = EVENTS.try_with(|events| events.set(events.get() + 1));
    }
}

// Safety: every method forwards to `System` with the caller's arguments
// unchanged, and `record` neither allocates nor unwinds.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record();
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Run `work` with this thread armed, and return the heap events it caused.
fn heap_events<R>(work: impl FnOnce() -> R) -> (R, usize) {
    EVENTS.with(|events| events.set(0));
    ARMED.with(|armed| armed.set(true));
    let result = work();
    ARMED.with(|armed| armed.set(false));
    (result, EVENTS.with(Cell::get))
}

const SR: u32 = 48_000;

/// The block SDL hands the callback, in interleaved samples. It is larger
/// than the runner's staging buffer, so it also covers chunked rendering.
const CALLBACK_SAMPLES: usize = 4096;

/// Every pedal type that `Pedal::from_name` knows.
const PEDAL_NAMES: [&str; 29] = [
    "overdrive",
    "distortion",
    "fuzz",
    "tape_saturation",
    "auto_wah",
    "wah_manual",
    "chorus",
    "flanger",
    "phaser",
    "tremolo",
    "digital_delay",
    "analog_delay",
    "ping_pong_delay",
    "plate_reverb",
    "hall_reverb",
    "room_reverb",
    "bitcrusher",
    "ring_modulator",
    "pitch_shifter",
    "auto_pan",
    "noise_gate",
    "compressor",
    "eq",
    "volume",
    "buffer",
    "wavefolder",
    "filter",
    "vibrato",
    "pitch_corrector",
];

fn full_chain() -> PedalChain {
    let mut chain = PedalChain::new();
    for name in PEDAL_NAMES {
        chain.push(Pedal::from_name(name, f64::from(SR)).expect(name));
    }
    chain.active = true;
    chain
}

/// An engine that exercises every stage of the signal path: all pedals in
/// every chain position, a polyphonic channel, a PCM oscillator, positional
/// audio and the metronome. It is built before any thread is armed.
fn busy_engine() -> AudioEngine {
    let mut engine = AudioEngine::new(SR);
    engine.channels[0].pre_pedals = full_chain();
    engine.channels[1].post_pedals_left = full_chain();
    engine.channels[1].post_pedals_right = full_chain();
    engine.channels[2].synth.set_voice_mode(VoiceMode::High(8));
    let table: Vec<f64> = (0..256).map(|i| f64::from(i) / 256.0 - 0.5).collect();
    let table = Arc::new(table);
    for voice in &mut engine.channels[3].synth.voices {
        voice.vco1.waveform = Waveform::Pcm;
        voice.vco1.set_pcm_samples(Arc::clone(&table));
    }
    engine.metronome.enabled = true;
    engine
}

/// One of every fixed-size command, aimed at channel `ch`.
fn hot_commands(ch: u8) -> Vec<EngineCommand> {
    vec![
        EngineCommand::NoteOn {
            channel: ch,
            freq: 220.0,
            velocity: 0.9,
        },
        EngineCommand::SetPosition {
            channel: ch,
            azimuth_deg: 30.0,
            elevation_deg: 10.0,
            distance: 2.0,
        },
        EngineCommand::SetChannelEnabled {
            channel: ch,
            enabled: true,
        },
        EngineCommand::SetChannelVolume {
            channel: ch,
            left: 0.7,
            right: 0.6,
        },
        EngineCommand::SetLpfCutoff {
            channel: ch,
            value: 0.6,
        },
        EngineCommand::SetLpfResonance {
            channel: ch,
            value: 0.3,
        },
        EngineCommand::SetHpfCutoff {
            channel: ch,
            value: 0.1,
        },
        EngineCommand::SetHpfResonance {
            channel: ch,
            value: 0.2,
        },
        EngineCommand::SetVcoMix {
            channel: ch,
            value: 0.4,
        },
        EngineCommand::SetVco1PitchFine {
            channel: ch,
            value: 0.1,
        },
        EngineCommand::SetVco2PitchFine {
            channel: ch,
            value: -0.1,
        },
        EngineCommand::SetEg1Adsr {
            channel: ch,
            attack: 0.01,
            decay: 0.2,
            sustain: 0.5,
            release: 0.3,
        },
        EngineCommand::SetEg2Adsr {
            channel: ch,
            attack: 0.01,
            decay: 0.2,
            sustain: 0.5,
            release: 0.3,
        },
        EngineCommand::SetLfoRate {
            channel: ch,
            value: 3.0,
        },
        EngineCommand::SetVolume {
            channel: ch,
            left: 0.8,
            right: 0.8,
        },
        EngineCommand::SetPan {
            channel: ch,
            pan: -0.3,
        },
        EngineCommand::SetVco1PulseWidth {
            channel: ch,
            value: 0.3,
        },
        EngineCommand::SetVco2PulseWidth {
            channel: ch,
            value: 0.7,
        },
        EngineCommand::ClickBeat { sample_rate: SR },
        EngineCommand::ClickDownbeat { sample_rate: SR },
        EngineCommand::SetMetronomeEnabled { enabled: true },
        EngineCommand::NoteOffFreq {
            channel: ch,
            freq: 220.0,
        },
        EngineCommand::NoteOff { channel: ch },
    ]
}

#[test]
fn the_counter_sees_heap_events() {
    // Guards the other tests against passing because nothing is counted.
    let ((), events) = heap_events(|| drop(std::hint::black_box(vec![0u8; 64])));
    assert!(
        events >= 2,
        "allocation and free should count, saw {events}"
    );
    let ((), events) = heap_events(|| ());
    assert_eq!(events, 0);
}

#[test]
fn rendering_and_fixed_size_commands_do_not_allocate() {
    let (mut tx, rx) = command_queue(1024);
    let mut runner = EngineRunner::new(busy_engine(), rx, 8192);
    let mut out = vec![0.0f32; CALLBACK_SAMPLES];
    for round in 0..4u8 {
        for ch in 0..8u8 {
            for cmd in hot_commands(ch) {
                assert!(tx.send(cmd).is_ok());
            }
        }
        // Hold more notes than the monophonic stack remembers, and more than
        // the polyphonic channel has voices, without releasing them.
        for i in 0..100u16 {
            let note = EngineCommand::NoteOn {
                channel: (i % 4) as u8,
                freq: 110.0 + f64::from(i) + f64::from(round),
                velocity: 1.0,
            };
            assert!(tx.send(note).is_ok());
        }
        let ((), events) = heap_events(|| {
            for _ in 0..3 {
                runner.render_f32(&mut out);
            }
        });
        assert_eq!(events, 0, "round {round} touched the heap {events} times");
    }
    assert!(
        out.iter().any(|&s| s != 0.0),
        "the engine should be audible"
    );
    assert!(out.iter().all(|s| s.is_finite()));
}

/// Edits of the kinds a host sends. Each builds its new state here, on the
/// sending thread, and only swaps or writes on the audio thread.
fn swapping_edits(table: &Arc<Vec<f64>>) -> Vec<EngineCommand> {
    let mut chain = full_chain();
    let mut pcm = Some(Arc::clone(table));
    let mut old_pcm: Vec<Arc<Vec<f64>>> = Vec::with_capacity(16);
    vec![
        EngineCommand::Bulk(Box::new(move |engine: &mut AudioEngine| {
            std::mem::swap(&mut engine.channels[4].pre_pedals, &mut chain);
        })),
        EngineCommand::Bulk(Box::new(|engine: &mut AudioEngine| {
            for pedal in &mut engine.channels[0].pre_pedals.pedals {
                pedal.set_param("mix", 0.5);
            }
        })),
        EngineCommand::Bulk(Box::new(move |engine: &mut AudioEngine| {
            for voice in &mut engine.channels[3].synth.voices {
                if let Some(old) = voice.vco1.replace_pcm_samples(pcm.clone()) {
                    old_pcm.push(old);
                }
            }
            pcm = None;
        })),
    ]
}

#[test]
fn bulk_edits_do_not_allocate_and_are_freed_by_the_sender() {
    let (mut tx, rx) = command_queue(64);
    let mut runner = EngineRunner::new(busy_engine(), rx, 8192);
    let mut out = vec![0.0f32; CALLBACK_SAMPLES];
    // A table the engine holds alone, so that replacing it would free it on
    // the audio thread unless the edit carries it back.
    let new_table = Arc::new(vec![0.25; 128]);
    for cmd in swapping_edits(&new_table) {
        assert!(tx.send(cmd).is_ok());
    }
    let ((), events) = heap_events(|| runner.render_f32(&mut out));
    assert_eq!(events, 0, "applying edits touched the heap {events} times");
    assert_eq!(
        runner.engine().channels[4].pre_pedals.len(),
        PEDAL_NAMES.len()
    );
    // The empty chain, the old table and the three boxes come back here.
    let (freed, events) = heap_events(|| tx.reclaim());
    assert_eq!(freed, 3);
    assert!(
        events >= 3,
        "the sender frees the spent boxes, saw {events}"
    );
}

/// Records the thread that drops it.
struct DropProbe(Arc<Mutex<Option<ThreadId>>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        *self.0.lock().unwrap() = Some(thread::current().id());
    }
}

#[test]
fn a_spent_edit_is_freed_on_the_sending_thread_not_the_audio_thread() {
    let (mut tx, rx) = command_queue(64);
    let runner = EngineRunner::new(AudioEngine::new(SR), rx, 8192);
    let applied = Arc::new(AtomicBool::new(false));
    let dropped_on = Arc::new(Mutex::new(None));

    // The audio thread renders, armed, until the edit has been applied.
    let audio_applied = Arc::clone(&applied);
    let audio = thread::spawn(move || {
        let mut runner = runner;
        let mut out = vec![0.0f32; CALLBACK_SAMPLES];
        let mut events = 0;
        for _ in 0..10_000 {
            let ((), n) = heap_events(|| runner.render_f32(&mut out));
            events += n;
            if audio_applied.load(Ordering::Acquire) {
                break;
            }
            thread::yield_now();
        }
        (thread::current().id(), events, runner)
    });

    let probe = DropProbe(Arc::clone(&dropped_on));
    let edit_applied = Arc::clone(&applied);
    let edit = move |engine: &mut AudioEngine| {
        let _ = &probe;
        engine.master_volume = 0.5;
        edit_applied.store(true, Ordering::Release);
    };
    assert!(tx.send(EngineCommand::Bulk(Box::new(edit))).is_ok());

    let (audio_id, audio_events, runner) = audio.join().unwrap();
    assert!(applied.load(Ordering::Acquire), "the edit should have run");
    assert_eq!(runner.engine().master_volume, 0.5);
    assert_eq!(audio_events, 0, "the audio thread touched the heap");
    assert!(
        dropped_on.lock().unwrap().is_none(),
        "not freed before reclaim"
    );

    assert_eq!(tx.reclaim(), 1);
    let dropped_on = dropped_on.lock().unwrap().expect("reclaim drops the edit");
    assert_eq!(dropped_on, thread::current().id());
    assert_ne!(dropped_on, audio_id);
}
