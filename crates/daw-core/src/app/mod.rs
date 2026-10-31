//! The DAW application: editing state, playback scheduling, input handling
//! and drawing, independent of any platform.
//!
//! `DawApp` is generic over a `Platform`, which supplies files, logging and
//! time, and an `audio::CommandSink`, which carries engine commands to the
//! audio thread. Drawing goes through the `Renderer` trait. An entry point
//! constructs the platform pieces, configures an engine from the app, starts
//! audio, and then runs the loop of `handle_input`, `update` and `render`.

// DAW input/edit dispatch tables use explicit if guards inside match
// arms rather than collapsing into match guards. Loop boundary
// snapping uses a guarded division rather than checked_div.
#![allow(clippy::collapsible_match)]
#![allow(clippy::manual_checked_ops)]

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use audio::position::compute_orbital_position;
use audio::{AudioEngine, CommandSink, EngineCommand};

use crate::engine_setup::{
    self, apply_chain_to_engine, apply_patch_to_engine, apply_position_to_engine,
    audio_command_to_engine_command, join_path, load_pcm_samples,
};
use crate::input::DawInput;
use crate::midi::{MidiNote, MidiTrack};
use crate::patch::{
    EffectChainConfig, PEDAL_TYPES, PatchConfig, PedalConfig, default_pedal_config,
    pedal_description,
};
use crate::persistence;
use crate::platform::{LogLevel, Platform};
use crate::presets;
use crate::project::{DawProject, NUM_CHANNELS};
use crate::render::{
    self as colors, BG_DARK, BG_PANEL, BG_TRACK, CHANNEL_COLORS, GRID_BAR, GRID_BEAT, GRID_LINE,
    KEY_BLACK, KEY_LABEL, KEY_WHITE, LOOP_MARKER, NOTE_SELECTED, PLAYHEAD, TEXT_ACCENT,
    TEXT_PRIMARY, TEXT_SECONDARY,
};
use crate::render::{Color, FontSize, Renderer};
use crate::script::ScriptRunner;
use crate::transport::Transport;

mod handlers;
mod render;
mod studio;
#[cfg(test)]
mod tests;
mod widgets;

use widgets::*;

/// The window size the layout is designed for. The entry point opens a
/// window of this size.
pub const WINDOW_WIDTH: u32 = 1440;
pub const WINDOW_HEIGHT: u32 = 900;

/// Piano roll constants.
const PIANO_KEY_WIDTH: f32 = 60.0;
const NOTE_HEIGHT: f32 = 12.0;
const TOOLBAR_HEIGHT: f32 = 32.0;
/// The row of page tabs and transport buttons under the toolbar.
const PAGE_TAB_HEIGHT: f32 = 28.0;
const CHANNEL_BAR_HEIGHT: f32 = 28.0;
/// Top of the channel selector bar.
const CHANNEL_BAR_Y: f32 = TOOLBAR_HEIGHT + PAGE_TAB_HEIGHT;
/// Top of the page content, below the toolbar, page tabs and channel bar.
const CONTENT_Y: f32 = CHANNEL_BAR_Y + CHANNEL_BAR_HEIGHT;
const RULER_HEIGHT: f32 = 22.0;
const VELOCITY_LANE_HEIGHT: f32 = 60.0;

/// Full MIDI note range: C-2 (note 0) to G8 (note 127) = 128 notes.
const VISIBLE_NOTE_RANGE: u8 = 128;
const LOWEST_NOTE: u8 = 0; // C-2

/// Pixels per beat at zoom level 1.0.
const BASE_PIXELS_PER_BEAT: f32 = 80.0;

/// Default note duration in ticks (one beat).
const DEFAULT_NOTE_DURATION: u64 = 480;

/// Default note velocity.
const DEFAULT_VELOCITY: u8 = 100;

/// Default project root directory.
pub const PROJECT_ROOT: &str = "daw_projects";

/// How long a preview note sounds, in milliseconds.
const PREVIEW_NOTE_MS: u64 = 200;

/// Minimum interval between script hot-reload checks, in milliseconds.
const SCRIPT_CHECK_INTERVAL_MS: u64 = 1000;

/// Setup page layout constants.
const NAV_BAR_HEIGHT: f32 = 30.0;
const SETUP_MARGIN: f32 = 10.0;
const SETUP_COL_GAP: f32 = 10.0;
const SETUP_ROW_GAP: f32 = 6.0;
const SECTION_HEADER_H: f32 = 24.0;
const PARAM_ROW_H: f32 = 24.0;

/// An undoable action stored in the undo stack.
#[derive(Debug, Clone)]
enum UndoAction {
    /// MIDI track snapshot (for note edits).
    Track { channel: usize, snapshot: MidiTrack },
    /// Single synth parameter change.
    SynthParam {
        channel: usize,
        param: SynthParam,
        old_value: f64,
    },
    /// Single pedal parameter change.
    PedalParam {
        channel: usize,
        chain: u8,
        pedal_idx: usize,
        param_idx: usize,
        old_value: f64,
    },
    /// Pedal chain structural change (add, remove, reorder, bypass toggle).
    PedalChain {
        channel: usize,
        chain: u8,
        old_pedals: Vec<PedalConfig>,
    },
}

/// Identifies a synthesizer parameter on the setup page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SynthParam {
    Vco1PitchFine,
    Vco1Level,
    Vco1PulseWidth,
    Vco2PitchFine,
    Vco2Level,
    Vco2PulseWidth,
    VcoMix,
    RingModDepth,
    LfoRate,
    Eg1Attack,
    Eg1Decay,
    Eg1Sustain,
    Eg1Release,
    Eg2Attack,
    Eg2Decay,
    Eg2Sustain,
    Eg2Release,
    HpfCutoff,
    HpfResonance,
    LpfCutoff,
    LpfResonance,
    LpfSaturation,
    VelocityVca,
    VelocityFilter,
    VelocityEnv,
    VelocityPitch,
    GlideTime,
    Pan,
    Azimuth,
    Elevation,
    PositionDistance,
    OrbitalSemiMajor,
    OrbitalEccentricity,
    OrbitalInclination,
    OrbitalAscendingNode,
    OrbitalPeriapsis,
    OrbitalPeriod,
    VoiceCount,
}

/// Identifies a parameter being dragged on a setup page.
#[derive(Debug, Clone)]
enum DragTarget {
    /// A synth parameter bar.
    Synth {
        param: SynthParam,
        bar_x: f32,
        bar_w: f32,
        min_val: f64,
        max_val: f64,
        log_scale: bool,
    },
    /// A pedal parameter bar.
    Pedal {
        /// Which chain: 0 = pre (mono), 1 = post-left, 2 = post-right.
        chain: u8,
        pedal_idx: usize,
        param_idx: usize,
        bar_x: f32,
        bar_w: f32,
        min_val: f64,
        max_val: f64,
        log_scale: bool,
    },
}

/// DAW page view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DawPage {
    /// The home page: instrument and effect browser, track list and the
    /// effects rack of the selected track, all driven by the mouse.
    Studio,
    PianoRoll,
    Overview,
    SynthSetup,
    MonoChain,
    StereoChain,
}

impl DawPage {
    /// Every page, in tab order.
    const ALL: [DawPage; 6] = [
        Self::Studio,
        Self::PianoRoll,
        Self::Overview,
        Self::SynthSetup,
        Self::MonoChain,
        Self::StereoChain,
    ];

    fn next(self) -> Self {
        match self {
            Self::Studio => Self::PianoRoll,
            Self::PianoRoll => Self::Overview,
            Self::Overview => Self::SynthSetup,
            Self::SynthSetup => Self::MonoChain,
            Self::MonoChain => Self::StereoChain,
            Self::StereoChain => Self::Studio,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Studio => Self::StereoChain,
            Self::PianoRoll => Self::Studio,
            Self::Overview => Self::PianoRoll,
            Self::SynthSetup => Self::Overview,
            Self::MonoChain => Self::SynthSetup,
            Self::StereoChain => Self::MonoChain,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Studio => "Studio",
            Self::PianoRoll => "Piano Roll",
            Self::Overview => "Overview",
            Self::SynthSetup => "Synth Setup",
            Self::MonoChain => "Mono Chain",
            Self::StereoChain => "Stereo Chain",
        }
    }

    /// Whether this page has the setup navigation bar.
    fn is_setup(self) -> bool {
        matches!(self, Self::SynthSetup | Self::MonoChain | Self::StereoChain)
    }

    /// Whether this page shows a pedal chain that the pedal keys act on.
    fn has_pedals(self) -> bool {
        matches!(self, Self::Studio | Self::MonoChain | Self::StereoChain)
    }

    /// Navigate left from setup pages (circular among the 3 setup pages).
    fn setup_left(self) -> Self {
        match self {
            Self::SynthSetup => Self::StereoChain,
            Self::MonoChain => Self::SynthSetup,
            Self::StereoChain => Self::MonoChain,
            _ => self,
        }
    }

    /// Navigate right from setup pages.
    fn setup_right(self) -> Self {
        match self {
            Self::SynthSetup => Self::MonoChain,
            Self::MonoChain => Self::StereoChain,
            Self::StereoChain => Self::SynthSetup,
            _ => self,
        }
    }
}

/// Which toolbar field is being edited inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolbarEdit {
    SongName,
    Bpm,
    TimeSigNum,
    TimeSigDen,
}

/// DAW application state.
pub struct DawApp<P: Platform, S: CommandSink> {
    /// File, log and clock services from the platform layer.
    platform: P,
    /// Output sample rate, which the transport and pedal constructors use.
    sample_rate: u32,
    project: DawProject,
    transport: Transport,
    /// Destination of engine commands, normally the queue to the audio
    /// thread.
    sink: S,
    selected_channel: usize,
    /// Solo channel index, if any. When set, only this channel plays.
    solo_channel: Option<usize>,
    /// Cached metronome enabled state for rendering (synced with engine).
    metronome_enabled: bool,
    scroll_x: f32,
    scroll_y: f32,
    zoom: f32,
    tracks: [MidiTrack; 8],
    patches: [PatchConfig; 8],
    effects: [EffectChainConfig; 8],
    running: bool,
    /// Current page/view being displayed.
    current_page: DawPage,
    /// Indices of currently selected notes in the active channel.
    /// The last element is the "primary" selection for edge-dragging.
    selected_notes: Vec<usize>,
    /// Clipboard for note copy/paste. Tick values are relative to the earliest note.
    clipboard: Vec<MidiNote>,
    /// Snap quantization in ticks. Notes snap to this grid. Default is one beat (480).
    snap_ticks: u64,
    /// Project root directory for save/load.
    project_root: String,
    /// Status message displayed temporarily.
    status_message: Option<String>,
    /// Per-channel: indices of currently playing notes (supports polyphonic playback).
    active_notes: [Vec<usize>; 8],
    /// Previous transport tick position (for detecting tick advancement).
    prev_ticks: u64,
    /// Per-channel preset index into `presets::preset_names()`. None means
    /// the channel uses a custom (non-preset) patch.
    preset_indices: [Option<usize>; 8],
    /// Undo stack of reversible actions.
    undo_stack: Vec<UndoAction>,
    /// Redo stack: filled when undoing, cleared on any new edit.
    redo_stack: Vec<UndoAction>,
    /// Pending parameter undo: captures the pre-drag value for continuous edits.
    /// Flushed to the undo stack on mouse-up or when the drag target changes.
    pending_param_undo: Option<UndoAction>,
    /// Per-channel Keleusma script runners. None if no script assigned.
    scripts: [Option<ScriptRunner>; 8],
    /// Currently active drag target on a setup page parameter.
    dragging_param: Option<DragTarget>,
    /// True when dragging a note's right edge to resize its duration.
    dragging_note_edge: bool,
    /// True when dragging a note velocity in the velocity lane.
    dragging_velocity: bool,
    /// Currently selected pedal on a chain page: (chain_id, pedal_index).
    /// chain_id: 0 = pre (mono), 1 = post-left, 2 = post-right.
    selected_pedal: Option<(u8, usize)>,
    /// Last known mouse position for scroll-over-parameter.
    last_mouse: (f32, f32),
    /// Whether the add-pedal type picker overlay is open.
    pedal_picker_open: bool,
    /// Which chain the pedal picker will insert into.
    pedal_picker_chain: u8,
    /// Preview note: (channel, frequency, start in platform milliseconds) for
    /// auto-off after `PREVIEW_NOTE_MS`.
    preview_note: Option<(usize, f64, u64)>,
    /// Last known modification times of script files for hot-reload.
    script_mtimes: [Option<u64>; 8],
    /// Last time script files were checked for changes.
    last_script_check: u64,
    /// Which toolbar field is being edited inline (None = normal mode).
    toolbar_edit: Option<ToolbarEdit>,
    /// Text buffer for inline toolbar editing.
    edit_buffer: String,
    /// Screen-space origin of a rubber-band drag selection (x, y).
    drag_select_start: Option<(f32, f32)>,
    /// True once the mouse has moved far enough from the drag origin to
    /// activate the marquee rectangle.
    drag_select_active: bool,
    /// When true, drag selection adds to the existing selection instead of
    /// replacing it (Shift+drag behavior).
    drag_select_additive: bool,
    /// True while dragging on the ruler bar to define the loop region.
    dragging_loop: bool,
    /// Which list the Studio browser shows.
    browser_tab: studio::BrowserTab,
    /// Scroll offset of the Studio browser list, in pixels.
    browser_scroll: f32,
    /// Horizontal scroll of the Studio effects rack, in pixels.
    rack_scroll: f32,
    /// Channel whose Studio volume slider is being dragged.
    dragging_volume: Option<usize>,
}

/// Maximum undo history depth.
const MAX_UNDO_DEPTH: usize = 64;

impl<P: Platform, S: CommandSink> DawApp<P, S> {
    /// A new, empty project at the default root.
    pub fn new(platform: P, sink: S, sample_rate: u32) -> Self {
        let last_script_check = platform.elapsed_ms();
        Self {
            platform,
            sample_rate,
            project: DawProject::default(),
            transport: Transport::new(sample_rate, 480),
            sink,
            selected_channel: 0,
            solo_channel: None,
            metronome_enabled: false,
            scroll_x: 0.0,
            scroll_y: (VISIBLE_NOTE_RANGE as f32 / 2.0) * NOTE_HEIGHT,
            zoom: 1.0,
            tracks: core::array::from_fn(|_| MidiTrack::new()),
            patches: core::array::from_fn(|_| PatchConfig::default()),
            effects: core::array::from_fn(|_| EffectChainConfig::default()),
            running: true,
            current_page: DawPage::Studio,
            selected_notes: Vec::new(),
            clipboard: Vec::new(),
            snap_ticks: DEFAULT_NOTE_DURATION,
            project_root: String::from(PROJECT_ROOT),
            status_message: None,
            active_notes: core::array::from_fn(|_| Vec::new()),
            prev_ticks: 0,
            preset_indices: [None; 8],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            pending_param_undo: None,
            scripts: core::array::from_fn(|_| None),
            dragging_param: None,
            dragging_note_edge: false,
            dragging_velocity: false,
            last_mouse: (0.0, 0.0),
            selected_pedal: None,
            pedal_picker_open: false,
            pedal_picker_chain: 0,
            preview_note: None,
            script_mtimes: [None; 8],
            last_script_check,
            toolbar_edit: None,
            edit_buffer: String::new(),
            drag_select_start: None,
            drag_select_active: false,
            drag_select_additive: false,
            dragging_loop: false,
            browser_tab: studio::BrowserTab::Instruments,
            browser_scroll: 0.0,
            rack_scroll: 0.0,
            dragging_volume: None,
        }
    }

    /// Push a command to the audio thread via the lock-free ring buffer.
    /// Logs a warning if the ring buffer is full (should not happen under
    /// normal operation with 1024-slot capacity).
    fn send_command(&mut self, cmd: EngineCommand) {
        if self.sink.send(cmd).is_err() {
            self.platform.log(
                LogLevel::Warn,
                "Audio command ring buffer full, command dropped",
            );
        }
    }

    /// Load PCM samples on the UI thread, then send a Bulk closure to
    /// apply the full patch to the engine on the audio thread.
    fn send_patch(&mut self, ch: usize) {
        let samples_dir = join_path(&self.project_root, "samples");
        let (s1, s2) = load_pcm_samples(&self.platform, &self.patches[ch], &samples_dir);
        let patch = self.patches[ch].clone();
        self.send_command(EngineCommand::Bulk(Box::new(move |engine| {
            apply_patch_to_engine(engine, ch, &patch, s1, s2);
        })));
    }

    /// Get the primary (last) selected note index, if any.
    fn primary_selected_note(&self) -> Option<usize> {
        self.selected_notes.last().copied()
    }

    /// Check if a note index is in the selection.
    fn is_note_selected(&self, idx: usize) -> bool {
        self.selected_notes.contains(&idx)
    }

    /// Clear the note selection.
    fn clear_note_selection(&mut self) {
        self.selected_notes.clear();
    }

    /// Set single note selection, replacing any existing selection.
    fn select_note(&mut self, idx: usize) {
        self.selected_notes.clear();
        self.selected_notes.push(idx);
    }

    /// Toggle a note in the selection (Shift+click behavior).
    fn toggle_note_selection(&mut self, idx: usize) {
        if let Some(pos) = self.selected_notes.iter().position(|&i| i == idx) {
            self.selected_notes.remove(pos);
        } else {
            self.selected_notes.push(idx);
        }
    }

    /// Update the note selection based on a rubber-band rectangle defined by
    /// the drag start (sx, sy) and the current mouse position (mx, my).
    fn update_drag_selection(&mut self, sx: f32, sy: f32, mx: f32, my: f32) {
        let roll_y = Self::roll_y();

        // Convert screen-space rectangle to tick/pitch space.
        let t0 = self.x_to_tick_raw(sx.min(mx));
        let t1 = self.x_to_tick_raw(sx.max(mx));
        let p_top = self.y_to_pitch(sy.min(my), roll_y);
        let p_bot = self.y_to_pitch(sy.max(my), roll_y);
        // y_to_pitch returns higher pitch for lower y, so p_top >= p_bot.
        let pitch_lo = p_bot.min(p_top);
        let pitch_hi = p_bot.max(p_top);

        if !self.drag_select_additive {
            self.selected_notes.clear();
        }
        let track = &self.tracks[self.selected_channel];
        for (i, note) in track.notes.iter().enumerate() {
            let note_end = note.tick + note.duration;
            if note.tick <= t1
                && note_end > t0
                && note.pitch >= pitch_lo
                && note.pitch <= pitch_hi
                && !self.selected_notes.contains(&i)
            {
                self.selected_notes.push(i);
            }
        }
    }

    /// Place a new note at the given screen-space position.
    /// Extracted from the original handle_click empty-space branch.
    fn place_note_at(&mut self, mx: f32, my: f32) {
        let roll_y = Self::roll_y();
        if my < roll_y || mx < PIANO_KEY_WIDTH {
            return;
        }

        let tick = self.x_to_tick(mx);
        let pitch = self.y_to_pitch(my, roll_y);

        if !(LOWEST_NOTE..LOWEST_NOTE + VISIBLE_NOTE_RANGE).contains(&pitch) {
            return;
        }

        // Do not place a note in the velocity lane area.
        if my >= roll_y + Self::roll_h() {
            return;
        }

        self.push_undo();
        let new_note = MidiNote {
            tick,
            duration: self.snap_ticks,
            pitch,
            velocity: DEFAULT_VELOCITY,
        };
        self.tracks[self.selected_channel].notes.push(new_note);
        self.tracks[self.selected_channel].sort();

        // Select the newly placed note.
        let idx = self.tracks[self.selected_channel]
            .notes
            .iter()
            .position(|n| n.tick == tick && n.pitch == pitch);
        if let Some(i) = idx {
            self.select_note(i);
        }
        // Preview the note (only when not playing back).
        if !self.transport.playing {
            self.preview_note(pitch, DEFAULT_VELOCITY);
        }
    }

    /// Assign default file references for any channels that do not have them.
    fn ensure_file_references(&mut self) {
        for i in 0..8 {
            let ch = &mut self.project.channels[i];
            if ch.patch_file.is_empty() {
                ch.patch_file = format!("ch{}_patch.toml", i + 1);
            }
            if ch.effect_chain_file.is_empty() {
                ch.effect_chain_file = format!("ch{}_effects.toml", i + 1);
            }
            if ch.midi_file.is_empty() {
                ch.midi_file = format!("ch{}.toml", i + 1);
            }
        }
    }

    /// Save the project to disk.
    fn save(&mut self) {
        self.ensure_file_references();
        let filename = self.project.name.replace(' ', "_").to_lowercase();
        match persistence::save_project(
            &self.platform,
            &self.project_root,
            &filename,
            &self.project,
            &self.tracks,
            &self.patches,
            &self.effects,
        ) {
            Ok(path) => {
                self.status_message = Some(format!("Saved: {path}"));
                self.platform
                    .log(LogLevel::Info, &format!("Project saved to {path}"));
            }
            Err(e) => {
                self.status_message = Some(format!("Save failed: {e}"));
                self.platform
                    .log(LogLevel::Error, &format!("Save failed: {e}"));
            }
        }
    }

    /// Export all tracks as a Standard MIDI File (.mid).
    fn export_midi(&mut self) {
        let filename = self.project.name.replace(' ', "_").to_lowercase();
        let export_dir = join_path(&self.project_root, "exports");
        let path = join_path(&export_dir, &format!("{filename}.mid"));
        let written = persistence::validate_file_name(&filename).and_then(|()| {
            let midi_data = crate::smf::build_smf(
                &self.tracks,
                self.project.ticks_per_beat,
                self.project.tempo,
                self.project.beats_per_bar,
                self.project.beat_denominator,
            )
            .map_err(|e| e.to_string())?;
            self.platform.write_file(&path, &midi_data)?;
            Ok(midi_data.len())
        });
        match written {
            Ok(bytes) => {
                self.status_message = Some(format!("Exported: {path}"));
                self.platform.log(
                    LogLevel::Info,
                    &format!("MIDI exported to {path} ({bytes} bytes)"),
                );
            }
            Err(e) => {
                self.status_message = Some(format!("Export failed: {e}"));
                self.platform
                    .log(LogLevel::Error, &format!("MIDI export failed: {e}"));
            }
        }
    }

    /// Load Keleusma scripts for all channels that have a script_file set.
    ///
    /// Reads each script file from the scripts/ folder under the project root
    /// and compiles it into a ScriptRunner. Errors are logged but do not
    /// prevent other channels from loading. The first error is also shown
    /// in the status bar with a count of any others, because a composer
    /// started outside a terminal has no visible standard error.
    pub fn load_scripts(&mut self) {
        let scripts_dir = join_path(&self.project_root, "scripts");
        let mut failures: Vec<String> = Vec::new();
        for ch in 0..8 {
            let filename = &self.project.channels[ch].script_file;
            if filename.is_empty() {
                self.scripts[ch] = None;
                self.script_mtimes[ch] = None;
                continue;
            }
            let path = join_path(&scripts_dir, filename);
            self.script_mtimes[ch] = self.platform.modified_stamp(&path);
            match read_script(&self.platform, &path, filename) {
                Ok(source) => match ScriptRunner::from_source(&source) {
                    Ok(runner) => {
                        self.platform.log(
                            LogLevel::Info,
                            &format!("Ch{}: loaded script {}", ch + 1, filename),
                        );
                        self.scripts[ch] = Some(runner);
                    }
                    Err(e) => {
                        self.platform.log(
                            LogLevel::Error,
                            &format!(
                                "Ch{}: script compile error in {}: {}",
                                ch + 1,
                                filename,
                                e.0
                            ),
                        );
                        failures.push(format!("Script Ch{}: {}", ch + 1, e.0));
                        self.scripts[ch] = None;
                    }
                },
                Err(e) => {
                    self.platform.log(
                        LogLevel::Error,
                        &format!("Ch{}: could not read script {}: {}", ch + 1, filename, e),
                    );
                    failures.push(format!("Script Ch{}: could not read {filename}", ch + 1));
                    self.scripts[ch] = None;
                }
            }
        }
        if let Some(first) = failures.first() {
            let more = failures.len() - 1;
            self.status_message = Some(if more == 0 {
                first.clone()
            } else {
                format!("{first} (and {more} more, see the log)")
            });
        }
    }

    /// Check for modified script files and reload them (hot-reload).
    /// Checks at most once per second to avoid excessive filesystem access.
    pub fn check_script_reload(&mut self) {
        let now = self.platform.elapsed_ms();
        if now.saturating_sub(self.last_script_check) < SCRIPT_CHECK_INTERVAL_MS {
            return;
        }
        self.last_script_check = now;

        let scripts_dir = join_path(&self.project_root, "scripts");
        for ch in 0..8 {
            let filename = &self.project.channels[ch].script_file;
            if filename.is_empty() {
                continue;
            }
            let path = join_path(&scripts_dir, filename);
            let current_mtime = self.platform.modified_stamp(&path);

            if current_mtime != self.script_mtimes[ch] && current_mtime.is_some() {
                self.script_mtimes[ch] = current_mtime;
                match read_script(&self.platform, &path, filename) {
                    Ok(source) => match ScriptRunner::from_source(&source) {
                        Ok(runner) => {
                            self.platform.log(
                                LogLevel::Info,
                                &format!("Ch{}: hot-reloaded script {}", ch + 1, filename),
                            );
                            self.scripts[ch] = Some(runner);
                            self.status_message = Some(format!("Reloaded: {filename}"));
                        }
                        Err(e) => {
                            self.platform.log(
                                LogLevel::Error,
                                &format!(
                                    "Ch{}: script compile error in {}: {}",
                                    ch + 1,
                                    filename,
                                    e.0
                                ),
                            );
                            self.status_message = Some(format!("Script error: {}", e.0));
                        }
                    },
                    Err(e) => {
                        self.platform.log(
                            LogLevel::Error,
                            &format!("Ch{}: could not read script {}: {}", ch + 1, filename, e),
                        );
                    }
                }
            }
        }
    }

    /// Advance playback by one frame. Scans all channels for note start/end
    /// events in the tick range since the last frame. Sends note_on/note_off
    /// commands to the audio thread via the lock-free ring buffer. Also ticks
    /// per-channel Keleusma scripts and converts their audio commands.
    pub fn advance_playback(&mut self) {
        if !self.transport.playing {
            return;
        }

        // Advance transport by approximately one frame of audio samples.
        let samples_per_frame = self.transport.sample_rate / 60;
        self.transport.advance_samples(samples_per_frame);

        let current_ticks = self.transport.position_ticks;
        let prev_ticks = self.prev_ticks;

        // Detect loop wrap: transport jumped backward.
        let looped = current_ticks < prev_ticks;

        // Compute beat position for scripts.
        let beat = current_ticks as f64 / self.transport.ticks_per_beat as f64;

        // No lock needed: commands are pushed to the ring buffer.
        self.prev_ticks = current_ticks;

        // On loop wrap, release all active notes so nothing gets stuck.
        if looped {
            for ch in 0..8u8 {
                if !self.active_notes[ch as usize].is_empty() {
                    self.send_command(EngineCommand::NoteOff { channel: ch });
                    self.active_notes[ch as usize].clear();
                }
            }
        }

        // Trigger metronome clicks on beat-unit boundaries.
        if self.metronome_enabled && !looped && prev_ticks < current_ticks {
            let tpbu = self.transport.ticks_per_beat_unit();
            let prev_beat = prev_ticks / tpbu;
            let current_beat = current_ticks / tpbu;
            if current_beat > prev_beat {
                let is_downbeat =
                    (current_ticks / tpbu).is_multiple_of(self.transport.beats_per_bar as u64);
                let sr = self.transport.sample_rate;
                if is_downbeat {
                    self.send_command(EngineCommand::ClickDownbeat { sample_rate: sr });
                } else {
                    self.send_command(EngineCommand::ClickBeat { sample_rate: sr });
                }
            }
        }

        // Update orbital positions for channels with orbital mode enabled.
        let tpbar = self.transport.ticks_per_bar() as f64;
        for ch in 0..8u8 {
            if self.effects[ch as usize].orbital_enabled {
                let eff = &self.effects[ch as usize];
                let period_ticks = eff.orbital_period * tpbar;
                let phase = if period_ticks > 0.0 {
                    (current_ticks as f64 % period_ticks) / period_ticks
                } else {
                    0.0
                };
                let (az, el, dist) = compute_orbital_position(
                    phase,
                    eff.orbital_semi_major,
                    eff.orbital_eccentricity,
                    eff.orbital_inclination,
                    eff.orbital_ascending_node,
                    eff.orbital_periapsis,
                );
                self.send_command(EngineCommand::SetPosition {
                    channel: ch,
                    azimuth_deg: az,
                    elevation_deg: el,
                    distance: dist,
                });
            }
        }

        for ch in 0..8u8 {
            let ch_idx = ch as usize;

            // Release active notes on muted channels instead of skipping.
            if self.project.channels[ch_idx].muted {
                if !self.active_notes[ch_idx].is_empty() {
                    self.send_command(EngineCommand::NoteOff { channel: ch });
                    self.active_notes[ch_idx].clear();
                }
                continue;
            }

            // Tick the channel script if present. Collect commands into a
            // local vec to avoid borrowing self.scripts and self.producer
            // simultaneously.
            let mut script_cmds = Vec::new();
            let mut script_err: Option<String> = None;
            if let Some(runner) = &mut self.scripts[ch_idx] {
                let cmds = runner.tick(current_ticks, self.transport.tempo, beat);
                for cmd in &cmds {
                    script_cmds.push(audio_command_to_engine_command(ch, cmd));
                }
                if let Some(err) = &runner.last_error {
                    script_err = Some(format!("Script Ch{}: {err}", ch_idx + 1));
                }
            }
            for cmd in script_cmds {
                self.send_command(cmd);
            }
            if let Some(msg) = script_err {
                self.status_message = Some(msg);
            }

            // Collect all note commands into local vecs to avoid
            // simultaneous borrows of self.tracks and self.producer.
            let mut pending_cmds: Vec<EngineCommand> = Vec::new();
            let mut new_active: Vec<usize> = Vec::new();

            {
                let track = &self.tracks[ch_idx];

                // Check if any active notes should end.
                self.active_notes[ch_idx].retain(|&note_idx| {
                    if note_idx >= track.notes.len() {
                        pending_cmds.push(EngineCommand::NoteOff { channel: ch });
                        return false;
                    }
                    let note = &track.notes[note_idx];
                    let note_end = note.tick + note.duration;
                    if current_ticks >= note_end {
                        let freq = MidiTrack::note_to_freq(note.pitch);
                        pending_cmds.push(EngineCommand::NoteOffFreq { channel: ch, freq });
                        false
                    } else {
                        true
                    }
                });

                // After a loop wrap, scan from the loop start up to the
                // current position for new note-ons.
                let scan_start = if looped {
                    self.transport.loop_start
                } else {
                    prev_ticks
                };

                for (i, note) in track.notes.iter().enumerate() {
                    if note.tick >= scan_start && note.tick < current_ticks {
                        let freq = MidiTrack::note_to_freq(note.pitch);
                        let vel = note.velocity as f64 / 127.0;
                        pending_cmds.push(EngineCommand::NoteOn {
                            channel: ch,
                            freq,
                            velocity: vel,
                        });
                        pending_cmds.push(EngineCommand::SetChannelEnabled {
                            channel: ch,
                            enabled: true,
                        });
                        new_active.push(i);
                    }
                }
            }

            // Send collected commands and update active notes.
            for cmd in pending_cmds {
                self.send_command(cmd);
            }
            self.active_notes[ch_idx].extend(new_active);
        }

        // Auto-scroll to follow the playhead on scrollable pages.
        if self.current_page == DawPage::PianoRoll || self.current_page == DawPage::Overview {
            let tw = self.tick_width();
            let label_w = if self.current_page == DawPage::Overview {
                100.0
            } else {
                PIANO_KEY_WIDTH
            };
            let playhead_x = current_ticks as f32 * tw - self.scroll_x;
            let roll_w = WINDOW_WIDTH as f32 - label_w;
            if playhead_x > roll_w * 0.85 {
                self.scroll_x = current_ticks as f32 * tw - roll_w * 0.15;
            }
        }
    }

    /// Stop all active notes across all channels.
    ///
    /// Pushes NoteOff commands for every channel with active notes. With the
    /// lock-free architecture, commands are always delivered (no contention).
    fn stop_all_notes(&mut self) {
        for ch in 0..8u8 {
            if !self.active_notes[ch as usize].is_empty() {
                self.send_command(EngineCommand::NoteOff { channel: ch });
                self.active_notes[ch as usize].clear();
            }
        }
    }

    /// Play a brief preview of a note on the selected channel (for note placement feedback).
    fn preview_note(&mut self, pitch: u8, velocity: u8) {
        // Stop any existing preview first.
        self.stop_preview();
        let ch = self.selected_channel;
        let freq = MidiTrack::note_to_freq(pitch);
        let vel = velocity as f64 / 127.0;
        self.send_command(EngineCommand::NoteOn {
            channel: ch as u8,
            freq,
            velocity: vel,
        });
        self.send_command(EngineCommand::SetChannelEnabled {
            channel: ch as u8,
            enabled: true,
        });
        self.preview_note = Some((ch, freq, self.platform.elapsed_ms()));
    }

    /// Stop any currently playing preview note if its duration has elapsed.
    pub fn check_preview_timeout(&mut self) {
        if let Some((ch, freq, start)) = self.preview_note
            && self.platform.elapsed_ms().saturating_sub(start) >= PREVIEW_NOTE_MS
        {
            self.send_command(EngineCommand::NoteOffFreq {
                channel: ch as u8,
                freq,
            });
            self.preview_note = None;
        }
    }

    /// Immediately stop any playing preview note.
    fn stop_preview(&mut self) {
        if let Some((ch, freq, _)) = self.preview_note.take() {
            self.send_command(EngineCommand::NoteOffFreq {
                channel: ch as u8,
                freq,
            });
        }
    }

    /// Synchronize project channel settings (volume, pan, mute) to the audio engine.
    fn sync_engine_channels(&mut self) {
        // Collect all commands first to avoid borrow conflicts between
        // self.project.channels (immutable) and self.producer (mutable).
        let mut cmds = Vec::with_capacity(16);
        for ch in 0..8u8 {
            let conf = &self.project.channels[ch as usize];
            let vol = conf.volume;
            let left = vol * (1.0 - conf.pan.max(0.0));
            let right = vol * (1.0 + conf.pan.min(0.0));
            cmds.push(EngineCommand::SetChannelVolume {
                channel: ch,
                left,
                right,
            });
            let enabled = if let Some(solo) = self.solo_channel {
                ch as usize == solo && !conf.muted
            } else {
                !conf.muted
            };
            cmds.push(EngineCommand::SetChannelEnabled {
                channel: ch,
                enabled,
            });
        }
        for cmd in cmds {
            self.send_command(cmd);
        }
    }

    /// Compute the tick width in pixels for the current zoom level.
    fn tick_width(&self) -> f32 {
        let ppb = BASE_PIXELS_PER_BEAT * self.zoom;
        ppb / self.project.ticks_per_beat as f32
    }

    /// Y-coordinate where the piano roll grid begins (below toolbar, channel bar, and ruler).
    fn roll_y() -> f32 {
        CONTENT_Y + RULER_HEIGHT
    }

    /// Height of the piano roll grid area.
    fn roll_h() -> f32 {
        WINDOW_HEIGHT as f32 - Self::roll_y() - VELOCITY_LANE_HEIGHT
    }

    /// Maximum scroll_y before the piano roll content runs out.
    fn max_scroll_y() -> f32 {
        (VISIBLE_NOTE_RANGE as f32 * NOTE_HEIGHT - Self::roll_h()).max(0.0)
    }

    /// Convert a pixel x-coordinate on the grid to a tick value, snapped to the grid.
    fn x_to_tick(&self, x: f32) -> u64 {
        let raw_tick = ((x - PIANO_KEY_WIDTH + self.scroll_x) / self.tick_width()).max(0.0) as u64;
        // Snap to the nearest grid boundary.
        (raw_tick / self.snap_ticks) * self.snap_ticks
    }

    /// Convert a pixel x-coordinate to a tick value without snap quantization.
    /// Used for rubber-band selection so the rectangle matches visual pixel boundaries.
    fn x_to_tick_raw(&self, x: f32) -> u64 {
        ((x - PIANO_KEY_WIDTH + self.scroll_x) / self.tick_width()).max(0.0) as u64
    }

    /// Convert a pixel y-coordinate on the grid to a MIDI pitch value.
    fn y_to_pitch(&self, y: f32, roll_y: f32) -> u8 {
        let relative = ((y - roll_y + self.scroll_y) / NOTE_HEIGHT) as u8;
        (LOWEST_NOTE + VISIBLE_NOTE_RANGE - 1).saturating_sub(relative)
    }
}

impl<P: Platform, S: CommandSink> DawApp<P, S> {
    pub fn platform(&self) -> &P {
        &self.platform
    }

    /// The command sink, so the entry point can install the audio thread's
    /// queue once the engine has been configured and started.
    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    pub fn project_root(&self) -> &str {
        &self.project_root
    }

    pub fn set_project_root(&mut self, root: &str) {
        self.project_root = String::from(root);
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Whether an inline text field is being edited. The entry point turns
    /// platform text input on and off to match.
    pub fn wants_text_input(&self) -> bool {
        self.toolbar_edit.is_some()
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    /// Load a saved project and its referenced files, replacing the current
    /// one. On failure the current project is unchanged.
    pub fn load_project(&mut self, name: &str) -> Result<(), String> {
        let (project, tracks, patches, effects) =
            persistence::load_project(&self.platform, &self.project_root, name)?;
        self.project = project;
        self.tracks = tracks;
        self.patches = patches;
        self.effects = effects;
        self.sync_transport_to_project();
        self.platform
            .log(LogLevel::Info, &format!("Loaded project: {name}"));
        Ok(())
    }

    /// Import a Standard MIDI File, replacing the notes of each channel it
    /// fills. Returns the number of channels filled.
    ///
    /// A Type 0 file, such as the composer's own export, tells its parts
    /// apart only by MIDI channel, so MIDI channel `c` goes to channel
    /// `first_channel + c`. An export imported at channel 0 therefore comes
    /// back on the channels it left, gaps included. A Type 1 file keeps its
    /// parts in separate track chunks, so its tracks fill consecutive
    /// channels from `first_channel` in file order. Either way, a track that
    /// would land past the last channel is dropped with a warning.
    ///
    /// Resolution. If a channel that the import leaves alone has notes, the
    /// project keeps its ticks per beat and the imported notes are converted
    /// to it, rounded as [`MidiTrack::rescale`] describes, because those notes
    /// are counted in it. Otherwise no note of the old resolution remains, and
    /// the project adopts the file's ticks per beat, so the notes arrive
    /// exactly as written.
    pub fn import_midi(&mut self, data: &[u8], first_channel: usize) -> Result<usize, String> {
        let smf = crate::smf::parse_smf(data).map_err(|e| format!("MIDI import failed: {e}"))?;
        self.platform.log(
            LogLevel::Info,
            &format!("MIDI import: {}", crate::smf::describe_import(&smf)),
        );
        // Each track's destination, or None past the last channel, decided
        // before anything changes so that the resolution can be chosen first.
        let mut next = first_channel;
        let dests: Vec<Option<usize>> = smf
            .tracks
            .iter()
            .map(|smf_track| {
                let dest = if smf.format == 0 {
                    first_channel.checked_add(usize::from(smf_track.channel))
                } else {
                    let dest = next;
                    next = next.saturating_add(1);
                    Some(dest)
                };
                dest.filter(|&d| d < NUM_CHANNELS)
            })
            .collect();
        let replaced = |ch: usize| dests.contains(&Some(ch));
        let keeps_notes =
            (0..NUM_CHANNELS).any(|ch| !replaced(ch) && !self.tracks[ch].notes.is_empty());
        let ticks_per_beat = if keeps_notes {
            self.project.ticks_per_beat
        } else {
            smf.ticks_per_beat
        };

        // Converted in full before any channel changes, so that a failure,
        // possible only for a project at 0 ticks per beat, changes nothing.
        let mut placed = Vec::with_capacity(smf.tracks.len());
        let mut dropped_notes = 0usize;
        for (smf_track, dest) in smf.tracks.iter().zip(dests.iter().copied()) {
            let Some(d) = dest else {
                dropped_notes += smf_track.track.notes.len();
                continue;
            };
            let mut track = smf_track.track.clone();
            track
                .rescale(ticks_per_beat)
                .map_err(|e| format!("MIDI import failed: {e}"))?;
            placed.push((d, track));
        }
        let mut filled = [false; NUM_CHANNELS];
        for (d, track) in placed {
            if filled[d] {
                // Only a Type 0 file with more than one chunk, which the
                // format forbids but some writers emit, can repeat a channel.
                self.tracks[d].notes.extend_from_slice(&track.notes);
                self.tracks[d].sort();
            } else {
                self.tracks[d] = track;
                filled[d] = true;
            }
        }
        for (ch, _) in filled.iter().enumerate().filter(|(_, f)| **f) {
            self.platform.log(
                LogLevel::Info,
                &format!(
                    "Ch{}: imported {} notes",
                    ch + 1,
                    self.tracks[ch].notes.len()
                ),
            );
        }
        if dropped_notes > 0 {
            self.platform.log(
                LogLevel::Warn,
                &format!("MIDI import: dropped {dropped_notes} notes past channel {NUM_CHANNELS}"),
            );
        }
        if smf.ticks_per_beat != ticks_per_beat {
            self.platform.log(
                LogLevel::Info,
                &format!(
                    "MIDI import: converted notes from {} to the project's {ticks_per_beat} ticks per beat",
                    smf.ticks_per_beat
                ),
            );
        } else if ticks_per_beat != self.project.ticks_per_beat {
            self.platform.log(
                LogLevel::Info,
                &format!(
                    "Adjusting ticks per beat: {} -> {ticks_per_beat}",
                    self.project.ticks_per_beat
                ),
            );
            self.project.ticks_per_beat = ticks_per_beat;
        }
        Ok(filled.iter().filter(|f| **f).count())
    }

    /// Assign a script file to a channel. The script loads on the next
    /// `load_scripts`.
    pub fn assign_script(&mut self, channel: usize, file: &str) -> Result<(), String> {
        let Some(conf) = self.project.channels.get_mut(channel) else {
            return Err(format!(
                "channel {} is out of range 1 to {NUM_CHANNELS}",
                channel + 1
            ));
        };
        conf.script_file = String::from(file);
        Ok(())
    }

    /// Bring a fresh engine into the state of the current project, before it
    /// moves to the audio thread. PCM samples load through the platform.
    pub fn configure_engine(&self, engine: &mut AudioEngine) {
        let samples_dir = join_path(&self.project_root, "samples");
        let pcm = core::array::from_fn(|ch| {
            load_pcm_samples(&self.platform, &self.patches[ch], &samples_dir)
        });
        engine_setup::configure_engine(engine, &self.project, &self.patches, &self.effects, pcm);
    }

    /// One frame of the control loop, without input. Advances playback,
    /// ends an expired preview note and checks scripts for changes.
    pub fn update(&mut self) {
        self.advance_playback();
        self.check_preview_timeout();
        self.check_script_reload();
    }

    fn sync_transport_to_project(&mut self) {
        self.transport.tempo = self.project.tempo;
        self.transport.beats_per_bar = self.project.beats_per_bar;
        self.transport.beat_denominator = self.project.beat_denominator;
    }
}

/// Read a script as UTF-8 text after checking that its name stays inside the
/// project.
fn read_script<P: Platform>(platform: &P, path: &str, filename: &str) -> Result<String, String> {
    persistence::validate_file_name(filename)?;
    let bytes = platform.read_file(path)?;
    String::from_utf8(bytes).map_err(|e| format!("{filename} is not UTF-8: {e}"))
}
