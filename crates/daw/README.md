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
`--window-size` chooses another size.

The built binary also runs directly, as `target/debug/daw`. SDL3 and SDL3_ttf are
built as shared libraries and copied beside it, and `build.rs` sets a run path to that
directory on macOS and Linux. Copying the binary elsewhere needs the two libraries
copied with it.

### Command Line Arguments

| Argument | Description |
|----------|-------------|
| `--root <dir>` | Project root directory (default: `daw_projects`) |
| `--project <name>` | Load a saved project on startup |
| `--import-midi <path>` | Import a Standard MIDI File (.mid) |
| `--import-channel <n>` | First channel to import into (0-based, default: 0) |
| `--script <n> <file>` | Assign a Keleusma script from `scripts/` to channel `n` (0-based) |
| `--window-size <w>x<h>` | Window size in pixels, from 1280x800 to 7680x4320 (default: 1440x900) |

Examples:

```sh
# Load a saved project
cargo run -p daw -- --project my_song

# Import a MIDI file into channel 0
cargo run -p daw -- --import-midi path/to/file.mid

# Import a MIDI file into channel 3 with a custom project root
cargo run -p daw -- --root my_music --import-midi track.mid --import-channel 3

# Open a larger window
cargo run -p daw -- --window-size 1920x1080
```

The layout follows the window size. A size outside the bounds is reported as
a warning and the default is used. The window cannot be resized while the
composer runs. In a window narrower than 1420 pixels, a long status message in
the toolbar is cut off at the right edge.

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

## Standard MIDI Files

Ctrl+E writes a Type 0 file with channel `n` of the project on MIDI channel
`n`, so importing it at channel 0 gives back every note on the channel it
came from. Import places a Type 0 file by MIDI channel, from the channel that
`--import-channel` names. A Type 1 file fills consecutive channels from that
channel, one for each MIDI channel used in each track chunk, in file order.
Notes that would land past channel 8 are dropped with a warning in the log.
If a channel that the import leaves alone has notes, the project keeps its
ticks per beat and the imported notes are converted to it, with each start
and end rounded half up. Otherwise the project adopts the file's ticks per
beat, and the notes arrive exactly.

Export refuses, with a message in the status bar, a note with a pitch above
127, a velocity of 0 or above 127, or a duration of 0, a gap of more than
268,435,455 ticks between two events, and a channel with notes whose ticks per
beat differs from the project's, because the file would read back differently
or not at all. Two overlapping notes of the same pitch on one
channel are written, but read back with the first one ending where the second
starts, because the format pairs each note off with the open note of its
pitch.

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

Each tick may run at most 4,096 loop iterations, function calls and native calls such as `audio::set_volume`, counted together. Keleusma rejects a loop body that can skip its yield, an unbounded loop and recursion at load, but it admits a bounded loop of any length, which would otherwise freeze the composer. A script that exceeds the budget is stopped with an error in the status bar, and reloading it starts it again. A loop whose bound is so large that Keleusma's cost estimate overflows, such as one billion iterations, makes Keleusma 0.2.2 panic at load in a debug build.

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

The DAW binary has 4 unit tests, for argument parsing and the README script example, and 1 integration test, which starts the built binary with an empty environment to check its run path. The daw-core library has 162 tests covering the application and its Studio page, MIDI track operations, transport logic, project persistence, preset loading, rendering calculations, WAV decoding, Standard MIDI File parsing, and Keleusma script integration.
