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
rendering. Nothing on the audio thread allocates or frees. A `Bulk` edit is built on
the control thread, applied on the audio thread, and returned through
`command_queue` so that `CommandSender` frees it on the control thread with whatever it
swapped out of the engine.

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

Audio should stop before the host calls `SDL_Quit`, which destroys every audio stream.
`SdlAudioOutput` checks that order at run time, because `SDL_Quit` is a free function
that no type in the workspace can constrain without a change to
`AudioSystem::start`. It registers a stream property whose cleanup SDL runs when it
destroys the stream, and a later `stop` or drop then skips the second destruction and
the release of the audio subsystem, and returns the source intact.
`tests/shutdown_order.rs` in `audio-sdl` exercises both orders on SDL's dummy driver.

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
- **Playback of composed songs is not shared.** `volaw-audio` exposes the engine and its
  command queue. Scheduling a saved project's notes lives in `DawApp`, so the game
  cannot yet play a DAW project without reimplementing it.
- **`DawApp` is split by concern, but two modules are still large.** On 2026-10-30 the
  former `app.rs` became the `app` module, with `mod.rs` for state and playback,
  `handlers.rs` for input and editing, `render.rs` for drawing, `widgets.rs` for shared
  drawing and layout helpers, and `studio.rs` for the Studio page. `handlers.rs` holds
  about 2,500 lines and `render.rs` about 1,600, and splitting them per page is
  technical debt.
- **The layout assumes a 1440 by 900 window.** `WINDOW_WIDTH` and `WINDOW_HEIGHT` are
  constants in `daw-core`, as they were in the binary.
