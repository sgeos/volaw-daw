# Vows of Love and War - Composer

A digital audio workstation (DAW) for composing music for the Vows of Love and War game audio engine. This repository was split out of the [volaw](https://github.com/sgeos/volaw) game repository and preserves the history of the crates it contains.

## Workspace Structure

| Crate | Type | Purpose |
|-------|------|---------|
| `audio` | Library (no_std) | 8-channel audio engine with 25-pedal DSP effects chain. The volaw game client also depends on this crate. |
| `daw-core` | Library (no_std) | Shared DAW logic for MIDI, transport, patches, presets, the SMF parser, and Keleusma scripting. |
| `daw` | Binary (std) | SDL3 music composer with an 8-channel piano roll and audio playback. |

## Usage

```sh
cargo run -p daw
```

See the [Composer README](crates/daw/README.md) for prerequisites, controls, and the project file format.

## Development

```sh
cargo fmt --check
cargo build --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --tests -- -D warnings
```

The volaw repository consumes the `audio` crate from this repository. Changes to the public API of `audio` therefore affect the game client.
