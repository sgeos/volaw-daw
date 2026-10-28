# Reverse Prompt

> **Navigation**: [Process](./README.md)

A state file, overwritten whole after each task. It reports what was done, what was
decided and by whom, what was verified and how, what was not verified, and what the
operator needs to decide. Facts, inferences and hypotheses are marked as such.

## Report on 2026-10-28

**Task.** The operator asked for every known issue to be resolved and for the findings
to be communicated to the other lines. The issues came from the audit of `4aba8b9` and
from the open items of the previous handoff.

**Done, each with a test that fails when its fix is reverted.**

| Issue | Resolution |
|---|---|
| A mismatched `TickState` declaration or `main` signature misread values silently | Rejected at load by `check_entry_contract` |
| NaN and infinite native arguments reached the engine | Rejected in `extract_f64`, the single native boundary |
| A tick count above `i64::MAX` wrapped to a negative value | Saturates at `i64::MAX` |
| Startup load errors appeared only on standard error | Shown in the status bar with a count |
| Two tests matched Keleusma message wording | Assert only the stage prefix and the type name |
| No test covered an undeclared, unread `TickState` | Test added |
| The README did not document the script contract | Section added, with an example the tests read from the README |

**Resolved by checking, with no change needed.**
- Fact. No `.kma` file exists on this machine, so there were no local scripts to
  migrate. The search excluded `/System` and `/private/var`.
- Fact. The volaw workspace at `5306508` passes `cargo check` and 625 tests, with none
  failed, against the `audio` crate on volaw-daw `main`.

**Communicated.**
- The Keleusma defect, with a reproduction, to `Op55-V0.2.X-keleusma`, and a pointer to
  `Op55-V0.3.X-keleusma`.
- A template follow-up to `Op55-admin-polybasite`, on the trailer ruling reaching only
  synced repositories.
- An audit note to `volaw-64`, which is inferred and not confirmed to be the author of
  `4aba8b9`.

**Verified.** Fact. The gate passes locally on macOS with 220, 17 and 75 tests and no
warnings.

Fact. Continuous integration on Ubuntu passed on pull request 3 with the same counts.

**Not verified.** The
composer was not run interactively, so the status bar change is tested only through
`DawState`.

**For the operator.**
- Merge the pull request.
- Review Dependabot pull request 2, which moves `toml` to a new major version.
