# Reverse Prompt

> **Navigation**: [Process](./README.md)

A state file, overwritten whole after each task. It reports what was done, what was
decided and by whom, what was verified and how, what was not verified, and what the
operator needs to decide. Facts, inferences and hypotheses are marked as such.

## Report on 2026-10-30

**Task.** The operator asked this repository to adopt the template's scratch-volume
support from `sgeos/project-template` at `3954eba`, and to trial it for the next couple
of rounds.

**Done.**
- `scripts/scratch.sh` added, byte for byte as at `3954eba`.
- `scripts/worktree.sh` takes the template's scratch linking in `new` and `rm` and sends
  `git worktree add` progress to standard error. Its local header, which omits the
  template's paragraph on sibling path dependencies, is kept.
- `CLAUDE.md` gains a section on build output on a scratch volume, adjusted to name
  Cargo's `target` and to say that volaw's builds are unaffected, and a `status` line in
  Common Commands. `CHANGELOG.md` names `3954eba`.
- `target` in the main checkout moved to `/Volumes/Scratch/volaw-daw/volaw-daw/target`.

**Decided by the operator.** Adoption, the trial and its scope. The new `CLAUDE.md`
section adds a rule and changes none this repository already stated, so the sync rule
on confirming changed rules did not apply. The operator's request is the confirmation.

**Verified.** Facts, observed on the macOS authoring host inside Claude Code's sandbox.
- `scratch.sh link` moved 1.0 GB. `status` reports a link. `git check-ignore` matches
  `target` with `**/target`, and `git status` showed only the sync's changes.
- The gate passed in the tree before the move, 398 tests and 1 doctest, in 2.85 s with
  nothing to compile. On scratch it passed with the same counts in 5.40 s and then 2.33
  s, compiling nothing either time. `cargo build --workspace` took 0.15 s.
- No sandbox refusal. `bash -n` and `shellcheck` are clean on both scripts.
- No cargo build lock was held before the move. The sandbox denies `ps` and `pgrep`, so
  running processes were inferred from the locks, not listed.

**Not verified.**
- A cold build on scratch. Every run reused existing output, so the times measure
  startup and test execution, not compilation. Inference. The 5.40 s run reflects first
  access to the volume.
- `worktree.sh new` and `rm` with `SCRATCH_BASE` set. No worktree was made.
- Continuous integration at the time of writing.

**For the operator.** Merge pull request for `chore/scratch-volume` once its checks pass.
