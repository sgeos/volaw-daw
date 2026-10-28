# Handoff

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

This is a state file. It is overwritten whole on every update and holds only what is in
flight, why, what is owed and the next step, within a budget of 8 KB. History lives in
git. A rewrite that drops an open item says in its commit message whether the item was
completed, moved or abandoned.

## State on 2026-10-28

- **Remote.** `https://github.com/sgeos/volaw-daw` was created as a public repository
  at the operator's choice, and `main` was pushed with its full history.
- **Template.** The repository adopted `sgeos/project-template` at its commit `5838ce3`
  on the branch `chore/template-sync`, with the 0BSD license at the operator's choice.
- **Keleusma.** `daw-core` depends on the published `keleusma` 0.2.2. Its script tests
  declare `TickState` and use the 0.2 type names `Word` and `Float`.

## Next step

Choose the next feature or fix with the operator. No milestone is defined in
`docs/roadmap/`.

## Open

- **Existing scripts.** Keleusma scripts kept in local `daw_projects/` folders were
  written for the older surface language and have not been checked against 0.2.2. A
  script that names `i64` or `f64`, or reads `TickState` without declaring it, fails to
  compile at load. How the composer surfaces that error has not been checked.
- **The volaw dependency.** The private volaw repository consumes `audio` by the sibling
  path `../volaw-daw/crates/audio`. It has not been built against this tree since the
  split.

## Environment

Observed on 2026-10-28 in a macOS Claude Code sandbox.

- `sccache` is the configured `RUSTC_WRAPPER` and its socket is denied, so cargo fails
  with `Operation not permitted`. Run cargo and the gate with `RUSTC_WRAPPER=` set to
  empty.
- `.git/config` is write-protected, so `git remote`, `git config` and setting an
  upstream need escalation. `git worktree add` does not.
- `shellcheck` and `python3` are installed under `/opt/local/bin`.

## Validity

```sh
git status --short          # clean
git log --oneline -5        # the newest commit should describe this state
RUSTC_WRAPPER= ./scripts/check.sh   # exits zero
```

Report any check not run, and any commit after the one that last rewrote this file.

## Mailbox

None assigned. Name the line's mailbox here once it has one, with the commands that
check it.
