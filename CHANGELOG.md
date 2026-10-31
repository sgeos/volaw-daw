# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `audio::command_queue`, `CommandSender`, `CommandReceiver` and `EngineEdit`, and
  `Vco::replace_pcm_samples`, which returns the replaced table so that it is not freed
  on the audio thread.
- A Studio page, now the home page, laid out after browser-based workstations. It has
  an instrument and effect browser, a track list with mute, solo, volume and a note
  preview, and the selected track's effects rack, all usable with the mouse alone.
- A page tab row with Play, Stop and Save buttons on every page.
- ON/OFF, move and Remove buttons on every pedal, and an Add effect card at the end of
  every chain. The pedal picker shows every effect at once, with a description.
- Four effects. Wavefolder, Filter with an LFO sweep, Vibrato, and Pitch Corrector,
  which snaps pitch to a chromatic, major or minor scale in a chosen key.
- Readable parameter labels, and named values for stepped parameters such as filter
  mode and scale.
- `SdlRenderer::save_bmp` and the `screenshots` example of the `daw` crate, which
  render every page offscreen for layout checks.
- `audio-sdl`, the SDL3 audio output, and `daw-sdl`, the SDL3 renderer, input and file
  system platform for the DAW.
- `volaw-audio`, an entry library that starts the engine on the default audio device and
  returns a command sink. The DAW uses it, and it is meant to replace volaw's own bridge.
- Handoff support for Codex and Grok Build: launch commands and sandbox needs in
  `AGENTS.md`, and a tool-neutral `docs/process/RESUME_PROMPT.md`.
- `scripts/scratch.sh`, which keeps build output on a scratch volume named by
  `SCRATCH_BASE`, and scratch linking in `scripts/worktree.sh new` and `rm`, synced from
  `sgeos/project-template` at commit `3954eba`, released as `v1.0.8`.
- Repository conventions adopted from `sgeos/project-template`, including the local
  gate `scripts/check.sh`, the worktree helper, the process documents and the 0BSD
  license.

### Changed

- **Breaking for `audio`.** `EngineCommand::Bulk` carries a `Box<dyn EngineEdit>`
  instead of a `Box<dyn FnOnce(&mut AudioEngine) + Send>`. Every `FnMut` closure over
  the engine is an `EngineEdit`. `EngineRunner::new` takes the `CommandReceiver` half
  of the new `command_queue` instead of a `spsc::Consumer`. The runner returns each
  applied edit through the queue, and `CommandSender` frees it on the control thread.
  `AudioSystem`, `AudioConfig` and `AudioEngine::new` are unchanged.
- The DAW builds every new pedal chain, voice pool, modulation matrix and PCM table on
  the UI thread, through edit builders in `daw_core::engine_setup`, and the audio
  thread only swaps them in.
- `EngineRunner` renders in chunks no larger than the engine's preallocated scratch
  buffers, at most 2048 samples.
- The monophonic held-note stack holds at most 64 notes and forgets the oldest when a
  65th is held.
- The `daw-core` application module is split into files by concern.
- The chorus, flanger and vibrato depth slider spans 0 to 10 ms instead of 0 to 2000 ms.
- Escape on a setup or overview page returns to the Studio page.
- The workspace is layered into a `no_std` core, a std platform layer and entry points.
  The DAW state machine moved from the `daw` binary into `daw-core` as `DawApp`, which
  reaches files, logging and time only through the `Platform` trait. The engine command
  protocol, the lock-free queue and the audio-thread runner moved into `audio`.
- Project persistence and WAV decoding moved into `daw-core` and no longer need `std`.
- The gate builds `audio` and `daw-core` for `thumbv7em-none-eabihf` to prove they are
  `no_std`. Continuous integration installs that target.
- A malformed `--import-channel` or `--script` argument is reported and ignored instead
  of being read as channel 0.
- `.claude/worktrees/`, where Claude Code creates its worktrees, is ignored.
- The gate bypasses a configured rustc wrapper such as `sccache` unless
  `RUSTC_WRAPPER` is set, so it passes deterministically inside agent sandboxes.
- `daw-core` depends on the published `keleusma` 0.2.2 crate in place of the in-tree
  crate. Keleusma scripts must use the `Word` and `Float` type names and declare
  `TickState` before reading its fields.
- Continuous integration runs the local gate, and it lints the workflows with
  actionlint and zizmor.

### Fixed

- The Phaser `stages` slider spanned 0 to 1, so dragging it set one stage. It now spans
  1 to 8.
- Moving a pedal after switching to a track with fewer pedals could panic on an
  out-of-range index. Switching track now clears the pedal selection, and moving checks
  the index.
- Clicks while the pedal picker was open could reach the toolbar and page navigation
  under it.
- Commands still queued when the audio queue is dropped are now dropped too. Before,
  a pending boxed closure leaked at shutdown.
- The audio callback drains at most one queue capacity of commands per block, and never
  reallocates its staging buffer on the audio thread.
- The audio thread no longer allocates or frees. Before, a `Bulk` closure was freed on
  the audio thread with whatever it captured, a patch or chain rebuild allocated there,
  the first held note of a monophonic channel grew a vector there, and the first large
  callback grew the engine's scratch buffers there. Tests with a counting allocator in
  `audio` and `daw-core` check it.
- Stopping or dropping an `SdlAudioOutput`, and so an `AudioSystem`, after the host
  called `SDL_Quit` destroyed the audio stream a second time through a dangling
  pointer. The output now detects that `SDL_Quit` destroyed its stream, skips the
  second destruction and the release of the audio subsystem, and reports the state
  through the new `SdlAudioOutput::stream_lost`. Tests in `audio-sdl` exercise both
  orders on SDL's dummy audio driver.
- A project, patch, effect, MIDI or script file name that is absolute or contains `..`
  is rejected, so a project file cannot read or write outside its root.
- `spsc::ring_buffer` panics with a message for a capacity above `MAX_CAPACITY`,
  instead of wrapping in a release build to a queue that holds nothing.
  `AudioConfig` capacities and block sizes are bounded and checked by
  `AudioSystem::start`, which returns the engine on failure.
- A WAV chunk size near `u32::MAX` cannot overflow the offset arithmetic on a 32-bit
  target.
- A Keleusma script whose `TickState` declaration or `main` signature differs from the
  host layout is rejected at load. Under `keleusma` 0.2.2 such a script read the wrong
  values without any error.
- The `audio::` script functions reject NaN and infinite arguments as a script error,
  so a non-finite value cannot reach the synthesizer.
- A tick count above the range of a Keleusma `Word` saturates instead of wrapping to a
  negative value.
- A script that fails to load at startup is reported in the status bar, as hot-reload
  errors already were.
