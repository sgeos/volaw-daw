# AGENTS.md

Guidance for AI coding assistants other than Claude Code, such as Codex or Grok Build.

## Authoritative source

The project context, conventions and per-session protocol live in
[`CLAUDE.md`](./CLAUDE.md). Despite the name, its content is agent agnostic. Read it before
making any non-trivial change.

## Reading order

1. [`CLAUDE.md`](./CLAUDE.md).
2. [`docs/process/HANDOFF.md`](./docs/process/HANDOFF.md), the state of the work and its
   validity checks.
3. [`docs/process/REVERSE_PROMPT.md`](./docs/process/REVERSE_PROMPT.md), the last report.
4. [`docs/README.md`](./docs/README.md), the documentation index.

## Flagged up front

- **Commit once per prompt,** with no `Co-Authored-By` trailer.
- **Ask before outward-facing actions,** such as pushing, changing visibility or
  settings, or publishing.
- **Scratch directories.** Use `tmp/` for transient files. Its contents are ignored. Never
  commit anything under `secret/`, and never copy its contents into tracked files.
- **Run `./scripts/check.sh` before committing.**
- **One writer per checkout.** If another line is already working here, use
  `scripts/worktree.sh` to work in a separate worktree.
