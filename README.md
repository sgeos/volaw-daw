# Vows of Love and War - Composer

[![CI](https://github.com/sgeos/volaw-daw/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/sgeos/volaw-daw/actions/workflows/ci.yml)
[![License: 0BSD](https://img.shields.io/badge/license-0BSD-blue.svg)](LICENSE)

A digital audio workstation (DAW) for composing music for the Vows of Love and War game audio engine. This repository was split out of the volaw game repository and preserves the history of the crates it contains.

## Status

**Working.** The composer builds and runs, with an 8-channel piano roll, effect chains, Keleusma scripting and real-time playback. No release has been published. Current work is tracked in [`docs/process/HANDOFF.md`](docs/process/HANDOFF.md).

## Workspace Structure

The workspace follows three layers, a `no_std` core, a platform layer that implements
the core's traits, and entry points that wire them. [`docs/architecture/LAYERS.md`](docs/architecture/LAYERS.md)
describes them.

| Crate | Layer | Purpose |
|-------|-------|---------|
| `audio` | Core (no_std) | 8-channel audio engine with a 25-pedal DSP effects chain, the engine command queue and the audio output traits. The volaw game client depends on this crate. |
| `daw-core` | Core (no_std) | The DAW application state machine, MIDI, transport, patches, presets, project persistence, the SMF parser and Keleusma scripting. |
| `audio-sdl` | Platform (std) | SDL3 audio output for the engine. |
| `daw-sdl` | Platform (std) | SDL3 rendering and input, and file system access, for the DAW. |
| `volaw-audio` | Entry (library) | Audio playback wiring shared by the DAW and the volaw game client. |
| `daw` | Entry (binary) | The SDL3 music composer. |

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

The gate runs repository hygiene checks, then `cargo fmt`, `cargo clippy` with warnings denied, `cargo build` and `cargo test` across the workspace, and finally builds the core crates for the bare-metal target `thumbv7em-none-eabihf` to prove they are `no_std`.

The volaw repository consumes the `audio` crate from this repository by a sibling path, and is meant to move to `volaw-audio`. Changes to the public API of either therefore affect the game client.

## Documentation

The documentation index is [`docs/README.md`](docs/README.md). Agent guidance is in
[`CLAUDE.md`](CLAUDE.md), and [`AGENTS.md`](AGENTS.md) points other assistants to it.
Participation is governed by the [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## License

[0BSD](LICENSE).
