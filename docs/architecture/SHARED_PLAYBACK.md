# Proposal: shared song playback

**Proposed**, not implemented. Written by line `audio` on 2026-10-31 against
`f3d3b64` with the changes later merged in pull request 11, and against a reading of the
game client's source. The open decisions are in section 7.

**Status since writing.** Pull request 12 fixed defect 2, the dropped fraction of a
tick, inside `Transport`. Pull request 10 split `app/` by page, so the merge risk in
section 9 no longer applies. The other defects stand.

## 1. Problem

The game cannot play a song composed in the DAW without reimplementing the
scheduling that lives in `DawApp`. `docs/architecture/LAYERS.md` records this as
open. The scheduling is `DawApp::advance_playback` in
`crates/daw-core/src/app/mod.rs`, about 180 lines. It mixes four concerns.

| Concern | Where it belongs |
|---|---|
| Note on and note off from the tracks, loop wrap, mute | shared playback |
| Orbital position of each channel, derived from the playhead | shared playback |
| Keleusma channel scripts | shared, behind an option, see section 6 |
| Metronome clicks, playhead auto-scroll, status messages | the DAW only |

The game client starts an `AudioSystem` and sends no engine commands. It has no song
model.

## 2. Defects in the current scheduling

The following were found while reading the code. Each is marked as fact or
inference. A shared module should fix them rather than carry them into the game.

1. **Tempo follows the frame rate.** Fact. `advance_playback` advances the transport by
   `sample_rate / 60` samples on every call, and the `daw` binary calls it once per
   frame with `SDL_Delay(16)` plus the render time between calls. Inference. Any frame
   longer than 16.7 ms makes the song play slower than its tempo, and the error is not
   corrected later, because nothing measures elapsed time.
2. **Fractional ticks are lost every frame.** Fact. `Transport::advance_samples`
   truncates `seconds_to_ticks` to an integer on each call. At 48 kHz, 480 ticks per
   beat and 100 beats per minute, one frame is 13.33 ticks and advances 13, so the song
   runs 2.5 percent slow. At 120 beats per minute it is exactly 16 ticks, in `f64`
   arithmetic, checked with the same order of operations as `seconds_to_ticks`, so the
   default project hides the defect.
3. **Each frame scans every note of every channel.** Fact. The note-on search iterates
   all of `track.notes` for each of 8 channels. Inference. This is linear in the song
   length per frame, which is acceptable in the DAW and wasteful in a game frame
   budget.
4. **Commands are dropped when the queue is full.** Fact. `send_command` logs a warning
   and discards the command. A dropped note off leaves a note stuck until the next
   loop wrap or stop.
5. **Timed events are not played.** Fact. `DawProject::events`, holding `FeedSample`
   events, is saved and loaded but nothing schedules it. Whether that is intended is
   not known.
6. **Timing is quantized to the frame.** Fact. A note starts in the frame after its
   tick, so its onset jitters by up to one frame, about 17 ms. Inference. This is
   audible on fast passages and is the main argument for audio-thread scheduling in
   section 5.

## 3. Goals and non-goals

Goals.

- One implementation of song playback that the DAW and the game both call.
- It is `no_std` with `alloc`, in the core layer, and names no platform crate.
- Time comes from the caller in audio frames, never from an assumed frame rate.
- It allocates nothing per advance, so a game can call it in its frame loop.
- It never drops a command silently. A full queue delays playback and is reported.
- The game depends on it without depending on `daw-core`, which carries the editor,
  Keleusma, TOML and the page state machine.
- No change to `AudioSystem::start`, `AudioConfig` or `AudioEngine::new`.

Non-goals for the first phase.

- Sample-accurate onsets. Section 5 keeps this as a second phase.
- Editing. The DAW keeps every editing concern.
- Mixing several songs at once, or crossfades. A game can build those on top.

## 4. Proposed design

### 4.1 A new core crate, `song`

A new crate `crates/song`, `no_std` with `alloc`, depending only on `audio`. It sits
in the core layer beside `audio` and `daw-core`, and the gate's
`thumbv7em-none-eabihf` build covers it. `daw-core` depends on it and re-exports what
it moves. `volaw-audio` re-exports it as it re-exports `audio`, so the game keeps one
dependency. That re-export is an additive change to the public API of `volaw-audio`,
and needs the operator's approval.

The alternatives were weighed as follows.

| Option | Judgement |
|---|---|
| A module in `daw-core` that the game depends on | Rejected. The game would compile Keleusma, TOML, serde and the editor |
| A module in `audio` | Rejected. It mixes musical structure into the synthesizer, and widens the API the game pins |
| A new crate `song` | Recommended. It is small, the game takes only what it needs, and the layers stay intact |

### 4.2 Types

A sketch, not a final signature.

```rust
/// An immutable, validated song, compiled from a project or a bundle.
pub struct Song {
    tempo: f64,                       // beats per minute, validated finite and positive
    ticks_per_beat: u16,              // nonzero
    beats_per_bar: u8,
    beat_denominator: u8,             // power of two
    loop_region: Option<(u64, u64)>,  // start < end, in ticks
    channels: [SongChannel; NUM_CHANNELS],
}

pub struct SongChannel {
    /// Sorted by tick. At equal ticks, offs precede ons, so a repeated pitch
    /// retriggers instead of being cut by its own predecessor's release.
    events: Vec<NoteEvent>,
    orbit: Option<OrbitalPath>,
    volume: f64,
    pan: f64,
    muted: bool,
}

pub struct NoteEvent { tick: u64, kind: NoteKind }
pub enum NoteKind { On { freq: f64, velocity: f64 }, Off { freq: f64 } }

pub struct SongPlayer {
    song: Song,
    sample_rate: u32,
    /// Position in ticks as a fixed-point value with 32 fractional bits, so
    /// that no fraction of a tick is lost between advances.
    position: u64,
    cursors: [usize; NUM_CHANNELS],
    sounding: [u16; NUM_CHANNELS],   // ons sent without their offs
    pending: Option<EngineCommand>,  // the one command a full queue refused
    state: PlayState,                // Stopped, Playing, Paused
    solo: Option<usize>,
}
```

`Song::from_tracks` compiles `MidiTrack`s into event lists once, which removes the
per-frame scan of defect 3. A player keeps a cursor per channel and only ever looks at
the next event, so an advance costs time in proportion to the events it emits.

### 4.3 The advance contract

```rust
impl SongPlayer {
    pub fn advance(&mut self, frames: u32, sink: &mut impl CommandSink) -> Advance;
    pub fn play(&mut self);
    pub fn pause(&mut self, sink: &mut impl CommandSink);   // releases sounding notes
    pub fn stop(&mut self, sink: &mut impl CommandSink);    // pause and rewind
    pub fn seek(&mut self, tick: u64, sink: &mut impl CommandSink);
    pub fn set_muted(&mut self, channel: usize, muted: bool, sink: &mut impl CommandSink);
    pub fn position_ticks(&self) -> u64;
}

pub struct Advance { pub sent: u32, pub blocked: bool, pub looped: bool }
```

- `frames` is the number of audio frames that elapsed. The DAW and the game derive it
  from a clock, which fixes defects 1 and 2. Section 5.1 discusses which clock.
- Events in the half-open tick interval since the last advance are sent in order. A
  loop wrap releases sounding notes, then continues from the loop start, as today.
- When the sink refuses a command, the player keeps it in `pending`, stops this
  advance, and returns `blocked`. The next advance sends it first. The playhead keeps
  moving, so late events are sent late rather than lost. That fixes defect 4. A host
  that sees `blocked` repeatedly can log it, as the DAW does now.
- An advance is bounded. It sends at most a configurable number of commands, by
  default the queue capacity, so a long pause or a seek cannot flood the queue.
- It allocates nothing. Every buffer is sized when the song is compiled.
- Orbital positions are sent at most once per advance per channel, as now.

### 4.4 What moves, and what stays

| Item | Today | Proposed |
|---|---|---|
| `Transport` tick arithmetic | `daw-core::transport` | `song`, with the fixed-point position |
| Note and loop scheduling | `DawApp::advance_playback` | `song::SongPlayer` |
| `compute_orbital_position` | `daw-core` | `song` |
| `MidiNote`, `MidiTrack::note_to_freq` | `daw-core::midi` | `song`, re-exported by `daw-core` |
| `PatchConfig`, `EffectChainConfig` and the engine builders in `engine_setup` | `daw-core` | Second step, see section 4.5 |
| Metronome, auto-scroll, status messages, preview notes | `DawApp` | stay in `DawApp`, reading the player's position |
| Editing, undo, selection, MIDI import and export | `DawApp` | stay |

`DawApp` keeps a `SongPlayer` and rebuilds the affected channel's events after each
edit, through a `SongPlayer::replace_channel` that releases that channel's sounding
notes and moves its cursor to the playhead by binary search. An edit costs time in
proportion to that channel's notes, which is far below a frame.

### 4.5 Getting a song into the game

The game needs the song's notes and the engine state, meaning patches, effect chains
and PCM tables. Two routes exist.

1. **The game reads DAW projects.** It would need the project layout, TOML, the patch
   and effect formats, WAV decoding, and a file system trait. That is most of
   `daw-core::persistence`.
2. **The DAW exports a song bundle, and the game loads it.** Recommended. The bundle
   is one file holding the compiled events, the tempo and the loop, the patch and
   effect configurations, and the PCM tables it uses. `song::Song::from_bundle(&[u8])`
   validates every field, as an untrusted input, and returns an error rather than
   panicking. The game ships bundles as assets, and the DAW's project format stays
   free to change.

Configuring the engine for a song needs `PatchConfig`, `EffectChainConfig` and the
builders now in `daw-core::engine_setup`. They move to `song` in a second step,
together with `configure_engine`. They depend only on `audio`, `alloc` and string
parsing, so the move is mechanical. It is still a large diff, which is why it is a
separate step. The builders already produce allocation-free edits, so the game
inherits that property.

## 5. Where scheduling runs

### 5.1 Phase 1, the control thread with an audio clock

The player runs where it runs today, on the DAW's UI thread or the game's frame loop.
The open question is the source of `frames`.

| Clock | Accuracy | Cost |
|---|---|---|
| Wall clock through `Platform::elapsed_ms` | Tracks real time, drifts against the device clock over minutes | None, the trait exists |
| Frames rendered by `EngineRunner`, published through an atomic counter | Locked to the device, no drift | An additive API in `audio` and `volaw-audio` |

Recommendation. Start with the wall clock, converted to frames with the remainder
carried, because it needs no API change and already fixes defects 1 and 2. Add the
rendered-frame counter afterwards if drift is measured. Section 7 lists it as an open
decision.

### 5.2 Phase 2, optional, sample-accurate onsets on the audio thread

Defect 6 remains in phase 1. Removing it requires scheduling inside the callback. The
compiled event lists are already allocation-free to walk, so a `SongPlayer` could run
inside `EngineRunner`, and emit events at their exact frame within a block. That
needs the following.

- An `EngineRunner` hook, or a sequencer type in `audio`, which is an API change.
- Control commands such as play, stop, seek and mute sent as fixed-size engine
  commands, and the song itself handed over as a `Bulk` edit that swaps it in.
- Scripts staying on the control thread, because Keleusma allocates.

This is worth doing only if the game or the composer needs tighter timing than one
frame. It is recorded here so that the phase 1 design does not foreclose it. The
player's state is plain data, and its advance takes a frame count, so the same type
can later run on either thread.

## 6. Keleusma scripts

Channel scripts generate commands at playback time, so they cannot be compiled into a
bundle. `ScriptRunner::tick` allocates a `Vec` and several `String`s per call, so it
must stay off the audio thread. Options.

1. **Scripts stay in the DAW.** The bundle contains notes only, and a scripted channel
   is silent in the game. Simplest, and it does not affect the game's dependencies.
2. **`song` gains an optional `scripts` feature** that depends on `keleusma` and runs
   scripts beside the player, on the control thread.
3. **The DAW renders scripted channels to notes on export,** by running the script over
   the song length. Deterministic only if the script is.

Recommendation. Option 1 now, with option 3 as an export choice later. The operator,
and the owner of the game's audio design, should decide whether scripted music is a
game requirement.

## 7. Open decisions for the operator

1. Approve a new core crate `song`, and its re-export from `volaw-audio`, an additive
   change to the API the game uses.
2. The clock for phase 1. The wall clock, as recommended, or the rendered-frame
   counter now.
3. The bundle format. A serde format already in the tree, TOML, is readable but large
   for PCM data. A small versioned binary format is compact but needs its own parser
   and tests. Recommendation. A binary format with a version field and a length check
   on every section, with PCM stored as `f32`.
4. Whether `FeedSample` timed events, defect 5, are meant to play.
5. Whether scripted channels must play in the game, section 6.

## 8. Plan, if approved

Each step keeps the gate green and the DAW's behaviour unchanged, except where a step
fixes a listed defect, which its tests then pin.

1. Create `song` with `Song`, `SongPlayer` and the fixed-point position. Unit tests
   cover the following.
   - Event order at equal ticks.
   - Loop wrap releasing notes.
   - Tempo accuracy over a long run at 100 beats per minute against an exact rational
     expectation, which fails on today's `Transport`.
   - A refused command resent first on the next advance.
   - A bounded advance after a long pause.
   - Zero heap events per advance under the counting allocator used in
     `tests/audio_thread_alloc.rs`.
2. Make `DawApp::advance_playback` delegate to a `SongPlayer`, passing frames from
   `Platform::elapsed_ms`. Keep the metronome and auto-scroll in `DawApp`. The existing
   `daw-core` tests must pass unchanged.
3. Move `PatchConfig`, `EffectChainConfig`, the engine builders and `configure_engine`
   to `song`, re-exported by `daw-core`.
4. Add the bundle export to the DAW and `Song::from_bundle`, with round-trip tests,
   tests that reject truncated and oversized input, and a fuzz-style test over
   corrupted bytes.
5. Re-export `song` from `volaw-audio`, after approval, and add an example that plays a
   bundle with `AudioSystem`.
6. Optionally, the rendered-frame clock, then phase 2.

## 9. Risks

- **Merge conflict with the `app` line.** Its split of `handlers.rs` and `render.rs` by
  page, and step 2 here, both touch `app/`. Step 2 should follow that split.
- **Behaviour change in the DAW.** Fixing defects 1 and 2 changes how fast a song plays
  on a slow frame or at a tempo that is not a multiple of the frame tick. That is
  intended, but a composer may notice it.
- **The game's pin.** volaw pins `volaw-audio` by commit. Steps 1 to 4 do not change
  its API, and step 5 is additive. The `EngineCommand::Bulk` change from task 1 already
  breaks any game code that builds `Bulk`, and the game has none today.
- **Unverified assumptions.** The 16.7 ms frame and the cost of the linear scan are
  inferred from code, not measured. The game's source was read at one commit, and
  its audio plans were not consulted beyond its source.
