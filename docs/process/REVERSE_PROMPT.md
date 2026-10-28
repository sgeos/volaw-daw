# Reverse Prompt

> **Navigation**: [Process](./README.md)

A state file, overwritten whole after each task. It reports what was done, what was
decided and by whom, what was verified and how, what was not verified, and what the
operator needs to decide. Facts, inferences and hypotheses are marked as such.

## Report on 2026-10-28

**Task.** The operator asked for the project to be prepared for a handoff to Codex,
Grok Build or both, following the projects already prepared. Epagoge and the
Adventurers Creator served as the pattern.

**Done.**
- `AGENTS.md` gives launch commands checked against Codex CLI 0.164.0, Grok Build 1.0.70
  and Claude Code 2.1.312, which instruction files each tool loads, what a sandboxed
  agent needs, and what runs in a fresh clone. It names no absolute home path, because
  the repository is public.
- `docs/process/RESUME_PROMPT.md` is a tool-neutral prompt for a new session.
- The handoff gains measurable validity checks and a section for a session that is not
  Claude Code.
- The gate bypasses a configured rustc wrapper unless `RUSTC_WRAPPER` is set.

**Why the gate changed.** Fact. Inside both Codex's and Claude Code's sandboxes,
`sccache` could use an already running server but could not start one, and the shared
server stops and starts with other activity on the host. Before the change, the gate
under Codex's sandbox failed at Clippy with `sccache: error: Operation not permitted`.
After it, the gate passed. Fact. Toggling the wrapper caused no rebuild.

**Verified.** Facts.
- Under `codex sandbox -P :workspace`, the gate on this tree exited zero with 220, 17 and
  75 tests passed. A cold run on `main` at `5de8b55` took 88 seconds.
- In Claude Code's sandbox, the gate on this tree exited zero with the same counts.
- With `RUSTC_WRAPPER` set explicitly, the gate used it and printed no bypass note.

**Not verified.**
- Grok Build's sandbox. It has no single-command runner, and a trial session was not
  started. The inference, from its documentation, is that its `workspace` profile
  confines writes as Codex's does.
- No Codex or Grok model session was started, so how either tool follows the resume
  prompt is untested.

**For the operator.**
- Merge pull request 3, then the handoff pull request, and review Dependabot pull
  request 2.
- Trust this folder in Grok Build at its first launch, if Grok is used.
- Optionally run one trial session in each tool with the commands in `AGENTS.md`, as
  was done for epagoge, so that the handoff is exercised before real work depends on it.
