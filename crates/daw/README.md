# Vows of Love and War - Composer

> [Top-level README](../../README.md) | [Server README](../server/README.md) | [Client README](../client/README.md)

SDL3-based Digital Audio Workstation (DAW) for composing music using the game audio engine. An 8-channel MIDI sequencer with piano roll editing, patch preset selection, effect chains, and real-time audio playback through the MS-20 synthesizer architecture.

## Prerequisites

- **Rust** 1.85 or later (edition 2024)
- **FreeType** (required by SDL3_ttf for font rendering)
- **CMake** (required for building SDL3 from source)

On macOS:

```sh
brew install freetype cmake
```

## Running

```sh
cargo run -p daw
```

Opens a 1440x900 window titled "Vows of Love and War - Composer" with an empty 8-channel project.

### Command Line Arguments

| Argument | Description |
|----------|-------------|
| `--root <dir>` | Project root directory (default: `daw_projects`) |
| `--project <name>` | Load a saved project on startup |
| `--import-midi <path>` | Import a Standard MIDI File (.mid) |
| `--import-channel <n>` | Channel index to import into (0-based, default: 0) |

Examples:

```sh
# Load a saved project
cargo run -p daw -- --project my_song

# Import a MIDI file into channel 0
cargo run -p daw -- --import-midi path/to/file.mid

# Import a MIDI file into channel 3 with a custom project root
cargo run -p daw -- --root my_music --import-midi track.mid --import-channel 3
```

## Controls

### Keyboard

| Key | Action |
|-----|--------|
| Space | Toggle play/pause |
| S | Stop playback |
| 1-8 | Select channel |
| P | Next preset |
| Shift+P | Previous preset |
| M | Toggle channel mute |
| Up/Down | Scroll vertically |
| Left/Right | Scroll horizontally |
| +/= | Zoom in |
| - | Zoom out |
| Enter | Confirm selection |
| Escape | Cancel |
| Delete/Backspace | Delete selected note |
| Ctrl+S (Cmd+S) | Save project |
| Ctrl+Z | Undo |
| Q or Ctrl+Q | Quit |

### Mouse

| Action | Effect |
|--------|--------|
| Click on grid | Place or select a note |
| Click on piano key | Place a note at that pitch |
| Click on channel bar | Select that channel |
| Scroll wheel | Scroll vertically |

## Features

- **8-channel MIDI sequencer** with per-channel patch, effect chain, and mute state
- **Piano roll editor** spanning C2 to C7 (5 octaves, 60 notes) with note snap quantization
- **16 built-in presets**: bass, lead, pad, strings, organ, brass, pluck, bell, flute, piano, kick, snare, hihat, synth_lead, synth_bass, choir
- **Real-time audio playback** at 48 kHz through the MS-20 synthesizer engine
- **Patch-to-engine bridge** mapping TOML configuration to VCO waveforms, filters, envelopes, LFO, and modulation matrix
- **25-pedal DSP effect chain** per channel
- **Standard MIDI File import** with monophonic autofix (latest note, highest note, or lowest note strategies)
- **TOML project save/load** with per-channel patch, effects, and MIDI files
- **Undo system** with 64-level undo stack
- **Zoom and scroll** for navigating large compositions

## Project File Format

Projects are saved under the project root directory (default: `daw_projects/`). Each project consists of:

```
daw_projects/
  my_song/
    project.toml          # Metadata (tempo, ticks per beat, channel assignments)
    patches/
      ch0_patch.toml      # Patch configuration per channel
      ch1_patch.toml
      ...
    effects/
      ch0_effects.toml    # Effect chain configuration per channel
      ...
    midi/
      ch0.mid             # Monophonic MIDI data per channel
      ch1.mid
      ...
```

## Architecture

| Module | Purpose |
|--------|---------|
| `main.rs` | Application entry point, DAW state machine, event loop, input handling, rendering |
| `save_load.rs` | Project persistence (TOML save/load, MIDI export/import) |
| `sdl_audio.rs` | Audio device bridge (SDL3 audio callback to AudioEngine) |
| `sdl_input.rs` | SDL3 keyboard and mouse event translation |
| `sdl_platform.rs` | Platform trait implementation for std file I/O |
| `sdl_renderer.rs` | SDL3 renderer with TTF text caching |

### Library Crates

- **daw-core** (no_std+alloc): Shared DAW logic including MIDI track representation, transport state, patch configuration, preset definitions, rendering trait, and Standard MIDI File parser
- **audio** (no_std+alloc): 8-channel synthesizer engine with dual VCO, dual envelope generator, HPF/LPF filters, LFO, modulation matrix, and 25-pedal DSP effect chain

## Testing

```sh
cargo test -p daw
cargo test -p daw-core
```

The DAW binary has 3 unit tests. The daw-core library has 45 tests covering MIDI track operations, transport logic, project serialization, preset loading, rendering calculations, and Standard MIDI File parsing.
