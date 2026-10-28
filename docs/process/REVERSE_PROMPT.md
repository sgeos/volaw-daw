# Reverse Prompt

> **Navigation**: [Process](./README.md)

A state file, overwritten whole after each task. It reports what was done, what was
decided and by whom, what was verified and how, what was not verified, and what the
operator needs to decide. Facts, inferences and hypotheses are marked as such.

## Report on 2026-10-28

**Done.** The repository was synchronized with `sgeos/project-template` at commit
`5838ce3`. Template files were added, the placeholders were replaced, and the gate,
continuous integration, Dependabot and ignore rules were adapted to the Rust workspace.
The previous workflow's SDL3 dependency installation was carried into the gate's job.

**Decided by the operator.** The GitHub repository is public, and the license is 0BSD.

**Verified.** Fact. `scripts/check.sh` exits zero locally on macOS with
`RUSTC_WRAPPER=` set to empty. It reported 220 `audio` tests, 14 `daw` tests and 69
`daw-core` tests passed, none skipped. The gate rejects an unknown argument with exit
status 2 and passes arguments after `--` to `cargo test` as a filter.

**Not verified.** The continuous integration run on Ubuntu is observed on the pull
request, not here. The disclosure scan was skipped because no
`secret/disclosure-terms.txt` exists.

**For the operator.** Merging the pull request and applying the repository settings
and ruleset are outward-facing, as are the remaining items under Open in
[`HANDOFF.md`](HANDOFF.md).
