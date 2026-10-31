# Handoff

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

This is a state file. It is overwritten whole on every update and holds only what is in
flight, why, what is owed and the next step, within a budget of 8 KB. History lives in
git. A rewrite that drops an open item says in its commit message whether the item was
completed, moved or abandoned.

## State on 2026-10-31

**Holding.** Nothing is in flight. `main` is green, pull requests 1 to 13 are merged, no
pull request is open, and no worktree or feature branch remains. The Dromon daemon is
stopped. Its configuration in `.dromon/` is local and excluded through
`.git/info/exclude`.

- **Last round.** A PyDromon team fixed the known bugs and open items while trialling
  the scratch volume. Pull request 10 split the page code and added `--window-size`.
  Pull request 11 took allocation off the audio thread, a breaking change to `audio`,
  and survives `SDL_Quit` before stop. Pull request 12 added the run path, hardened the
  MIDI file parser, bounded script ticks and fixed the transport resolution. Pull
  request 13 recorded this holding state and the shared playback proposal.
- **Scratch-volume trial.** Every worktree kept `target` as a link to
  `/Volumes/Scratch` with no sandbox denial. Cold gates took about 106 seconds and warm
  gates 6 to 26 seconds. A `cargo clean` was not exercised.
- **Local only.** The harness experience reports are in `tmp/harness-reports/`, for the
  pydromon implementation line.

## Next step

Wait for the operator. The decisions that block further work are listed in
`REVERSE_PROMPT.md`, each with its obvious choices. The largest is the shared song
playback design in
[`../architecture/SHARED_PLAYBACK.md`](../architecture/SHARED_PLAYBACK.md).

## Open

- **volaw pins `3dd668a`.** Keep it reachable, so never rewrite the history that
  contains it. volaw at `d1884bc` used only `AudioSystem::start`, `AudioConfig` and
  `AudioEngine::new`, which pull request 11 left unchanged. A repin onto the new
  `EngineCommand::Bulk` needs no change in volaw, by that reading.
- **Interactive use unverified.** No one has played the composer on a real display,
  mouse and audio device since the layered refactor. Pages were checked only offscreen.
- **"Tune" reading unconfirmed.** It was built as pitch correction.
- **Playback defects.** Listed in section 2 of `SHARED_PLAYBACK.md`, as facts read from
  the code. Tempo follows the interface frame rate. Every note of every channel is
  scanned each frame. Commands are dropped when the queue is full. `FeedSample` timed
  events never play. Note onsets are quantized to the frame. Only the dropped fraction
  of a tick is fixed.
- **Layout.** The window cannot be resized while running, and the chain pages do not
  scroll, so a very long chain runs off the bottom.
- **Resolution.** Loaded tracks whose own ticks per beat differs from the project's are
  not converted on load, and export refuses them. A loop shorter than one tick at a
  coarser resolution stops wrapping.
- **Keleusma defects for its operator.** 0.2.2 does not check a host struct argument.
  `verify.rs:338` adds costs without saturating, so `for 0..1000000000` panics at load
  in debug and is admitted in release, where the step budget stops it. The cost estimate
  leaves out callees and `fn main`, and an unbounded `loop` is rejected only through
  that overflow.
- **Grok.** The `grok` line stayed held, because a nested `.grok` directory under
  `~/.grok/dromon-home/home` would load another harness's settings.
- **Template feedback.** Sent to `Op55-admin-polybasite` on 2026-10-28. No reply had
  been seen. Adopt any template change through a sync.
- **`Co-Authored-By` trailer on `4aba8b9`.** It stays because `main` forbids
  force-pushes.

## If this session is not Claude Code

The protocol names no tool-specific feature, but every commit so far was made with
Claude Code, so no other assistant has exercised it yet. Anything in it that assumes a
Claude Code feature is a defect in the protocol. Record it in `REVERSE_PROMPT.md`
rather than working around it silently. Codex does not load `CLAUDE.md`, so read it
explicitly. Grok Build loads it once the folder is trusted. `AGENTS.md` describes what
each sandbox needs.

## Environment

Observed on 2026-10-28 on the macOS authoring host.

- `~/.cargo/config.toml` sets `rustc-wrapper = "sccache"`. A sandbox cannot start an
  `sccache` server, so plain cargo inside one needs `RUSTC_WRAPPER=`. The gate sets it
  itself.
- Inside Claude Code's sandbox, `.git/config` is write-protected, so `git remote`,
  `git config` and branch deletion need escalation or print a harmless warning. Nested
  sandboxes are refused, so `codex sandbox` must be launched from outside one.
- `gh pr merge` works when the operator has asked in session for the work to be
  merged. Otherwise the operator merges.
- Another session committed into the main checkout on 2026-10-28. Work only in a
  worktree.

## Validity

Run from the checkout under test, on `main`.

```sh
git status --short          # clean, apart from ignored files
git log --oneline -1 -- docs/process/HANDOFF.md   # the commit that wrote this file
git log --oneline "$(git log -1 --format=%H -- docs/process/HANDOFF.md)"..HEAD
./scripts/check.sh          # exits zero
gh pr list --state all --limit 5   # needs an authenticated gh
```

1. The gate reports `audio` 262 and 4, `audio-sdl` 5 and 5, `daw` 7 and 1, `daw-core`
   195 and 6, `daw-sdl` 4 and `volaw-audio` 5 tests passed, unit then integration, and
   1 doctest, with none failed or ignored, and reports that the core crates build for
   `thumbv7em-none-eabihf`, and that the disclosure scan found no listed term.
2. Every commit after the one that wrote this file is reported, since this file does
   not describe them.
3. Without an authenticated `gh` or network access, report the pull request states as
   unverified, naming the cause, rather than as failed.

## Mailbox

None assigned. Cross-session messages from other lines reached the Claude Code session
`volaw-daw-b2`, which wrote this file. A session in another tool has no such mailbox.
