# Layers

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

**Status.** Decided and implemented on 2026-10-28. The reasons are in
[the decision record](../decisions/LAYERED_ARCHITECTURE.md).

The workspace has three layers. Code depends only downward. A crate in a lower layer
never names a crate above it.

| Layer | Crate | `std` | Role |
|-------|-------|-------|------|
| Core | `audio` | no | Synthesizer engine, effect chain, `EngineCommand` protocol, lock-free `spsc` queue, `EngineRunner`, and the `AudioSource`, `AudioOutput` and `CommandSink` traits |
| Core | `daw-core` | no | `DawApp` editing state machine, MIDI tracks, transport, patches, presets, project persistence as TOML, WAV decoding, the SMF parser, Keleusma scripts, and the `Platform` and `Renderer` traits |
| Platform | `audio-sdl` | yes | `SdlAudioOutput`, which implements `audio::AudioOutput` over the SDL3 audio stream |
| Platform | `daw-sdl` | yes | `SdlPlatform`, which implements `daw_core::Platform` over the file system and clock, `SdlRenderer`, which implements `daw_core::Renderer`, and SDL3 event translation |
| Entry | `volaw-audio` | yes | Library that wires the engine to `audio-sdl`. The DAW uses it, and the volaw game client is meant to |
| Entry | `daw` | yes | The composer binary, which parses arguments and wires `daw-core`, `daw-sdl` and `volaw-audio` |

## Core

The core holds every decision about music, editing and file formats. It reaches the
outside world only through traits it defines. `Platform` supplies files, a modification
stamp, logging and a monotonic clock. `Renderer` draws. `CommandSink` carries engine
commands to wherever the engine runs. `AudioSource` is what an audio device pulls
samples from, and `EngineRunner` implements it by draining queued commands and then
rendering.

`daw-core` provides `MemoryPlatform`, an in-memory `Platform` with a manual clock. The
application tests use it, so the save, load, export, script reload and preview timing
paths are tested without touching the disk or waiting on time.

The gate proves the core is `no_std` by building `audio` and `daw-core` for the
bare-metal target `thumbv7em-none-eabihf`. A host build cannot prove it, because a
test module that declares `extern crate std` brings std's float methods into scope for
the whole crate. This happened during the move and was caught only by the bare-metal
build.

## Platform

The platform crates hold all `unsafe` foreign function interface code and all use of
`std`. `SdlAudioOutput` owns the boxed source that it hands SDL as callback userdata,
and reclaims it after destroying the stream. Earlier, the binary leaked and reclaimed
that box itself. `SdlAudioOutput` initializes the SDL audio subsystem itself, which SDL
reference counts, so a host may start audio before or after its own `SDL_Init`.

## Entry

`volaw-audio` is the one crate a host needs for sound. It re-exports `audio`, and
`AudioSystem::start` moves an engine to the audio thread and returns a `CommandSink`.
If the device cannot open, the engine comes back inside `StartFailure`. The `daw`
binary configures the engine from the loaded project before starting it, and keeps an
`Option<AudioSystem>` as its sink, which discards commands when audio is unavailable.

## Open

- **volaw pins an unmerged commit.** The volaw line reported on 2026-10-30 that volaw
  now depends on `volaw-audio` through a git dependency on `3dd668a`, the head of this
  repository's pull request 5, and will repin to the merge commit. This line has not
  inspected volaw's tree. Until the repin, that commit must stay reachable, so the
  branch is not rebased or force-pushed.
- **`Bulk` commands allocate on the audio thread.** An `EngineCommand::Bulk` closure
  runs on the audio thread and its box is freed there, as `audio::runtime` records. The
  DAW sends `Bulk` only on cold paths. Removing it is technical debt.
- **Audio must stop before `SDL_Quit`.** `SDL_Quit` destroys every audio stream, so an
  `SdlAudioOutput` or `AudioSystem` dropped afterwards destroys its stream twice. The
  requirement is documented on both types but cannot be checked at run time.
- **Playback of composed songs is not shared.** `volaw-audio` exposes the engine and its
  command queue. Scheduling a saved project's notes lives in `DawApp`, so the game
  cannot yet play a DAW project without reimplementing it.
- **`app.rs` is large.** `DawApp` was moved whole, about 5,800 lines, so that the move
  stayed reviewable. Splitting it into input, playback and drawing modules is
  technical debt.
- **The layout assumes a 1440 by 900 window.** `WINDOW_WIDTH` and `WINDOW_HEIGHT` are
  constants in `daw-core`, as they were in the binary.
