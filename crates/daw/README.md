# Vows of Love and War - Composer

> [Top-level README](../../README.md) | [Server README](../server/README.md) | [Client README](../client/README.md)

SDL3-based Digital Audio Workstation (DAW) for composing music using the game audio engine. An 8-channel MIDI sequencer with piano roll editing, velocity lane, patch preset selection, effect chains, Keleusma scripting, and real-time audio playback through the MS-20 synthesizer architecture.

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

### Global

| Key | Action |
|-----|--------|
| Space | Toggle play/pause |
| S | Stop playback |
| 0 / Home | Reset to beginning |
| 1-8 | Select channel |
| Tab | Next page (Piano Roll, Overview, Synth, Mono Chain, Stereo Chain) |
| Shift+Tab | Previous page |
| P | Next preset |
| Shift+P | Previous preset |
| M | Toggle channel mute |
| N | Toggle metronome |
| O | Toggle channel solo |
| Ctrl+S | Save project |
| Ctrl+E | Export MIDI file |
| Ctrl+Z | Undo |
| Ctrl+Shift+Z / Ctrl+Y | Redo |
| Q / Ctrl+Q | Quit |

### Piano Roll

| Key | Action |
|-----|--------|
| Up/Down | Transpose selected notes by semitone (scroll if none selected) |
| Shift+Up/Down | Transpose selected notes by octave |
| Left/Right | Nudge selected notes by grid unit (scroll if none selected) |
| +/= | Zoom in |
| - | Zoom out |
| G | Cycle grid subdivision (1/1 Bar, 1/2, 1/4, 1/8, 1/16, 1/32) |
| Shift+G | Quantize selected notes to grid |
| Shift+T | Cycle time signature (4/4, 3/4, 6/4, 5/4, 7/4) |
| L | Toggle loop mode |
| Ctrl+A | Select all notes |
| Ctrl+C | Copy selected notes |
| Ctrl+V | Paste (at playhead if playing, at original position otherwise) |
| Ctrl+D | Duplicate selected notes |
| Delete/Backspace | Delete selected notes |
| Escape | Clear selection |

### Mouse

| Action | Effect |
|--------|--------|
| Click on grid | Place or select a note |
| Click on piano key | Audition note (play preview) |
| Click on channel bar | Select channel |
| Click note right edge + drag | Resize note duration |
| Shift+Click | Toggle note in selection |
| Click in velocity lane | Select note and set velocity |
| Drag in velocity lane | Adjust velocity continuously |
| Scroll wheel | Scroll vertically (adjust velocity if notes selected) |
| Scroll over tempo | Adjust BPM |

### Setup Pages

| Key | Action |
|-----|--------|
| A | Add pedal to chain |
| B | Toggle pedal bypass |
| Delete | Remove selected pedal |
| Shift+Left/Right | Reorder pedals |
| Click parameter bar + drag | Adjust parameter value |
| Click enum label | Cycle enum value |
| Scroll wheel | Adjust focused parameter |

## Features

- **8-channel MIDI sequencer** with per-channel patch, effect chain, mute, solo, volume, and pan
- **Piano roll editor** spanning C-2 to G8 (full MIDI range, 128 notes) with note snap quantization
- **Velocity lane** below the piano roll for visual velocity feedback and direct editing
- **Multi-track overview** page showing all 8 channels with note blocks, bar/beat lines, and playhead
- **Time signature support** with Shift+T cycling (4/4, 3/4, 6/4, 5/4, 7/4)
- **Note preview** with 200ms audition on placement and piano key click
- **Per-channel note counts** displayed on the channel bar
- **16 built-in presets**: bass, lead, pad, strings, organ, brass, pluck, bell, flute, piano, kick, snare, hihat, synth_lead, synth_bass, choir
- **Real-time audio playback** at 48 kHz through the MS-20 synthesizer engine
- **Synth setup page** with VCO, filter, envelope, LFO, and modulation parameters
- **25-pedal DSP effect chain** with mono and stereo chain setup pages
- **Pedal picker** with scrollable category list
- **Metronome click** with distinct downbeat and beat sounds (N key toggle)
- **Loop region** with auto-sizing to bar boundaries
- **Playback auto-scroll** following the playhead
- **Standard MIDI File import** with monophonic autofix strategies
- **Standard MIDI File export** (Ctrl+E) to exports/ directory
- **TOML project save/load** with per-channel patch, effects, and MIDI files
- **Keleusma scripting** with per-channel script assignment and hot-reload
- **Script hot-reload** checking file modifications once per second
- **Script error display** in the status bar
- **Undo/redo system** with 64-level stack and inverse capture pattern
- **Zoom and scroll** for navigating large compositions

## Project File Format

Projects are saved under the project root directory (default: `daw_projects/`). Each project consists of:

```
daw_projects/
  my_song/
    saves/
      my_song.toml        # Metadata (tempo, time signature, channel assignments)
    patches/
      ch1.toml            # Patch configuration per channel
      ch2.toml
      ...
    effects/
      ch1.toml            # Effect chain configuration per channel
      ...
    midi/
      ch1.toml            # MIDI track data per channel
      ch2.toml
      ...
    scripts/
      ch1.kma             # Keleusma script per channel (optional)
      ...
    exports/
      my_song.mid         # Exported Standard MIDI File
```

## Architecture

| Module | Purpose |
|--------|---------|
| `main.rs` | Application entry point, DAW state machine, event loop, input handling, rendering |
| `save_load.rs` | Project persistence (TOML save/load) |
| `sdl_audio.rs` | Audio device bridge (SDL3 audio callback to AudioEngine) |
| `sdl_input.rs` | SDL3 keyboard and mouse event translation |
| `sdl_platform.rs` | Platform trait implementation for std file I/O |
| `sdl_renderer.rs` | SDL3 renderer with TTF text caching |

### Library Crates

- **daw-core** (no_std+alloc): Shared DAW logic including MIDI track representation, transport state, patch configuration, preset definitions, rendering trait, Standard MIDI File parser/builder, and Keleusma script runner
- **audio** (no_std+alloc): 8-channel synthesizer engine with dual VCO, dual envelope generator, HPF/LPF filters, LFO, modulation matrix, 25-pedal DSP effect chain, and metronome click generator
- **keleusma** (no_std+alloc): Keleusma scripting language with lexer, parser, compiler, and virtual machine

## Testing

```sh
cargo test -p daw
cargo test -p daw-core
```

The DAW binary has 3 unit tests. The daw-core library has 60 tests covering MIDI track operations, transport logic, project serialization, preset loading, rendering calculations, Standard MIDI File parsing, and Keleusma script integration.
