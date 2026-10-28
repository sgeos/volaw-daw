# Resume Prompt

> **Navigation**: [Process](./README.md)

The prompt a new session is given, in any of Codex, Grok Build or Claude Code.
[`AGENTS.md`](../../AGENTS.md) gives the command that passes it to each tool. Everything
below the line is the prompt.

---

Resume continuity in this existing working directory. This prompt is written for any
agent, including Codex, Grok Build and Claude Code.

Read AGENTS.md, CLAUDE.md, docs/process/HANDOFF.md and docs/process/REVERSE_PROMPT.md.
Run the handoff validity checks and report the handoff as valid or as stale, separating
what you verified from what you could not, and naming the cause of anything unverified.
Then wait for the operator's task unless the handoff names unfinished work that the
operator has already asked for.

Check `git worktree list` before writing anything. If another line has uncommitted work
in this checkout, do not touch it, and start your own worktree with
scripts/worktree.sh. Preserve ignored files under tmp/ and secret/.

Historical reports, open items and suggestions in the handoff are context, not
authorization for new scope. Ask before pushing, merging, publishing or changing
repository settings. Run ./scripts/check.sh before any commit, inspect its exit status
and its test counts, and report the sandbox you ran it in. Check continuous integration
separately for anything you push. Report actual results and their limitations.
