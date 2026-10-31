//! Drawing of `DawApp` through the `Renderer` trait. This module draws the
//! frame shared by every page and dispatches to each page's module for the
//! content below it.

use super::*;

impl<P: Platform, S: CommandSink> DawApp<P, S> {
    pub fn render<R: Renderer>(&self, renderer: &mut R) {
        renderer.clear(BG_DARK);

        // Layout follows the app's window size rather than the renderer's,
        // because hit testing has no renderer and must use the same figures.
        // The entry point opens the window at this size.
        let screen_w = self.window.width_f32();
        let screen_h = self.window.height_f32();

        // Toolbar area.
        self.render_toolbar(renderer, screen_w);

        // Page tabs and transport buttons.
        self.render_page_tabs(renderer, screen_w);

        // Channel selector bar.
        self.render_channel_bar(renderer, screen_w);

        // Content area.
        let content_y = CONTENT_Y;
        let content_h = screen_h - content_y;
        match self.current_page {
            DawPage::Studio => {
                self.render_studio(renderer, screen_w, content_y, content_h);
            }
            DawPage::PianoRoll => {
                let ruler_y = content_y;
                let roll_y = content_y + RULER_HEIGHT;
                let roll_h = content_h - VELOCITY_LANE_HEIGHT - RULER_HEIGHT;
                self.render_ruler(renderer, screen_w, ruler_y);
                self.render_piano_roll(renderer, screen_w, roll_y, roll_h);
                self.render_velocity_lane(
                    renderer,
                    screen_w,
                    roll_y + roll_h,
                    VELOCITY_LANE_HEIGHT,
                );
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

    pub(super) fn render_toolbar<R: Renderer>(&self, renderer: &mut R, screen_w: f32) {
        renderer.fill_rect(0.0, 0.0, screen_w, TOOLBAR_HEIGHT, BG_PANEL);

        // Project name (clickable, editable).
        if self.toolbar_edit == Some(ToolbarEdit::SongName) {
            renderer.fill_rect(8.0, 4.0, 184.0, 24.0, Color::rgb(50, 50, 70));
            let display = format!("{}|", self.edit_buffer);
            renderer.draw_text(&display, 10.0, 8.0, FontSize::Medium, TEXT_ACCENT);
        } else {
            renderer.draw_text(
                &self.project.name,
                10.0,
                8.0,
                FontSize::Medium,
                TEXT_PRIMARY,
            );
        }

        // Page indicator.
        renderer.draw_text(
            self.current_page.name(),
            200.0,
            8.0,
            FontSize::Medium,
            TEXT_ACCENT,
        );

        // Transport controls.
        let play_text = if self.transport.playing {
            "PLAYING"
        } else {
            "STOPPED"
        };
        let play_color = if self.transport.playing {
            colors::NOTE_BLOCK
        } else {
            TEXT_SECONDARY
        };
        renderer.draw_text(play_text, 380.0, 8.0, FontSize::Medium, play_color);

        // Position display (bar:beat:tick).
        let (bar, beat, tick) = self.transport.position_bbt();
        let pos_text = format!("{bar}:{beat}:{tick:03}");
        renderer.draw_text(&pos_text, 500.0, 8.0, FontSize::Medium, TEXT_ACCENT);

        // Tempo display (clickable, editable).
        if self.toolbar_edit == Some(ToolbarEdit::Bpm) {
            renderer.fill_rect(618.0, 4.0, 94.0, 24.0, Color::rgb(50, 50, 70));
            let display = format!("{}| BPM", self.edit_buffer);
            renderer.draw_text(&display, 620.0, 8.0, FontSize::Medium, TEXT_ACCENT);
        } else {
            let tempo_text = format!("{:.0} BPM", self.project.tempo);
            renderer.draw_text(&tempo_text, 620.0, 8.0, FontSize::Medium, TEXT_PRIMARY);
        }

        // Time signature display (clickable numerator and denominator, editable).
        if self.toolbar_edit == Some(ToolbarEdit::TimeSigNum) {
            renderer.fill_rect(718.0, 4.0, 28.0, 24.0, Color::rgb(50, 50, 70));
            let display = format!("{}|", self.edit_buffer);
            renderer.draw_text(&display, 720.0, 8.0, FontSize::Medium, TEXT_ACCENT);
            let slash = format!("/{}", self.project.beat_denominator);
            renderer.draw_text(&slash, 748.0, 8.0, FontSize::Medium, TEXT_SECONDARY);
        } else if self.toolbar_edit == Some(ToolbarEdit::TimeSigDen) {
            let num_text = format!("{}/", self.project.beats_per_bar);
            renderer.draw_text(&num_text, 720.0, 8.0, FontSize::Medium, TEXT_SECONDARY);
            renderer.fill_rect(748.0, 4.0, 28.0, 24.0, Color::rgb(50, 50, 70));
            let display = format!("{}|", self.edit_buffer);
            renderer.draw_text(&display, 750.0, 8.0, FontSize::Medium, TEXT_ACCENT);
        } else {
            let ts_text = format!(
                "{}/{}",
                self.project.beats_per_bar, self.project.beat_denominator
            );
            renderer.draw_text(&ts_text, 720.0, 8.0, FontSize::Medium, TEXT_SECONDARY);
        }

        // Grid subdivision display (clickable, cycles on click).
        let grid_text = grid_label(
            self.snap_ticks,
            self.project.ticks_per_beat as u64,
            self.transport.ticks_per_bar(),
        );
        renderer.draw_text(grid_text, 780.0, 8.0, FontSize::Medium, TEXT_SECONDARY);

        // Loop indicator.
        if self.transport.loop_enabled {
            renderer.draw_text("LOOP", 870.0, 8.0, FontSize::Medium, LOOP_MARKER);
        }

        // Metronome indicator.
        if self.metronome_enabled {
            renderer.draw_text("MET", 940.0, 8.0, FontSize::Medium, TEXT_ACCENT);
        }

        // Status message, selected note info, or page-specific hints. It is
        // anchored to the right edge, but never starts left of the end of the
        // indicators, so in a narrow window a long message is cut off at the
        // edge rather than drawn over them.
        let hint_x = (screen_w - TOOLBAR_HINT_W).max(TOOLBAR_HINT_MIN_X);
        if let Some(ref msg) = self.status_message {
            renderer.draw_text(msg, hint_x, 12.0, FontSize::Small, TEXT_ACCENT);
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
                "{}{} vel:{} dur:{:.1}b{}",
                name, octave, note.velocity, beats, sel_info
            );
            renderer.draw_text(&info, hint_x, 12.0, FontSize::Small, TEXT_ACCENT);
        } else {
            let hints = if self.current_page == DawPage::PianoRoll {
                "Space=Play  G=Grid  L=Loop  Click=Note"
            } else {
                "Click=Edit  Drag=Adjust  P=Preset  Ctrl+S=Save"
            };
            renderer.draw_text(hints, hint_x, 12.0, FontSize::Small, TEXT_SECONDARY);
        }
    }

    pub(super) fn render_channel_bar<R: Renderer>(&self, renderer: &mut R, screen_w: f32) {
        let y = CHANNEL_BAR_Y;
        renderer.fill_rect(0.0, y, screen_w, CHANNEL_BAR_HEIGHT, Color::rgb(35, 35, 42));

        let tab_w = screen_w / 8.0;
        for (i, &color) in CHANNEL_COLORS.iter().enumerate() {
            let x = i as f32 * tab_w;

            if i == self.selected_channel {
                renderer.fill_rect(
                    x,
                    y,
                    tab_w,
                    CHANNEL_BAR_HEIGHT,
                    Color::rgba(color.r, color.g, color.b, 60),
                );
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
                renderer.draw_text(
                    &count_text,
                    x + tab_w - 60.0,
                    y + 7.0,
                    FontSize::Small,
                    TEXT_SECONDARY,
                );
            }

            if self.solo_channel == Some(i) {
                renderer.draw_text(
                    "S",
                    x + tab_w - 40.0,
                    y + 7.0,
                    FontSize::Small,
                    Color::rgb(255, 200, 80),
                );
            }
            if self.project.channels[i].muted {
                renderer.draw_text(
                    "M",
                    x + tab_w - 24.0,
                    y + 7.0,
                    FontSize::Small,
                    Color::rgb(255, 80, 80),
                );
            }
        }
    }

    // --- Setup page rendering ---

    /// Render the navigation bar for setup pages.
    pub(super) fn render_setup_nav<R: Renderer>(
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
        renderer.draw_text(
            &left_label,
            20.0,
            nav_y + 6.0,
            FontSize::Small,
            TEXT_PRIMARY,
        );

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
        renderer.draw_text(
            &right_label,
            rx + 10.0,
            nav_y + 6.0,
            FontSize::Small,
            TEXT_PRIMARY,
        );
    }
}
