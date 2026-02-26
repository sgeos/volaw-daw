mod save_load;
mod sdl_audio;
mod sdl_input;
#[allow(dead_code)]
mod sdl_platform;
mod sdl_renderer;

use std::sync::{Arc, Mutex};

use std::path::PathBuf;

use audio::modmatrix::{ModDest, ModRouting, ModSource};
use audio::vco::{Scale, Waveform};
use audio::{AudioEngine, VoiceMode};
use daw_core::input::DawInput;
use daw_core::midi::{MidiNote, MidiTrack};
use daw_core::patch::{EffectChainConfig, PatchConfig, PedalConfig};
use daw_core::presets;
use daw_core::project::DawProject;
use daw_core::script::{self, ScriptRunner};
use daw_core::render::{
    self as colors, BG_DARK, BG_PANEL, BG_TRACK, CHANNEL_COLORS, GRID_BAR, GRID_BEAT, GRID_LINE,
    KEY_BLACK, KEY_LABEL, KEY_WHITE, NOTE_SELECTED, PLAYHEAD, TEXT_ACCENT,
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
    },
}

/// DAW page view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DawPage {
    PianoRoll,
    SynthSetup,
    MonoChain,
    StereoChain,
}

impl DawPage {
    fn next(self) -> Self {
        match self {
            Self::PianoRoll => Self::SynthSetup,
            Self::SynthSetup => Self::MonoChain,
            Self::MonoChain => Self::StereoChain,
            Self::StereoChain => Self::PianoRoll,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::PianoRoll => Self::StereoChain,
            Self::SynthSetup => Self::PianoRoll,
            Self::MonoChain => Self::SynthSetup,
            Self::StereoChain => Self::MonoChain,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::PianoRoll => "Piano Roll",
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
            Self::PianoRoll => Self::PianoRoll,
        }
    }

    /// Navigate right from setup pages.
    fn setup_right(self) -> Self {
        match self {
            Self::SynthSetup => Self::MonoChain,
            Self::MonoChain => Self::StereoChain,
            Self::StereoChain => Self::SynthSetup,
            Self::PianoRoll => Self::PianoRoll,
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

    // Modulation matrix: clear defaults and apply patch routings.
    synth.mod_matrix.clear();
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
    scroll_x: f32,
    scroll_y: f32,
    zoom: f32,
    tracks: [MidiTrack; 8],
    patches: [PatchConfig; 8],
    effects: [EffectChainConfig; 8],
    running: bool,
    /// Current page/view being displayed.
    current_page: DawPage,
    /// Index of the currently selected note in the active channel, if any.
    selected_note: Option<usize>,
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
    /// Undo stack: each entry is (channel_index, track_snapshot_before_edit).
    undo_stack: Vec<(usize, MidiTrack)>,
    /// Per-channel Keleos script runners. None if no script assigned.
    scripts: [Option<ScriptRunner>; 8],
    /// Currently active drag target on a setup page parameter.
    dragging_param: Option<DragTarget>,
    /// True when dragging a note's right edge to resize its duration.
    dragging_note_edge: bool,
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
            scroll_x: 0.0,
            scroll_y: (VISIBLE_NOTE_RANGE as f32 / 2.0) * NOTE_HEIGHT,
            zoom: 1.0,
            tracks: core::array::from_fn(|_| MidiTrack::new()),
            patches: core::array::from_fn(|_| PatchConfig::default()),
            effects: core::array::from_fn(|_| EffectChainConfig::default()),
            running: true,
            current_page: DawPage::PianoRoll,
            selected_note: None,
            snap_ticks: DEFAULT_NOTE_DURATION,
            project_root: PathBuf::from(PROJECT_ROOT),
            status_message: None,
            active_notes: core::array::from_fn(|_| Vec::new()),
            prev_ticks: 0,
            preset_indices: [None; 8],
            undo_stack: Vec::new(),
            scripts: core::array::from_fn(|_| None),
            dragging_param: None,
            dragging_note_edge: false,
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
                continue;
            }
            let path = scripts_dir.join(filename);
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
        self.prev_ticks = current_ticks;

        // Compute beat position for scripts.
        let beat = current_ticks as f64 / self.transport.ticks_per_beat as f64;

        // Lock the engine to send note events and apply script commands.
        let Ok(mut engine) = self.engine.try_lock() else {
            return;
        };

        for ch in 0..8 {
            if self.project.channels[ch].muted {
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
            }

            let track = &self.tracks[ch];

            // Check if any active notes should end (per-note release for polyphony).
            self.active_notes[ch].retain(|&note_idx| {
                if note_idx >= track.notes.len() {
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

            // Check if new notes should start in the tick range [prev_ticks, current_ticks).
            for (i, note) in track.notes.iter().enumerate() {
                if note.tick >= prev_ticks && note.tick < current_ticks {
                    let freq = MidiTrack::note_to_freq(note.pitch);
                    let vel = note.velocity as f64 / 127.0;
                    engine.channels[ch].synth.note_on_with_velocity(freq, vel);
                    engine.channels[ch].enabled = true;
                    self.active_notes[ch].push(i);
                }
            }
        }
    }

    /// Stop all active notes across all channels.
    fn stop_all_notes(&mut self) {
        let Ok(mut engine) = self.engine.try_lock() else {
            return;
        };
        for ch in 0..8 {
            if !self.active_notes[ch].is_empty() {
                engine.channels[ch].synth.note_off();
                self.active_notes[ch].clear();
            }
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
            engine.channels[ch].enabled = !conf.muted;
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
                    self.selected_note = None;
                }
            }
            DawInput::Save => self.save(),
            DawInput::Undo => self.undo(),
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
                if self.current_page != DawPage::PianoRoll {
                    self.current_page = DawPage::PianoRoll;
                } else {
                    self.selected_note = None;
                }
            }
            DawInput::Click(mx, my) => self.handle_click(mx, my),
            DawInput::MouseUp(_, _) => {
                self.dragging_param = None;
                self.dragging_note_edge = false;
            }

            // Piano roll inputs.
            DawInput::Left => {
                if self.current_page == DawPage::PianoRoll {
                    self.scroll_x = (self.scroll_x - 20.0).max(0.0);
                } else {
                    self.current_page = self.current_page.setup_left();
                }
            }
            DawInput::Right => {
                if self.current_page == DawPage::PianoRoll {
                    self.scroll_x += 20.0;
                } else {
                    self.current_page = self.current_page.setup_right();
                }
            }
            DawInput::Up => {
                if self.current_page == DawPage::PianoRoll {
                    self.scroll_y = (self.scroll_y - 20.0).max(0.0);
                }
            }
            DawInput::Down => {
                if self.current_page == DawPage::PianoRoll {
                    self.scroll_y += 20.0;
                }
            }
            DawInput::Scroll(delta) => {
                if self.current_page == DawPage::PianoRoll {
                    // If a note is selected, scroll adjusts velocity.
                    if let Some(idx) = self.selected_note {
                        let ch = self.selected_channel;
                        if let Some(note) = self.tracks[ch].notes.get_mut(idx) {
                            let new_vel = (note.velocity as i32 + delta * 4).clamp(1, 127) as u8;
                            note.velocity = new_vel;
                            self.status_message = Some(format!("Vel: {}", new_vel));
                        }
                    } else {
                        self.scroll_y = (self.scroll_y - delta as f32 * 20.0).max(0.0);
                    }
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
                    self.delete_selected_note();
                }
            }
            DawInput::MouseMove(mx, _my) => {
                if let Some(ref target) = self.dragging_param {
                    let value = drag_value(mx, target);
                    self.apply_drag_value(target.clone(), value);
                } else if self.dragging_note_edge {
                    self.drag_note_duration(mx);
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
                self.selected_note = None;
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

        // Dispatch to page-specific click handling.
        match self.current_page {
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

        // Piano roll grid click (to the right of the piano keys).
        let roll_y = content_y;
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
                    self.selected_note = Some(i);
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
                self.selected_note = idx;
            }
        }
    }

    /// Save a snapshot of the current channel track to the undo stack.
    fn push_undo(&mut self) {
        let ch = self.selected_channel;
        if self.undo_stack.len() >= MAX_UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push((ch, self.tracks[ch].clone()));
    }

    /// Undo the last edit operation by restoring the track snapshot.
    fn undo(&mut self) {
        if let Some((ch, snapshot)) = self.undo_stack.pop() {
            self.tracks[ch] = snapshot;
            self.selected_note = None;
            self.status_message = Some(format!("Undo (Ch{})", ch + 1));
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

    /// Drag the selected note's right edge to adjust duration.
    fn drag_note_duration(&mut self, mx: f32) {
        if let Some(idx) = self.selected_note {
            let ch = self.selected_channel;
            let tw = self.tick_width();
            let sx = self.scroll_x;
            let snap = self.snap_ticks;
            if let Some(note) = self.tracks[ch].notes.get_mut(idx) {
                let note_x = PIANO_KEY_WIDTH + note.tick as f32 * tw - sx;
                let raw_end_tick = ((mx - note_x) / tw).max(0.0) as u64 + note.tick;
                let snapped = ((raw_end_tick - note.tick) / snap).max(1) * snap;
                note.duration = snapped;
            }
        }
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
        let target = DragTarget::Synth {
            param,
            bar_x,
            bar_w,
            min_val,
            max_val,
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
        let mut px = SETUP_MARGIN;

        for (pedal_idx, pedal) in pedals.iter().enumerate() {
            let header_h = 28.0;
            let param_start_y = pedals_y + header_h;

            for (param_idx, param) in pedal.params.iter().enumerate() {
                let py = param_start_y + param_idx as f32 * PARAM_ROW_H;
                if hit_row(mx, my, px, py, pedal_w) {
                    let (min, max) = pedal_param_range(&param.name);
                    let target = DragTarget::Pedal {
                        chain: chain_id,
                        pedal_idx,
                        param_idx,
                        bar_x: px + bar_inset,
                        bar_w,
                        min_val: min,
                        max_val: max,
                    };
                    let value = drag_value(mx, &target);
                    self.apply_drag_value(target.clone(), value);
                    self.dragging_param = Some(target);
                    return;
                }
            }

            px += pedal_w + pedal_gap;
        }
    }

    /// Delete the currently selected note, if any.
    fn delete_selected_note(&mut self) {
        if let Some(idx) = self.selected_note {
            if idx < self.tracks[self.selected_channel].notes.len() {
                self.push_undo();
                self.tracks[self.selected_channel].notes.remove(idx);
            }
            self.selected_note = None;
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
                self.render_piano_roll(renderer, screen_w, content_y, content_h);
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

        renderer.present();
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

        // Status message, selected note info, or page-specific hints.
        if let Some(ref msg) = self.status_message {
            renderer.draw_text(msg, screen_w - 620.0, 12.0, FontSize::Small, TEXT_ACCENT);
        } else if self.current_page == DawPage::PianoRoll
            && let Some(idx) = self.selected_note
            && let Some(note) = self.tracks[self.selected_channel].notes.get(idx)
        {
            let (name, octave) = MidiTrack::note_name(note.pitch);
            let beats = note.duration as f64 / self.project.ticks_per_beat as f64;
            let info = format!(
                "{}{} vel:{} dur:{:.1}b  Scroll=Vel  Drag edge=Dur  Del=Remove",
                name, octave, note.velocity, beats
            );
            renderer.draw_text(&info, screen_w - 720.0, 12.0, FontSize::Small, TEXT_ACCENT);
        } else {
            let hints = if self.current_page == DawPage::PianoRoll {
                "Space=Play  S=Stop  1-8=Ch  P=Preset  M=Mute  Click=Note  Del=Remove  Tab=Setup"
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

            let line_color = if note.is_multiple_of(12) {
                GRID_BAR
            } else {
                GRID_LINE
            };
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

        for tick_offset in 0..visible_ticks {
            let tick = first_tick + tick_offset;
            let x = PIANO_KEY_WIDTH + tick as f32 * tick_width - self.scroll_x;
            if x < PIANO_KEY_WIDTH || x > screen_w {
                continue;
            }

            let ticks_per_bar = self.project.ticks_per_beat as u64 * 4;
            let color = if tick.is_multiple_of(ticks_per_bar) {
                GRID_BAR
            } else if tick.is_multiple_of(self.project.ticks_per_beat as u64) {
                GRID_BEAT
            } else {
                continue; // Only draw beat and bar lines.
            };
            renderer.draw_vline(x, roll_y, roll_y + roll_h, color);

            // Bar number label.
            if tick.is_multiple_of(ticks_per_bar) {
                let bar = tick / ticks_per_bar + 1;
                let bar_label = format!("{bar}");
                renderer.draw_text(&bar_label, x + 2.0, roll_y + 2.0, FontSize::Small, TEXT_SECONDARY);
            }
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

            let is_selected = self.selected_note == Some(i);
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

        // Playhead.
        if self.transport.playing || self.transport.position_ticks > 0 {
            let playhead_x = PIANO_KEY_WIDTH + self.transport.position_ticks as f32 * tick_width - self.scroll_x;
            if playhead_x >= PIANO_KEY_WIDTH && playhead_x <= screen_w {
                renderer.draw_vline(playhead_x, roll_y, roll_y + roll_h, PLAYHEAD);
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
                "No pedals in chain. Signal passes through unprocessed.",
                SETUP_MARGIN + 20.0,
                pedals_y + 20.0,
                FontSize::Medium,
                TEXT_SECONDARY,
            );
        } else {
            draw_pedal_chain(renderer, SETUP_MARGIN, pedals_y, &chain.pre_pedals, accent);
        }
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
                "(empty chain)",
                left_x + 20.0,
                left_y + 8.0,
                FontSize::Small,
                TEXT_SECONDARY,
            );
        } else {
            draw_pedal_chain(renderer, left_x, left_y, &chain.post_pedals_left, accent);
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
                "(empty chain)",
                right_x + 20.0,
                right_y + 8.0,
                FontSize::Small,
                TEXT_SECONDARY,
            );
        } else {
            draw_pedal_chain(renderer, right_x, right_y, &chain.post_pedals_right, accent);
        }
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
    let display = format!("{value:.2}");
    renderer.draw_text(label, x + 8.0, y, FontSize::Small, TEXT_SECONDARY);
    renderer.draw_text(&display, x + w - 64.0, y, FontSize::Small, TEXT_PRIMARY);
    let bar_y = y + 16.0;
    let bar_w = w - 16.0;
    renderer.fill_rect(x + 8.0, bar_y, bar_w, 5.0, colors::KNOB_TRACK);
    let frac = param_fraction(value, min, max);
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
fn draw_pedal_chain<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    pedals: &[PedalConfig],
    accent: Color,
) {
    let pedal_w = 180.0;
    let pedal_gap = 16.0;
    let mut px = x;

    for (i, pedal) in pedals.iter().enumerate() {
        let param_count = pedal.params.len().max(1);
        let pedal_h = 28.0 + (param_count as f32) * PARAM_ROW_H + 8.0;
        draw_pedal_box(renderer, px, y, pedal_w, pedal_h, pedal, accent);

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
fn draw_pedal_box<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    pedal: &PedalConfig,
    accent: Color,
) {
    // Pedal body (warm tone).
    renderer.fill_rect(x, y, w, h, colors::PEDAL_BG);
    // Header bar.
    renderer.fill_rect(x, y, w, 24.0, colors::PEDAL_HEADER);
    let type_name = pedal.pedal_type.to_uppercase();
    renderer.draw_text(&type_name, x + 6.0, y + 4.0, FontSize::Small, TEXT_PRIMARY);
    // Border.
    renderer.draw_rect(x, y, w, h, accent);

    // LED indicator dot.
    renderer.fill_rect(x + w - 16.0, y + 8.0, 8.0, 8.0, Color::rgb(80, 220, 80));

    // Parameters.
    let mut py = y + 28.0;
    for param in &pedal.params {
        let (min, max) = pedal_param_range(&param.name);
        draw_param_bar(renderer, x, py, w, &param.name, param.value, min, max, accent);
        py += PARAM_ROW_H;
    }
}

/// Map pedal parameter names to reasonable display ranges.
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

/// Compute a linear fraction for a parameter value within a range.
fn param_fraction(value: f64, min: f64, max: f64) -> f32 {
    if max <= min {
        return 0.0;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0) as f32
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
    min_val + frac * (max_val - min_val)
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

    // Sync channel configuration and patches to the audio engine.
    state.sync_engine_channels();
    state.apply_all_patches();
    state.transport.tempo = state.project.tempo;

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

        // Render.
        state.render(&mut renderer);

        // Frame delay (~60 FPS).
        unsafe {
            sdl3_sys::timer::SDL_Delay(16);
        }
    }

    println!("[INFO] DAW shutdown");
}
