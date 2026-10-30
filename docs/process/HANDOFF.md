# Handoff

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

This is a state file. It is overwritten whole on every update and holds only what is in
flight, why, what is owed and the next step, within a budget of 8 KB. History lives in
git. A rewrite that drops an open item says in its commit message whether the item was
completed, moved or abandoned.

## State on 2026-10-30

- **Studio page and new effects.** Branch `feat/daw-ui-overhaul` adds the Studio home
  page, mouse controls on every pedal, a grid pedal picker, and the Wavefolder, Filter,
  Vibrato and Pitch Corrector effects, as `docs/decisions/STUDIO_PAGE.md` records. It
  also splits the `daw-core` application module by concern. It merges through a pull
  request.
- Pull requests 1 to 6 are merged.

## Next step

None is queued. Wait for the operator's prompt.

## Open

- **volaw pins `3dd668a`.** `volaw-64` reported on 2026-10-30 that volaw depends on
  `volaw-audio` by a git dependency on `3dd668a`, a commit of pull request 5, and will
  repin to the merge commit. Keep `3dd668a` reachable, so never rewrite the history
  that contains it. Not verified by this line.
- **Interactive use unverified.** No one has played the composer on a real display,
  mouse and audio device since the layered refactor or the Studio page. Pages were
  checked only as offscreen screenshots from the `screenshots` example.
- **"Tune" reading unconfirmed.** It was built as pitch correction. If the user meant
  a tuner, that needs a path from the audio thread back to the interface.
- **`Bulk` allocates on the audio thread.** Recorded in `audio::runtime` as debt.
- **Shared song playback.** Scheduling a project's notes lives in `DawApp`. The game
  cannot play a DAW project without it. Moving it into a shared module is unscheduled.
- **`app/handlers.rs` and `app/render.rs` are large.** About 2,500 and 1,600 lines.
  Splitting them per page is technical debt.
- **Keleusma defect.** `keleusma` 0.2.2 does not check a host struct argument against
  the script declaration. `Op55-V0.2.X-keleusma` reported on 2026-10-28 that the defect
  reproduces on v0.2.3, that a module records only a composite's kind and size, and
  that the fix awaits the keleusma operator. `daw-core` is unaffected meanwhile,
  because `ScriptRunner::from_source` rejects any `TickState` declaration that differs
  from the contract and `tick` builds the struct in that order. When a fixed release
  appears, decide whether the check stays as defence in depth.
- **Grok Build sandbox unmeasured.** The first Grok session reports its outcome.
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
- Merging with `gh pr merge` was denied by Claude Code's auto mode, so the operator
  merges.
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

1. The gate reports 253 `audio`, 5 `audio-sdl`, 4 `daw`, 126 `daw-core`, 4 `daw-sdl`
   and 5 `volaw-audio` tests passed, and 1 doctest, with none failed or ignored, and reports that the
   core crates build for `thumbv7em-none-eabihf`.
2. Every commit after the one that wrote this file is reported, since this file does
   not describe them.
3. Without an authenticated `gh` or network access, report the pull request states as
   unverified, naming the cause, rather than as failed.

## Mailbox

None assigned. Cross-session messages from other lines reached the Claude Code session
`volaw-daw-b2`, which wrote this file. A session in another tool has no such mailbox.
