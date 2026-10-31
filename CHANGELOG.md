# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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

- Importing a Type 0 Standard MIDI File places each MIDI channel it uses on its own
  channel, offset from the chosen first channel, instead of merging every note into
  that one channel. A file with MIDI channels 1 and 3 imported at channel 2 now fills
  channels 2 and 4. Notes that would land past channel 8 are dropped with a warning.
- Exporting a Standard MIDI File refuses a track with notes whose ticks per beat
  differs from the project's, with the reason in the status bar, instead of writing
  its note times at the wrong resolution.
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
- A channel script could freeze the composer with a long bounded loop, such as
  `for i in 0..10000000`, which held one tick for 34 seconds in a debug build. Keleusma
  0.2.2 admits such a loop, and its cost estimate leaves out `fn main` and loops inside
  helper functions. `ScriptRunner` now counts loop iterations and calls through VM
  breakpoints, native calls included, and stops a script that runs more than
  `MAX_STEPS_PER_TICK`, 4,096, in one tick.
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
