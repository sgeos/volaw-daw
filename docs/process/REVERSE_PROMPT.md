# Reverse Prompt

> **Navigation**: [Process](./README.md)

A state file, overwritten whole after each task. It reports what was done, what was
decided and by whom, what was verified and how, what was not verified, and what the
operator needs to decide. Facts, inferences and hypotheses are marked as such.

## Report on 2026-10-31

**Task.** The operator asked for a PyDromon team to fix all known bugs and open issues
while trialling the scratch volume, for every line to write a harness experience
report, and for all work to be committed, pushed and merged at the end.

**Done.**
- Line `app`, pull request 10. The page code is split into `piano_roll`, `overview`,
  `synth_setup` and `chains`. `WindowSize` and `--window-size` replace the fixed
  window constants. The Synth Setup row 3 overlap, which made a click on GLIDE Enabled
  drag LPF Saturation, is fixed, and chain pages wrap.
- Line `audio`, pull request 11. `EngineCommand::Bulk` carries a `Box<dyn EngineEdit>`,
  and applied edits return through `command_queue` to be freed off the audio thread.
  Counting-allocator tests check that edits touch no heap. `SdlAudioOutput` detects
  that `SDL_Quit` destroyed its stream. It also wrote a design proposal for shared song
  playback.
- Line `fixes`, the last pull request. The `daw` binary carries a run path to its SDL
  libraries. The MIDI file parser bounds every chunk and splits tracks by channel, and
  export returns errors instead of panicking. Channel scripts have a step budget of
  4,096 per tick. The transport follows the project's ticks per beat and keeps the
  fraction of a tick it used to drop.
- Line `luna` reviewed every task and found three defects the writing lines had missed.
- The operator's session committed each line's tree, merged `main` into each branch in
  turn, and ported five `Bulk` call sites into the split page files.

**Decided by the operator's session, under the operator's instruction.** The breaking
`audio` change, after checking that volaw uses none of the changed items. The step
budget of 4,096. Equality rather than conversion when exporting tracks at mixed
resolutions. Converting imported notes when other channels hold notes.

**Verified.** Facts, local macOS, inside Claude Code's sandbox.
- The gate passed on every branch after each merge with `main`. The last run counted
  `audio` 262 and 4, `audio-sdl` 5 and 5, `daw` 7 and 1, `daw-core` 195 and 6, `daw-sdl`
  4 and `volaw-audio` 5, and 1 doctest, none failed or ignored.
- Continuous integration passed on pull requests 10 and 11 before they merged.
- The disclosure scan cannot run in a worktree, which has no `secret/`. The terms were
  checked by hand against each branch's diff, and the lines checked their own files.
- Each new test was shown by a mutation check to fail without its fix, per the lines.

**Not verified.**
- Interactive use on a real display and audio device.
- The run path on Linux, except through continuous integration.
- `cargo clean` with the scratch link.

**For the operator.** The five questions in section 7 of
`tmp/proposal-shared-playback.md`, and passing `tmp/harness-reports/` to the pydromon
implementation line.
