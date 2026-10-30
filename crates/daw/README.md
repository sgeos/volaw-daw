# Vows of Love and War - Composer

> [Top-level README](../../README.md)

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
| `--script <n> <file>` | Assign a Keleusma script from `scripts/` to channel `n` (0-based) |

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

Everything on the Studio page works with the mouse alone. The keys below are
shortcuts.

### Studio Page

The composer opens on the Studio page, which is laid out after browser-based
workstations such as Audiotool and Soundtrap.

| Area | What it does |
|------|--------------|
| Page tabs | Switch between Studio, Piano Roll, Overview, Synth Setup, Mono Chain and Stereo Chain |
| Play, Stop, Save | Transport and saving, at the right of the page tabs |
| Browser, Instruments | Click a preset to play it on the selected track |
| Browser, Effects | Every effect by category with a short description. Click one to add it to the selected track |
| Track rows | Click to select a track. M mutes, S solos, drag Vol to set the volume, Notes opens the piano roll |
| Effects rack | The selected track's effects, in signal order. Drag a bar to adjust a setting, ON/OFF bypasses, < and > reorder, Remove deletes, and + Add effect opens the effect picker |
| Scroll wheel | Scrolls the browser or a long rack, adjusts a setting under the pointer, or nudges a track's volume |

### Global

| Key | Action |
|-----|--------|
| Space | Toggle play/pause |
| S | Stop playback |
| 0 / Home | Reset to beginning |
| 1-8 | Select channel |
| Tab | Next page (Studio, Piano Roll, Overview, Synth, Mono Chain, Stereo Chain) |
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

Every pedal box has its own ON/OFF, <, > and Remove buttons, and each chain
ends with a + Add effect card, so the keys are optional.

| Key | Action |
|-----|--------|
| Escape | Return to the Studio page |
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
- **Studio page** with an instrument and effect browser, track list and per-track effects rack, all driven by the mouse
- **40 built-in presets**, including drums, NES and Game Boy chiptune voices, and classic synthesizer sounds
- **Real-time audio playback** at 48 kHz through the MS-20 synthesizer engine
- **Synth setup page** with VCO, filter, envelope, LFO, and modulation parameters
- **29-pedal DSP effect chain** with mono and stereo chain setup pages. Drive and distortion are Overdrive, Distortion, Fuzz, Tape Saturation and Wavefolder. Bitcrusher, a resonant Filter with an LFO sweep, Vibrato and a Pitch Corrector that snaps pitch to a chromatic, major or minor scale are also included
- **Pedal picker** showing every effect at once in a category grid with descriptions
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

### Channel Scripts

Each tick, the composer passes the transport state to the script's `main` function. The entry function takes exactly one parameter of type `TickState`. A script that reads the transport state declares the struct exactly as follows, with the same field names, types and order.

```
use audio::set_lpf_cutoff

struct TickState { ticks: Word, tempo: Float, beat: Float }

loop main(state: TickState) -> () {
  audio::set_lpf_cutoff(200.0 + state.beat * 1000.0);
  let state = yield ();
  state
}
```

A script that never reads the transport state may omit the declaration. The composer rejects a script with a different entry signature or a different `TickState` declaration at load, because Keleusma 0.2.2 would otherwise read the wrong values without reporting an error. Arguments to the `audio::` functions must be finite numbers, so a NaN or an infinity stops the script with an error. Load errors and runtime errors both appear in the status bar.

## Architecture

The composer is the entry point of a layered workspace, described in
[`docs/architecture/LAYERS.md`](../../docs/architecture/LAYERS.md). `src/main.rs` only
parses arguments and wires the layers.

| Crate | Role for the composer |
|-------|-----------------------|
| `daw-core` | `DawApp`, the state machine for editing, playback scheduling, input handling and drawing, plus persistence, presets, scripts and MIDI files. `no_std` |
| `audio` | The synthesizer engine and the lock-free command queue to it. `no_std` |
| `daw-sdl` | The SDL3 renderer, SDL3 event translation and the std file system platform |
| `volaw-audio` | Starts the engine on the SDL3 audio device, through `audio-sdl` |

### External Dependencies

- **keleusma** 0.2.2 (no_std+alloc, published on crates.io): Keleusma scripting language with lexer, parser, compiler, and virtual machine. It is consumed by daw-core and is no longer part of this workspace.

## Testing

```sh
cargo test -p daw
cargo test -p daw-core
```

The DAW binary has 4 unit tests, for argument parsing and the README script example. The daw-core library has 126 tests covering the application and its Studio page, MIDI track operations, transport logic, project persistence, preset loading, rendering calculations, WAV decoding, Standard MIDI File parsing, and Keleusma script integration.
