# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- A Keleusma script whose `TickState` declaration or `main` signature differs from the
  host layout is rejected at load. Under `keleusma` 0.2.2 such a script read the wrong
  values without any error.
- The `audio::` script functions reject NaN and infinite arguments as a script error,
  so a non-finite value cannot reach the synthesizer.
- A tick count above the range of a Keleusma `Word` saturates instead of wrapping to a
  negative value.
- A script that fails to load at startup is reported in the status bar, as hot-reload
  errors already were.

### Added

- Handoff support for Codex and Grok Build: launch commands and sandbox needs in
  `AGENTS.md`, and a tool-neutral `docs/process/RESUME_PROMPT.md`.
- Repository conventions adopted from `sgeos/project-template`, including the local
  gate `scripts/check.sh`, the worktree helper, the process documents and the 0BSD
  license.

### Changed

- The gate bypasses a configured rustc wrapper such as `sccache` unless
  `RUSTC_WRAPPER` is set, so it passes deterministically inside agent sandboxes.
- `daw-core` depends on the published `keleusma` 0.2.2 crate in place of the in-tree
  crate. Keleusma scripts must use the `Word` and `Float` type names and declare
  `TickState` before reading its fields.
- Continuous integration runs the local gate, and it lints the workflows with
  actionlint and zizmor.

