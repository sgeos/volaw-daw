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
use audio::AudioEngine;
use daw_core::input::DawInput;
use daw_core::midi::{MidiNote, MidiTrack};
use daw_core::patch::{EffectChainConfig, PatchConfig};
use daw_core::presets;
use daw_core::project::DawProject;
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
/// writes all synthesizer parameters directly to the Ms20Voice fields.
fn apply_patch_to_engine(engine: &mut AudioEngine, channel: usize, patch: &PatchConfig) {
    let synth = &mut engine.channels[channel].synth;

    // VCO1.
    synth.vco1.waveform = parse_waveform(&patch.vco1_waveform);
    synth.vco1.scale = parse_scale(&patch.vco1_scale);
    synth.vco1.pitch_fine = patch.vco1_pitch_fine;
    synth.vco1.level = patch.vco1_level;
    synth.vco1.pulse_width = patch.vco1_pulse_width;
    synth.vco1.quantization = patch.vco1_quantization;

    // VCO2.
    synth.vco2.waveform = parse_waveform(&patch.vco2_waveform);
    synth.vco2.scale = parse_scale(&patch.vco2_scale);
    synth.vco2.pitch_fine = patch.vco2_pitch_fine;
    synth.vco2.level = patch.vco2_level;
    synth.vco2.pulse_width = patch.vco2_pulse_width;
    synth.vco2.quantization = patch.vco2_quantization;

    // VCO mix.
    synth.vco_mix = patch.vco_mix;

    // EG1 (HPF envelope).
    synth.eg1.attack = patch.eg1_attack;
    synth.eg1.decay = patch.eg1_decay;
    synth.eg1.sustain = patch.eg1_sustain;
    synth.eg1.release = patch.eg1_release;

    // EG2 (LPF and VCA envelope).
    synth.eg2.attack = patch.eg2_attack;
    synth.eg2.decay = patch.eg2_decay;
    synth.eg2.sustain = patch.eg2_sustain;
    synth.eg2.release = patch.eg2_release;

    // Filters.
    synth.hpf.cutoff = patch.hpf_cutoff;
    synth.hpf.resonance = patch.hpf_resonance;
    synth.lpf.cutoff = patch.lpf_cutoff;
    synth.lpf.resonance = patch.lpf_resonance;

    // LFO.
    synth.lfo_rate = patch.lfo_rate;

    // Pitch glide.
    synth.glide.enabled = patch.glide_enabled;
    synth.glide.time = patch.glide_time;

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
    /// Index of the currently selected note in the active channel, if any.
    selected_note: Option<usize>,
    /// Snap quantization in ticks. Notes snap to this grid. Default is one beat (480).
    snap_ticks: u64,
    /// Project root directory for save/load.
    project_root: PathBuf,
    /// Status message displayed temporarily.
    status_message: Option<String>,
    /// Per-channel: index of the currently playing note, if any.
    active_notes: [Option<usize>; 8],
    /// Previous transport tick position (for detecting tick advancement).
    prev_ticks: u64,
    /// Per-channel preset index into `presets::preset_names()`. None means
    /// the channel uses a custom (non-preset) patch.
    preset_indices: [Option<usize>; 8],
}

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
            selected_note: None,
            snap_ticks: DEFAULT_NOTE_DURATION,
            project_root: PathBuf::from(PROJECT_ROOT),
            status_message: None,
            active_notes: [None; 8],
            prev_ticks: 0,
            preset_indices: [None; 8],
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

    /// Advance playback by one frame. Scans all channels for note start/end
    /// events in the tick range since the last frame. Sends note_on/note_off
    /// to the audio engine via the shared Arc<Mutex>.
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

        // Lock the engine to send note events.
        let Ok(mut engine) = self.engine.try_lock() else {
            return;
        };

        for ch in 0..8 {
            if self.project.channels[ch].muted {
                continue;
            }

            let track = &self.tracks[ch];

            // Check if the currently active note should end.
            if let Some(note_idx) = self.active_notes[ch] {
                if note_idx < track.notes.len() {
                    let note = &track.notes[note_idx];
                    let note_end = note.tick + note.duration;
                    if current_ticks >= note_end {
                        engine.channels[ch].synth.note_off();
                        self.active_notes[ch] = None;
                    }
                } else {
                    self.active_notes[ch] = None;
                }
            }

            // Check if a new note should start in the tick range [prev_ticks, current_ticks).
            for (i, note) in track.notes.iter().enumerate() {
                if note.tick >= prev_ticks && note.tick < current_ticks {
                    let freq = MidiTrack::note_to_freq(note.pitch);
                    let vel = note.velocity as f64 / 127.0;
                    engine.channels[ch].synth.note_on_with_velocity(freq, vel);
                    engine.channels[ch].enabled = true;
                    self.active_notes[ch] = Some(i);
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
            if self.active_notes[ch].is_some() {
                engine.channels[ch].synth.note_off();
                self.active_notes[ch] = None;
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
            DawInput::ZoomIn => {
                self.zoom = (self.zoom * 1.2).min(8.0);
            }
            DawInput::ZoomOut => {
                self.zoom = (self.zoom / 1.2).max(0.25);
            }
            DawInput::Left => self.scroll_x = (self.scroll_x - 20.0).max(0.0),
            DawInput::Right => self.scroll_x += 20.0,
            DawInput::Up => self.scroll_y = (self.scroll_y - 20.0).max(0.0),
            DawInput::Down => self.scroll_y += 20.0,
            DawInput::Scroll(delta) => {
                self.scroll_y = (self.scroll_y - delta as f32 * 20.0).max(0.0);
            }
            DawInput::Click(mx, my) => self.handle_click(mx, my),
            DawInput::Delete => self.delete_selected_note(),
            DawInput::Save => self.save(),
            DawInput::Cancel => {
                self.selected_note = None;
            }
            DawInput::NextPreset => self.cycle_preset(1),
            DawInput::PrevPreset => self.cycle_preset(-1),
            DawInput::ToggleMute => {
                let ch = self.selected_channel;
                self.project.channels[ch].muted = !self.project.channels[ch].muted;
                self.sync_engine_channels();
            }
            _ => {}
        }
    }

    /// Handle a mouse click. If over the grid area, either select an existing
    /// note or place a new one. If over the channel bar, select a channel.
    fn handle_click(&mut self, mx: f32, my: f32) {
        let roll_y = TOOLBAR_HEIGHT + CHANNEL_BAR_HEIGHT;

        // Channel bar click.
        if my >= TOOLBAR_HEIGHT && my < roll_y {
            let tab_w = WINDOW_WIDTH as f32 / 8.0;
            let ch = (mx / tab_w) as usize;
            if ch < 8 {
                self.selected_channel = ch;
                self.selected_note = None;
            }
            return;
        }

        // Piano roll grid click (to the right of the piano keys).
        if my >= roll_y && mx >= PIANO_KEY_WIDTH {
            let tick = self.x_to_tick(mx);
            let pitch = self.y_to_pitch(my, roll_y);

            // Check if clicking on an existing note.
            let tw = self.tick_width();
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
                    return;
                }
            }

            // No existing note hit. Place a new note.
            if (LOWEST_NOTE..LOWEST_NOTE + VISIBLE_NOTE_RANGE).contains(&pitch) {
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

    /// Delete the currently selected note, if any.
    fn delete_selected_note(&mut self) {
        if let Some(idx) = self.selected_note {
            let track = &mut self.tracks[self.selected_channel];
            if idx < track.notes.len() {
                track.notes.remove(idx);
            }
            self.selected_note = None;
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

        // Piano roll area.
        let roll_y = TOOLBAR_HEIGHT + CHANNEL_BAR_HEIGHT;
        let roll_h = screen_h - roll_y;
        self.render_piano_roll(renderer, screen_w, roll_y, roll_h);

        renderer.present();
    }

    fn render_toolbar<R: Renderer>(&self, renderer: &mut R, screen_w: f32) {
        renderer.fill_rect(0.0, 0.0, screen_w, TOOLBAR_HEIGHT, BG_PANEL);

        // Project name.
        renderer.draw_text(&self.project.name, 10.0, 8.0, FontSize::Medium, TEXT_PRIMARY);

        // Transport controls.
        let play_text = if self.transport.playing { "PLAYING" } else { "STOPPED" };
        let play_color = if self.transport.playing { colors::NOTE_BLOCK } else { TEXT_SECONDARY };
        renderer.draw_text(play_text, 250.0, 8.0, FontSize::Medium, play_color);

        // Position display (bar:beat:tick).
        let (bar, beat, tick) = self.transport.position_bbt();
        let pos_text = format!("{bar}:{beat}:{tick:03}");
        renderer.draw_text(&pos_text, 380.0, 8.0, FontSize::Medium, TEXT_ACCENT);

        // Tempo display.
        let tempo_text = format!("{:.0} BPM", self.project.tempo);
        renderer.draw_text(&tempo_text, 500.0, 8.0, FontSize::Medium, TEXT_PRIMARY);

        // Zoom and note count display.
        let note_count = self.tracks[self.selected_channel].notes.len();
        let info_text = format!("x{:.1}  Notes: {note_count}", self.zoom);
        renderer.draw_text(&info_text, 620.0, 8.0, FontSize::Small, TEXT_SECONDARY);

        // Status message or key hints.
        if let Some(ref msg) = self.status_message {
            renderer.draw_text(msg, screen_w - 620.0, 12.0, FontSize::Small, TEXT_ACCENT);
        } else {
            renderer.draw_text(
                "Space=Play  S=Stop  1-8=Ch  P=Preset  M=Mute  Click=Note  Del=Remove  Ctrl+S=Save",
                screen_w - 720.0,
                12.0,
                FontSize::Small,
                TEXT_SECONDARY,
            );
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
            let fill_color = if is_selected { NOTE_SELECTED } else { ch_color };
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
}

fn main() {
    // Parse command line arguments.
    let args: Vec<String> = std::env::args().collect();
    let mut project_root = PROJECT_ROOT.to_string();
    let mut load_name: Option<String> = None;
    let mut import_midi: Option<String> = None;
    let mut import_channel: usize = 0;
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
                            // Apply monophonic autofix if the imported track has overlaps.
                            if !state.tracks[ch].is_monophonic() {
                                eprintln!("[INFO] Ch{}: autofix monophonic (latest-note strategy)", ch + 1);
                                state.tracks[ch].autofix(daw_core::midi::AutofixStrategy::LatestNote);
                            }
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
