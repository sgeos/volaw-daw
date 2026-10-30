# Reverse Prompt

> **Navigation**: [Process](./README.md)

A state file, overwritten whole after each task. It reports what was done, what was
decided and by whom, what was verified and how, what was not verified, and what the
operator needs to decide. Facts, inferences and hypotheses are marked as such.

## Report on 2026-10-30

**Task.** The operator asked for a specification of the composer, to include Keleusma
scripting, and directed that the directory holding it be ignored by git for now.

**Done.**
- A specification under `openspec/` covers arrangement, note editing, projects,
  scripting, instruments and effects. It is not tracked.
- `.gitignore` excludes `/openspec/`.

**Decided by the operator.** The specification stays out of version control for now.
Keleusma scripting is in scope.

**Verified.** Facts, observed on the macOS authoring host.
- `git check-ignore` reports that `/openspec/` in `.gitignore` covers the directory.
- The gate passes, since no code changed.

**Not verified.** Requirements in the specification that no test measures are listed in
the notes beside it, not here.

**For the operator.** Decide when, if ever, the specification enters version control.
