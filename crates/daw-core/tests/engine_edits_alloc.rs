//! The edits that `DawApp` sends to a running engine must not allocate or
//! free heap memory on the audio thread.
//!
//! A counting global allocator records the current thread's heap events while
//! it is armed, as in `audio`'s `tests/audio_thread_alloc.rs`. Each test
//! builds its edits unarmed, which is the UI thread's share of the work, and
//! arms only around `EngineRunner::render_f32`, which applies them.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use audio::{
    AudioEngine, AudioSource, CommandSink, EngineCommand, EngineRunner, VoiceMode, command_queue,
};
use daw_core::engine_setup::{
    MAX_PATCH_VOICES, chain_command, effects_command, patch_command, pedal_param_command,
    position_command,
};
use daw_core::patch::{PEDAL_TYPES, PedalConfig, default_pedal_config};
use daw_core::{EffectChainConfig, PatchConfig};

struct CountingAllocator;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static EVENTS: Cell<usize> = const { Cell::new(0) };
}

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

fn heap_events<R>(work: impl FnOnce() -> R) -> (R, usize) {
    EVENTS.with(|events| events.set(0));
    ARMED.with(|armed| armed.set(true));
    let result = work();
    ARMED.with(|armed| armed.set(false));
    (result, EVENTS.with(Cell::get))
}

const SR: u32 = 48_000;

/// Every pedal type the DAW offers, with its default parameters.
fn every_pedal() -> Vec<PedalConfig> {
    PEDAL_TYPES
        .iter()
        .flat_map(|(_, names)| names.iter())
        .map(|name| default_pedal_config(name).expect(name))
        .collect()
}

/// A running engine with its command queue, rendered on the test thread.
struct Rig {
    tx: audio::CommandSender,
    runner: EngineRunner,
    out: Vec<f32>,
}

impl Rig {
    fn new() -> Self {
        Self::with_engine(AudioEngine::new(SR))
    }

    fn with_engine(engine: AudioEngine) -> Self {
        let (tx, rx) = command_queue(256);
        Self {
            tx,
            runner: EngineRunner::new(engine, rx, 8192),
            out: vec![0.0; 4096],
        }
    }

    /// Send `cmds` unarmed, apply them in one armed callback, and return the
    /// heap events of that callback. The spent edits are then reclaimed.
    fn apply(&mut self, cmds: Vec<EngineCommand>) -> usize {
        let sent = cmds.len();
        for cmd in cmds {
            assert!(self.tx.send(cmd).is_ok());
        }
        let ((), events) = heap_events(|| self.runner.render_f32(&mut self.out));
        assert_eq!(self.tx.reclaim(), sent, "every edit should come back");
        events
    }

    fn engine(&self) -> &AudioEngine {
        self.runner.engine()
    }
}

fn pcm_table(value: f64) -> Option<Arc<Vec<f64>>> {
    Some(Arc::new(vec![value; 256]))
}

#[test]
fn patch_edits_do_not_allocate_while_the_pool_grows_and_shrinks() {
    let mut rig = Rig::new();
    let mut patch = PatchConfig {
        vco1_waveform: String::from("pcm"),
        ..PatchConfig::default()
    };
    // From the default single voice to the largest pool, which outgrows the
    // engine's storage, then down to one voice and back up, each time with a
    // new table that only the engine holds.
    for (round, count) in [MAX_PATCH_VOICES, 1, 4, MAX_PATCH_VOICES]
        .into_iter()
        .enumerate()
    {
        patch.voice_count = count;
        patch.lpf_cutoff = 0.1 * (round + 1) as f64;
        let cmd = patch_command(1, &patch, pcm_table(round as f64), pcm_table(0.5));
        let events = rig.apply(vec![cmd]);
        assert_eq!(events, 0, "round {round} touched the heap {events} times");
        let synth = &rig.engine().channels[1].synth;
        assert_eq!(synth.voices.len(), usize::from(count));
        assert!(
            synth
                .voices
                .iter()
                .all(|v| v.lpf.cutoff == patch.lpf_cutoff)
        );
    }
}

#[test]
fn patch_edits_shrink_an_engine_larger_than_any_patch_without_allocating() {
    // `audio` itself does not bound a voice pool, so a host may configure an
    // engine with more voices than `MAX_PATCH_VOICES` before a patch arrives.
    let oversized = 3 * usize::from(MAX_PATCH_VOICES) + 2;
    let mut engine = AudioEngine::new(SR);
    engine.channels[1]
        .synth
        .set_voice_mode(VoiceMode::High(u8::try_from(oversized).expect("fits")));
    let mut rig = Rig::with_engine(engine);
    assert_eq!(rig.engine().channels[1].synth.voices.len(), oversized);

    let mut patch = PatchConfig::default();
    for (round, count) in [MAX_PATCH_VOICES, 1, MAX_PATCH_VOICES]
        .into_iter()
        .enumerate()
    {
        patch.voice_count = count;
        let cmd = patch_command(1, &patch, pcm_table(round as f64), None);
        let events = rig.apply(vec![cmd]);
        assert_eq!(events, 0, "round {round} touched the heap {events} times");
        assert_eq!(
            rig.engine().channels[1].synth.voices.len(),
            usize::from(count)
        );
    }
}

#[test]
fn patch_edit_matches_apply_patch_to_engine() {
    // The edit and the direct function must leave the same engine state.
    let mut patch = PatchConfig {
        voice_count: 3,
        voice_mode: String::from("low"),
        lpf_resonance: 0.77,
        ..PatchConfig::default()
    };
    patch.mod_routings.truncate(1);
    let mut rig = Rig::new();
    assert_eq!(rig.apply(vec![patch_command(2, &patch, None, None)]), 0);
    let mut direct = AudioEngine::new(SR);
    daw_core::engine_setup::apply_patch_to_engine(&mut direct, 2, &patch, None, None);
    let (a, b) = (&rig.engine().channels[2].synth, &direct.channels[2].synth);
    assert_eq!(a.voice_mode, b.voice_mode);
    assert_eq!(a.voices.len(), b.voices.len());
    assert_eq!(a.mod_matrix.routings.len(), b.mod_matrix.routings.len());
    assert_eq!(a.voices[0].lpf.resonance, b.voices[0].lpf.resonance);
}

#[test]
fn chain_effects_position_and_pedal_edits_do_not_allocate() {
    let mut rig = Rig::new();
    let sr = f64::from(SR);
    let pedals = every_pedal();
    let config = EffectChainConfig {
        pre_pedals: pedals.clone(),
        post_pedals_left: pedals.clone(),
        post_pedals_right: pedals.clone(),
        pan: 0.4,
        position_3d_enabled: true,
        ..EffectChainConfig::default()
    };

    let mut cmds: Vec<EngineCommand> = (0..3u8)
        .filter_map(|chain_id| chain_command(0, chain_id, &pedals, sr))
        .collect();
    cmds.push(effects_command(5, &config, sr));
    cmds.push(position_command(6, &config));
    assert_eq!(rig.apply(cmds), 0, "chain, effects and position edits");
    assert_eq!(rig.engine().channels[0].pre_pedals.len(), pedals.len());
    assert_eq!(
        rig.engine().channels[5].post_pedals_right.len(),
        pedals.len()
    );
    assert!(rig.engine().channels[6].position.position_3d_enabled);

    // Every parameter of every pedal, live in the chain just installed.
    let mut cmds = Vec::new();
    for (idx, pedal) in pedals.iter().enumerate() {
        for param in &pedal.params {
            cmds.push(pedal_param_command(
                0,
                0,
                idx,
                param.name.clone(),
                param.value,
            ));
        }
    }
    assert!(cmds.len() > pedals.len());
    let mut rig_params = Rig::new();
    let install = chain_command(0, 0, &pedals, sr).expect("chain 0");
    assert_eq!(rig_params.apply(vec![install]), 0);
    // In batches the queue holds, at most `cmds.len()` of them.
    while !cmds.is_empty() {
        let batch: Vec<EngineCommand> = cmds.drain(..cmds.len().min(200)).collect();
        assert_eq!(rig_params.apply(batch), 0, "pedal parameter edits");
    }

    // Replacing a full chain hands the old one back instead of freeing it.
    assert_eq!(
        rig.apply(vec![chain_command(0, 0, &[], sr).expect("chain 0")]),
        0
    );
    assert!(rig.engine().channels[0].pre_pedals.is_empty());
}

#[test]
fn unknown_chain_identifier_builds_no_edit() {
    assert!(chain_command(0, 3, &every_pedal(), f64::from(SR)).is_none());
}

#[test]
fn out_of_range_channel_is_ignored_without_allocating() {
    let mut rig = Rig::new();
    let patch = PatchConfig::default();
    let cmds = vec![
        patch_command(99, &patch, pcm_table(0.1), None),
        chain_command(99, 1, &every_pedal(), f64::from(SR)).expect("chain 1"),
        position_command(99, &EffectChainConfig::default()),
        pedal_param_command(99, 0, 0, String::from("gain"), 1.0),
    ];
    assert_eq!(rig.apply(cmds), 0);
}
