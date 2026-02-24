mod sdl_audio;
mod sdl_input;
#[allow(dead_code)]
mod sdl_platform;
mod sdl_renderer;

use std::sync::{Arc, Mutex};

use audio::AudioEngine;
use daw_core::input::DawInput;
use daw_core::midi::MidiTrack;
use daw_core::project::DawProject;
use daw_core::render::{
    self as colors, BG_DARK, BG_PANEL, BG_TRACK, CHANNEL_COLORS, GRID_BAR, GRID_BEAT, GRID_LINE,
    KEY_BLACK, KEY_LABEL, KEY_WHITE, PLAYHEAD, TEXT_ACCENT,
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
    running: bool,
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
            running: true,
        }
    }

    fn handle_input(&mut self, input: DawInput) {
        match input {
            DawInput::Quit => self.running = false,
            DawInput::TogglePlay => {
                if self.transport.playing {
                    self.transport.stop();
                } else {
                    self.transport.play();
                }
            }
            DawInput::Stop => self.transport.reset(),
            DawInput::SelectChannel(ch) => {
                if (ch as usize) < 8 {
                    self.selected_channel = ch as usize;
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
            _ => {}
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

        // Zoom display.
        let zoom_text = format!("x{:.1}", self.zoom);
        renderer.draw_text(&zoom_text, 620.0, 8.0, FontSize::Small, TEXT_SECONDARY);

        // Key hints.
        renderer.draw_text(
            "Space=Play  S=Stop  1-8=Channel  +/-=Zoom  Arrows=Scroll  Q=Quit",
            screen_w - 620.0,
            12.0,
            FontSize::Small,
            TEXT_SECONDARY,
        );
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
        for note in &track.notes {
            let note_y = self.note_to_y(note.pitch, roll_y);
            let note_x = PIANO_KEY_WIDTH + note.tick as f32 * tick_width - self.scroll_x;
            let note_w = note.duration as f32 * tick_width;

            if note_x + note_w < PIANO_KEY_WIDTH || note_x > screen_w {
                continue;
            }
            if note_y < roll_y - NOTE_HEIGHT || note_y > roll_y + roll_h {
                continue;
            }

            renderer.fill_rect(note_x, note_y, note_w, NOTE_HEIGHT - 1.0, ch_color);
            renderer.draw_rect(note_x, note_y, note_w, NOTE_HEIGHT - 1.0, Color::rgba(255, 255, 255, 40));
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

    // Create DAW state.
    let mut state = DawState::new(engine_arc);

    println!("[INFO] DAW started: {WINDOW_WIDTH}x{WINDOW_HEIGHT} @ {SAMPLE_RATE}Hz");
    println!("[INFO] Controls: Space=Play/Pause, S=Stop, 1-8=Channel, +/-=Zoom, Q=Quit");

    // Main event loop.
    let mut event = unsafe { std::mem::zeroed::<sdl3_sys::events::SDL_Event>() };
    while state.running {
        // Poll input events.
        while unsafe { sdl3_sys::events::SDL_PollEvent(&mut event) } {
            if let Some(input) = unsafe { sdl_input::translate_event(&event) } {
                state.handle_input(input);
            }
        }

        // Render.
        state.render(&mut renderer);

        // Frame delay (~60 FPS).
        unsafe {
            sdl3_sys::timer::SDL_Delay(16);
        }
    }

    println!("[INFO] DAW shutdown");
}
