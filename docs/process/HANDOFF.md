# Handoff

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

This is a state file. It is overwritten whole on every update and holds only what is in
flight, why, what is owed and the next step, within a budget of 8 KB. History lives in
git. A rewrite that drops an open item says in its commit message whether the item was
completed, moved or abandoned.

## State on 2026-10-28

- **Script contract.** `daw-core` checks each Keleusma script at load against the entry
  contract in the `script` module documentation, and it rejects non-finite arguments to
  the `audio::` natives. The branch is `fix/script-contract-hardening`.
- **Template.** The repository follows `sgeos/project-template` from commit `5838ce3`,
  merged in pull request 1.

## Next step

Merge the pull request for `fix/script-contract-hardening` once its checks pass. Then
choose the next feature or fix with the operator. No milestone is defined in
`docs/roadmap/`.

## Open

- **Keleusma defect.** `keleusma` 0.2.2 does not check a host struct argument against
  the script declaration, so a mismatch misreads values silently. It was reported to
  `Op55-V0.2.X-keleusma` on 2026-10-28, with a pointer to `Op55-V0.3.X-keleusma`. When
  a fixed release appears, decide whether the check in `daw-core` stays as defence in
  depth.
- **Template feedback.** Sent to `Op55-admin-polybasite` on 2026-10-28 in two messages.
  No reply had been seen. Adopt any template change through a sync.
- **`Co-Authored-By` trailer on `4aba8b9`.** It breaks the operator ruling of 2026-10-25.
  It cannot be removed without a force-push, which the ruleset forbids, so it is
  accepted unless the operator decides otherwise.

## Environment

Observed on 2026-10-28 in a macOS Claude Code sandbox.

- `sccache` is the configured `RUSTC_WRAPPER` and its socket is denied, so cargo fails
  with `Operation not permitted`. Run cargo and the gate with `RUSTC_WRAPPER=` set to
  empty.
- `.git/config` is write-protected, so `git remote`, `git config`, setting an upstream
  and the branch deletion in `scripts/worktree.sh rm` need escalation or print a
  harmless warning. `git worktree add` does not.
- Merging a pull request with `gh pr merge` was denied by the auto mode classifier, so
  the operator merges.
- Another session committed into the main checkout on 2026-10-28 while this line
  worked there. Work only in a worktree.

## Validity

```sh
git status --short          # clean
git log --oneline -5        # the newest commit should describe this state
RUSTC_WRAPPER= ./scripts/check.sh   # exits zero
```

Report any check not run, and any commit after the one that last rewrote this file.

## Mailbox

None assigned. Messages from other lines arrive as cross-session messages to the
session that sent the reports, which was `volaw-daw-b2`.
