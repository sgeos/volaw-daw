# Vows of Love and War - Composer

[![CI](https://github.com/sgeos/volaw-daw/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/sgeos/volaw-daw/actions/workflows/ci.yml)
[![License: 0BSD](https://img.shields.io/badge/license-0BSD-blue.svg)](LICENSE)

A digital audio workstation (DAW) for composing music for the Vows of Love and War game audio engine. This repository was split out of the volaw game repository and preserves the history of the crates it contains.

## Status

**Working.** The composer builds and runs, with an 8-channel piano roll, effect chains, Keleusma scripting and real-time playback. No release has been published. Current work is tracked in [`docs/process/HANDOFF.md`](docs/process/HANDOFF.md).

## Workspace Structure

| Crate | Type | Purpose |
|-------|------|---------|
| `audio` | Library (no_std) | 8-channel audio engine with 25-pedal DSP effects chain. The volaw game client also depends on this crate. |
| `daw-core` | Library (no_std) | Shared DAW logic for MIDI, transport, patches, presets, the SMF parser, and Keleusma scripting. |
| `daw` | Binary (std) | SDL3 music composer with an 8-channel piano roll and audio playback. |

Keleusma scripting comes from the published [`keleusma`](https://crates.io/crates/keleusma) crate, version 0.2.2.

## Usage

```sh
cargo run -p daw
```

See the [Composer README](crates/daw/README.md) for prerequisites, controls, and the project file format.

## Development

```sh
./scripts/check.sh                       # the local gate, which continuous integration also runs
scripts/worktree.sh new feat/some-thing  # an isolated worktree for a parallel line
```

The gate runs repository hygiene checks, then `cargo fmt`, `cargo clippy` with warnings denied, `cargo build` and `cargo test` across the workspace.

The volaw repository consumes the `audio` crate from this repository by a sibling path. Changes to the public API of `audio` therefore affect the game client.

## Documentation

The documentation index is [`docs/README.md`](docs/README.md). Agent guidance is in
[`CLAUDE.md`](CLAUDE.md), and [`AGENTS.md`](AGENTS.md) points other assistants to it.
Participation is governed by the [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## License

[0BSD](LICENSE).
