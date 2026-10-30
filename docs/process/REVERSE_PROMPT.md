# Reverse Prompt

> **Navigation**: [Process](./README.md)

A state file, overwritten whole after each task. It reports what was done, what was
decided and by whom, what was verified and how, what was not verified, and what the
operator needs to decide. Facts, inferences and hypotheses are marked as such.

## Report on 2026-10-30

**Task.** The operator asked that the workspace follow the operator's preferred Rust
architecture, a `no_std` core, a platform layer implementing core traits, and entry
points that wire them, with a library that both volaw and the DAW depend on.

**Done.**
- `audio` gained the engine command protocol, the lock-free queue, `EngineRunner`, and
  the `AudioSource`, `AudioOutput` and `CommandSink` traits, all moved or written as
  `no_std`.
- `daw-core` gained `DawApp`, the state machine formerly in the binary, and persistence,
  WAV decoding and engine setup. `Platform` gained `path_exists` and `modified_stamp`,
  and `MemoryPlatform` implements it for tests.
- New crates `audio-sdl` and `daw-sdl` hold the SDL3 and std code. New entry library
  `volaw-audio` wires the engine to SDL3 output. The `daw` binary only wires.
- The gate and continuous integration build the core for `thumbv7em-none-eabihf`.
- Records are in `docs/architecture/LAYERS.md` and
  `docs/decisions/LAYERED_ARCHITECTURE.md`.

**Decided by this line, open to revision.** The crate names `audio-sdl`, `daw-sdl` and
`volaw-audio`. Keeping `audio` and `daw-core` under their names so volaw's path
dependency keeps working. Rejecting project file names that escape the project root,
which changes behaviour for such a project.

**Verified.** Facts, observed on the macOS authoring host.
- The gate exited zero with 358 tests passed, against 312 on `main` at `aa086d5`. Every
  former test was moved or replaced, none removed without a successor.
- `audio` and `daw-core` build for `thumbv7em-none-eabihf`.
- volaw's `client-core` and `client-sdl` at `969dfbb` pass `cargo check --locked`
  against this tree, in a scratch copy. With `client-sdl` changed to use `volaw-audio`,
  the copy also passed `cargo check`.
- The composer started under SDL's dummy video and audio drivers, opened the audio
  stream without a warning, reported malformed arguments, and ran its frame loop for
  five seconds until stopped.

**Not verified.**
- Interactive use with a real display and audio device. Inference: the event loop,
  drawing and input code moved without change apart from mechanical substitutions, so
  behaviour should match, but no one has played a note on this tree.
- Continuous integration on this branch at the time of writing. The operator asked on
  2026-10-30 for the branch to be pushed, and the pull request reports the result.
- Running `target/debug/daw` directly fails to find `libSDL3_ttf.0.dylib`, because the
  binary has no runtime search path. `cargo run` sets the path. Inference, not tested
  by running `main`. Neither `main` at `aa086d5` nor this branch has a build script or
  linker setting that adds a search path, and both link the same `sdl3-sys` and
  `sdl3-ttf-sys` builds, so `main` most likely behaves the same.

**Keleusma status.** `Op55-V0.2.X-keleusma` reported on 2026-10-28 that the host
struct defect reproduces on keleusma v0.2.3 and that its fix awaits the keleusma
operator. `daw-core` is unaffected, because its load-time contract check rejects any
`TickState` declaration that differs from the host layout.

**For the operator.**
- Review the pull request and merge it once continuous integration passes.
- Run `cargo run -p daw` once and play a few notes.
- Decide whether the volaw line should adopt `volaw-audio` now.
