# Handoff

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

This is a state file. It is overwritten whole on every update and holds only what is in
flight, why, what is owed and the next step, within a budget of 8 KB. History lives in
git. A rewrite that drops an open item says in its commit message whether the item was
completed, moved or abandoned.

## State on 2026-10-28

- **Layered architecture in review.** The branch `refactor/layered-architecture` splits
  the workspace into a `no_std` core, a std platform layer and entry points, as
  `docs/architecture/LAYERS.md` describes. It adds `audio-sdl`, `daw-sdl` and the
  shared entry library `volaw-audio`, and moves the DAW state machine into `daw-core`
  as `DawApp`. It awaits the operator's push and review.
- Pull requests 1 to 4 are merged, including Dependabot's `toml` 1.1 bump.

## Next step

The operator reviews the branch, confirms a push, and merges the pull request. Then
volaw can move from its own SDL audio bridge to `volaw-audio`, which is a change in the
volaw repository and needs its line.

## Open

- **volaw adoption of `volaw-audio`.** A trial in a scratch copy of volaw at `969dfbb`
  replaced `client-sdl`'s bridge with `volaw-audio` and passed `cargo check`. Nothing
  was written to the volaw checkout. The volaw line decides when to adopt it.
- **Shared song playback.** Scheduling a project's notes lives in `DawApp`. The game
  cannot play a DAW project without it. Moving it into a shared module is unscheduled.
- **`daw-core/src/app.rs` is about 5,800 lines.** It was moved whole. Splitting it is
  technical debt.
- **Keleusma defect.** `keleusma` 0.2.2 does not check a host struct argument against
  the script declaration. `Op55-V0.2.X-keleusma` queued it for the v0.2.3 line on
  2026-10-28. When a fixed release appears, decide whether the check in `daw-core`
  stays as defence in depth.
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

Run from the checkout under test, on `refactor/layered-architecture` or on `main`
after it merges.

```sh
git status --short          # clean, apart from ignored files
git log --oneline -1 -- docs/process/HANDOFF.md   # the commit that wrote this file
git log --oneline "$(git log -1 --format=%H -- docs/process/HANDOFF.md)"..HEAD
./scripts/check.sh          # exits zero
gh pr list --state all --limit 5   # needs an authenticated gh
```

1. The gate reports 237 `audio`, 5 `audio-sdl`, 4 `daw`, 106 `daw-core`, 4 `daw-sdl`
   and 2 `volaw-audio` tests passed, with none failed or ignored, and reports that the
   core crates build for `thumbv7em-none-eabihf`.
2. Every commit after the one that wrote this file is reported, since this file does
   not describe them.
3. Without an authenticated `gh` or network access, report the pull request states as
   unverified, naming the cause, rather than as failed.

## Mailbox

None assigned. Cross-session messages from other lines reached the Claude Code session
`volaw-daw-b2`, which wrote this file. A session in another tool has no such mailbox.
