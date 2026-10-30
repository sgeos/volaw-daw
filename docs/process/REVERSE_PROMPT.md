# Reverse Prompt

> **Navigation**: [Process](./README.md)

A state file, overwritten whole after each task. It reports what was done, what was
decided and by whom, what was verified and how, what was not verified, and what the
operator needs to decide. Facts, inferences and hypotheses are marked as such.

## Report on 2026-10-30

**Task.** The operator asked that known loose ends be tied up, that the follow-up
proposed for the `volaw-64` review notes be made, and that all work be committed,
pushed and merged.

**Done.**
- `spsc::ring_buffer` asserts that its capacity is at most the new `MAX_CAPACITY`.
  Before, a capacity near `usize::MAX` wrapped in a release build to a queue that held
  nothing.
- `AudioSystem::start` rejects a command capacity above `MAX_COMMAND_CAPACITY` and a
  block above `MAX_BLOCK_SAMPLES`, returning the engine.
- `SdlAudioOutput` and `AudioSystem` document that audio must stop before `SDL_Quit`.
- `audio::runtime` and `audio::command` no longer claim that the audio thread never
  allocates, and record `Bulk` as technical debt. `LAYERS.md` lists both items.
- The scratch file left in `tmp/` was removed.

**Decided by this line, open to revision.** The bounds of 65,536 commands and
1,048,576 block samples. The default configuration of 1,024 and 8,192 is well inside
them.

**Verified.** Facts, observed on the macOS authoring host. The gate exited zero with
363 tests and 1 doctest passed, and the core built for `thumbv7em-none-eabihf`.
Pull request 5 had already merged at `3dd668a` before the
follow-up was pushed, so the follow-up goes through pull request 6, whose continuous
integration results are reported there.

**Not verified.**
- Interactive use with a real display and audio device.
- volaw's pin on `3dd668a`, which `volaw-64` reported and this line did not inspect.
- `target/debug/daw` run directly cannot find `libSDL3_ttf.0.dylib`. Inference, not
  tested on `main`. Neither `main` at `aa086d5` nor the merged branch adds a runtime
  search path, so the behaviour most likely predates the refactor.

**For the operator.**
- Run `cargo run -p daw` once and play a few notes.
- The `Bulk` debt and the shutdown-order requirement are documented but not removed.
