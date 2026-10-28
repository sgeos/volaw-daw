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

## Starting a session in Codex, Grok Build or Claude Code

**Current state is in [`docs/process/HANDOFF.md`](docs/process/HANDOFF.md)**, and
[`docs/process/RESUME_PROMPT.md`](docs/process/RESUME_PROMPT.md) is the prompt a new
session should be given. Both are written for any agent.

Checked on 2026-10-28 against the help output of the installed tools, Codex CLI 0.164.0,
Grok Build 1.0.70 and Claude Code 2.1.312. Recheck the flags if any of these tools has
been upgraded. Run each command from the root of the checkout.

```sh
codex --cd "$PWD" --sandbox workspace-write --ask-for-approval on-request \
  "Read docs/process/RESUME_PROMPT.md and carry out its instructions."

grok --cwd "$PWD" \
  "Read docs/process/RESUME_PROMPT.md and carry out its instructions."

claude "Read docs/process/RESUME_PROMPT.md and carry out its instructions."
```

**Which instruction files each tool loads.** Codex reads this file and not `CLAUDE.md`,
so this file and the resume prompt tell it to read `CLAUDE.md`. Grok Build's bundled
documentation says it reads this file and `CLAUDE.md`, and only once the folder is
trusted. On 2026-10-28 this repository was not among Grok's trusted folders on the
authoring host, so the operator confirms trust at the first launch. Claude Code reads
`CLAUDE.md` unless `--bare` is passed. None of the three loads `HANDOFF.md` unless asked,
which is why the resume prompt names it.

**Do not pass `--bare` or `--dangerously-skip-permissions` to Claude Code.** `--bare`
skips `CLAUDE.md` discovery, and nothing in this repository needs the permission bypass.

**Do not pass `--sandbox` to Grok Build on the authoring host.** Its global configuration
selects a custom profile that extends `workspace` and also denies credential paths. A
command-line `--sandbox` overrides the configuration and would drop that deny list. On a
host without such a configuration, pass `--sandbox workspace`.

## What a sandboxed agent needs

| Need | Used by | Grant it when |
| --- | --- | --- |
| Write the workspace | All work, including the SDL3 source build under `target/` | Always |
| Read `~/.cargo` and `~/.rustup` | Every cargo command | Always. All three sandboxes allow reads everywhere |
| Write `~/.cargo` and the network | Downloading a crate not yet in the local registry | Only after a dependency change. Run `cargo fetch --locked` outside the sandbox instead |
| Network and git credentials | `git push` over HTTPS with the operator's `gh` login | Only when the operator asks a session to publish. Never force-push |

**The gate passes inside Codex's sandbox, measured on 2026-10-28.** `codex sandbox -P
:workspace` applies the same Seatbelt policy as `--sandbox workspace-write`. It can write
the workspace but not `~/.cargo`. Under it, on this tree with a warm `target/`,
`./scripts/check.sh` exited zero with 220, 17 and 75 tests passed and none skipped. An
earlier run from a cold `target/`, on `main` at `5de8b55` with the wrapper bypassed by
hand, also exited zero, in 88 seconds including the SDL3 source build. The gate on this
tree also passes inside Claude Code's sandbox.

**Grok Build's sandbox was not measured.** Grok has no command that runs a single
command under its profile, so a measurement needs a trial session. Its documentation
gives its `workspace` profile writes to the working directory, `~/.grok` and the
temporary directories only, the same confinement as Codex's. The inference is that the
gate passes there too. The first Grok session should report the gate's outcome.

**The gate bypasses `sccache`.** The authoring host's `~/.cargo/config.toml` sets
`rustc-wrapper = "sccache"`. Inside a sandbox, `sccache` can use a server that is
already running but cannot start one, and an idle server exits on its own. Before
2026-10-28 the gate therefore passed or failed depending on timing, and it failed at
Clippy with `sccache: error: Operation not permitted`. The gate now sets
`RUSTC_WRAPPER` to empty when it is unset, and says so in its output. A plain `cargo`
command run outside the gate still uses the wrapper, so inside a sandbox prefix it with
`RUSTC_WRAPPER=`.

`.codex/` holds host-specific Codex settings and is ignored rather than tracked.

## What runs in a fresh clone

**Everything the gate checks.** Continuous integration runs the same gate on Ubuntu from
a clone, which shows that the tree validates without the authoring host.

| Needed by | What a clone must provide |
| --- | --- |
| Building `daw` | A C toolchain and CMake, because SDL3 and SDL3_ttf build from source. On Linux, also the development headers that `.github/workflows/ci.yml` installs |
| The gate | `git`, `python3` and `shellcheck`, as `CLAUDE.md` and the README describe |
| The disclosure scan | `secret/disclosure-terms.txt`. No list exists on the authoring host either, so the scan skips everywhere and says so |
| Running the composer | A display and an audio device. No test needs them |

**The volaw game repository is private and is not needed here.** It consumes the
`audio` crate by the sibling path `../volaw-daw/crates/audio`. A change to the public API
of `audio` should be checked against it, which needs the authoring host or access to
that repository.

## Obligations that matter most here

- Keleusma scripts are untrusted input. The entry contract in the `script` module of
  `daw-core` exists because `keleusma` 0.2.2 misreads a mismatched struct silently. Do
  not relax it without a Keleusma release that checks the boundary itself.
- Script values reach the engine only through the `audio::` natives, and only as finite
  numbers. Keep that boundary single.
- A figure quoted in a tracked file was measured on that tree, or the same sentence says
  it was not.
