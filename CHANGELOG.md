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
- A `--window-size WxH` option of the `daw` binary, from 1280x800 to 7680x4320 with
  1440x900 as the default, and a `WindowSize` type in `daw-core` that the layout follows.
- Repository conventions adopted from `sgeos/project-template`, including the local
  gate `scripts/check.sh`, the worktree helper, the process documents and the 0BSD
  license.

### Changed

- Importing a Type 0 Standard MIDI File places each MIDI channel it uses on its own
  channel, offset from the chosen first channel, instead of merging every note into
  that one channel. A file with MIDI channels 1 and 3 imported at channel 2 now fills
  channels 2 and 4. Notes that would land past channel 8 are dropped with a warning.
- Exporting a Standard MIDI File refuses a track with notes whose ticks per beat
  differs from the project's, with the reason in the status bar, instead of writing
  its note times at the wrong resolution.
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
- The `daw-core` application module is split into files by concern, and the page code
  further into `piano_roll`, `overview`, `synth_setup` and `chains`.
- The Stereo and Mono Chain pages wrap a long chain into rows instead of running off
  the page. `WINDOW_WIDTH` and `WINDOW_HEIGHT` are replaced by `WindowSize`.
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

- Running `target/debug/daw` directly, outside `cargo run`, failed because the loader
  could not find `libSDL3_ttf.0.dylib`. The `daw` binary and its examples now carry a
  run path to the directory beside them, where the SDL3 libraries are copied, on macOS
  and on Linux.
- A project exported to a Standard MIDI File and imported again came back with
  every note on one channel, because the parser ignored the MIDI channel. The parser
  now splits each track chunk by channel, and import places a Type 0 file by MIDI
  channel from the chosen first channel. A Type 1 file still fills consecutive
  channels.
- Exporting a Standard MIDI File panicked on a gap of 2^28 ticks or more between
  events. `build_smf` now returns an error for that and for a pitch, velocity, duration,
  tempo or resolution the format cannot carry, and the export shows it in the status bar.
- A note event with a data byte above 127 on MIDI channel 16 made the parser index
  past its pending-note table and panic. Such a file is now rejected as invalid.
- The Standard MIDI File parser read each track chunk against the whole file, so an
  event or a meta or SysEx length could run past the stated end of its chunk into the
  next one, and the bound arithmetic could overflow. Every chunk is now read within its
  stated length with checked arithmetic, and a chunk that runs past the end of the file,
  a header length below six and a division of zero ticks per beat are rejected.
- Importing a Standard MIDI File into a project with notes on other channels set the
  project's ticks per beat to the file's without converting those notes, so they
  played and exported at the wrong speed. The imported notes are now converted to the
  project's resolution, with each start and end rounded half up, by the new
  `MidiTrack::rescale`. A project with no other notes still adopts the file's.
- A project loaded or imported at a resolution other than 480 ticks per beat
  played at the wrong speed, five times too fast at 96, because the transport
  stayed at 480. The transport now follows the project's ticks per beat
  through `Transport::set_ticks_per_beat`, which converts the playhead and the
  loop points as `MidiTrack::rescale` converts notes. The snap grid, and with
  it the length of a new note, follows too, and a grid subdivision stays the
  same subdivision.
- Playback ran slow at any resolution and tempo whose ticks per frame are not
  a whole number, such as 3.2 at 96 ticks per beat and 120 beats per minute,
  or 13.3 at 480 and 100, because each frame dropped its fraction of a tick.
  `Transport::advance_samples` now carries the fraction to the next frame.
- At a resolution below 24 ticks per beat, the grid key could set a snap of 0
  ticks, which a click on the piano roll then divided by, and could cycle
  between two values without reaching the others. Each subdivision is now at
  least one tick, and the key visits each distinct value once.
- A project file with 0 ticks per beat is refused at load.
- A channel script could freeze the composer with a long bounded loop, such as
  `for i in 0..10000000`, which held one tick for 34 seconds in a debug build. Keleusma
  0.2.2 admits such a loop, and its cost estimate leaves out `fn main` and loops inside
  helper functions. `ScriptRunner` now counts loop iterations and calls through VM
  breakpoints, native calls included, and stops a script that runs more than
  `MAX_STEPS_PER_TICK`, 4,096, in one tick.
- On the Synth Setup page, the GLIDE row overlapped the LPF panel, so a click on GLIDE
  Enabled dragged LPF Saturation instead of toggling glide.
- On the Stereo Chain page in orbital mode, parameter lookup searched above where the
  pedal bars were drawn.
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
