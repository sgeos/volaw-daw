# Handoff

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

This is a state file. It is overwritten whole on every update and holds only what is in
flight, why, what is owed and the next step, within a budget of 8 KB. History lives in
git. A rewrite that drops an open item says in its commit message whether the item was
completed, moved or abandoned.

## State on 2026-10-28

- **Prepared for a session in any tool.** `AGENTS.md` gives the launch commands for
  Codex, Grok Build and Claude Code, and `docs/process/RESUME_PROMPT.md` is the prompt
  to give them. The gate bypasses `sccache`, so it passes inside agent sandboxes.
- **Two pull requests are stacked.** Pull request 3 on `fix/script-contract-hardening`
  enforces the Keleusma script entry contract. The branch `chore/agent-handoff` adds the
  handoff support on top of it, in a pull request whose base is that branch.
- **Dependabot pull request 2** bumps the cargo dependencies, including `toml` from 0.8
  to 1.1.

## Next step

The operator merges pull request 3, then the handoff pull request, which GitHub retargets
to `main` once its base branch is deleted, and reviews pull request 2. After that, the
next feature or fix is the operator's choice. No milestone is defined in
`docs/roadmap/`.

## Open

- **Keleusma defect.** `keleusma` 0.2.2 does not check a host struct argument against
  the script declaration. `Op55-V0.2.X-keleusma` acknowledged the report on 2026-10-28
  and queued it for reproduction on the v0.2.3 line, and it will report back only if
  this repository is affected. When a fixed release appears, decide whether the check in
  `daw-core` stays as defence in depth.
- **Grok Build sandbox unmeasured.** The gate was measured inside Codex's and Claude
  Code's sandboxes, not Grok's. The first Grok session reports its outcome.
- **Template feedback.** Sent to `Op55-admin-polybasite` on 2026-10-28 in three
  messages. No reply had been seen. Adopt any template change through a sync.
- **`Co-Authored-By` trailer on `4aba8b9`.** It breaks the operator ruling of 2026-10-25,
  and it stays because `main` forbids force-pushes. The operator stated the ruling in the
  user-level `~/.claude/CLAUDE.md` on 2026-10-28.

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

Run from the checkout under test, on `chore/agent-handoff` or on `main` after both
pull requests merge.

```sh
git status --short          # clean, apart from ignored files
git log --oneline -1 -- docs/process/HANDOFF.md   # the commit that wrote this file
git log --oneline "$(git log -1 --format=%H -- docs/process/HANDOFF.md)"..HEAD
./scripts/check.sh          # exits zero
gh pr list --state all --limit 5   # needs an authenticated gh
```

1. The gate reports 220 `audio` tests, 17 `daw` tests and 75 `daw-core` tests passed,
   with none failed or ignored.
2. Every commit after the one that wrote this file is reported, since this file does
   not describe them.
3. Without an authenticated `gh` or network access, report the pull request states as
   unverified, naming the cause, rather than as failed.

## Mailbox

None assigned. Cross-session messages from other lines reached the Claude Code session
`volaw-daw-b2`, which wrote this file. A session in another tool has no such mailbox.
