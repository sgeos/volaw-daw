mod save_load;
mod sdl_audio;
mod sdl_input;
#[allow(dead_code)]
mod sdl_platform;
mod sdl_renderer;

use std::sync::{Arc, Mutex};

use std::path::PathBuf;

use audio::modmatrix::{ModDest, ModMatrix, ModRouting, ModSource};
use audio::vco::{Scale, Waveform};
use audio::effects::Pedal;
use audio::{AudioEngine, VoiceMode};
use daw_core::input::DawInput;
use daw_core::midi::{MidiNote, MidiTrack};
use daw_core::patch::{
    default_pedal_config, EffectChainConfig, PatchConfig, PedalConfig, PEDAL_TYPES,
};
use daw_core::presets;
use daw_core::project::DawProject;
use daw_core::script::{self, ScriptRunner};
use daw_core::render::{
    self as colors, BG_DARK, BG_PANEL, BG_TRACK, CHANNEL_COLORS, GRID_BAR, GRID_BEAT, GRID_LINE,
    KEY_BLACK, KEY_LABEL, KEY_WHITE, LOOP_MARKER, NOTE_SELECTED, PLAYHEAD, TEXT_ACCENT,
    TEXT_PRIMARY, TEXT_SECONDARY,
};
use daw_core::render::{Color, FontSize, Renderer};
use daw_core::transport::Transport;

const WINDOW_WIDTH: u32 = 1440;
const WINDOW_HEIGHT: u32 = 900;
const WINDOW_TITLE: &str = "Vows of Love and War - Composer";
const SAMPLE_RATE: u32 = 48000;

/// Font file path (same font as the game client).
const FONT_PATH: &str = "assets/fonts/NotoSans-Regular.ttf";

/// Piano roll constants.
const PIANO_KEY_WIDTH: f32 = 60.0;
const NOTE_HEIGHT: f32 = 12.0;
const TOOLBAR_HEIGHT: f32 = 32.0;
const CHANNEL_BAR_HEIGHT: f32 = 28.0;
const VELOCITY_LANE_HEIGHT: f32 = 60.0;

/// Number of visible octaves in the piano roll (C2 to C7 = 5 octaves = 60 notes).
const VISIBLE_NOTE_RANGE: u8 = 60;
const LOWEST_NOTE: u8 = 36; // C2

/// Pixels per beat at zoom level 1.0.
const BASE_PIXELS_PER_BEAT: f32 = 80.0;

/// Default note duration in ticks (one beat).
const DEFAULT_NOTE_DURATION: u64 = 480;

/// Default note velocity.
const DEFAULT_VELOCITY: u8 = 100;

/// Default project root directory.
const PROJECT_ROOT: &str = "daw_projects";

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
    Track {
        channel: usize,
        snapshot: MidiTrack,
    },
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
    VelocityVca,
    VelocityFilter,
    VelocityEnv,
    VelocityPitch,
    GlideTime,
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
    PianoRoll,
    Overview,
    SynthSetup,
    MonoChain,
    StereoChain,
}

impl DawPage {
    fn next(self) -> Self {
        match self {
            Self::PianoRoll => Self::Overview,
            Self::Overview => Self::SynthSetup,
            Self::SynthSetup => Self::MonoChain,
            Self::MonoChain => Self::StereoChain,
            Self::StereoChain => Self::PianoRoll,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::PianoRoll => Self::StereoChain,
            Self::Overview => Self::PianoRoll,
            Self::SynthSetup => Self::Overview,
            Self::MonoChain => Self::SynthSetup,
            Self::StereoChain => Self::MonoChain,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::PianoRoll => "Piano Roll",
            Self::Overview => "Overview",
            Self::SynthSetup => "Synth Setup",
            Self::MonoChain => "Mono Chain",
            Self::StereoChain => "Stereo Chain",
        }
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

/// Parse a waveform name string into the audio engine Waveform enum.
fn parse_waveform(s: &str) -> Waveform {
    match s {
        "triangle" => Waveform::Triangle,
        "sawtooth" => Waveform::Sawtooth,
        "square" => Waveform::PwSquare,
        "sine" => Waveform::Sine,
        "noise" => Waveform::WhiteNoise,
        "ring" => Waveform::RingModulator,
        _ => Waveform::Sawtooth,
    }
}

/// Parse an organ pipe scale string into the audio engine Scale enum.
fn parse_scale(s: &str) -> Scale {
    match s {
        "32'" => Scale::Feet32,
        "16'" => Scale::Feet16,
        "8'" => Scale::Feet8,
        "4'" => Scale::Feet4,
        "2'" => Scale::Feet2,
        _ => Scale::Feet8,
    }
}

/// Parse a modulation source name into the engine ModSource enum.
fn parse_mod_source(s: &str) -> Option<ModSource> {
    match s {
        "eg1" => Some(ModSource::Eg1),
        "eg2" => Some(ModSource::Eg2),
        "lfo" => Some(ModSource::Lfo),
        "velocity" => Some(ModSource::Velocity),
        "key_track" => Some(ModSource::KeyTrack),
        _ => None,
    }
}

/// Parse a modulation destination name into the engine ModDest enum.
fn parse_mod_dest(s: &str) -> Option<ModDest> {
    match s {
        "hpf_cutoff" => Some(ModDest::HpfCutoff),
        "lpf_cutoff" => Some(ModDest::LpfCutoff),
        "lpf_resonance" => Some(ModDest::LpfResonance),
        "vco1_pitch" => Some(ModDest::Vco1Pitch),
        "vco2_pitch" => Some(ModDest::Vco2Pitch),
        "vca_level" => Some(ModDest::VcaLevel),
        "pwm_width" => Some(ModDest::PwmWidth),
        _ => None,
    }
}

/// Apply a PatchConfig to the audio engine channel synth.
///
/// Maps daw-core configuration strings to audio engine enum types and
/// Instantiate a `PedalChain` from a list of `PedalConfig` entries.
///
/// Creates each pedal from its type name, applies saved parameter values,
/// and activates the chain if it contains any pedals.
fn apply_chain_to_engine(
    chain: &mut audio::PedalChain,
    configs: &[PedalConfig],
    sample_rate: f64,
) {
    chain.clear();
    for config in configs {
        if config.bypassed {
            continue;
        }
        if let Some(mut pedal) = Pedal::from_name(&config.pedal_type, sample_rate) {
            for param in &config.params {
                pedal.set_param(&param.name, param.value);
            }
            chain.push(pedal);
        }
    }
    chain.active = !chain.is_empty();
}

/// writes all synthesizer parameters to the Ms20Voice and its voice pool.
fn apply_patch_to_engine(engine: &mut AudioEngine, channel: usize, patch: &PatchConfig) {
    let synth = &mut engine.channels[channel].synth;

    // Voice mode.
    synth.set_voice_mode(parse_voice_mode(&patch.voice_mode));

    // Velocity depth controls.
    synth.velocity_vca_depth = patch.velocity_vca_depth;
    synth.velocity_filter_depth = patch.velocity_filter_depth;
    synth.velocity_envelope_depth = patch.velocity_envelope_depth;
    synth.velocity_pitch_depth = patch.velocity_pitch_depth;

    // Per-voice parameters: apply to all voices in the pool.
    for voice in &mut synth.voices {
        // VCO1.
        voice.vco1.waveform = parse_waveform(&patch.vco1_waveform);
        voice.vco1.scale = parse_scale(&patch.vco1_scale);
        voice.vco1.pitch_fine = patch.vco1_pitch_fine;
        voice.vco1.level = patch.vco1_level;
        voice.vco1.pulse_width = patch.vco1_pulse_width;
        voice.vco1.quantization = patch.vco1_quantization;

        // VCO2.
        voice.vco2.waveform = parse_waveform(&patch.vco2_waveform);
        voice.vco2.scale = parse_scale(&patch.vco2_scale);
        voice.vco2.pitch_fine = patch.vco2_pitch_fine;
        voice.vco2.level = patch.vco2_level;
        voice.vco2.pulse_width = patch.vco2_pulse_width;
        voice.vco2.quantization = patch.vco2_quantization;

        // EG1 (HPF envelope).
        voice.eg1.attack = patch.eg1_attack;
        voice.eg1.decay = patch.eg1_decay;
        voice.eg1.sustain = patch.eg1_sustain;
        voice.eg1.release = patch.eg1_release;

        // EG2 (LPF and VCA envelope).
        voice.eg2.attack = patch.eg2_attack;
        voice.eg2.decay = patch.eg2_decay;
        voice.eg2.sustain = patch.eg2_sustain;
        voice.eg2.release = patch.eg2_release;

        // Filters.
        voice.hpf.cutoff = patch.hpf_cutoff;
        voice.hpf.resonance = patch.hpf_resonance;
        voice.lpf.cutoff = patch.lpf_cutoff;
        voice.lpf.resonance = patch.lpf_resonance;

        // Pitch glide.
        voice.glide.enabled = patch.glide_enabled;
        voice.glide.time = patch.glide_time;
    }

    // Shared state.
    synth.vco_mix = patch.vco_mix;
    synth.lfo_rate = patch.lfo_rate;

    // Modulation matrix: start from MS-20 defaults and apply patch routings
    // additively. The MS-20 defaults include EG1->HPF, EG2->LPF, and the
    // critical EG2->VCA routing that controls output amplitude. Patch configs
    // add character-defining routings (vibrato, filter sweep, etc.) on top.
    synth.mod_matrix = ModMatrix::ms20_default();
    for routing_cfg in &patch.mod_routings {
        if let (Some(src), Some(dst)) = (
            parse_mod_source(&routing_cfg.source),
            parse_mod_dest(&routing_cfg.dest),
        ) {
            synth.mod_matrix.add_routing(ModRouting {
                source: src,
                dest: dst,
                depth: routing_cfg.depth,
            });
        }
    }
}

/// Parse a voice mode string into a VoiceMode enum.
fn parse_voice_mode(s: &str) -> VoiceMode {
    match s {
        "low_1" => VoiceMode::Low(1),
        "high_10" => VoiceMode::High(10),
        "low_10" => VoiceMode::Low(10),
        _ => VoiceMode::High(1),
    }
}

/// DAW application state.
struct DawState {
    project: DawProject,
    transport: Transport,
    #[allow(dead_code)]
    engine: Arc<Mutex<AudioEngine>>,
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
    project_root: PathBuf,
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
    /// Per-channel Keleos script runners. None if no script assigned.
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
    /// Scroll offset for the pedal picker overlay.
    pedal_picker_scroll: f32,
    /// Preview note: (channel, frequency, timestamp in ms) for auto-off after 200ms.
    preview_note: Option<(usize, f64, std::time::Instant)>,
    /// Last known modification times of script files for hot-reload.
    script_mtimes: [Option<std::time::SystemTime>; 8],
    /// Last time script files were checked for changes.
    last_script_check: std::time::Instant,
}

/// Maximum undo history depth.
const MAX_UNDO_DEPTH: usize = 64;

impl DawState {
    fn new(engine: Arc<Mutex<AudioEngine>>) -> Self {
        Self {
            project: DawProject::default(),
            transport: Transport::new(SAMPLE_RATE, 480),
            engine,
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
            current_page: DawPage::PianoRoll,
            selected_notes: Vec::new(),
            clipboard: Vec::new(),
            snap_ticks: DEFAULT_NOTE_DURATION,
            project_root: PathBuf::from(PROJECT_ROOT),
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
            pedal_picker_scroll: 0.0,
            preview_note: None,
            script_mtimes: [None; 8],
            last_script_check: std::time::Instant::now(),
        }
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
        match save_load::save_project(
            &self.project_root,
            &filename,
            &self.project,
            &self.tracks,
            &self.patches,
            &self.effects,
        ) {
            Ok(path) => {
                self.status_message = Some(format!("Saved: {}", path.display()));
                eprintln!("[INFO] Project saved to {}", path.display());
            }
            Err(e) => {
                self.status_message = Some(format!("Save failed: {e}"));
                eprintln!("[ERROR] Save failed: {e}");
            }
        }
    }

    /// Export all tracks as a Standard MIDI File (.mid).
    fn export_midi(&mut self) {
        let filename = self.project.name.replace(' ', "_").to_lowercase();
        let midi_data = daw_core::smf::build_smf(
            &self.tracks,
            self.project.ticks_per_beat,
            self.project.tempo,
            self.project.beats_per_bar,
        );
        let export_dir = self.project_root.join("exports");
        if let Err(e) = std::fs::create_dir_all(&export_dir) {
            self.status_message = Some(format!("Export failed: {e}"));
            eprintln!("[ERROR] Export failed: {e}");
            return;
        }
        let path = export_dir.join(format!("{filename}.mid"));
        match std::fs::write(&path, &midi_data) {
            Ok(()) => {
                self.status_message = Some(format!("Exported: {}", path.display()));
                eprintln!("[INFO] MIDI exported to {} ({} bytes)", path.display(), midi_data.len());
            }
            Err(e) => {
                self.status_message = Some(format!("Export failed: {e}"));
                eprintln!("[ERROR] MIDI export failed: {e}");
            }
        }
    }

    /// Load Keleos scripts for all channels that have a script_file set.
    ///
    /// Reads each script file from the scripts/ folder under the project root
    /// and compiles it into a ScriptRunner. Errors are logged but do not
    /// prevent other channels from loading.
    fn load_scripts(&mut self) {
        let scripts_dir = self.project_root.join("scripts");
        for ch in 0..8 {
            let filename = &self.project.channels[ch].script_file;
            if filename.is_empty() {
                self.scripts[ch] = None;
                self.script_mtimes[ch] = None;
                continue;
            }
            let path = scripts_dir.join(filename);
            self.script_mtimes[ch] = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok();
            match std::fs::read_to_string(&path) {
                Ok(source) => {
                    match ScriptRunner::from_source(&source) {
                        Ok(runner) => {
                            eprintln!("[INFO] Ch{}: loaded script {}", ch + 1, filename);
                            self.scripts[ch] = Some(runner);
                        }
                        Err(e) => {
                            eprintln!(
                                "[ERROR] Ch{}: script compile error in {}: {}",
                                ch + 1,
                                filename,
                                e.0
                            );
                            self.scripts[ch] = None;
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[ERROR] Ch{}: could not read script {}: {}",
                        ch + 1,
                        filename,
                        e
                    );
                    self.scripts[ch] = None;
                }
            }
        }
    }

    /// Check for modified script files and reload them (hot-reload).
    /// Checks at most once per second to avoid excessive filesystem access.
    fn check_script_reload(&mut self) {
        if self.last_script_check.elapsed() < std::time::Duration::from_secs(1) {
            return;
        }
        self.last_script_check = std::time::Instant::now();

        let scripts_dir = self.project_root.join("scripts");
        for ch in 0..8 {
            let filename = &self.project.channels[ch].script_file;
            if filename.is_empty() {
                continue;
            }
            let path = scripts_dir.join(filename);
            let current_mtime = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok();

            if current_mtime != self.script_mtimes[ch] && current_mtime.is_some() {
                self.script_mtimes[ch] = current_mtime;
                match std::fs::read_to_string(&path) {
                    Ok(source) => {
                        match ScriptRunner::from_source(&source) {
                            Ok(runner) => {
                                eprintln!("[INFO] Ch{}: hot-reloaded script {}", ch + 1, filename);
                                self.scripts[ch] = Some(runner);
                                self.status_message =
                                    Some(format!("Reloaded: {filename}"));
                            }
                            Err(e) => {
                                eprintln!(
                                    "[ERROR] Ch{}: script compile error in {}: {}",
                                    ch + 1, filename, e.0
                                );
                                self.status_message =
                                    Some(format!("Script error: {}", e.0));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "[ERROR] Ch{}: could not read script {}: {}",
                            ch + 1, filename, e
                        );
                    }
                }
            }
        }
    }

    /// Advance playback by one frame. Scans all channels for note start/end
    /// events in the tick range since the last frame. Sends note_on/note_off
    /// to the audio engine via the shared Arc<Mutex>. Also ticks per-channel
    /// Keleos scripts and applies their audio commands.
    fn advance_playback(&mut self) {
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

        // Lock the engine to send note events and apply script commands.
        // prev_ticks is updated only after a successful lock so that if
        // try_lock fails, the tick range is retried on the next frame
        // rather than silently dropping notes in the skipped range.
        let Ok(mut engine) = self.engine.try_lock() else {
            return;
        };
        self.prev_ticks = current_ticks;

        // On loop wrap, release all active notes so nothing gets stuck.
        if looped {
            for ch in 0..8 {
                if !self.active_notes[ch].is_empty() {
                    engine.channels[ch].synth.note_off();
                    self.active_notes[ch].clear();
                }
            }
        }

        // Trigger metronome clicks on beat boundaries.
        if engine.metronome.enabled && !looped && prev_ticks < current_ticks {
            let tpb = self.transport.ticks_per_beat as u64;
            let prev_beat = prev_ticks / tpb;
            let current_beat = current_ticks / tpb;
            if current_beat > prev_beat {
                let is_downbeat = (current_ticks / tpb).is_multiple_of(self.transport.beats_per_bar as u64);
                let sr = self.transport.sample_rate;
                if is_downbeat {
                    engine.metronome.click_downbeat(sr);
                } else {
                    engine.metronome.click_beat(sr);
                }
            }
        }

        for ch in 0..8 {
            // Release active notes on muted channels instead of skipping.
            if self.project.channels[ch].muted {
                if !self.active_notes[ch].is_empty() {
                    engine.channels[ch].synth.note_off();
                    self.active_notes[ch].clear();
                }
                continue;
            }

            // Tick the channel script if present.
            if let Some(runner) = &mut self.scripts[ch] {
                let cmds = runner.tick(
                    current_ticks,
                    self.transport.tempo,
                    beat,
                );
                if !cmds.is_empty() {
                    script::apply_commands(&mut engine, ch, &cmds);
                }
                if let Some(err) = &runner.last_error {
                    self.status_message = Some(format!("Script Ch{}: {err}", ch + 1));
                }
            }

            let track = &self.tracks[ch];

            // Check if any active notes should end (per-note release for polyphony).
            self.active_notes[ch].retain(|&note_idx| {
                if note_idx >= track.notes.len() {
                    engine.channels[ch].synth.note_off();
                    return false;
                }
                let note = &track.notes[note_idx];
                let note_end = note.tick + note.duration;
                if current_ticks >= note_end {
                    let freq = MidiTrack::note_to_freq(note.pitch);
                    engine.channels[ch].synth.note_off_freq(freq);
                    false
                } else {
                    true
                }
            });

            // After a loop wrap, scan from the loop start (0 if no explicit
            // loop_start) up to the current position for new note-ons.
            let scan_start = if looped { self.transport.loop_start } else { prev_ticks };

            // Check if new notes should start in the tick range [scan_start, current_ticks).
            for (i, note) in track.notes.iter().enumerate() {
                if note.tick >= scan_start && note.tick < current_ticks {
                    let freq = MidiTrack::note_to_freq(note.pitch);
                    let vel = note.velocity as f64 / 127.0;
                    engine.channels[ch].synth.note_on_with_velocity(freq, vel);
                    engine.channels[ch].enabled = true;
                    self.active_notes[ch].push(i);
                }
            }
        }

        // Auto-scroll to follow the playhead on scrollable pages.
        if self.current_page == DawPage::PianoRoll || self.current_page == DawPage::Overview {
            let tw = self.tick_width();
            let label_w = if self.current_page == DawPage::Overview { 100.0 } else { PIANO_KEY_WIDTH };
            let playhead_x = current_ticks as f32 * tw - self.scroll_x;
            let roll_w = WINDOW_WIDTH as f32 - label_w;
            if playhead_x > roll_w * 0.85 {
                self.scroll_x = current_ticks as f32 * tw - roll_w * 0.15;
            }
        }
    }

    /// Stop all active notes across all channels.
    ///
    /// Uses a blocking lock instead of try_lock to guarantee that note-off
    /// events are delivered. This is called during user-initiated stop/reset
    /// where a brief block is acceptable to prevent stuck notes.
    fn stop_all_notes(&mut self) {
        let Ok(mut engine) = self.engine.lock() else {
            return;
        };
        for ch in 0..8 {
            if !self.active_notes[ch].is_empty() {
                engine.channels[ch].synth.note_off();
                self.active_notes[ch].clear();
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
        if let Ok(mut engine) = self.engine.try_lock() {
            engine.channels[ch].synth.note_on_with_velocity(freq, vel);
            engine.channels[ch].enabled = true;
        }
        self.preview_note = Some((ch, freq, std::time::Instant::now()));
    }

    /// Stop any currently playing preview note if its duration has elapsed.
    fn check_preview_timeout(&mut self) {
        if let Some((ch, freq, start)) = self.preview_note
            && start.elapsed() >= std::time::Duration::from_millis(200)
        {
            // Only clear the preview state after a successful note-off.
            // If the lock fails, retry next frame to avoid stuck notes.
            if let Ok(mut engine) = self.engine.try_lock() {
                engine.channels[ch].synth.note_off_freq(freq);
                self.preview_note = None;
            }
        }
    }

    /// Immediately stop any playing preview note.
    fn stop_preview(&mut self) {
        if let Some((ch, freq, _)) = self.preview_note.take()
            && let Ok(mut engine) = self.engine.try_lock()
        {
            engine.channels[ch].synth.note_off_freq(freq);
        }
    }

    /// Synchronize project channel settings (volume, pan, mute) to the audio engine.
    fn sync_engine_channels(&self) {
        let Ok(mut engine) = self.engine.try_lock() else {
            return;
        };
        for ch in 0..8 {
            let conf = &self.project.channels[ch];
            let vol = conf.volume;
            // Simple stereo pan: pan = -1.0 (left) to 1.0 (right), 0.0 center.
            let left = vol * (1.0 - conf.pan.max(0.0));
            let right = vol * (1.0 + conf.pan.min(0.0));
            engine.channels[ch].set_volume(left, right);
            // Solo overrides mute: when a channel is soloed, all others are disabled.
            let enabled = if let Some(solo) = self.solo_channel {
                ch == solo && !conf.muted
            } else {
                !conf.muted
            };
            engine.channels[ch].enabled = enabled;
        }
    }

    /// Compute the tick width in pixels for the current zoom level.
    fn tick_width(&self) -> f32 {
        let ppb = BASE_PIXELS_PER_BEAT * self.zoom;
        ppb / self.project.ticks_per_beat as f32
    }

    /// Convert a pixel x-coordinate on the grid to a tick value, snapped to the grid.
    fn x_to_tick(&self, x: f32) -> u64 {
        let raw_tick = ((x - PIANO_KEY_WIDTH + self.scroll_x) / self.tick_width()).max(0.0) as u64;
        // Snap to the nearest grid boundary.
        (raw_tick / self.snap_ticks) * self.snap_ticks
    }

    /// Convert a pixel y-coordinate on the grid to a MIDI pitch value.
    fn y_to_pitch(&self, y: f32, roll_y: f32) -> u8 {
        let relative = ((y - roll_y + self.scroll_y) / NOTE_HEIGHT) as u8;
        (LOWEST_NOTE + VISIBLE_NOTE_RANGE - 1).saturating_sub(relative)
    }

    fn handle_input(&mut self, input: DawInput) {
        match input {
            // Global inputs (all pages).
            DawInput::Quit => self.running = false,
            DawInput::TogglePlay => {
                if self.transport.playing {
                    self.transport.stop();
                    self.stop_all_notes();
                } else {
                    self.prev_ticks = self.transport.position_ticks;
                    self.transport.play();
                }
            }
            DawInput::Stop => {
                self.transport.reset();
                self.stop_all_notes();
                self.prev_ticks = 0;
            }
            DawInput::SelectChannel(ch) => {
                if (ch as usize) < 8 {
                    self.selected_channel = ch as usize;
                    self.clear_note_selection();
                }
            }
            DawInput::Save => self.save(),
            DawInput::ExportMidi => self.export_midi(),
            DawInput::Undo => self.undo(),
            DawInput::Redo => self.redo(),
            DawInput::NextPreset => self.cycle_preset(1),
            DawInput::PrevPreset => self.cycle_preset(-1),
            DawInput::ToggleMute => {
                let ch = self.selected_channel;
                self.project.channels[ch].muted = !self.project.channels[ch].muted;
                self.sync_engine_channels();
            }
            DawInput::NextPage => self.current_page = self.current_page.next(),
            DawInput::PrevPage => self.current_page = self.current_page.prev(),
            DawInput::Cancel => {
                if self.pedal_picker_open {
                    self.pedal_picker_open = false;
                } else if self.current_page != DawPage::PianoRoll {
                    self.current_page = DawPage::PianoRoll;
                } else {
                    self.clear_note_selection();
                }
            }
            DawInput::Click(mx, my) => self.handle_click(mx, my),
            DawInput::ShiftClick(mx, my) => self.handle_shift_click(mx, my),
            DawInput::Copy => self.copy_selected_notes(),
            DawInput::Paste => self.paste_notes(),
            DawInput::MouseUp(_, _) => {
                self.flush_pending_undo();
                self.dragging_param = None;
                self.dragging_note_edge = false;
                self.dragging_velocity = false;
            }

            // Piano roll inputs.
            DawInput::Left => {
                if self.current_page == DawPage::PianoRoll {
                    if !self.selected_notes.is_empty() {
                        self.nudge_notes(-(self.snap_ticks as i64));
                    } else {
                        self.scroll_x = (self.scroll_x - 20.0).max(0.0);
                    }
                } else {
                    self.current_page = self.current_page.setup_left();
                }
            }
            DawInput::Right => {
                if self.current_page == DawPage::PianoRoll {
                    if !self.selected_notes.is_empty() {
                        self.nudge_notes(self.snap_ticks as i64);
                    } else {
                        self.scroll_x += 20.0;
                    }
                } else {
                    self.current_page = self.current_page.setup_right();
                }
            }
            DawInput::Up => {
                if self.current_page == DawPage::PianoRoll {
                    if !self.selected_notes.is_empty() {
                        self.transpose_notes(1);
                    } else {
                        self.scroll_y = (self.scroll_y - 20.0).max(0.0);
                    }
                }
            }
            DawInput::Down => {
                if self.current_page == DawPage::PianoRoll {
                    if !self.selected_notes.is_empty() {
                        self.transpose_notes(-1);
                    } else {
                        self.scroll_y += 20.0;
                    }
                }
            }
            DawInput::Scroll(delta) => {
                // Pedal picker scroll.
                if self.pedal_picker_open {
                    // Compute total content height for pedal picker.
                    let item_h = 22.0_f32;
                    let header_h = 28.0_f32;
                    let total_h: f32 = PEDAL_TYPES
                        .iter()
                        .map(|(_, types)| header_h + types.len() as f32 * item_h)
                        .sum();
                    let visible_h = 500.0 - 32.0 - 4.0;
                    let max_scroll = (total_h - visible_h).max(0.0);
                    self.pedal_picker_scroll =
                        (self.pedal_picker_scroll - delta as f32 * 20.0).clamp(0.0, max_scroll);
                    return;
                }
                // Scroll over tempo display adjusts BPM.
                let (mx, my) = self.last_mouse;
                if my < TOOLBAR_HEIGHT && (600.0..=720.0).contains(&mx) {
                    self.adjust_tempo(delta as f64);
                    return;
                }
                if self.current_page == DawPage::PianoRoll {
                    // If notes are selected, scroll adjusts velocity.
                    if !self.selected_notes.is_empty() {
                        let ch = self.selected_channel;
                        let mut last_vel = 0u8;
                        for &idx in &self.selected_notes.clone() {
                            if let Some(note) = self.tracks[ch].notes.get_mut(idx) {
                                let new_vel =
                                    (note.velocity as i32 + delta * 4).clamp(1, 127) as u8;
                                note.velocity = new_vel;
                                last_vel = new_vel;
                            }
                        }
                        let count = self.selected_notes.len();
                        if count == 1 {
                            self.status_message = Some(format!("Vel: {last_vel}"));
                        } else {
                            self.status_message =
                                Some(format!("Vel: {last_vel} ({count} notes)"));
                        }
                    } else {
                        self.scroll_y = (self.scroll_y - delta as f32 * 20.0).max(0.0);
                    }
                } else if self.current_page == DawPage::SynthSetup
                    || self.current_page == DawPage::MonoChain
                    || self.current_page == DawPage::StereoChain
                {
                    self.scroll_param(delta);
                }
            }
            DawInput::ZoomIn => {
                if self.current_page == DawPage::PianoRoll {
                    self.zoom = (self.zoom * 1.2).min(8.0);
                }
            }
            DawInput::ZoomOut => {
                if self.current_page == DawPage::PianoRoll {
                    self.zoom = (self.zoom / 1.2).max(0.25);
                }
            }
            DawInput::Delete => {
                if self.current_page == DawPage::PianoRoll {
                    self.delete_selected_notes();
                } else if self.current_page == DawPage::MonoChain
                    || self.current_page == DawPage::StereoChain
                {
                    self.delete_selected_pedal();
                }
            }
            DawInput::Insert => {
                if self.current_page == DawPage::MonoChain {
                    self.pedal_picker_open = true;
                    self.pedal_picker_chain = 0;
                    self.pedal_picker_scroll = 0.0;
                } else if self.current_page == DawPage::StereoChain {
                    // Default to left chain. Can be toggled.
                    self.pedal_picker_open = true;
                    let chain = if let Some((c, _)) = self.selected_pedal { c } else { 1 };
                    self.pedal_picker_chain = chain;
                    self.pedal_picker_scroll = 0.0;
                }
            }
            DawInput::ShiftLeft => {
                if self.current_page == DawPage::MonoChain
                    || self.current_page == DawPage::StereoChain
                {
                    self.move_selected_pedal(-1);
                }
            }
            DawInput::ShiftRight => {
                if self.current_page == DawPage::MonoChain
                    || self.current_page == DawPage::StereoChain
                {
                    self.move_selected_pedal(1);
                }
            }
            DawInput::ToggleBypass => {
                if self.current_page == DawPage::MonoChain
                    || self.current_page == DawPage::StereoChain
                {
                    self.toggle_pedal_bypass();
                }
            }
            DawInput::GridCycle => self.cycle_grid(),
            DawInput::TempoUp => self.adjust_tempo(1.0),
            DawInput::TempoDown => self.adjust_tempo(-1.0),
            DawInput::Quantize => self.quantize_notes(),
            DawInput::Duplicate => self.duplicate_notes(),
            DawInput::SelectAll => self.select_all_notes(),
            DawInput::ToggleLoop => self.toggle_loop(),
            DawInput::ToggleSolo => self.toggle_solo(),
            DawInput::ToggleMetronome => self.toggle_metronome(),
            DawInput::CycleTimeSig => self.cycle_time_sig(),
            DawInput::Home => {
                self.transport.reset();
                self.stop_all_notes();
                self.scroll_x = 0.0;
                self.status_message = Some(String::from("Home"));
            }
            DawInput::ShiftUp => {
                if self.current_page == DawPage::PianoRoll && !self.selected_notes.is_empty() {
                    self.transpose_notes(12);
                }
            }
            DawInput::ShiftDown => {
                if self.current_page == DawPage::PianoRoll && !self.selected_notes.is_empty() {
                    self.transpose_notes(-12);
                }
            }
            DawInput::MouseMove(mx, my) => {
                self.last_mouse = (mx, my);
                if let Some(ref target) = self.dragging_param {
                    let value = drag_value(mx, target);
                    self.apply_drag_value(target.clone(), value);
                } else if self.dragging_note_edge {
                    self.drag_note_duration(mx);
                } else if self.dragging_velocity {
                    self.drag_velocity(my);
                }
            }
            _ => {}
        }
    }

    /// Handle a mouse click. If over the grid area, either select an existing
    /// note or place a new one. If over the channel bar, select a channel.
    fn handle_click(&mut self, mx: f32, my: f32) {
        let content_y = TOOLBAR_HEIGHT + CHANNEL_BAR_HEIGHT;

        // Channel bar click (all pages).
        if my >= TOOLBAR_HEIGHT && my < content_y {
            let tab_w = WINDOW_WIDTH as f32 / 8.0;
            let ch = (mx / tab_w) as usize;
            if ch < 8 {
                self.selected_channel = ch;
                self.clear_note_selection();
            }
            return;
        }

        // Setup page navigation bar click.
        if self.current_page != DawPage::PianoRoll
            && my >= content_y
            && my < content_y + NAV_BAR_HEIGHT
        {
            if mx < 200.0 {
                self.current_page = self.current_page.setup_left();
            } else if mx > WINDOW_WIDTH as f32 - 200.0 {
                self.current_page = self.current_page.setup_right();
            }
            return;
        }

        // Handle pedal picker overlay clicks.
        if self.pedal_picker_open {
            self.handle_pedal_picker_click(mx, my);
            return;
        }

        // Dispatch to page-specific click handling.
        match self.current_page {
            DawPage::Overview => {
                self.handle_overview_click(mx, my, content_y);
                return;
            }
            DawPage::SynthSetup => {
                self.handle_synth_setup_click(mx, my, content_y);
                return;
            }
            DawPage::MonoChain => {
                self.handle_mono_chain_click(mx, my, content_y);
                return;
            }
            DawPage::StereoChain => {
                self.handle_stereo_chain_click(mx, my, content_y);
                return;
            }
            DawPage::PianoRoll => {}
        }

        // Determine the velocity lane boundary.
        let screen_h = WINDOW_HEIGHT as f32;
        let roll_h = screen_h - content_y - VELOCITY_LANE_HEIGHT;
        let vel_lane_y = content_y + roll_h;

        // Velocity lane click.
        if my >= vel_lane_y && mx >= PIANO_KEY_WIDTH {
            self.handle_velocity_lane_click(mx, my, vel_lane_y);
            return;
        }

        // Piano key click (audition note).
        let roll_y = content_y;
        if my >= roll_y && my < vel_lane_y && mx < PIANO_KEY_WIDTH {
            let pitch = self.y_to_pitch(my, roll_y);
            if (LOWEST_NOTE..LOWEST_NOTE + VISIBLE_NOTE_RANGE).contains(&pitch) {
                self.preview_note(pitch, DEFAULT_VELOCITY);
            }
            return;
        }

        // Piano roll grid click (to the right of the piano keys).
        if my >= roll_y && mx >= PIANO_KEY_WIDTH {
            let tick = self.x_to_tick(mx);
            let pitch = self.y_to_pitch(my, roll_y);

            // Check if clicking on an existing note.
            let tw = self.tick_width();
            let edge_zone = 8.0_f32; // Pixels from right edge that triggers duration drag.
            let track = &self.tracks[self.selected_channel];
            for (i, note) in track.notes.iter().enumerate() {
                let note_x = PIANO_KEY_WIDTH + note.tick as f32 * tw - self.scroll_x;
                let note_w = note.duration as f32 * tw;
                let note_y = self.note_to_y(note.pitch, roll_y);

                if mx >= note_x
                    && mx <= note_x + note_w
                    && my >= note_y
                    && my <= note_y + NOTE_HEIGHT
                {
                    self.select_note(i);
                    // If click is near the right edge, start a duration drag.
                    if mx >= note_x + note_w - edge_zone {
                        self.push_undo();
                        self.dragging_note_edge = true;
                    }
                    return;
                }
            }

            // No existing note hit. Place a new note.
            if (LOWEST_NOTE..LOWEST_NOTE + VISIBLE_NOTE_RANGE).contains(&pitch) {
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
        }
    }

    /// Save a snapshot of the current channel track to the undo stack.
    fn push_undo(&mut self) {
        let ch = self.selected_channel;
        self.push_undo_action(UndoAction::Track {
            channel: ch,
            snapshot: self.tracks[ch].clone(),
        });
    }

    /// Push any undo action onto the stack. Clears the redo stack.
    fn push_undo_action(&mut self, action: UndoAction) {
        if self.undo_stack.len() >= MAX_UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    /// Flush the pending parameter undo to the stack if present.
    fn flush_pending_undo(&mut self) {
        if let Some(action) = self.pending_param_undo.take() {
            self.push_undo_action(action);
        }
    }

    /// Snapshot a pedal chain before a structural modification.
    fn push_chain_undo(&mut self, ch: usize, chain_id: u8) {
        let old_pedals = match chain_id {
            0 => self.effects[ch].pre_pedals.clone(),
            1 => self.effects[ch].post_pedals_left.clone(),
            2 => self.effects[ch].post_pedals_right.clone(),
            _ => return,
        };
        self.push_undo_action(UndoAction::PedalChain {
            channel: ch,
            chain: chain_id,
            old_pedals,
        });
    }

    /// Capture the current state corresponding to an undo action (for redo).
    fn capture_inverse(&self, action: &UndoAction) -> UndoAction {
        match action {
            UndoAction::Track { channel, .. } => UndoAction::Track {
                channel: *channel,
                snapshot: self.tracks[*channel].clone(),
            },
            UndoAction::SynthParam { channel, param, .. } => UndoAction::SynthParam {
                channel: *channel,
                param: *param,
                old_value: self.get_synth_param_value(*channel, *param),
            },
            UndoAction::PedalParam {
                channel,
                chain,
                pedal_idx,
                param_idx,
                ..
            } => UndoAction::PedalParam {
                channel: *channel,
                chain: *chain,
                pedal_idx: *pedal_idx,
                param_idx: *param_idx,
                old_value: self.effects[*channel]
                    .get_pedal_param(*chain, *pedal_idx, *param_idx)
                    .unwrap_or(0.0),
            },
            UndoAction::PedalChain { channel, chain, .. } => {
                let old_pedals = match chain {
                    0 => self.effects[*channel].pre_pedals.clone(),
                    1 => self.effects[*channel].post_pedals_left.clone(),
                    2 => self.effects[*channel].post_pedals_right.clone(),
                    _ => Vec::new(),
                };
                UndoAction::PedalChain {
                    channel: *channel,
                    chain: *chain,
                    old_pedals,
                }
            }
        }
    }

    /// Apply an undo/redo action, restoring the stored state.
    fn apply_action(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Track { channel, snapshot } => {
                self.tracks[*channel] = snapshot.clone();
                self.clear_note_selection();
            }
            UndoAction::SynthParam {
                channel,
                param,
                old_value,
            } => {
                self.set_synth_param(*channel, *param, *old_value);
            }
            UndoAction::PedalParam {
                channel,
                chain,
                pedal_idx,
                param_idx,
                old_value,
            } => {
                self.set_pedal_param(*channel, *chain, *pedal_idx, *param_idx, *old_value);
            }
            UndoAction::PedalChain {
                channel,
                chain,
                old_pedals,
            } => {
                let pedals = match chain {
                    0 => &mut self.effects[*channel].pre_pedals,
                    1 => &mut self.effects[*channel].post_pedals_left,
                    2 => &mut self.effects[*channel].post_pedals_right,
                    _ => return,
                };
                *pedals = old_pedals.clone();
                self.rebuild_engine_chain(*channel, *chain);
                self.selected_pedal = None;
            }
        }
    }

    /// Undo the last edit operation.
    fn undo(&mut self) {
        self.flush_pending_undo();
        if let Some(action) = self.undo_stack.pop() {
            let redo_action = self.capture_inverse(&action);
            self.apply_action(&action);
            self.redo_stack.push(redo_action);
            self.status_message = Some(String::from("Undo"));
        }
    }

    /// Redo the last undone operation.
    fn redo(&mut self) {
        if let Some(action) = self.redo_stack.pop() {
            let undo_action = self.capture_inverse(&action);
            self.apply_action(&action);
            // Push directly to avoid clearing redo stack.
            if self.undo_stack.len() >= MAX_UNDO_DEPTH {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(undo_action);
            self.status_message = Some(String::from("Redo"));
        }
    }

    /// Cycle the preset on the selected channel by the given offset (+1 or -1).
    ///
    /// On the first press, if the channel has no preset index, starts at the
    /// first preset. Wraps around at both ends of the preset list.
    fn cycle_preset(&mut self, offset: i32) {
        let names = presets::preset_names();
        if names.is_empty() {
            return;
        }
        let ch = self.selected_channel;
        let current = self.preset_indices[ch].unwrap_or(0);
        let next = if offset > 0 {
            (current + 1) % names.len()
        } else if current == 0 {
            names.len() - 1
        } else {
            current - 1
        };
        let name = names[next];
        if let Some(patch) = presets::preset(name) {
            self.patches[ch] = patch;
            self.preset_indices[ch] = Some(next);
            self.project.channels[ch].name = String::from(name);
            if let Ok(mut engine) = self.engine.try_lock() {
                apply_patch_to_engine(&mut engine, ch, &self.patches[ch]);
            }
            self.status_message = Some(format!("Ch{}: {}", ch + 1, name));
        }
    }

    /// Cycle the snap grid subdivision through common note values.
    /// 1/1 (whole) -> 1/2 -> 1/4 -> 1/8 -> 1/16 -> 1/32 -> 1/1.
    fn cycle_grid(&mut self) {
        let tpb = self.project.ticks_per_beat as u64;
        let tpbar = self.transport.ticks_per_bar();
        // Grid subdivisions in ticks: whole bar, half, quarter, eighth, sixteenth, thirty-second.
        let subdivisions = [tpbar, tpb * 2, tpb, tpb / 2, tpb / 4, tpb / 8];
        let current_idx = subdivisions.iter().position(|&s| s == self.snap_ticks);
        let next_idx = match current_idx {
            Some(i) => (i + 1) % subdivisions.len(),
            None => 2, // Default to quarter note if current value is non-standard.
        };
        self.snap_ticks = subdivisions[next_idx];
        let label = grid_label(self.snap_ticks, tpb, tpbar);
        self.status_message = Some(format!("Grid: {label}"));
    }

    /// Adjust the project tempo by a given BPM delta, clamped to 20-300 BPM.
    fn adjust_tempo(&mut self, delta: f64) {
        self.project.tempo = (self.project.tempo + delta).clamp(20.0, 300.0);
        self.transport.tempo = self.project.tempo;
        self.status_message = Some(format!("{:.0} BPM", self.project.tempo));
    }

    /// Nudge selected notes horizontally by a tick delta.
    /// Clamps so no note goes below tick 0.
    fn nudge_notes(&mut self, delta_ticks: i64) {
        if self.selected_notes.is_empty() || self.current_page != DawPage::PianoRoll {
            return;
        }
        // Check if any note would go below 0.
        let ch = self.selected_channel;
        let min_tick = self
            .selected_notes
            .iter()
            .filter_map(|&idx| self.tracks[ch].notes.get(idx))
            .map(|n| n.tick)
            .min()
            .unwrap_or(0);
        if (min_tick as i64 + delta_ticks) < 0 {
            return;
        }
        self.push_undo();
        let indices = self.selected_notes.clone();
        // Nudge all selected notes and capture their new (tick, pitch) for re-selection.
        let mut nudged: Vec<(u64, u8)> = Vec::new();
        for &idx in &indices {
            if let Some(note) = self.tracks[ch].notes.get_mut(idx) {
                note.tick = (note.tick as i64 + delta_ticks) as u64;
                nudged.push((note.tick, note.pitch));
            }
        }
        self.tracks[ch].sort();
        // Re-find selected notes after sort using tick+pitch identity.
        self.selected_notes.clear();
        for (i, note) in self.tracks[ch].notes.iter().enumerate() {
            if nudged.iter().any(|&(t, p)| t == note.tick && p == note.pitch) {
                self.selected_notes.push(i);
            }
        }
    }

    /// Transpose selected notes by a number of semitones.
    /// Clamps to MIDI range (0-127).
    fn transpose_notes(&mut self, semitones: i8) {
        if self.selected_notes.is_empty() || self.current_page != DawPage::PianoRoll {
            return;
        }
        self.push_undo();
        let ch = self.selected_channel;
        let indices = self.selected_notes.clone();
        for &idx in &indices {
            if let Some(note) = self.tracks[ch].notes.get_mut(idx) {
                let new_pitch = (note.pitch as i16 + semitones as i16).clamp(0, 127) as u8;
                note.pitch = new_pitch;
            }
        }
        let dir = if semitones > 0 { "up" } else { "down" };
        let amount = semitones.unsigned_abs();
        let unit = if amount == 12 { "octave" } else { "semitone(s)" };
        let display = if amount == 12 { 1 } else { amount };
        self.status_message = Some(format!("Transpose {dir} {display} {unit}"));
    }

    /// Duplicate selected notes, offset by one grid unit to the right.
    /// Select all notes in the current channel track.
    fn select_all_notes(&mut self) {
        if self.current_page != DawPage::PianoRoll {
            return;
        }
        let ch = self.selected_channel;
        let count = self.tracks[ch].notes.len();
        self.selected_notes = (0..count).collect();
        self.status_message = Some(format!("Selected {count} note(s)"));
    }

    fn duplicate_notes(&mut self) {
        if self.selected_notes.is_empty() || self.current_page != DawPage::PianoRoll {
            return;
        }
        self.push_undo();
        let ch = self.selected_channel;
        let offset = self.snap_ticks;
        let indices = self.selected_notes.clone();
        let mut new_notes: Vec<MidiNote> = Vec::new();
        for &idx in &indices {
            if let Some(note) = self.tracks[ch].notes.get(idx) {
                new_notes.push(MidiNote {
                    tick: note.tick + offset,
                    duration: note.duration,
                    pitch: note.pitch,
                    velocity: note.velocity,
                });
            }
        }
        let count = new_notes.len();
        for note in &new_notes {
            self.tracks[ch].notes.push(*note);
        }
        self.tracks[ch].sort();
        // Select the duplicated notes.
        self.selected_notes.clear();
        for (i, note) in self.tracks[ch].notes.iter().enumerate() {
            if new_notes
                .iter()
                .any(|n| n.tick == note.tick && n.pitch == note.pitch)
            {
                self.selected_notes.push(i);
            }
        }
        self.status_message = Some(format!("Duplicated {count} note(s)"));
    }

    /// Quantize selected notes (or all notes if none selected) to the current grid.
    fn quantize_notes(&mut self) {
        if self.current_page != DawPage::PianoRoll {
            return;
        }
        self.push_undo();
        let ch = self.selected_channel;
        let snap = self.snap_ticks;
        let indices: Vec<usize> = if self.selected_notes.is_empty() {
            (0..self.tracks[ch].notes.len()).collect()
        } else {
            self.selected_notes.clone()
        };
        let mut count = 0usize;
        for &idx in &indices {
            if let Some(note) = self.tracks[ch].notes.get_mut(idx) {
                let quantized = ((note.tick + snap / 2) / snap) * snap;
                if quantized != note.tick {
                    note.tick = quantized;
                    count += 1;
                }
            }
        }
        self.tracks[ch].sort();
        self.status_message = Some(format!("Quantized {count} note(s)"));
    }

    /// Toggle solo on the selected channel.
    /// When soloed, only that channel plays. Toggle again to un-solo.
    fn toggle_solo(&mut self) {
        let ch = self.selected_channel;
        if self.solo_channel == Some(ch) {
            self.solo_channel = None;
            self.status_message = Some(String::from("Solo OFF"));
        } else {
            self.solo_channel = Some(ch);
            self.status_message = Some(format!("Solo Ch{}", ch + 1));
        }
        self.sync_engine_channels();
    }

    /// Toggle metronome click on or off.
    fn toggle_metronome(&mut self) {
        if let Ok(mut engine) = self.engine.try_lock() {
            let enabled = !engine.metronome.enabled;
            engine.metronome.enabled = enabled;
            self.metronome_enabled = enabled;
            self.status_message = Some(if enabled {
                String::from("Metronome ON")
            } else {
                String::from("Metronome OFF")
            });
        }
    }

    /// Cycle through common time signatures: 4/4 -> 3/4 -> 6/4 -> 5/4 -> 7/4 -> 4/4.
    fn cycle_time_sig(&mut self) {
        let signatures: &[u8] = &[4, 3, 6, 5, 7];
        let current_idx = signatures
            .iter()
            .position(|&s| s == self.project.beats_per_bar);
        let next_idx = match current_idx {
            Some(i) => (i + 1) % signatures.len(),
            None => 0,
        };
        let bpb = signatures[next_idx];
        self.project.beats_per_bar = bpb;
        self.transport.beats_per_bar = bpb;
        self.status_message = Some(format!("{bpb}/4 Time"));
    }

    /// Toggle loop mode. When enabling, auto-set the loop region to cover
    /// all notes in the current channel track.
    fn toggle_loop(&mut self) {
        self.transport.loop_enabled = !self.transport.loop_enabled;
        if self.transport.loop_enabled {
            let ch = self.selected_channel;
            let tpbar = self.transport.ticks_per_bar();
            if !self.tracks[ch].notes.is_empty() {
                let max_end = self.tracks[ch]
                    .notes
                    .iter()
                    .map(|n| n.tick + n.duration)
                    .max()
                    .unwrap_or(tpbar);
                // Round up to the next bar boundary.
                let bars = max_end.div_ceil(tpbar);
                self.transport.loop_start = 0;
                self.transport.loop_end = bars * tpbar;
            } else {
                self.transport.loop_start = 0;
                self.transport.loop_end = tpbar * 4; // Default 4 bars.
            }
            self.status_message = Some(String::from("Loop ON"));
        } else {
            self.status_message = Some(String::from("Loop OFF"));
        }
    }

    /// Apply all channel patches to the audio engine. Called after loading
    /// a project to synchronize saved patch data with the engine state.
    fn apply_all_patches(&self) {
        let Ok(mut engine) = self.engine.try_lock() else {
            return;
        };
        for ch in 0..8 {
            apply_patch_to_engine(&mut engine, ch, &self.patches[ch]);
        }
    }

    /// Apply all channel effect chains to the audio engine. Called after loading
    /// a project to instantiate saved pedal configs into live audio pedal chains.
    fn apply_all_effects(&self) {
        let Ok(mut engine) = self.engine.try_lock() else {
            return;
        };
        let sr = SAMPLE_RATE as f64;
        for ch in 0..8 {
            let config = &self.effects[ch];
            apply_chain_to_engine(&mut engine.channels[ch].pre_pedals, &config.pre_pedals, sr);
            apply_chain_to_engine(
                &mut engine.channels[ch].post_pedals_left,
                &config.post_pedals_left,
                sr,
            );
            apply_chain_to_engine(
                &mut engine.channels[ch].post_pedals_right,
                &config.post_pedals_right,
                sr,
            );
        }
    }

    /// Drag the selected note's right edge to adjust duration.
    ///
    /// When multiple notes are selected, the primary (last) note determines
    /// the new duration and the delta is applied to all other selected notes.
    fn drag_note_duration(&mut self, mx: f32) {
        let Some(primary_idx) = self.primary_selected_note() else {
            return;
        };
        let ch = self.selected_channel;
        let tw = self.tick_width();
        let sx = self.scroll_x;
        let snap = self.snap_ticks;
        let Some(primary) = self.tracks[ch].notes.get(primary_idx) else {
            return;
        };
        let note_x = PIANO_KEY_WIDTH + primary.tick as f32 * tw - sx;
        let raw_end_tick = ((mx - note_x) / tw).max(0.0) as u64 + primary.tick;
        let new_duration = ((raw_end_tick - primary.tick) / snap).max(1) * snap;
        let old_duration = primary.duration;
        let delta = new_duration as i64 - old_duration as i64;

        // Apply to all selected notes.
        let indices = self.selected_notes.clone();
        for &idx in &indices {
            if let Some(note) = self.tracks[ch].notes.get_mut(idx) {
                let adjusted = (note.duration as i64 + delta).max(snap as i64) as u64;
                // Snap to grid.
                note.duration = (adjusted / snap).max(1) * snap;
            }
        }
    }

    /// Handle a click in the velocity lane. Selects the nearest note at the
    /// clicked x position and begins a velocity drag.
    fn handle_velocity_lane_click(&mut self, mx: f32, my: f32, vel_lane_y: f32) {
        let tw = self.tick_width();
        let track = &self.tracks[self.selected_channel];

        // Find the note whose horizontal span contains the click x.
        let mut hit = None;
        for (i, note) in track.notes.iter().enumerate() {
            let note_x = PIANO_KEY_WIDTH + note.tick as f32 * tw - self.scroll_x;
            let note_w = (note.duration as f32 * tw).max(3.0);
            if mx >= note_x && mx <= note_x + note_w {
                hit = Some(i);
                break;
            }
        }

        if let Some(idx) = hit {
            self.push_undo();
            self.select_note(idx);
            self.dragging_velocity = true;
            // Apply the velocity from the click position immediately.
            self.set_velocity_from_y(my, vel_lane_y);
        }
    }

    /// Set velocity of selected notes from a y position within the velocity lane.
    fn set_velocity_from_y(&mut self, my: f32, vel_lane_y: f32) {
        let bar_margin = 1.0_f32;
        let usable_h = VELOCITY_LANE_HEIGHT - 2.0 * bar_margin;
        // Bottom of lane = velocity 0, top of lane = velocity 127.
        let frac = ((vel_lane_y + VELOCITY_LANE_HEIGHT - bar_margin - my) / usable_h).clamp(0.0, 1.0);
        let velocity = (frac * 127.0).round() as u8;
        let velocity = velocity.max(1); // Minimum velocity is 1.

        let ch = self.selected_channel;
        let indices = self.selected_notes.clone();
        for &idx in &indices {
            if let Some(note) = self.tracks[ch].notes.get_mut(idx) {
                note.velocity = velocity;
            }
        }
    }

    /// Handle continuous velocity drag in the velocity lane.
    fn drag_velocity(&mut self, my: f32) {
        let content_y = TOOLBAR_HEIGHT + CHANNEL_BAR_HEIGHT;
        let screen_h = WINDOW_HEIGHT as f32;
        let roll_h = screen_h - content_y - VELOCITY_LANE_HEIGHT;
        let vel_lane_y = content_y + roll_h;
        self.set_velocity_from_y(my, vel_lane_y);
    }

    /// Handle a click on the synth setup page.
    ///
    /// Tests all parameter bar regions and enum label regions. On a bar hit,
    /// starts a drag. On an enum label hit, cycles to the next value.
    fn handle_synth_setup_click(&mut self, mx: f32, my: f32, content_y: f32) {
        let screen_w = WINDOW_WIDTH as f32;
        let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;
        let col_w = (screen_w - 2.0 * SETUP_MARGIN - 2.0 * SETUP_COL_GAP) / 3.0;
        let x1 = SETUP_MARGIN;
        let x2 = SETUP_MARGIN + col_w + SETUP_COL_GAP;
        let x3 = SETUP_MARGIN + 2.0 * (col_w + SETUP_COL_GAP);

        let bar_inset = 8.0;
        let bar_w = col_w - 16.0;

        // Row 1: VCO1, VCO2, Mix/LFO.
        let row1_h = SECTION_HEADER_H + 5.0 * PARAM_ROW_H + 8.0;

        // VCO1 enum labels (rows 0 and 1 within VCO1 section).
        let vco1_y = y0 + SECTION_HEADER_H;
        if hit_row(mx, my, x1, vco1_y, col_w) {
            self.cycle_vco1_waveform();
            return;
        }
        if hit_row(mx, my, x1, vco1_y + PARAM_ROW_H, col_w) {
            self.cycle_vco1_scale();
            return;
        }
        // VCO1 param bars (rows 2, 3, 4).
        let vco1_bars: [(f32, SynthParam, f64, f64); 3] = [
            (vco1_y + 2.0 * PARAM_ROW_H, SynthParam::Vco1PitchFine, -1.0, 1.0),
            (vco1_y + 3.0 * PARAM_ROW_H, SynthParam::Vco1Level, 0.0, 1.0),
            (vco1_y + 4.0 * PARAM_ROW_H, SynthParam::Vco1PulseWidth, 0.0, 1.0),
        ];
        for (py, param, min, max) in vco1_bars {
            if hit_row(mx, my, x1, py, col_w) {
                self.start_bar_drag(mx, x1 + bar_inset, bar_w, param, min, max);
                return;
            }
        }

        // VCO2 enum labels.
        let vco2_y = y0 + SECTION_HEADER_H;
        if hit_row(mx, my, x2, vco2_y, col_w) {
            self.cycle_vco2_waveform();
            return;
        }
        if hit_row(mx, my, x2, vco2_y + PARAM_ROW_H, col_w) {
            self.cycle_vco2_scale();
            return;
        }
        // VCO2 param bars.
        let vco2_bars: [(f32, SynthParam, f64, f64); 3] = [
            (vco2_y + 2.0 * PARAM_ROW_H, SynthParam::Vco2PitchFine, -1.0, 1.0),
            (vco2_y + 3.0 * PARAM_ROW_H, SynthParam::Vco2Level, 0.0, 1.0),
            (vco2_y + 4.0 * PARAM_ROW_H, SynthParam::Vco2PulseWidth, 0.0, 1.0),
        ];
        for (py, param, min, max) in vco2_bars {
            if hit_row(mx, my, x2, py, col_w) {
                self.start_bar_drag(mx, x2 + bar_inset, bar_w, param, min, max);
                return;
            }
        }

        // Mix / LFO bars.
        let mix_y = y0 + SECTION_HEADER_H;
        let mix_bars: [(f32, SynthParam, f64, f64); 2] = [
            (mix_y, SynthParam::VcoMix, 0.0, 1.0),
            (mix_y + PARAM_ROW_H, SynthParam::LfoRate, 0.1, 20.0),
        ];
        for (py, param, min, max) in mix_bars {
            if hit_row(mx, my, x3, py, col_w) {
                self.start_bar_drag(mx, x3 + bar_inset, bar_w, param, min, max);
                return;
            }
        }

        // Row 2: EG1, EG2, Filters.
        let row2_y = y0 + row1_h + SETUP_ROW_GAP;

        // EG1 bars.
        let eg1_y = row2_y + SECTION_HEADER_H;
        let eg1_bars: [(f32, SynthParam, f64, f64); 4] = [
            (eg1_y, SynthParam::Eg1Attack, 0.0, 5.0),
            (eg1_y + PARAM_ROW_H, SynthParam::Eg1Decay, 0.0, 5.0),
            (eg1_y + 2.0 * PARAM_ROW_H, SynthParam::Eg1Sustain, 0.0, 1.0),
            (eg1_y + 3.0 * PARAM_ROW_H, SynthParam::Eg1Release, 0.0, 5.0),
        ];
        for (py, param, min, max) in eg1_bars {
            if hit_row(mx, my, x1, py, col_w) {
                self.start_bar_drag(mx, x1 + bar_inset, bar_w, param, min, max);
                return;
            }
        }

        // EG2 bars.
        let eg2_y = row2_y + SECTION_HEADER_H;
        let eg2_bars: [(f32, SynthParam, f64, f64); 4] = [
            (eg2_y, SynthParam::Eg2Attack, 0.0, 5.0),
            (eg2_y + PARAM_ROW_H, SynthParam::Eg2Decay, 0.0, 5.0),
            (eg2_y + 2.0 * PARAM_ROW_H, SynthParam::Eg2Sustain, 0.0, 1.0),
            (eg2_y + 3.0 * PARAM_ROW_H, SynthParam::Eg2Release, 0.0, 5.0),
        ];
        for (py, param, min, max) in eg2_bars {
            if hit_row(mx, my, x2, py, col_w) {
                self.start_bar_drag(mx, x2 + bar_inset, bar_w, param, min, max);
                return;
            }
        }

        // HPF bars (column 3, top of row 2).
        let filter_h = SECTION_HEADER_H + 2.0 * PARAM_ROW_H + 8.0;
        let hpf_y = row2_y + SECTION_HEADER_H;
        let hpf_bars: [(f32, SynthParam, f64, f64); 2] = [
            (hpf_y, SynthParam::HpfCutoff, 20.0, 20000.0),
            (hpf_y + PARAM_ROW_H, SynthParam::HpfResonance, 0.0, 4.0),
        ];
        for (py, param, min, max) in hpf_bars {
            if hit_row(mx, my, x3, py, col_w) {
                self.start_bar_drag(mx, x3 + bar_inset, bar_w, param, min, max);
                return;
            }
        }

        // LPF bars (column 3, below HPF).
        let lpf_y = row2_y + filter_h + SETUP_ROW_GAP + SECTION_HEADER_H;
        let lpf_bars: [(f32, SynthParam, f64, f64); 2] = [
            (lpf_y, SynthParam::LpfCutoff, 20.0, 20000.0),
            (lpf_y + PARAM_ROW_H, SynthParam::LpfResonance, 0.0, 4.0),
        ];
        for (py, param, min, max) in lpf_bars {
            if hit_row(mx, my, x3, py, col_w) {
                self.start_bar_drag(mx, x3 + bar_inset, bar_w, param, min, max);
                return;
            }
        }

        // Row 3: Voice, Velocity, Glide.
        let row2_h = SECTION_HEADER_H + 4.0 * PARAM_ROW_H + 8.0;
        let row3_y = row2_y + row2_h + SETUP_ROW_GAP;

        // Voice mode label (click to cycle).
        let voice_y = row3_y + SECTION_HEADER_H;
        if hit_row(mx, my, x1, voice_y, col_w) {
            self.cycle_voice_mode();
            return;
        }

        // Velocity bars.
        let vel_y = row3_y + SECTION_HEADER_H;
        let vel_bars: [(f32, SynthParam, f64, f64); 4] = [
            (vel_y, SynthParam::VelocityVca, 0.0, 1.0),
            (vel_y + PARAM_ROW_H, SynthParam::VelocityFilter, 0.0, 4.0),
            (vel_y + 2.0 * PARAM_ROW_H, SynthParam::VelocityEnv, 0.0, 1.0),
            (vel_y + 3.0 * PARAM_ROW_H, SynthParam::VelocityPitch, 0.0, 12.0),
        ];
        for (py, param, min, max) in vel_bars {
            if hit_row(mx, my, x2, py, col_w) {
                self.start_bar_drag(mx, x2 + bar_inset, bar_w, param, min, max);
                return;
            }
        }

        // Glide toggle (click to cycle on/off).
        let glide_y = row3_y + SECTION_HEADER_H;
        if hit_row(mx, my, x3, glide_y, col_w) {
            self.toggle_glide();
            return;
        }
        // Glide time bar.
        let glide_time_y = glide_y + PARAM_ROW_H;
        if hit_row(mx, my, x3, glide_time_y, col_w) {
            self.start_bar_drag(mx, x3 + bar_inset, bar_w, SynthParam::GlideTime, 0.0, 2.0);
        }
    }

    /// Start a parameter bar drag operation.
    fn start_bar_drag(
        &mut self,
        mx: f32,
        bar_x: f32,
        bar_w: f32,
        param: SynthParam,
        min_val: f64,
        max_val: f64,
    ) {
        // Capture pre-drag value for undo.
        let ch = self.selected_channel;
        let old_value = self.get_synth_param_value(ch, param);
        self.flush_pending_undo();
        self.pending_param_undo = Some(UndoAction::SynthParam {
            channel: ch,
            param,
            old_value,
        });
        let target = DragTarget::Synth {
            param,
            bar_x,
            bar_w,
            min_val,
            max_val,
            log_scale: is_log_synth_param(param),
        };
        let value = drag_value(mx, &target);
        self.apply_drag_value(target.clone(), value);
        self.dragging_param = Some(target);
    }

    /// Cycle VCO1 waveform to the next value.
    fn cycle_vco1_waveform(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].vco1_waveform = next_waveform(&self.patches[ch].vco1_waveform);
        if let Ok(mut engine) = self.engine.try_lock() {
            apply_patch_to_engine(&mut engine, ch, &self.patches[ch]);
        }
    }

    /// Cycle VCO2 waveform to the next value.
    fn cycle_vco2_waveform(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].vco2_waveform = next_waveform(&self.patches[ch].vco2_waveform);
        if let Ok(mut engine) = self.engine.try_lock() {
            apply_patch_to_engine(&mut engine, ch, &self.patches[ch]);
        }
    }

    /// Cycle VCO1 scale to the next value.
    fn cycle_vco1_scale(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].vco1_scale = next_scale(&self.patches[ch].vco1_scale);
        if let Ok(mut engine) = self.engine.try_lock() {
            apply_patch_to_engine(&mut engine, ch, &self.patches[ch]);
        }
    }

    /// Cycle VCO2 scale to the next value.
    fn cycle_vco2_scale(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].vco2_scale = next_scale(&self.patches[ch].vco2_scale);
        if let Ok(mut engine) = self.engine.try_lock() {
            apply_patch_to_engine(&mut engine, ch, &self.patches[ch]);
        }
    }

    /// Cycle voice mode to the next value.
    fn cycle_voice_mode(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].voice_mode = next_voice_mode(&self.patches[ch].voice_mode);
        if let Ok(mut engine) = self.engine.try_lock() {
            apply_patch_to_engine(&mut engine, ch, &self.patches[ch]);
        }
    }

    /// Toggle glide enabled on the selected channel.
    fn toggle_glide(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].glide_enabled = !self.patches[ch].glide_enabled;
        if let Ok(mut engine) = self.engine.try_lock() {
            apply_patch_to_engine(&mut engine, ch, &self.patches[ch]);
        }
    }

    /// Handle a click on the mono chain page (pedal parameter editing).
    fn handle_mono_chain_click(&mut self, mx: f32, my: f32, content_y: f32) {
        let chain = &self.effects[self.selected_channel];
        if chain.pre_pedals.is_empty() {
            return;
        }
        let pedals_y = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP + 48.0;
        self.handle_pedal_chain_click(mx, my, pedals_y, 0);
    }

    /// Handle a click on the stereo chain page (pedal parameter editing).
    fn handle_stereo_chain_click(&mut self, mx: f32, my: f32, content_y: f32) {
        let screen_w = WINDOW_WIDTH as f32;
        let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;
        let pos_h = 44.0;
        let chains_y = y0 + pos_h + SETUP_ROW_GAP;
        let left_header_h = 28.0;
        let pedals_y = chains_y + left_header_h + 4.0;

        let half_w = (screen_w - 3.0 * SETUP_MARGIN) / 2.0;
        let right_x = SETUP_MARGIN + half_w + SETUP_MARGIN;

        // Check if click is in left or right half.
        if mx < right_x {
            self.handle_pedal_chain_click(mx, my, pedals_y, 1);
        } else {
            self.handle_pedal_chain_click(mx, my, pedals_y, 2);
        }
    }

    /// Handle a click within a pedal chain layout.
    ///
    /// `chain_id`: 0 = pre (mono), 1 = post-left, 2 = post-right.
    fn handle_pedal_chain_click(
        &mut self,
        mx: f32,
        my: f32,
        pedals_y: f32,
        chain_id: u8,
    ) {
        let ch = self.selected_channel;
        let pedals = match chain_id {
            0 => &self.effects[ch].pre_pedals,
            1 => &self.effects[ch].post_pedals_left,
            2 => &self.effects[ch].post_pedals_right,
            _ => return,
        };

        let pedal_w = 180.0;
        let pedal_gap = 16.0;
        let bar_inset = 8.0;
        let bar_w = pedal_w - 16.0;
        let header_h = 28.0;
        let mut px = SETUP_MARGIN;

        // Scan for hit targets without mutable self access.
        enum HitKind {
            Led(usize),
            Header(usize, String),
            Param {
                pedal_idx: usize,
                param_idx: usize,
                old_value: f64,
                min: f64,
                max: f64,
                bar_x: f32,
                log_scale: bool,
            },
        }
        let mut hit: Option<HitKind> = None;

        for (pedal_idx, pedal) in pedals.iter().enumerate() {
            let led_x = px + pedal_w - 16.0;
            let led_y = pedals_y + 8.0;
            if mx >= led_x && mx < led_x + 8.0 && my >= led_y && my < led_y + 8.0 {
                hit = Some(HitKind::Led(pedal_idx));
                break;
            }
            if mx >= px && mx < px + pedal_w && my >= pedals_y && my < pedals_y + header_h {
                hit = Some(HitKind::Header(pedal_idx, pedal.pedal_type.clone()));
                break;
            }
            let param_start_y = pedals_y + header_h;
            let mut found = false;
            for (param_idx, param) in pedal.params.iter().enumerate() {
                let py = param_start_y + param_idx as f32 * PARAM_ROW_H;
                if hit_row(mx, my, px, py, pedal_w) {
                    let (min, max) = pedal_param_range(&param.name);
                    hit = Some(HitKind::Param {
                        pedal_idx,
                        param_idx,
                        old_value: param.value,
                        min,
                        max,
                        bar_x: px + bar_inset,
                        log_scale: is_log_pedal_param(&param.name),
                    });
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
            px += pedal_w + pedal_gap;
        }

        // Now process the hit with mutable self access.
        match hit {
            Some(HitKind::Led(pedal_idx)) => {
                self.selected_pedal = Some((chain_id, pedal_idx));
                self.toggle_pedal_bypass();
            }
            Some(HitKind::Header(pedal_idx, name)) => {
                self.selected_pedal = Some((chain_id, pedal_idx));
                self.status_message = Some(format!(
                    "Selected: {name} [Del=Remove, B=Bypass, A=Add, Shift+Arrows=Move]"
                ));
            }
            Some(HitKind::Param {
                pedal_idx,
                param_idx,
                old_value,
                min,
                max,
                bar_x,
                log_scale,
            }) => {
                self.flush_pending_undo();
                self.pending_param_undo = Some(UndoAction::PedalParam {
                    channel: ch,
                    chain: chain_id,
                    pedal_idx,
                    param_idx,
                    old_value,
                });
                let target = DragTarget::Pedal {
                    chain: chain_id,
                    pedal_idx,
                    param_idx,
                    bar_x,
                    bar_w,
                    min_val: min,
                    max_val: max,
                    log_scale,
                };
                let value = drag_value(mx, &target);
                self.apply_drag_value(target.clone(), value);
                self.dragging_param = Some(target);
            }
            None => {
                self.selected_pedal = None;
            }
        }
    }

    /// Delete all selected notes.
    fn delete_selected_notes(&mut self) {
        if self.selected_notes.is_empty() {
            return;
        }
        self.push_undo();
        let ch = self.selected_channel;
        // Sort indices descending so removal does not invalidate earlier indices.
        let mut indices = self.selected_notes.clone();
        indices.sort_unstable();
        indices.dedup();
        for &idx in indices.iter().rev() {
            if idx < self.tracks[ch].notes.len() {
                self.tracks[ch].notes.remove(idx);
            }
        }
        self.clear_note_selection();
    }

    /// Handle Shift+Click for multi-select on the piano roll.
    fn handle_shift_click(&mut self, mx: f32, my: f32) {
        if self.current_page != DawPage::PianoRoll {
            // On non-piano-roll pages, treat Shift+Click as a regular click.
            self.handle_click(mx, my);
            return;
        }
        let roll_y = TOOLBAR_HEIGHT + CHANNEL_BAR_HEIGHT;
        if my < roll_y || mx < PIANO_KEY_WIDTH {
            return;
        }
        let tw = self.tick_width();
        let track = &self.tracks[self.selected_channel];
        for (i, note) in track.notes.iter().enumerate() {
            let note_x = PIANO_KEY_WIDTH + note.tick as f32 * tw - self.scroll_x;
            let note_w = note.duration as f32 * tw;
            let note_y = self.note_to_y(note.pitch, roll_y);
            if mx >= note_x && mx <= note_x + note_w && my >= note_y && my <= note_y + NOTE_HEIGHT
            {
                self.toggle_note_selection(i);
                return;
            }
        }
    }

    /// Copy selected notes to the clipboard.
    fn copy_selected_notes(&mut self) {
        if self.selected_notes.is_empty() {
            return;
        }
        let ch = self.selected_channel;
        let notes: Vec<MidiNote> = self
            .selected_notes
            .iter()
            .filter_map(|&idx| self.tracks[ch].notes.get(idx).copied())
            .collect();
        if notes.is_empty() {
            return;
        }
        // Find earliest tick to make offsets relative.
        let min_tick = notes.iter().map(|n| n.tick).min().unwrap_or(0);
        self.clipboard = notes
            .into_iter()
            .map(|mut n| {
                n.tick -= min_tick;
                n
            })
            .collect();
        let count = self.clipboard.len();
        self.status_message = Some(format!("Copied {count} note(s)"));
    }

    /// Paste notes from the clipboard at the playhead (if playing or non-zero)
    /// or the current scroll position.
    fn paste_notes(&mut self) {
        if self.clipboard.is_empty() || self.current_page != DawPage::PianoRoll {
            return;
        }
        self.push_undo();
        let ch = self.selected_channel;
        // Paste at playhead when transport has a non-zero position, otherwise at scroll edge.
        let paste_tick = if self.transport.position_ticks > 0 {
            self.transport.position_ticks
        } else {
            self.x_to_tick(PIANO_KEY_WIDTH + self.scroll_x)
        };
        let mut new_indices = Vec::new();
        for clip_note in &self.clipboard {
            let note = MidiNote {
                tick: paste_tick + clip_note.tick,
                duration: clip_note.duration,
                pitch: clip_note.pitch,
                velocity: clip_note.velocity,
            };
            self.tracks[ch].notes.push(note);
        }
        self.tracks[ch].sort();
        // Find the pasted notes by matching tick and pitch.
        for clip_note in &self.clipboard {
            let target_tick = paste_tick + clip_note.tick;
            if let Some(idx) = self.tracks[ch]
                .notes
                .iter()
                .position(|n| n.tick == target_tick && n.pitch == clip_note.pitch)
                && !new_indices.contains(&idx)
            {
                new_indices.push(idx);
            }
        }
        self.selected_notes = new_indices;
        let count = self.clipboard.len();
        self.status_message = Some(format!("Pasted {count} note(s)"));
    }

    /// Adjust a parameter by scroll delta.
    ///
    /// On synth page: if actively dragging a param, scroll fine-adjusts it.
    /// On chain pages: hit-tests the mouse position against pedal param bars.
    /// Each scroll tick nudges the value by 2% of the parameter range
    /// (in log space for frequency parameters).
    fn scroll_param(&mut self, delta: i32) {
        // If actively dragging a synth or pedal param, fine-adjust it.
        if let Some(ref target) = self.dragging_param.clone() {
            match target {
                DragTarget::Synth { param, min_val, max_val, log_scale, .. } => {
                    let ch = self.selected_channel;
                    let current = self.get_synth_param_value(ch, *param);
                    let new_val = scroll_step(current, *min_val, *max_val, delta, *log_scale);
                    self.set_synth_param(ch, *param, new_val);
                }
                DragTarget::Pedal { chain, pedal_idx, param_idx, min_val, max_val, log_scale, .. } => {
                    let ch = self.selected_channel;
                    let current = self.effects[ch]
                        .get_pedal_param(*chain, *pedal_idx, *param_idx)
                        .unwrap_or(0.0);
                    let new_val = scroll_step(current, *min_val, *max_val, delta, *log_scale);
                    self.set_pedal_param(ch, *chain, *pedal_idx, *param_idx, new_val);
                }
            }
            return;
        }

        // On chain pages, hit-test the mouse position.
        if self.current_page == DawPage::MonoChain || self.current_page == DawPage::StereoChain {
            let (mx, my) = self.last_mouse;
            let content_y = TOOLBAR_HEIGHT + CHANNEL_BAR_HEIGHT;
            if let Some((chain_id, pedal_idx, param_idx, min, max)) =
                self.find_pedal_param_at(mx, my, content_y)
            {
                let ch = self.selected_channel;
                let current = self.effects[ch]
                    .get_pedal_param(chain_id, pedal_idx, param_idx)
                    .unwrap_or(0.0);
                // Capture undo if this is a new scroll target.
                let same_target = matches!(
                    &self.pending_param_undo,
                    Some(UndoAction::PedalParam {
                        channel: c, chain: ci, pedal_idx: pi, param_idx: pai, ..
                    }) if *c == ch && *ci == chain_id && *pi == pedal_idx && *pai == param_idx
                );
                if !same_target {
                    self.flush_pending_undo();
                    self.pending_param_undo = Some(UndoAction::PedalParam {
                        channel: ch,
                        chain: chain_id,
                        pedal_idx,
                        param_idx,
                        old_value: current,
                    });
                }
                // Pedal scroll: look up param name for log check.
                let log = self.effects[ch]
                    .get_chain_pedals(chain_id)
                    .and_then(|p| p.get(pedal_idx))
                    .and_then(|p| p.params.get(param_idx))
                    .is_some_and(|p| is_log_pedal_param(&p.name));
                let new_val = scroll_step(current, min, max, delta, log);
                self.set_pedal_param(ch, chain_id, pedal_idx, param_idx, new_val);
            }
        }

        // On synth setup page, hit-test the mouse position.
        if self.current_page == DawPage::SynthSetup {
            let (mx, my) = self.last_mouse;
            let content_y = TOOLBAR_HEIGHT + CHANNEL_BAR_HEIGHT;
            if let Some((param, min, max)) = self.find_synth_param_at(mx, my, content_y) {
                let ch = self.selected_channel;
                let current = self.get_synth_param_value(ch, param);
                // Capture undo if this is a new scroll target.
                let same_target = matches!(
                    &self.pending_param_undo,
                    Some(UndoAction::SynthParam { channel: c, param: p, .. })
                        if *c == ch && *p == param
                );
                if !same_target {
                    self.flush_pending_undo();
                    self.pending_param_undo = Some(UndoAction::SynthParam {
                        channel: ch,
                        param,
                        old_value: current,
                    });
                }
                let new_val = scroll_step(current, min, max, delta, is_log_synth_param(param));
                self.set_synth_param(ch, param, new_val);
            }
        }
    }

    /// Find which synth parameter bar is at the given mouse position.
    ///
    /// Returns (SynthParam, min_val, max_val) if the cursor is over a bar.
    fn find_synth_param_at(
        &self,
        mx: f32,
        my: f32,
        content_y: f32,
    ) -> Option<(SynthParam, f64, f64)> {
        let screen_w = WINDOW_WIDTH as f32;
        let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;
        let col_w = (screen_w - 2.0 * SETUP_MARGIN - 2.0 * SETUP_COL_GAP) / 3.0;
        let x1 = SETUP_MARGIN;
        let x2 = SETUP_MARGIN + col_w + SETUP_COL_GAP;
        let x3 = SETUP_MARGIN + 2.0 * (col_w + SETUP_COL_GAP);

        // Row 1: VCO1, VCO2, Mix/LFO.
        let row1_h = SECTION_HEADER_H + 5.0 * PARAM_ROW_H + 8.0;
        let vco1_y = y0 + SECTION_HEADER_H;

        // VCO1 param bars (rows 2, 3, 4 within VCO1 section).
        let vco1_bars: [(f32, SynthParam, f64, f64); 3] = [
            (vco1_y + 2.0 * PARAM_ROW_H, SynthParam::Vco1PitchFine, -1.0, 1.0),
            (vco1_y + 3.0 * PARAM_ROW_H, SynthParam::Vco1Level, 0.0, 1.0),
            (vco1_y + 4.0 * PARAM_ROW_H, SynthParam::Vco1PulseWidth, 0.0, 1.0),
        ];
        for (py, param, min, max) in vco1_bars {
            if hit_row(mx, my, x1, py, col_w) {
                return Some((param, min, max));
            }
        }

        // VCO2 param bars.
        let vco2_y = y0 + SECTION_HEADER_H;
        let vco2_bars: [(f32, SynthParam, f64, f64); 3] = [
            (vco2_y + 2.0 * PARAM_ROW_H, SynthParam::Vco2PitchFine, -1.0, 1.0),
            (vco2_y + 3.0 * PARAM_ROW_H, SynthParam::Vco2Level, 0.0, 1.0),
            (vco2_y + 4.0 * PARAM_ROW_H, SynthParam::Vco2PulseWidth, 0.0, 1.0),
        ];
        for (py, param, min, max) in vco2_bars {
            if hit_row(mx, my, x2, py, col_w) {
                return Some((param, min, max));
            }
        }

        // Mix / LFO bars.
        let mix_y = y0 + SECTION_HEADER_H;
        let mix_bars: [(f32, SynthParam, f64, f64); 2] = [
            (mix_y, SynthParam::VcoMix, 0.0, 1.0),
            (mix_y + PARAM_ROW_H, SynthParam::LfoRate, 0.1, 20.0),
        ];
        for (py, param, min, max) in mix_bars {
            if hit_row(mx, my, x3, py, col_w) {
                return Some((param, min, max));
            }
        }

        // Row 2: EG1, EG2, Filters.
        let row2_y = y0 + row1_h + SETUP_ROW_GAP;
        let filter_h = SECTION_HEADER_H + 2.0 * PARAM_ROW_H + 8.0;

        // EG1 bars.
        let eg1_y = row2_y + SECTION_HEADER_H;
        let eg1_bars: [(f32, SynthParam, f64, f64); 4] = [
            (eg1_y, SynthParam::Eg1Attack, 0.0, 5.0),
            (eg1_y + PARAM_ROW_H, SynthParam::Eg1Decay, 0.0, 5.0),
            (eg1_y + 2.0 * PARAM_ROW_H, SynthParam::Eg1Sustain, 0.0, 1.0),
            (eg1_y + 3.0 * PARAM_ROW_H, SynthParam::Eg1Release, 0.0, 5.0),
        ];
        for (py, param, min, max) in eg1_bars {
            if hit_row(mx, my, x1, py, col_w) {
                return Some((param, min, max));
            }
        }

        // EG2 bars.
        let eg2_y = row2_y + SECTION_HEADER_H;
        let eg2_bars: [(f32, SynthParam, f64, f64); 4] = [
            (eg2_y, SynthParam::Eg2Attack, 0.0, 5.0),
            (eg2_y + PARAM_ROW_H, SynthParam::Eg2Decay, 0.0, 5.0),
            (eg2_y + 2.0 * PARAM_ROW_H, SynthParam::Eg2Sustain, 0.0, 1.0),
            (eg2_y + 3.0 * PARAM_ROW_H, SynthParam::Eg2Release, 0.0, 5.0),
        ];
        for (py, param, min, max) in eg2_bars {
            if hit_row(mx, my, x2, py, col_w) {
                return Some((param, min, max));
            }
        }

        // HPF bars.
        let hpf_y = row2_y + SECTION_HEADER_H;
        let hpf_bars: [(f32, SynthParam, f64, f64); 2] = [
            (hpf_y, SynthParam::HpfCutoff, 20.0, 20000.0),
            (hpf_y + PARAM_ROW_H, SynthParam::HpfResonance, 0.0, 4.0),
        ];
        for (py, param, min, max) in hpf_bars {
            if hit_row(mx, my, x3, py, col_w) {
                return Some((param, min, max));
            }
        }

        // LPF bars.
        let lpf_y = row2_y + filter_h + SETUP_ROW_GAP + SECTION_HEADER_H;
        let lpf_bars: [(f32, SynthParam, f64, f64); 2] = [
            (lpf_y, SynthParam::LpfCutoff, 20.0, 20000.0),
            (lpf_y + PARAM_ROW_H, SynthParam::LpfResonance, 0.0, 4.0),
        ];
        for (py, param, min, max) in lpf_bars {
            if hit_row(mx, my, x3, py, col_w) {
                return Some((param, min, max));
            }
        }

        // Row 3: Velocity, Glide.
        let row2_h = SECTION_HEADER_H + 4.0 * PARAM_ROW_H + 8.0;
        let row3_y = row2_y + row2_h + SETUP_ROW_GAP;

        // Velocity bars.
        let vel_y = row3_y + SECTION_HEADER_H;
        let vel_bars: [(f32, SynthParam, f64, f64); 4] = [
            (vel_y, SynthParam::VelocityVca, 0.0, 1.0),
            (vel_y + PARAM_ROW_H, SynthParam::VelocityFilter, 0.0, 4.0),
            (vel_y + 2.0 * PARAM_ROW_H, SynthParam::VelocityEnv, 0.0, 1.0),
            (vel_y + 3.0 * PARAM_ROW_H, SynthParam::VelocityPitch, 0.0, 12.0),
        ];
        for (py, param, min, max) in vel_bars {
            if hit_row(mx, my, x2, py, col_w) {
                return Some((param, min, max));
            }
        }

        // Glide time bar.
        let glide_y = row3_y + SECTION_HEADER_H + PARAM_ROW_H;
        if hit_row(mx, my, x3, glide_y, col_w) {
            return Some((SynthParam::GlideTime, 0.0, 2.0));
        }

        None
    }

    /// Find which pedal parameter bar is at the given mouse position.
    fn find_pedal_param_at(
        &self,
        mx: f32,
        my: f32,
        content_y: f32,
    ) -> Option<(u8, usize, usize, f64, f64)> {
        let screen_w = WINDOW_WIDTH as f32;
        let pedal_w = 180.0;
        let pedal_gap = 16.0;
        let header_h = 28.0;

        if self.current_page == DawPage::MonoChain {
            let pedals_y = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP + 48.0;
            let pedals = &self.effects[self.selected_channel].pre_pedals;
            return find_pedal_param_in_chain(
                mx, my, pedals_y, SETUP_MARGIN, pedals, pedal_w, pedal_gap, header_h, 0,
            );
        }

        if self.current_page == DawPage::StereoChain {
            let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;
            let chains_y = y0 + 44.0 + SETUP_ROW_GAP;
            let left_header_h = 28.0;
            let pedals_y = chains_y + left_header_h + 4.0;

            let half_w = (screen_w - 3.0 * SETUP_MARGIN) / 2.0;
            let right_x = SETUP_MARGIN + half_w + SETUP_MARGIN;
            let ch = self.selected_channel;

            if mx < right_x {
                return find_pedal_param_in_chain(
                    mx, my, pedals_y, SETUP_MARGIN,
                    &self.effects[ch].post_pedals_left, pedal_w, pedal_gap, header_h, 1,
                );
            } else {
                return find_pedal_param_in_chain(
                    mx, my, pedals_y, right_x,
                    &self.effects[ch].post_pedals_right, pedal_w, pedal_gap, header_h, 2,
                );
            }
        }

        None
    }

    /// Get the current value of a synth parameter from PatchConfig.
    fn get_synth_param_value(&self, ch: usize, param: SynthParam) -> f64 {
        let patch = &self.patches[ch];
        match param {
            SynthParam::Vco1PitchFine => patch.vco1_pitch_fine,
            SynthParam::Vco1Level => patch.vco1_level,
            SynthParam::Vco1PulseWidth => patch.vco1_pulse_width,
            SynthParam::Vco2PitchFine => patch.vco2_pitch_fine,
            SynthParam::Vco2Level => patch.vco2_level,
            SynthParam::Vco2PulseWidth => patch.vco2_pulse_width,
            SynthParam::VcoMix => patch.vco_mix,
            SynthParam::LfoRate => patch.lfo_rate,
            SynthParam::Eg1Attack => patch.eg1_attack,
            SynthParam::Eg1Decay => patch.eg1_decay,
            SynthParam::Eg1Sustain => patch.eg1_sustain,
            SynthParam::Eg1Release => patch.eg1_release,
            SynthParam::Eg2Attack => patch.eg2_attack,
            SynthParam::Eg2Decay => patch.eg2_decay,
            SynthParam::Eg2Sustain => patch.eg2_sustain,
            SynthParam::Eg2Release => patch.eg2_release,
            SynthParam::HpfCutoff => patch.hpf_cutoff,
            SynthParam::HpfResonance => patch.hpf_resonance,
            SynthParam::LpfCutoff => patch.lpf_cutoff,
            SynthParam::LpfResonance => patch.lpf_resonance,
            SynthParam::VelocityVca => patch.velocity_vca_depth,
            SynthParam::VelocityFilter => patch.velocity_filter_depth,
            SynthParam::VelocityEnv => patch.velocity_envelope_depth,
            SynthParam::VelocityPitch => patch.velocity_pitch_depth,
            SynthParam::GlideTime => patch.glide_time,
        }
    }

    /// Toggle bypass on the currently selected pedal.
    fn toggle_pedal_bypass(&mut self) {
        let Some((chain_id, pedal_idx)) = self.selected_pedal else {
            return;
        };
        let ch = self.selected_channel;
        self.push_chain_undo(ch, chain_id);
        let pedals = match chain_id {
            0 => &mut self.effects[ch].pre_pedals,
            1 => &mut self.effects[ch].post_pedals_left,
            2 => &mut self.effects[ch].post_pedals_right,
            _ => return,
        };
        let Some(pedal) = pedals.get_mut(pedal_idx) else {
            return;
        };
        pedal.bypassed = !pedal.bypassed;
        let state = if pedal.bypassed { "BYPASSED" } else { "ACTIVE" };
        let name = pedal.pedal_type.clone();
        self.rebuild_engine_chain(ch, chain_id);
        self.status_message = Some(format!("{name}: {state}"));
    }

    /// Delete the currently selected pedal from its chain.
    fn delete_selected_pedal(&mut self) {
        let Some((chain_id, pedal_idx)) = self.selected_pedal else {
            return;
        };
        let ch = self.selected_channel;
        self.push_chain_undo(ch, chain_id);
        let pedals = match chain_id {
            0 => &mut self.effects[ch].pre_pedals,
            1 => &mut self.effects[ch].post_pedals_left,
            2 => &mut self.effects[ch].post_pedals_right,
            _ => return,
        };
        if pedal_idx >= pedals.len() {
            self.selected_pedal = None;
            return;
        }
        let name = pedals[pedal_idx].pedal_type.clone();
        pedals.remove(pedal_idx);
        self.selected_pedal = None;
        self.rebuild_engine_chain(ch, chain_id);
        self.status_message = Some(format!("Removed {name}"));
    }

    /// Move the selected pedal left or right within its chain.
    fn move_selected_pedal(&mut self, direction: i32) {
        let Some((chain_id, pedal_idx)) = self.selected_pedal else {
            return;
        };
        let ch = self.selected_channel;
        self.push_chain_undo(ch, chain_id);
        let pedals = match chain_id {
            0 => &mut self.effects[ch].pre_pedals,
            1 => &mut self.effects[ch].post_pedals_left,
            2 => &mut self.effects[ch].post_pedals_right,
            _ => return,
        };
        let new_idx = pedal_idx as i32 + direction;
        if new_idx < 0 || new_idx as usize >= pedals.len() {
            return;
        }
        let new_idx = new_idx as usize;
        pedals.swap(pedal_idx, new_idx);
        self.selected_pedal = Some((chain_id, new_idx));
        self.rebuild_engine_chain(ch, chain_id);
    }

    /// Add a pedal of the given type to a chain.
    fn add_pedal_to_chain(&mut self, chain_id: u8, pedal_type: &str) {
        let Some(config) = default_pedal_config(pedal_type) else {
            return;
        };
        let ch = self.selected_channel;
        self.push_chain_undo(ch, chain_id);
        let pedals = match chain_id {
            0 => &mut self.effects[ch].pre_pedals,
            1 => &mut self.effects[ch].post_pedals_left,
            2 => &mut self.effects[ch].post_pedals_right,
            _ => return,
        };
        pedals.push(config);
        let new_idx = pedals.len() - 1;
        self.selected_pedal = Some((chain_id, new_idx));
        self.rebuild_engine_chain(ch, chain_id);
        self.status_message = Some(format!("Added {pedal_type}"));
    }

    /// Rebuild one engine pedal chain from its config.
    fn rebuild_engine_chain(&self, ch: usize, chain_id: u8) {
        let Ok(mut engine) = self.engine.try_lock() else {
            return;
        };
        let sr = SAMPLE_RATE as f64;
        let (engine_chain, configs) = match chain_id {
            0 => (
                &mut engine.channels[ch].pre_pedals,
                &self.effects[ch].pre_pedals,
            ),
            1 => (
                &mut engine.channels[ch].post_pedals_left,
                &self.effects[ch].post_pedals_left,
            ),
            2 => (
                &mut engine.channels[ch].post_pedals_right,
                &self.effects[ch].post_pedals_right,
            ),
            _ => return,
        };
        apply_chain_to_engine(engine_chain, configs, sr);
    }

    /// Handle a click on the pedal type picker overlay.
    fn handle_pedal_picker_click(&mut self, mx: f32, my: f32) {
        let screen_w = WINDOW_WIDTH as f32;
        let screen_h = WINDOW_HEIGHT as f32;
        let picker_w = 400.0;
        let picker_h = 500.0;
        let picker_x = (screen_w - picker_w) / 2.0;
        let picker_y = (screen_h - picker_h) / 2.0;

        // Click outside picker closes it.
        if mx < picker_x || mx > picker_x + picker_w || my < picker_y || my > picker_y + picker_h {
            self.pedal_picker_open = false;
            return;
        }

        // Check which item was clicked (accounting for scroll offset).
        let item_h = 22.0;
        let header_h = 28.0;
        let mut cy = picker_y + 32.0 - self.pedal_picker_scroll;

        for (_category, types) in PEDAL_TYPES {
            // Category header.
            if my >= cy && my < cy + header_h {
                cy += header_h;
                continue;
            }
            cy += header_h;

            for pedal_type in *types {
                if my >= cy && my < cy + item_h && mx >= picker_x && mx < picker_x + picker_w {
                    let chain_id = self.pedal_picker_chain;
                    let pt = String::from(*pedal_type);
                    self.pedal_picker_open = false;
                    self.add_pedal_to_chain(chain_id, &pt);
                    return;
                }
                cy += item_h;
            }
        }
    }

    /// Apply a dragged parameter value to both PatchConfig and the audio engine.
    fn apply_drag_value(&mut self, target: DragTarget, value: f64) {
        let ch = self.selected_channel;
        match target {
            DragTarget::Synth { param, .. } => {
                self.set_synth_param(ch, param, value);
            }
            DragTarget::Pedal {
                chain,
                pedal_idx,
                param_idx,
                ..
            } => {
                self.set_pedal_param(ch, chain, pedal_idx, param_idx, value);
            }
        }
    }

    /// Write a synth parameter value to PatchConfig and sync to the audio engine.
    fn set_synth_param(&mut self, ch: usize, param: SynthParam, value: f64) {
        let patch = &mut self.patches[ch];
        match param {
            SynthParam::Vco1PitchFine => patch.vco1_pitch_fine = value,
            SynthParam::Vco1Level => patch.vco1_level = value,
            SynthParam::Vco1PulseWidth => patch.vco1_pulse_width = value,
            SynthParam::Vco2PitchFine => patch.vco2_pitch_fine = value,
            SynthParam::Vco2Level => patch.vco2_level = value,
            SynthParam::Vco2PulseWidth => patch.vco2_pulse_width = value,
            SynthParam::VcoMix => patch.vco_mix = value,
            SynthParam::LfoRate => patch.lfo_rate = value,
            SynthParam::Eg1Attack => patch.eg1_attack = value,
            SynthParam::Eg1Decay => patch.eg1_decay = value,
            SynthParam::Eg1Sustain => patch.eg1_sustain = value,
            SynthParam::Eg1Release => patch.eg1_release = value,
            SynthParam::Eg2Attack => patch.eg2_attack = value,
            SynthParam::Eg2Decay => patch.eg2_decay = value,
            SynthParam::Eg2Sustain => patch.eg2_sustain = value,
            SynthParam::Eg2Release => patch.eg2_release = value,
            SynthParam::HpfCutoff => patch.hpf_cutoff = value,
            SynthParam::HpfResonance => patch.hpf_resonance = value,
            SynthParam::LpfCutoff => patch.lpf_cutoff = value,
            SynthParam::LpfResonance => patch.lpf_resonance = value,
            SynthParam::VelocityVca => patch.velocity_vca_depth = value,
            SynthParam::VelocityFilter => patch.velocity_filter_depth = value,
            SynthParam::VelocityEnv => patch.velocity_envelope_depth = value,
            SynthParam::VelocityPitch => patch.velocity_pitch_depth = value,
            SynthParam::GlideTime => patch.glide_time = value,
        }
        // Sync the full patch to the engine. This is slightly wasteful (writes all
        // parameters) but ensures consistency and avoids per-parameter engine access.
        if let Ok(mut engine) = self.engine.try_lock() {
            apply_patch_to_engine(&mut engine, ch, &self.patches[ch]);
        }
    }

    /// Write a pedal parameter value to EffectChainConfig and sync to the engine.
    fn set_pedal_param(
        &mut self,
        ch: usize,
        chain: u8,
        pedal_idx: usize,
        param_idx: usize,
        value: f64,
    ) {
        // Update the config.
        let chain_config = &mut self.effects[ch];
        let pedals = match chain {
            0 => &mut chain_config.pre_pedals,
            1 => &mut chain_config.post_pedals_left,
            2 => &mut chain_config.post_pedals_right,
            _ => return,
        };
        let param_name = if let Some(pedal) = pedals.get_mut(pedal_idx) {
            if let Some(param) = pedal.params.get_mut(param_idx) {
                param.value = value;
                param.name.clone()
            } else {
                return;
            }
        } else {
            return;
        };
        // Sync the specific parameter to the engine's live pedal.
        if let Ok(mut engine) = self.engine.try_lock() {
            let engine_chain = match chain {
                0 => &mut engine.channels[ch].pre_pedals,
                1 => &mut engine.channels[ch].post_pedals_left,
                2 => &mut engine.channels[ch].post_pedals_right,
                _ => return,
            };
            if let Some(pedal) = engine_chain.pedals.get_mut(pedal_idx) {
                pedal.set_param(&param_name, value);
            }
        }
    }

    fn render<R: Renderer>(&self, renderer: &mut R) {
        renderer.clear(BG_DARK);

        let screen_w = renderer.screen_width();
        let screen_h = renderer.screen_height();

        // Toolbar area.
        self.render_toolbar(renderer, screen_w);

        // Channel selector bar.
        self.render_channel_bar(renderer, screen_w);

        // Content area.
        let content_y = TOOLBAR_HEIGHT + CHANNEL_BAR_HEIGHT;
        let content_h = screen_h - content_y;
        match self.current_page {
            DawPage::PianoRoll => {
                let roll_h = content_h - VELOCITY_LANE_HEIGHT;
                self.render_piano_roll(renderer, screen_w, content_y, roll_h);
                self.render_velocity_lane(renderer, screen_w, content_y + roll_h, VELOCITY_LANE_HEIGHT);
            }
            DawPage::Overview => {
                self.render_overview(renderer, screen_w, content_y, content_h);
            }
            DawPage::SynthSetup => {
                self.render_synth_setup(renderer, screen_w, content_y, content_h);
            }
            DawPage::MonoChain => {
                self.render_mono_chain(renderer, screen_w, content_y, content_h);
            }
            DawPage::StereoChain => {
                self.render_stereo_chain(renderer, screen_w, content_y, content_h);
            }
        }

        // Pedal picker overlay.
        if self.pedal_picker_open {
            self.render_pedal_picker(renderer, screen_w, screen_h);
        }

        renderer.present();
    }

    /// Render the pedal type picker overlay.
    fn render_pedal_picker<R: Renderer>(&self, renderer: &mut R, screen_w: f32, screen_h: f32) {
        // Semi-transparent backdrop.
        renderer.fill_rect(0.0, 0.0, screen_w, screen_h, Color::rgba(0, 0, 0, 180));

        let picker_w = 400.0;
        let picker_h = 500.0;
        let picker_x = (screen_w - picker_w) / 2.0;
        let picker_y = (screen_h - picker_h) / 2.0;

        // Picker background.
        renderer.fill_rect(picker_x, picker_y, picker_w, picker_h, colors::PANEL_BG);
        renderer.draw_rect(picker_x, picker_y, picker_w, picker_h, TEXT_ACCENT);

        // Title.
        let chain_name = match self.pedal_picker_chain {
            0 => "Mono Chain",
            1 => "Left Chain",
            2 => "Right Chain",
            _ => "Chain",
        };
        let title = format!("Add Pedal to {chain_name}  (Esc to cancel)");
        renderer.draw_text(&title, picker_x + 12.0, picker_y + 8.0, FontSize::Medium, TEXT_ACCENT);

        let item_h = 22.0;
        let header_h = 28.0;
        let content_top = picker_y + 32.0;
        let content_bottom = picker_y + picker_h - 4.0;
        let mut cy = content_top - self.pedal_picker_scroll;
        let accent = CHANNEL_COLORS[self.selected_channel];

        for (category, types) in PEDAL_TYPES {
            // Category header.
            if cy + header_h > content_top && cy < content_bottom {
                renderer.fill_rect(picker_x + 4.0, cy, picker_w - 8.0, header_h - 2.0, BG_DARK);
                renderer.draw_text(category, picker_x + 12.0, cy + 6.0, FontSize::Small, accent);
            }
            cy += header_h;

            for pedal_type in *types {
                if cy + item_h > content_top && cy < content_bottom {
                    renderer.draw_text(pedal_type, picker_x + 24.0, cy + 3.0, FontSize::Small, TEXT_PRIMARY);
                }
                cy += item_h;
            }
        }

        // Scroll indicator when content overflows.
        let total_content_h = cy + self.pedal_picker_scroll - content_top;
        let visible_h = content_bottom - content_top;
        if total_content_h > visible_h {
            let scrollbar_h = (visible_h / total_content_h * visible_h).max(20.0);
            let scrollbar_y = content_top
                + (self.pedal_picker_scroll / (total_content_h - visible_h))
                    * (visible_h - scrollbar_h);
            renderer.fill_rect(
                picker_x + picker_w - 8.0,
                scrollbar_y,
                4.0,
                scrollbar_h,
                Color::rgba(255, 255, 255, 60),
            );
        }
    }

    fn render_toolbar<R: Renderer>(&self, renderer: &mut R, screen_w: f32) {
        renderer.fill_rect(0.0, 0.0, screen_w, TOOLBAR_HEIGHT, BG_PANEL);

        // Project name.
        renderer.draw_text(&self.project.name, 10.0, 8.0, FontSize::Medium, TEXT_PRIMARY);

        // Page indicator.
        renderer.draw_text(self.current_page.name(), 200.0, 8.0, FontSize::Medium, TEXT_ACCENT);

        // Transport controls.
        let play_text = if self.transport.playing { "PLAYING" } else { "STOPPED" };
        let play_color = if self.transport.playing { colors::NOTE_BLOCK } else { TEXT_SECONDARY };
        renderer.draw_text(play_text, 380.0, 8.0, FontSize::Medium, play_color);

        // Position display (bar:beat:tick).
        let (bar, beat, tick) = self.transport.position_bbt();
        let pos_text = format!("{bar}:{beat}:{tick:03}");
        renderer.draw_text(&pos_text, 500.0, 8.0, FontSize::Medium, TEXT_ACCENT);

        // Tempo display.
        let tempo_text = format!("{:.0} BPM", self.project.tempo);
        renderer.draw_text(&tempo_text, 620.0, 8.0, FontSize::Medium, TEXT_PRIMARY);

        // Time signature display.
        let ts_text = format!("{}/4", self.project.beats_per_bar);
        renderer.draw_text(&ts_text, 720.0, 8.0, FontSize::Medium, TEXT_SECONDARY);

        // Grid subdivision display.
        let grid_text = grid_label(self.snap_ticks, self.project.ticks_per_beat as u64, self.transport.ticks_per_bar());
        renderer.draw_text(grid_text, 780.0, 8.0, FontSize::Medium, TEXT_SECONDARY);

        // Loop indicator.
        if self.transport.loop_enabled {
            renderer.draw_text("LOOP", 870.0, 8.0, FontSize::Medium, LOOP_MARKER);
        }

        // Metronome indicator.
        if self.metronome_enabled {
            renderer.draw_text("MET", 940.0, 8.0, FontSize::Medium, TEXT_ACCENT);
        }

        // Status message, selected note info, or page-specific hints.
        if let Some(ref msg) = self.status_message {
            renderer.draw_text(msg, screen_w - 620.0, 12.0, FontSize::Small, TEXT_ACCENT);
        } else if self.current_page == DawPage::PianoRoll
            && let Some(idx) = self.primary_selected_note()
            && let Some(note) = self.tracks[self.selected_channel].notes.get(idx)
        {
            let (name, octave) = MidiTrack::note_name(note.pitch);
            let beats = note.duration as f64 / self.project.ticks_per_beat as f64;
            let count = self.selected_notes.len();
            let sel_info = if count > 1 {
                format!(" [{count} sel]")
            } else {
                String::new()
            };
            let info = format!(
                "{}{} vel:{} dur:{:.1}b{}  Scroll=Vel  Del=Remove  Ctrl+C/V=Copy/Paste",
                name, octave, note.velocity, beats, sel_info
            );
            renderer.draw_text(&info, screen_w - 720.0, 12.0, FontSize::Small, TEXT_ACCENT);
        } else {
            let hints = if self.current_page == DawPage::PianoRoll {
                "Space=Play  S=Stop  1-8=Ch  G=Grid  Shift+G=Quantize  L=Loop  Click=Note  Tab=Setup"
            } else {
                "Click=Edit  Drag=Adjust  Left/Right=Navigate  Esc=Back  P=Preset  Ctrl+S=Save"
            };
            renderer.draw_text(hints, screen_w - 720.0, 12.0, FontSize::Small, TEXT_SECONDARY);
        }
    }

    fn render_channel_bar<R: Renderer>(&self, renderer: &mut R, screen_w: f32) {
        let y = TOOLBAR_HEIGHT;
        renderer.fill_rect(0.0, y, screen_w, CHANNEL_BAR_HEIGHT, Color::rgb(35, 35, 42));

        let tab_w = screen_w / 8.0;
        for (i, &color) in CHANNEL_COLORS.iter().enumerate() {
            let x = i as f32 * tab_w;

            if i == self.selected_channel {
                renderer.fill_rect(x, y, tab_w, CHANNEL_BAR_HEIGHT, Color::rgba(color.r, color.g, color.b, 60));
                renderer.draw_rect(x, y, tab_w, CHANNEL_BAR_HEIGHT, color);
            }

            let name = &self.project.channels[i].name;
            let label = if name.is_empty() {
                format!("Ch {}", i + 1)
            } else {
                name.clone()
            };
            renderer.draw_text(&label, x + 8.0, y + 7.0, FontSize::Small, color);

            // Note count.
            let note_count = self.tracks[i].notes.len();
            if note_count > 0 {
                let count_text = format!("{note_count}");
                renderer.draw_text(&count_text, x + tab_w - 60.0, y + 7.0, FontSize::Small, TEXT_SECONDARY);
            }

            if self.solo_channel == Some(i) {
                renderer.draw_text("S", x + tab_w - 40.0, y + 7.0, FontSize::Small, Color::rgb(255, 200, 80));
            }
            if self.project.channels[i].muted {
                renderer.draw_text("M", x + tab_w - 24.0, y + 7.0, FontSize::Small, Color::rgb(255, 80, 80));
            }
        }
    }

    fn render_piano_roll<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        roll_y: f32,
        roll_h: f32,
    ) {
        let roll_w = screen_w - PIANO_KEY_WIDTH;
        let ppb = BASE_PIXELS_PER_BEAT * self.zoom;

        // Piano keys on the left.
        self.render_piano_keys(renderer, roll_y, roll_h);

        // Grid area background.
        renderer.fill_rect(PIANO_KEY_WIDTH, roll_y, roll_w, roll_h, BG_TRACK);

        // Horizontal grid lines (one per note).
        for i in 0..VISIBLE_NOTE_RANGE {
            let note = LOWEST_NOTE + VISIBLE_NOTE_RANGE - 1 - i;
            let y = roll_y + i as f32 * NOTE_HEIGHT - self.scroll_y;
            if y < roll_y || y > roll_y + roll_h {
                continue;
            }

            // Solid line at the B/C boundary (bottom of C row, top of B row).
            let line_color = if note % 12 == 11 { GRID_BAR } else { GRID_LINE };
            renderer.draw_hline(PIANO_KEY_WIDTH, screen_w, y, line_color);

            // Shade black keys.
            if MidiTrack::is_black_key(note) {
                renderer.fill_rect(
                    PIANO_KEY_WIDTH,
                    y,
                    roll_w,
                    NOTE_HEIGHT,
                    Color::rgba(0, 0, 0, 20),
                );
            }
        }

        // Vertical grid lines (beats and bars).
        let tpb = self.project.ticks_per_beat as f32;
        let tick_width = ppb / tpb;
        let first_tick = (self.scroll_x / tick_width) as u64;
        let visible_ticks = (roll_w / tick_width) as u64 + 1;

        // Step through at the snap grid resolution for subdivision lines.
        let grid_step = self.snap_ticks.max(1);
        let first_grid = (first_tick / grid_step) * grid_step;
        let last_grid = first_tick + visible_ticks + grid_step;

        let mut tick = first_grid;
        while tick <= last_grid {
            let x = PIANO_KEY_WIDTH + tick as f32 * tick_width - self.scroll_x;
            if x < PIANO_KEY_WIDTH || x > screen_w {
                tick += grid_step;
                continue;
            }

            let ticks_per_bar = self.transport.ticks_per_bar();
            let color = if tick.is_multiple_of(ticks_per_bar) {
                GRID_BAR
            } else if tick.is_multiple_of(self.project.ticks_per_beat as u64) {
                GRID_BEAT
            } else {
                GRID_LINE
            };
            renderer.draw_vline(x, roll_y, roll_y + roll_h, color);

            // Bar number label.
            if tick.is_multiple_of(ticks_per_bar) {
                let bar = tick / ticks_per_bar + 1;
                let bar_label = format!("{bar}");
                renderer.draw_text(&bar_label, x + 2.0, roll_y + 2.0, FontSize::Small, TEXT_SECONDARY);
            }
            tick += grid_step;
        }

        // Render notes for the selected channel.
        let track = &self.tracks[self.selected_channel];
        let ch_color = CHANNEL_COLORS[self.selected_channel];
        for (i, note) in track.notes.iter().enumerate() {
            let note_y = self.note_to_y(note.pitch, roll_y);
            let note_x = PIANO_KEY_WIDTH + note.tick as f32 * tick_width - self.scroll_x;
            let note_w = note.duration as f32 * tick_width;

            if note_x + note_w < PIANO_KEY_WIDTH || note_x > screen_w {
                continue;
            }
            if note_y < roll_y - NOTE_HEIGHT || note_y > roll_y + roll_h {
                continue;
            }

            let is_selected = self.is_note_selected(i);
            // Scale brightness by velocity (vel 1 = 40%, vel 127 = 100%).
            let vel_scale = 0.4 + 0.6 * (note.velocity as f32 / 127.0);
            let fill_color = if is_selected {
                NOTE_SELECTED
            } else {
                Color::rgba(
                    (ch_color.r as f32 * vel_scale) as u8,
                    (ch_color.g as f32 * vel_scale) as u8,
                    (ch_color.b as f32 * vel_scale) as u8,
                    ch_color.a,
                )
            };
            renderer.fill_rect(note_x, note_y, note_w, NOTE_HEIGHT - 1.0, fill_color);
            let border = if is_selected {
                Color::rgba(255, 255, 255, 180)
            } else {
                Color::rgba(255, 255, 255, 40)
            };
            renderer.draw_rect(note_x, note_y, note_w, NOTE_HEIGHT - 1.0, border);
        }

        // Loop region markers.
        if self.transport.loop_enabled && self.transport.loop_end > self.transport.loop_start {
            let start_x = PIANO_KEY_WIDTH + self.transport.loop_start as f32 * tick_width - self.scroll_x;
            let end_x = PIANO_KEY_WIDTH + self.transport.loop_end as f32 * tick_width - self.scroll_x;
            // Shade the loop region.
            let vis_start = start_x.max(PIANO_KEY_WIDTH);
            let vis_end = end_x.min(screen_w);
            if vis_end > vis_start {
                renderer.fill_rect(
                    vis_start,
                    roll_y,
                    vis_end - vis_start,
                    roll_h,
                    Color::rgba(80, 200, 120, 15),
                );
            }
            // Draw loop boundary lines.
            if start_x >= PIANO_KEY_WIDTH && start_x <= screen_w {
                renderer.draw_vline(start_x, roll_y, roll_y + roll_h, LOOP_MARKER);
            }
            if end_x >= PIANO_KEY_WIDTH && end_x <= screen_w {
                renderer.draw_vline(end_x, roll_y, roll_y + roll_h, LOOP_MARKER);
            }
        }

        // Playhead.
        if self.transport.playing || self.transport.position_ticks > 0 {
            let playhead_x = PIANO_KEY_WIDTH + self.transport.position_ticks as f32 * tick_width - self.scroll_x;
            if playhead_x >= PIANO_KEY_WIDTH && playhead_x <= screen_w {
                renderer.draw_vline(playhead_x, roll_y, roll_y + roll_h, PLAYHEAD);
            }
        }
    }

    /// Render the velocity lane below the piano roll.
    ///
    /// Each note in the current channel is drawn as a vertical bar whose height
    /// is proportional to velocity (1-127). Selected notes use the selection color.
    /// Render the multi-track overview showing all 8 channels.
    fn render_overview<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        content_y: f32,
        content_h: f32,
    ) {
        let label_w = 100.0_f32;
        let lane_h = content_h / 8.0;
        let note_area_w = screen_w - label_w;

        // Compute visible tick range based on scroll and zoom.
        let tw = self.tick_width();
        let start_tick = (self.scroll_x / tw) as u64;
        let visible_ticks = (note_area_w / tw) as u64;
        let end_tick = start_tick + visible_ticks;

        for (ch, &color) in CHANNEL_COLORS.iter().enumerate() {
            let y = content_y + ch as f32 * lane_h;
            let is_selected = ch == self.selected_channel;

            // Lane background.
            let bg = if is_selected {
                Color::rgba(color.r, color.g, color.b, 30)
            } else {
                BG_TRACK
            };
            renderer.fill_rect(0.0, y, screen_w, lane_h - 1.0, bg);

            // Lane separator.
            renderer.fill_rect(0.0, y + lane_h - 1.0, screen_w, 1.0, GRID_LINE);

            // Channel label.
            let label_color = if is_selected { color } else { TEXT_SECONDARY };
            let name = &self.project.channels[ch].name;
            renderer.draw_text(name, 8.0, y + 4.0, FontSize::Small, label_color);

            // Note count and mute/solo indicators.
            let count = self.tracks[ch].notes.len();
            let mut info = format!("{count} notes");
            if self.project.channels[ch].muted {
                info.push_str(" [M]");
            }
            if self.solo_channel == Some(ch) {
                info.push_str(" [S]");
            }
            renderer.draw_text(&info, 8.0, y + 22.0, FontSize::Small, TEXT_SECONDARY);

            // Render note blocks.
            let track = &self.tracks[ch];
            for note in &track.notes {
                let note_end = note.tick + note.duration;
                if note_end < start_tick || note.tick > end_tick {
                    continue;
                }

                let nx = label_w + (note.tick as f32 - start_tick as f32) * tw;
                let nw = (note.duration as f32 * tw).max(1.0);

                // Map pitch to vertical position within the lane.
                // Pitch range: 36 (C2) to 96 (C7) = 60 semitones.
                let pitch_min = 36.0_f32;
                let pitch_range = 60.0_f32;
                let note_h = (lane_h - 10.0) / pitch_range;
                let ny = y + lane_h - 6.0 - (note.pitch as f32 - pitch_min).clamp(0.0, pitch_range) * note_h;

                let alpha = (note.velocity as f32 / 127.0 * 200.0 + 55.0) as u8;
                let note_color = Color::rgba(color.r, color.g, color.b, alpha);
                renderer.fill_rect(nx, ny - note_h, nw, note_h.max(2.0), note_color);
            }
        }

        // Bar lines.
        let ticks_per_bar = self.transport.ticks_per_bar();
        let first_bar = start_tick / ticks_per_bar;
        let last_bar = end_tick / ticks_per_bar + 1;
        for bar in first_bar..=last_bar {
            let bar_tick = bar * ticks_per_bar;
            let x = label_w + (bar_tick as f32 - start_tick as f32) * tw;
            if x >= label_w && x <= screen_w {
                renderer.fill_rect(x, content_y, 1.0, content_h, GRID_BAR);
            }
        }

        // Beat lines.
        let tpb = self.transport.ticks_per_beat as u64;
        let first_beat = start_tick / tpb;
        let last_beat = end_tick / tpb + 1;
        for beat in first_beat..=last_beat {
            let beat_tick = beat * tpb;
            if beat_tick.is_multiple_of(ticks_per_bar) {
                continue; // Already drawn as bar line.
            }
            let x = label_w + (beat_tick as f32 - start_tick as f32) * tw;
            if x >= label_w && x <= screen_w {
                renderer.fill_rect(x, content_y, 1.0, content_h, GRID_BEAT);
            }
        }

        // Playhead.
        let playhead_x = label_w + (self.transport.position_ticks as f32 - start_tick as f32) * tw;
        if playhead_x >= label_w && playhead_x <= screen_w {
            renderer.fill_rect(playhead_x, content_y, 2.0, content_h, PLAYHEAD);
        }
    }

    /// Handle click on the overview page. Clicking a track lane selects that channel.
    fn handle_overview_click(&mut self, _mx: f32, my: f32, content_y: f32) {
        let content_h = WINDOW_HEIGHT as f32 - content_y;
        let lane_h = content_h / 8.0;
        let relative_y = my - content_y;
        if relative_y < 0.0 {
            return;
        }
        let ch = (relative_y / lane_h) as usize;
        if ch < 8 {
            self.selected_channel = ch;
        }
    }

    fn render_velocity_lane<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        lane_y: f32,
        lane_h: f32,
    ) {
        let roll_w = screen_w - PIANO_KEY_WIDTH;

        // Lane background.
        renderer.fill_rect(0.0, lane_y, PIANO_KEY_WIDTH, lane_h, Color::rgb(35, 35, 40));
        renderer.fill_rect(PIANO_KEY_WIDTH, lane_y, roll_w, lane_h, Color::rgb(40, 40, 46));

        // Separator line at top of velocity lane.
        renderer.draw_hline(0.0, screen_w, lane_y, GRID_BAR);

        // Label in the key area.
        renderer.draw_text("Vel", 4.0, lane_y + 4.0, FontSize::Small, TEXT_SECONDARY);

        // Guideline at 50% and 100% velocity.
        let mid_y = lane_y + lane_h * 0.5;
        renderer.draw_hline(PIANO_KEY_WIDTH, screen_w, mid_y, GRID_LINE);
        renderer.draw_hline(PIANO_KEY_WIDTH, screen_w, lane_y + 1.0, GRID_LINE);

        // Draw velocity bars for each note.
        let tw = self.tick_width();
        let track = &self.tracks[self.selected_channel];
        let ch_color = CHANNEL_COLORS[self.selected_channel];
        let bar_margin = 1.0_f32;

        for (i, note) in track.notes.iter().enumerate() {
            let note_x = PIANO_KEY_WIDTH + note.tick as f32 * tw - self.scroll_x;
            let note_w = (note.duration as f32 * tw).max(3.0);

            // Skip notes outside visible range.
            if note_x + note_w < PIANO_KEY_WIDTH || note_x > screen_w {
                continue;
            }

            let vel_frac = note.velocity as f32 / 127.0;
            let bar_h = vel_frac * (lane_h - 2.0 * bar_margin);
            let bar_y = lane_y + lane_h - bar_margin - bar_h;

            let is_selected = self.is_note_selected(i);
            let fill = if is_selected {
                NOTE_SELECTED
            } else {
                Color::rgba(
                    (ch_color.r as f32 * 0.8) as u8,
                    (ch_color.g as f32 * 0.8) as u8,
                    (ch_color.b as f32 * 0.8) as u8,
                    200,
                )
            };

            renderer.fill_rect(note_x, bar_y, note_w, bar_h, fill);
        }

        // Playhead in velocity lane.
        if self.transport.playing || self.transport.position_ticks > 0 {
            let playhead_x =
                PIANO_KEY_WIDTH + self.transport.position_ticks as f32 * tw - self.scroll_x;
            if playhead_x >= PIANO_KEY_WIDTH && playhead_x <= screen_w {
                renderer.draw_vline(playhead_x, lane_y, lane_y + lane_h, PLAYHEAD);
            }
        }
    }

    fn render_piano_keys<R: Renderer>(&self, renderer: &mut R, roll_y: f32, roll_h: f32) {
        renderer.fill_rect(0.0, roll_y, PIANO_KEY_WIDTH, roll_h, Color::rgb(45, 45, 52));

        for i in 0..VISIBLE_NOTE_RANGE {
            let note = LOWEST_NOTE + VISIBLE_NOTE_RANGE - 1 - i;
            let y = roll_y + i as f32 * NOTE_HEIGHT - self.scroll_y;
            if y < roll_y - NOTE_HEIGHT || y > roll_y + roll_h {
                continue;
            }

            let is_black = MidiTrack::is_black_key(note);
            let key_color = if is_black { KEY_BLACK } else { KEY_WHITE };
            let key_w = if is_black { PIANO_KEY_WIDTH * 0.65 } else { PIANO_KEY_WIDTH };

            renderer.fill_rect(0.0, y, key_w, NOTE_HEIGHT - 1.0, key_color);

            // Label C notes (every octave).
            if note.is_multiple_of(12) {
                let (name, octave) = MidiTrack::note_name(note);
                let label = format!("{name}{octave}");
                renderer.draw_text(&label, 4.0, y + 1.0, FontSize::Small, KEY_LABEL);
            }
        }
    }

    fn note_to_y(&self, pitch: u8, roll_y: f32) -> f32 {
        let relative = (LOWEST_NOTE + VISIBLE_NOTE_RANGE - 1).saturating_sub(pitch);
        roll_y + relative as f32 * NOTE_HEIGHT - self.scroll_y
    }

    // --- Setup page rendering ---

    /// Render the navigation bar for setup pages.
    fn render_setup_nav<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        content_y: f32,
    ) {
        let nav_y = content_y;
        renderer.fill_rect(0.0, nav_y, screen_w, NAV_BAR_HEIGHT, BG_PANEL);

        // Left arrow button.
        let left_page = self.current_page.setup_left();
        let left_label = format!("<< {}", left_page.name());
        renderer.fill_rect(10.0, nav_y + 3.0, 180.0, 24.0, colors::NAV_BUTTON);
        renderer.draw_rect(10.0, nav_y + 3.0, 180.0, 24.0, colors::PANEL_BORDER);
        renderer.draw_text(&left_label, 20.0, nav_y + 6.0, FontSize::Small, TEXT_PRIMARY);

        // Page title (centered).
        let title = self.current_page.name();
        renderer.draw_text(
            title,
            screen_w / 2.0 - 50.0,
            nav_y + 5.0,
            FontSize::Medium,
            TEXT_ACCENT,
        );

        // Right arrow button.
        let right_page = self.current_page.setup_right();
        let right_label = format!("{} >>", right_page.name());
        let rx = screen_w - 190.0;
        renderer.fill_rect(rx, nav_y + 3.0, 180.0, 24.0, colors::NAV_BUTTON);
        renderer.draw_rect(rx, nav_y + 3.0, 180.0, 24.0, colors::PANEL_BORDER);
        renderer.draw_text(&right_label, rx + 10.0, nav_y + 6.0, FontSize::Small, TEXT_PRIMARY);
    }

    /// Render the synth setup page: skeuomorphic panel with parameter displays.
    fn render_synth_setup<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        content_y: f32,
        content_h: f32,
    ) {
        let patch = &self.patches[self.selected_channel];
        let accent = CHANNEL_COLORS[self.selected_channel];

        // Background.
        renderer.fill_rect(0.0, content_y, screen_w, content_h, colors::SECTION_BG);

        // Navigation bar.
        self.render_setup_nav(renderer, screen_w, content_y);

        let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;
        let col_w = (screen_w - 2.0 * SETUP_MARGIN - 2.0 * SETUP_COL_GAP) / 3.0;
        let x1 = SETUP_MARGIN;
        let x2 = SETUP_MARGIN + col_w + SETUP_COL_GAP;
        let x3 = SETUP_MARGIN + 2.0 * (col_w + SETUP_COL_GAP);

        // --- Row 1: VCO1, VCO2, Mix/LFO ---
        let row1_h = SECTION_HEADER_H + 5.0 * PARAM_ROW_H + 8.0;

        // VCO 1.
        draw_section_panel(renderer, x1, y0, col_w, row1_h, "VCO 1", accent);
        let mut py = y0 + SECTION_HEADER_H;
        draw_param_label(renderer, x1, py, col_w, "Waveform", &patch.vco1_waveform);
        py += PARAM_ROW_H;
        draw_param_label(renderer, x1, py, col_w, "Scale", &patch.vco1_scale);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x1, py, col_w, "Pitch Fine", patch.vco1_pitch_fine, -1.0, 1.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x1, py, col_w, "Level", patch.vco1_level, 0.0, 1.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x1, py, col_w, "Pulse Width", patch.vco1_pulse_width, 0.0, 1.0, accent);

        // VCO 2.
        draw_section_panel(renderer, x2, y0, col_w, row1_h, "VCO 2", accent);
        py = y0 + SECTION_HEADER_H;
        draw_param_label(renderer, x2, py, col_w, "Waveform", &patch.vco2_waveform);
        py += PARAM_ROW_H;
        draw_param_label(renderer, x2, py, col_w, "Scale", &patch.vco2_scale);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x2, py, col_w, "Pitch Fine", patch.vco2_pitch_fine, -1.0, 1.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x2, py, col_w, "Level", patch.vco2_level, 0.0, 1.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x2, py, col_w, "Pulse Width", patch.vco2_pulse_width, 0.0, 1.0, accent);

        // Mix / LFO.
        let mix_h = SECTION_HEADER_H + 2.0 * PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x3, y0, col_w, mix_h, "MIX / LFO", accent);
        py = y0 + SECTION_HEADER_H;
        draw_param_bar(renderer, x3, py, col_w, "VCO Mix", patch.vco_mix, 0.0, 1.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x3, py, col_w, "LFO Rate", patch.lfo_rate, 0.1, 20.0, accent);

        // --- Row 2: EG1, EG2, Filters ---
        let row2_y = y0 + row1_h + SETUP_ROW_GAP;
        let row2_h = SECTION_HEADER_H + 4.0 * PARAM_ROW_H + 8.0;

        // EG 1 (HPF).
        draw_section_panel(renderer, x1, row2_y, col_w, row2_h, "EG 1 (HPF)", accent);
        py = row2_y + SECTION_HEADER_H;
        draw_param_bar(renderer, x1, py, col_w, "Attack", patch.eg1_attack, 0.0, 5.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x1, py, col_w, "Decay", patch.eg1_decay, 0.0, 5.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x1, py, col_w, "Sustain", patch.eg1_sustain, 0.0, 1.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x1, py, col_w, "Release", patch.eg1_release, 0.0, 5.0, accent);

        // EG 2 (VCA/LPF).
        draw_section_panel(renderer, x2, row2_y, col_w, row2_h, "EG 2 (VCA/LPF)", accent);
        py = row2_y + SECTION_HEADER_H;
        draw_param_bar(renderer, x2, py, col_w, "Attack", patch.eg2_attack, 0.0, 5.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x2, py, col_w, "Decay", patch.eg2_decay, 0.0, 5.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x2, py, col_w, "Sustain", patch.eg2_sustain, 0.0, 1.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x2, py, col_w, "Release", patch.eg2_release, 0.0, 5.0, accent);

        // Filters (HPF + LPF stacked in column 3).
        let filter_h = SECTION_HEADER_H + 2.0 * PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x3, row2_y, col_w, filter_h, "HPF", accent);
        py = row2_y + SECTION_HEADER_H;
        draw_param_bar(renderer, x3, py, col_w, "Cutoff", patch.hpf_cutoff, 20.0, 20000.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x3, py, col_w, "Resonance", patch.hpf_resonance, 0.0, 4.0, accent);

        let lpf_y = row2_y + filter_h + SETUP_ROW_GAP;
        draw_section_panel(renderer, x3, lpf_y, col_w, filter_h, "LPF", accent);
        py = lpf_y + SECTION_HEADER_H;
        draw_param_bar(renderer, x3, py, col_w, "Cutoff", patch.lpf_cutoff, 20.0, 20000.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x3, py, col_w, "Resonance", patch.lpf_resonance, 0.0, 4.0, accent);

        // --- Row 3: Voice, Velocity, Glide ---
        let row3_y = row2_y + row2_h + SETUP_ROW_GAP;

        // Voice mode.
        let voice_h = SECTION_HEADER_H + PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x1, row3_y, col_w, voice_h, "VOICE", accent);
        py = row3_y + SECTION_HEADER_H;
        draw_param_label(renderer, x1, py, col_w, "Mode", &patch.voice_mode);

        // Velocity.
        let vel_h = SECTION_HEADER_H + 4.0 * PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x2, row3_y, col_w, vel_h, "VELOCITY", accent);
        py = row3_y + SECTION_HEADER_H;
        draw_param_bar(renderer, x2, py, col_w, "VCA Depth", patch.velocity_vca_depth, 0.0, 1.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x2, py, col_w, "Filter Depth", patch.velocity_filter_depth, 0.0, 4.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x2, py, col_w, "Env Depth", patch.velocity_envelope_depth, 0.0, 1.0, accent);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x2, py, col_w, "Pitch Depth", patch.velocity_pitch_depth, 0.0, 12.0, accent);

        // Glide.
        let glide_h = SECTION_HEADER_H + 2.0 * PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x3, row3_y, col_w, glide_h, "GLIDE", accent);
        py = row3_y + SECTION_HEADER_H;
        let glide_status = if patch.glide_enabled { "ON" } else { "OFF" };
        draw_param_label(renderer, x3, py, col_w, "Enabled", glide_status);
        py += PARAM_ROW_H;
        draw_param_bar(renderer, x3, py, col_w, "Time", patch.glide_time, 0.0, 2.0, accent);

        // --- Row 4: Mod Matrix (full width) ---
        let row4_y = row3_y + vel_h + SETUP_ROW_GAP;
        let mod_count = patch.mod_routings.len();
        let mod_h = SECTION_HEADER_H + (mod_count.max(1) as f32) * PARAM_ROW_H + 8.0;
        let full_w = screen_w - 2.0 * SETUP_MARGIN;
        draw_section_panel(renderer, x1, row4_y, full_w, mod_h, "MOD MATRIX", accent);
        py = row4_y + SECTION_HEADER_H;
        if patch.mod_routings.is_empty() {
            renderer.draw_text(
                "No modulation routings",
                x1 + 8.0,
                py,
                FontSize::Small,
                TEXT_SECONDARY,
            );
        } else {
            for routing in &patch.mod_routings {
                let desc = format!(
                    "{} -> {} ({:.2})",
                    routing.source, routing.dest, routing.depth
                );
                renderer.draw_text(&desc, x1 + 8.0, py, FontSize::Small, TEXT_PRIMARY);
                py += PARAM_ROW_H;
            }
        }
    }

    /// Render the mono effect chain setup page.
    fn render_mono_chain<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        content_y: f32,
        content_h: f32,
    ) {
        let chain = &self.effects[self.selected_channel];
        let accent = CHANNEL_COLORS[self.selected_channel];

        renderer.fill_rect(0.0, content_y, screen_w, content_h, colors::SECTION_BG);
        self.render_setup_nav(renderer, screen_w, content_y);

        let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;

        // Chain title.
        let title = format!("Pre-Split Mono Chain  [{}]", chain.name);
        renderer.draw_text(&title, SETUP_MARGIN + 8.0, y0, FontSize::Medium, TEXT_PRIMARY);

        // Signal flow indicator.
        renderer.draw_text(
            "Signal: Synth -> [Mono Chain] -> Positional Stage",
            SETUP_MARGIN + 8.0,
            y0 + 22.0,
            FontSize::Small,
            TEXT_SECONDARY,
        );

        let pedals_y = y0 + 48.0;

        if chain.pre_pedals.is_empty() {
            renderer.draw_text(
                "No pedals in chain. Press A to add a pedal.",
                SETUP_MARGIN + 20.0,
                pedals_y + 20.0,
                FontSize::Medium,
                TEXT_SECONDARY,
            );
        } else {
            let sel = self.selected_pedal
                .and_then(|(c, idx)| if c == 0 { Some(idx) } else { None });
            draw_pedal_chain(renderer, SETUP_MARGIN, pedals_y, &chain.pre_pedals, accent, sel);
        }

        // Controls hint.
        renderer.draw_text(
            "A=Add  Del=Remove  Shift+Arrows=Reorder  Click header=Select",
            SETUP_MARGIN + 8.0,
            content_y + content_h - 24.0,
            FontSize::Small,
            TEXT_SECONDARY,
        );
    }

    /// Render the stereo effect chain setup page.
    fn render_stereo_chain<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        content_y: f32,
        content_h: f32,
    ) {
        let chain = &self.effects[self.selected_channel];
        let accent = CHANNEL_COLORS[self.selected_channel];

        renderer.fill_rect(0.0, content_y, screen_w, content_h, colors::SECTION_BG);
        self.render_setup_nav(renderer, screen_w, content_y);

        let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;
        let full_w = screen_w - 2.0 * SETUP_MARGIN;

        // Positional stage indicator.
        let pos_h = 44.0;
        draw_section_panel(
            renderer,
            SETUP_MARGIN,
            y0,
            full_w,
            pos_h,
            "POSITIONAL STAGE",
            accent,
        );
        renderer.draw_text(
            "Mono signal splits to stereo here (Pan / 3D Position)",
            SETUP_MARGIN + 8.0,
            y0 + SECTION_HEADER_H,
            FontSize::Small,
            TEXT_SECONDARY,
        );

        let chains_y = y0 + pos_h + SETUP_ROW_GAP;
        let half_w = (screen_w - 3.0 * SETUP_MARGIN) / 2.0;

        // Left channel chain.
        let left_x = SETUP_MARGIN;
        let left_header_h = 28.0;
        renderer.fill_rect(left_x, chains_y, half_w, left_header_h, colors::PANEL_BG);
        renderer.fill_rect(left_x, chains_y, half_w, 2.0, accent);
        renderer.draw_text(
            "LEFT CHANNEL",
            left_x + 8.0,
            chains_y + 6.0,
            FontSize::Medium,
            TEXT_PRIMARY,
        );
        let left_y = chains_y + left_header_h + 4.0;
        if chain.post_pedals_left.is_empty() {
            renderer.draw_text(
                "(empty chain - press A)",
                left_x + 20.0,
                left_y + 8.0,
                FontSize::Small,
                TEXT_SECONDARY,
            );
        } else {
            let sel = self.selected_pedal
                .and_then(|(c, idx)| if c == 1 { Some(idx) } else { None });
            draw_pedal_chain(renderer, left_x, left_y, &chain.post_pedals_left, accent, sel);
        }

        // Right channel chain.
        let right_x = SETUP_MARGIN + half_w + SETUP_MARGIN;
        renderer.fill_rect(right_x, chains_y, half_w, left_header_h, colors::PANEL_BG);
        renderer.fill_rect(right_x, chains_y, half_w, 2.0, accent);
        renderer.draw_text(
            "RIGHT CHANNEL",
            right_x + 8.0,
            chains_y + 6.0,
            FontSize::Medium,
            TEXT_PRIMARY,
        );
        let right_y = chains_y + left_header_h + 4.0;
        if chain.post_pedals_right.is_empty() {
            renderer.draw_text(
                "(empty chain - press A)",
                right_x + 20.0,
                right_y + 8.0,
                FontSize::Small,
                TEXT_SECONDARY,
            );
        } else {
            let sel = self.selected_pedal
                .and_then(|(c, idx)| if c == 2 { Some(idx) } else { None });
            draw_pedal_chain(renderer, right_x, right_y, &chain.post_pedals_right, accent, sel);
        }

        // Controls hint.
        renderer.draw_text(
            "A=Add  Del=Remove  Shift+Arrows=Reorder  Click header=Select",
            SETUP_MARGIN + 8.0,
            content_y + content_h - 24.0,
            FontSize::Small,
            TEXT_SECONDARY,
        );
    }
}

// --- Setup page rendering helpers (standalone functions) ---

/// Draw a section panel with a colored accent bar and title.
fn draw_section_panel<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    title: &str,
    accent: Color,
) {
    renderer.fill_rect(x, y, w, h, colors::PANEL_BG);
    renderer.fill_rect(x, y, w, 2.0, accent);
    renderer.draw_rect(x, y, w, h, colors::PANEL_BORDER);
    renderer.draw_text(title, x + 8.0, y + 5.0, FontSize::Small, TEXT_PRIMARY);
}

/// Check whether a parameter label represents a frequency (for log mapping).
/// Return a human-readable label for a grid subdivision.
fn grid_label(snap_ticks: u64, ticks_per_beat: u64, ticks_per_bar: u64) -> &'static str {
    if snap_ticks == ticks_per_bar {
        "1/1 (Bar)"
    } else if snap_ticks == ticks_per_beat * 2 {
        "1/2 (Half)"
    } else if snap_ticks == ticks_per_beat {
        "1/4 (Quarter)"
    } else if snap_ticks == ticks_per_beat / 2 {
        "1/8 (Eighth)"
    } else if snap_ticks == ticks_per_beat / 4 {
        "1/16 (Sixteenth)"
    } else if snap_ticks == ticks_per_beat / 8 {
        "1/32 (Thirty-second)"
    } else {
        "Custom"
    }
}

fn is_log_label(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower == "cutoff"
        || lower.ends_with("_freq")
        || lower.ends_with("_hz")
        || lower == "frequency_hz"
        || lower == "min_freq"
        || lower == "max_freq"
        || lower == "low_freq"
        || lower == "mid_freq"
        || lower == "high_freq"
}

/// Draw a parameter with a label, numeric value, and horizontal bar indicator.
#[allow(clippy::too_many_arguments)]
fn draw_param_bar<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    w: f32,
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    accent: Color,
) {
    let log_scale = is_log_label(label) && min > 0.0;
    let display = if log_scale {
        format!("{value:.0} Hz")
    } else {
        format!("{value:.2}")
    };
    renderer.draw_text(label, x + 8.0, y, FontSize::Small, TEXT_SECONDARY);
    renderer.draw_text(&display, x + w - 64.0, y, FontSize::Small, TEXT_PRIMARY);
    let bar_y = y + 16.0;
    let bar_w = w - 16.0;
    renderer.fill_rect(x + 8.0, bar_y, bar_w, 5.0, colors::KNOB_TRACK);
    let frac = param_fraction(value, min, max, log_scale);
    if frac > 0.0 {
        renderer.fill_rect(x + 8.0, bar_y, bar_w * frac, 5.0, accent);
    }
}

/// Draw a parameter with a label and a text value in a display box.
fn draw_param_label<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    w: f32,
    label: &str,
    value: &str,
) {
    renderer.draw_text(label, x + 8.0, y, FontSize::Small, TEXT_SECONDARY);
    let box_x = x + w * 0.55;
    let box_w = w * 0.4;
    renderer.fill_rect(box_x, y, box_w, 18.0, Color::rgb(28, 28, 34));
    renderer.draw_rect(box_x, y, box_w, 18.0, Color::rgb(55, 55, 65));
    renderer.draw_text(value, box_x + 6.0, y + 1.0, FontSize::Small, TEXT_ACCENT);
}

/// Draw a horizontal chain of pedal boxes with connection lines.
///
/// `selected_idx` is the index of the selected pedal within this chain, if any.
fn draw_pedal_chain<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    pedals: &[PedalConfig],
    accent: Color,
    selected_idx: Option<usize>,
) {
    let pedal_w = 180.0;
    let pedal_gap = 16.0;
    let mut px = x;

    for (i, pedal) in pedals.iter().enumerate() {
        let param_count = pedal.params.len().max(1);
        let pedal_h = 28.0 + (param_count as f32) * PARAM_ROW_H + 8.0;
        let is_selected = selected_idx == Some(i);
        draw_pedal_box(renderer, px, y, pedal_w, pedal_h, pedal, accent, is_selected);

        // Connection line to next pedal.
        if i < pedals.len() - 1 {
            let line_y = y + pedal_h / 2.0;
            renderer.draw_hline(px + pedal_w, px + pedal_w + pedal_gap, line_y, accent);
            // Arrow head.
            let ax = px + pedal_w + pedal_gap - 4.0;
            renderer.draw_hline(ax - 4.0, ax, line_y - 2.0, accent);
            renderer.draw_hline(ax - 4.0, ax, line_y + 2.0, accent);
        }

        px += pedal_w + pedal_gap;
    }
}

/// Draw a single pedal box with type header and parameter bars.
#[allow(clippy::too_many_arguments)]
fn draw_pedal_box<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    pedal: &PedalConfig,
    accent: Color,
    is_selected: bool,
) {
    // Pedal body (warm tone).
    renderer.fill_rect(x, y, w, h, colors::PEDAL_BG);
    // Header bar. Selected pedals get an accent-colored header.
    let header_color = if is_selected { accent } else { colors::PEDAL_HEADER };
    renderer.fill_rect(x, y, w, 24.0, header_color);
    let type_name = pedal.pedal_type.to_uppercase();
    let header_text_color = if is_selected { BG_DARK } else { TEXT_PRIMARY };
    renderer.draw_text(&type_name, x + 6.0, y + 4.0, FontSize::Small, header_text_color);
    // Border. Selected pedals get a thicker visual indicator (double border).
    let border_color = if is_selected { TEXT_ACCENT } else { accent };
    renderer.draw_rect(x, y, w, h, border_color);
    if is_selected {
        renderer.draw_rect(x - 1.0, y - 1.0, w + 2.0, h + 2.0, border_color);
    }

    // LED indicator dot: green = active, red = bypassed.
    let led_color = if pedal.bypassed {
        Color::rgb(220, 60, 60)
    } else {
        Color::rgb(80, 220, 80)
    };
    renderer.fill_rect(x + w - 16.0, y + 8.0, 8.0, 8.0, led_color);

    // Parameters.
    let mut py = y + 28.0;
    for param in &pedal.params {
        let (min, max) = pedal_param_range(&param.name);
        draw_param_bar(renderer, x, py, w, &param.name, param.value, min, max, accent);
        py += PARAM_ROW_H;
    }
}

/// Map pedal parameter names to reasonable display ranges.
/// Find a pedal parameter bar at the given mouse position within a single chain.
#[allow(clippy::too_many_arguments)]
fn find_pedal_param_in_chain(
    mx: f32,
    my: f32,
    pedals_y: f32,
    start_x: f32,
    pedals: &[PedalConfig],
    pedal_w: f32,
    pedal_gap: f32,
    header_h: f32,
    chain_id: u8,
) -> Option<(u8, usize, usize, f64, f64)> {
    let mut px = start_x;
    for (pedal_idx, pedal) in pedals.iter().enumerate() {
        let param_start_y = pedals_y + header_h;
        for (param_idx, param) in pedal.params.iter().enumerate() {
            let py = param_start_y + param_idx as f32 * PARAM_ROW_H;
            if hit_row(mx, my, px, py, pedal_w) {
                let (min, max) = pedal_param_range(&param.name);
                return Some((chain_id, pedal_idx, param_idx, min, max));
            }
        }
        px += pedal_w + pedal_gap;
    }
    None
}

fn pedal_param_range(name: &str) -> (f64, f64) {
    match name {
        "mix" | "tone" | "depth" | "sensitivity" | "warmth" | "position"
        | "threshold" | "damping" | "decay" | "rate_factor" => (0.0, 1.0),
        "gain" => (0.0, 20.0),
        "drive" => (1.0, 5.0),
        "feedback" => (-1.0, 1.0),
        "rate_hz" => (0.1, 20.0),
        "time_ms" | "attack_ms" | "release_ms" | "depth_ms" => (0.0, 2000.0),
        "ratio" => (1.0, 20.0),
        "semitones" => (-24.0, 24.0),
        "bit_depth" => (1.0, 32.0),
        "frequency_hz" | "min_freq" | "max_freq" | "low_freq" | "mid_freq"
        | "high_freq" => (20.0, 20000.0),
        "q" => (0.1, 20.0),
        "low_gain_db" | "mid_gain_db" | "high_gain_db" => (-12.0, 12.0),
        "makeup_gain" => (0.0, 4.0),
        _ => (0.0, 1.0),
    }
}

/// Test whether a mouse click at (mx, my) falls within a parameter row.
///
/// A row occupies the rectangle `[col_x, row_y, col_x + col_w, row_y + PARAM_ROW_H]`.
fn hit_row(mx: f32, my: f32, col_x: f32, row_y: f32, col_w: f32) -> bool {
    mx >= col_x && mx < col_x + col_w && my >= row_y && my < row_y + PARAM_ROW_H
}

/// Cycle to the next waveform name in a fixed list.
fn next_waveform(current: &str) -> String {
    const WAVEFORMS: &[&str] = &["sawtooth", "triangle", "square", "sine", "noise", "ring"];
    let idx = WAVEFORMS.iter().position(|w| *w == current).unwrap_or(0);
    String::from(WAVEFORMS[(idx + 1) % WAVEFORMS.len()])
}

/// Cycle to the next organ pipe scale in a fixed list.
fn next_scale(current: &str) -> String {
    const SCALES: &[&str] = &["32'", "16'", "8'", "4'", "2'"];
    let idx = SCALES.iter().position(|s| *s == current).unwrap_or(2);
    String::from(SCALES[(idx + 1) % SCALES.len()])
}

/// Cycle to the next voice mode in a fixed list.
fn next_voice_mode(current: &str) -> String {
    const MODES: &[&str] = &["high_1", "low_1", "high_10", "low_10"];
    let idx = MODES.iter().position(|m| *m == current).unwrap_or(0);
    String::from(MODES[(idx + 1) % MODES.len()])
}

/// Check whether a synth parameter should use logarithmic display mapping.
fn is_log_synth_param(param: SynthParam) -> bool {
    matches!(param, SynthParam::HpfCutoff | SynthParam::LpfCutoff)
}

/// Check whether a pedal parameter name should use logarithmic display mapping.
fn is_log_pedal_param(name: &str) -> bool {
    matches!(
        name,
        "frequency_hz" | "min_freq" | "max_freq" | "low_freq" | "mid_freq" | "high_freq"
    )
}

/// Check whether a DragTarget uses logarithmic mapping.
fn is_log_target(target: &DragTarget) -> bool {
    match target {
        DragTarget::Synth { log_scale, .. } => *log_scale,
        DragTarget::Pedal { log_scale, .. } => *log_scale,
    }
}

/// Compute a display fraction for a parameter value within a range.
///
/// Uses logarithmic mapping when `log_scale` is true, for perceptually
/// uniform distribution of frequency parameters.
fn param_fraction(value: f64, min: f64, max: f64, log_scale: bool) -> f32 {
    if max <= min {
        return 0.0;
    }
    if log_scale && min > 0.0 {
        let log_min = min.ln();
        let log_max = max.ln();
        ((value.clamp(min, max).ln() - log_min) / (log_max - log_min)) as f32
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0) as f32
    }
}

/// Compute a parameter value from a fraction, using log mapping when appropriate.
fn value_from_fraction(frac: f64, min: f64, max: f64, log_scale: bool) -> f64 {
    if log_scale && min > 0.0 {
        let log_min = min.ln();
        let log_max = max.ln();
        (log_min + frac * (log_max - log_min)).exp()
    } else {
        min + frac * (max - min)
    }
}

/// Compute the new value after a scroll step, using log mapping for frequencies.
fn scroll_step(current: f64, min: f64, max: f64, delta: i32, log_scale: bool) -> f64 {
    if log_scale && min > 0.0 {
        // Step in log space: each tick is 2% of the log range.
        let log_min = min.ln();
        let log_max = max.ln();
        let log_current = current.clamp(min, max).ln();
        let step = (log_max - log_min) * 0.02 * delta as f64;
        (log_current + step).exp().clamp(min, max)
    } else {
        let step = (max - min) * 0.02 * delta as f64;
        (current + step).clamp(min, max)
    }
}

/// Compute a parameter value from a mouse x position within a drag target bar.
fn drag_value(mx: f32, target: &DragTarget) -> f64 {
    let (bar_x, bar_w, min_val, max_val) = match target {
        DragTarget::Synth {
            bar_x,
            bar_w,
            min_val,
            max_val,
            ..
        } => (*bar_x, *bar_w, *min_val, *max_val),
        DragTarget::Pedal {
            bar_x,
            bar_w,
            min_val,
            max_val,
            ..
        } => (*bar_x, *bar_w, *min_val, *max_val),
    };
    let frac = ((mx - bar_x) / bar_w).clamp(0.0, 1.0) as f64;
    let log_scale = is_log_target(target);
    value_from_fraction(frac, min_val, max_val, log_scale)
}

fn main() {
    // Parse command line arguments.
    let args: Vec<String> = std::env::args().collect();
    let mut project_root = PROJECT_ROOT.to_string();
    let mut load_name: Option<String> = None;
    let mut import_midi: Option<String> = None;
    let mut import_channel: usize = 0;
    let mut script_args: Vec<(usize, String)> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--root" if i + 1 < args.len() => {
                project_root = args[i + 1].clone();
                i += 2;
            }
            "--project" if i + 1 < args.len() => {
                load_name = Some(args[i + 1].clone());
                i += 2;
            }
            "--import-midi" if i + 1 < args.len() => {
                import_midi = Some(args[i + 1].clone());
                i += 2;
            }
            "--import-channel" if i + 1 < args.len() => {
                import_channel = args[i + 1].parse::<usize>().unwrap_or(0);
                i += 2;
            }
            "--script" if i + 2 < args.len() => {
                // --script <channel> <file.kel>
                let ch = args[i + 1].parse::<usize>().unwrap_or(0);
                let file = args[i + 2].clone();
                script_args.push((ch, file));
                i += 3;
            }
            _ => {
                eprintln!("[WARN] Unknown argument: {}", args[i]);
                i += 1;
            }
        }
    }

    // Initialize SDL3.
    let font_path = if std::path::Path::new(FONT_PATH).exists() {
        FONT_PATH.to_string()
    } else {
        eprintln!("[WARN] Font not found at {FONT_PATH}, trying system font");
        "/System/Library/Fonts/Helvetica.ttc".to_string()
    };

    let mut renderer = match sdl_renderer::SdlRenderer::new(
        WINDOW_TITLE,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        &font_path,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ERROR] Failed to create renderer: {e}");
            return;
        }
    };

    // Initialize audio engine.
    let audio_engine = AudioEngine::new(SAMPLE_RATE);
    let engine_arc = Arc::new(Mutex::new(audio_engine));
    let mut audio_bridge = sdl_audio::AudioBridge::new(Arc::clone(&engine_arc));

    if let Err(e) = audio_bridge.open_device(SAMPLE_RATE) {
        eprintln!("[WARN] Audio initialization failed: {e}");
    } else {
        audio_bridge.resume();
    }

    // Create DAW state, optionally loading a project.
    let mut state = DawState::new(engine_arc);
    state.project_root = PathBuf::from(&project_root);

    if let Some(name) = load_name {
        match save_load::load_project(&state.project_root, &name) {
            Ok((project, tracks, patches, effects)) => {
                state.transport.tempo = project.tempo;
                state.transport.beats_per_bar = project.beats_per_bar;
                state.project = project;
                state.tracks = tracks;
                state.patches = patches;
                state.effects = effects;
                eprintln!("[INFO] Loaded project: {name}");
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to load project {name}: {e}");
            }
        }
    }

    // Import MIDI file if requested.
    if let Some(midi_path) = import_midi {
        match std::fs::read(&midi_path) {
            Ok(midi_data) => {
                match daw_core::smf::parse_smf(&midi_data) {
                    Ok(smf_result) => {
                        eprintln!("[INFO] MIDI import: {}", daw_core::smf::describe_import(&smf_result));

                        // Import tracks into DAW channels starting at import_channel.
                        // Skip empty tracks (e.g., tempo conductor track in Type 1 files).
                        let mut ch = import_channel;
                        for smf_track in &smf_result.tracks {
                            if smf_track.notes.is_empty() || ch >= 8 {
                                continue;
                            }
                            state.tracks[ch] = smf_track.clone();
                            eprintln!(
                                "[INFO] Ch{}: imported {} notes",
                                ch + 1,
                                state.tracks[ch].notes.len()
                            );
                            ch += 1;
                        }

                        // Update ticks per beat if they differ.
                        if smf_result.ticks_per_beat != state.project.ticks_per_beat {
                            eprintln!(
                                "[INFO] Adjusting ticks per beat: {} -> {}",
                                state.project.ticks_per_beat,
                                smf_result.ticks_per_beat
                            );
                            state.project.ticks_per_beat = smf_result.ticks_per_beat;
                        }
                    }
                    Err(e) => {
                        eprintln!("[ERROR] MIDI import failed: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("[ERROR] Could not read MIDI file {midi_path}: {e}");
            }
        }
    }

    // Apply script assignments from CLI arguments.
    for (ch, file) in &script_args {
        if *ch < 8 {
            state.project.channels[*ch].script_file = file.clone();
        }
    }

    // Load Keleos scripts for channels that have them.
    state.load_scripts();

    // Sync channel configuration, patches, and effect chains to the audio engine.
    state.sync_engine_channels();
    state.apply_all_patches();
    state.apply_all_effects();
    state.transport.tempo = state.project.tempo;
    state.transport.beats_per_bar = state.project.beats_per_bar;

    println!("[INFO] DAW started: {WINDOW_WIDTH}x{WINDOW_HEIGHT} @ {SAMPLE_RATE}Hz");
    println!("[INFO] Controls: Space=Play/Pause, S=Stop, 1-8=Ch, P=Preset, Shift+P=Prev, M=Mute, Ctrl+S=Save, Q=Quit");

    // Main event loop.
    let mut event = unsafe { std::mem::zeroed::<sdl3_sys::events::SDL_Event>() };
    while state.running {
        // Poll input events.
        while unsafe { sdl3_sys::events::SDL_PollEvent(&mut event) } {
            if let Some(input) = unsafe { sdl_input::translate_event(&event) } {
                state.handle_input(input);
            }
        }

        // Advance playback (note events to audio engine).
        state.advance_playback();

        // Auto-off preview notes after timeout.
        state.check_preview_timeout();

        // Check for script file changes (hot-reload).
        state.check_script_reload();

        // Render.
        state.render(&mut renderer);

        // Frame delay (~60 FPS).
        unsafe {
            sdl3_sys::timer::SDL_Delay(16);
        }
    }

    println!("[INFO] DAW shutdown");
}
