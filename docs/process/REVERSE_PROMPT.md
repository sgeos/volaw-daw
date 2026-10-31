# Reverse Prompt

> **Navigation**: [Process](./README.md)

A state file, overwritten whole after each task. It reports what was done, what was
decided and by whom, what was verified and how, what was not verified, and what the
operator needs to decide. Facts, inferences and hypotheses are marked as such.

## Report on 2026-10-31

**Task.** The operator asked for the project to be put into a nominal holding state, for
all work to be committed, pushed and merged, and for a consolidated list of the concerns
that need decisions, with the obvious choices for each.

**Done.**
- The shared playback proposal moved from local scratch into
  [`../architecture/SHARED_PLAYBACK.md`](../architecture/SHARED_PLAYBACK.md), marked
  Proposed. Details of the private game repository, its commits and crate names, were
  removed. A status note records that defect 2 is fixed.
- `HANDOFF.md` describes the holding state. The merged remote branches of pull requests
  9 to 12 and the local `chore/scratch-volume` branch were deleted.

**Verified.** Facts, local macOS. The gate passed on `main` at `2282ff8` with the
disclosure scan, `audio` 262 and 4, `audio-sdl` 5 and 5, `daw` 7 and 1, `daw-core` 195 and
6, `daw-sdl` 4, `volaw-audio` 5 and 1 doctest. This branch changes documentation only.

**Not verified.** Interactive use, and a `cargo clean` with the scratch link.

## Decisions for the operator

Each concern is followed by its obvious choices. The first choice is the one this
session would recommend, where it has a recommendation, and the reason is given.

1. **Shared song playback, approve the design.** A new `no_std` crate `song` with a
   `SongPlayer`, re-exported from `volaw-audio`.
   - Approve as proposed. This is additive to the game's interface and fixes most
     playback defects in one place.
   - Approve the crate, and defer the re-export until the game needs songs.
   - Decline, keep scheduling in `DawApp`, and fix the defects in place.
2. **Playback clock.**
   - The wall clock through `Platform`, as the proposal recommends. It needs no change
     to `audio`.
   - A counter of rendered frames published by `EngineRunner`. It is locked to the
     device but adds to the public interface of `audio`.
   - Keep counting interface frames. Tempo then keeps following the frame rate.
3. **Song bundle format for the game.**
   - A small versioned binary format with a length check on every section, as
     recommended. It is compact but needs its own parser and tests.
   - TOML, already in the tree. It is readable but large for sample data.
   - Defer until the game needs songs.
4. **`FeedSample` timed events.** They are saved and loaded but never played.
   - Schedule them in playback.
   - Remove them from the project format.
   - Leave them unplayed and document that they are reserved.
5. **Scripted channels in the game.**
   - Render scripts into ordinary events at export, so the game needs no script engine.
   - Run Keleusma in the game as the composer does.
   - Defer, and export only unscripted channels for now.
6. **Commands dropped on a full queue.** A dropped note off leaves a note stuck.
   - Fix it inside the `SongPlayer`, which holds a refused command for the next advance.
   - Fix it now in `DawApp`, independently of the design.
7. **Interactive verification.** Nobody has used the composer on a real display and
   audio device since the layered refactor.
   - The operator runs a short manual check from a written checklist.
   - Accept the offscreen screenshots and tests until a release.
8. **What "Tune" means.** It was built as a pitch corrector.
   - Keep the pitch corrector.
   - Add a tuner display as well, which needs a path from the audio thread back to the
     interface.
   - Replace it with a tuner.
9. **Window resizing while running.**
   - Leave the size fixed at start through `--window-size`.
   - Handle resize events, which the layout already supports through `WindowSize`.
10. **Long chains on the chain pages.** A chain past the page height runs off the bottom.
    - Add vertical scrolling, as the Studio rack scrolls sideways.
    - Cap the number of pedals per chain to what fits.
    - Leave it documented.
11. **Tracks at another resolution than their project.** Load accepts them and export
    refuses them.
    - Convert them to the project's resolution on load.
    - Remove the per-track field, so the project's resolution is the only one.
    - Keep refusing at export.
12. **Keleusma defects.** Unchecked host struct arguments, a cost sum that overflows,
    and a cost estimate that leaves out callees.
    - Report them to the Keleusma operator, and keep this crate's guards until a fixed
      release.
    - Keep the guards permanently as defence in depth, whatever upstream does.
13. **The game's pin.** volaw pins `3dd668a`.
    - Tell the volaw line it may repin to the current `main`. By a reading of its source
      it uses nothing that changed.
    - Wait until the `song` crate exists, and repin once.
14. **Scratch-volume trial.**
    - Conclude it and keep the scratch volume. No refusal or slowdown was seen.
    - Run one `cargo clean` round first, the only untested case.
15. **The held Grok line.** A nested `.grok` directory in its home holds it.
    - The operator removes the nested directory, and Grok joins the next team.
    - Leave Grok out of teams for now.
16. **Harness reports for pydromon.** They are in local scratch here.
    - Copy them into the pydromon repository's report directory for its next round.
    - Send them by Dromon mail to its implementation line.
17. **Template feedback without a reply since 2026-10-28.**
    - Ask once more.
    - Drop it and adopt any change through a later sync.
