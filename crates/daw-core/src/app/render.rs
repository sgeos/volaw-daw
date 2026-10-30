//! Drawing of every `DawApp` page through the `Renderer` trait.

use super::*;

impl<P: Platform, S: CommandSink> DawApp<P, S> {
    pub fn render<R: Renderer>(&self, renderer: &mut R) {
        renderer.clear(BG_DARK);

        let screen_w = renderer.screen_width();
        let screen_h = renderer.screen_height();

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

    /// Render the pedal type picker overlay, a grid of every pedal by
    /// category with a one-line description of each.
    pub(super) fn render_pedal_picker<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        screen_h: f32,
    ) {
        renderer.fill_rect(0.0, 0.0, screen_w, screen_h, Color::rgba(0, 0, 0, 180));

        let p = picker_rect();
        renderer.fill_rect(p.x, p.y, p.w, p.h, colors::PANEL_BG);
        renderer.draw_rect(p.x, p.y, p.w, p.h, TEXT_ACCENT);

        let chain_name = match self.pedal_picker_chain {
            0 => "Mono Chain",
            1 => "Left Chain",
            2 => "Right Chain",
            _ => "Chain",
        };
        let title = format!(
            "Add an effect to Track {} {chain_name}   (click outside or Esc to cancel)",
            self.selected_channel + 1
        );
        renderer.draw_text(
            &title,
            p.x + 12.0,
            p.y + 12.0,
            FontSize::Medium,
            TEXT_ACCENT,
        );

        let accent = CHANNEL_COLORS[self.selected_channel];
        for (cat, (category, types)) in PEDAL_TYPES.iter().enumerate() {
            let c = picker_category_rect(cat);
            renderer.fill_rect(c.x, c.y, c.w, c.h, BG_DARK);
            renderer.draw_text(category, c.x + 8.0, c.y + 4.0, FontSize::Small, accent);
            for (item, pedal_type) in types.iter().enumerate() {
                let r = picker_item_rect(cat, item);
                renderer.fill_rect(r.x, r.y, r.w, r.h, colors::NAV_BUTTON);
                renderer.draw_rect(r.x, r.y, r.w, r.h, colors::PANEL_BORDER);
                renderer.draw_text(
                    &pedal_display_name(pedal_type),
                    r.x + 8.0,
                    r.y + 2.0,
                    FontSize::Small,
                    TEXT_PRIMARY,
                );
                if let Some(text) = pedal_description(pedal_type) {
                    renderer.draw_text(
                        text,
                        r.x + 8.0,
                        r.y + 18.0,
                        FontSize::Small,
                        TEXT_SECONDARY,
                    );
                }
            }
        }
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

        // Status message, selected note info, or page-specific hints.
        // Positioned after all toolbar indicators to avoid overlap.
        let hint_x = screen_w - 430.0;
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

    /// Render the ruler bar above the piano roll showing measure numbers and
    /// the loop region.
    pub(super) fn render_ruler<R: Renderer>(&self, renderer: &mut R, screen_w: f32, ruler_y: f32) {
        // Background.
        renderer.fill_rect(
            PIANO_KEY_WIDTH,
            ruler_y,
            screen_w - PIANO_KEY_WIDTH,
            RULER_HEIGHT,
            BG_PANEL,
        );

        let tick_width = self.tick_width();
        let ticks_per_bar = self.transport.ticks_per_bar();
        if ticks_per_bar == 0 {
            return;
        }

        // Loop region overlay in the ruler.
        if self.transport.loop_enabled && self.transport.loop_end > self.transport.loop_start {
            let start_x = (PIANO_KEY_WIDTH + self.transport.loop_start as f32 * tick_width
                - self.scroll_x)
                .max(PIANO_KEY_WIDTH);
            let end_x = (PIANO_KEY_WIDTH + self.transport.loop_end as f32 * tick_width
                - self.scroll_x)
                .min(screen_w);
            if end_x > start_x {
                renderer.fill_rect(
                    start_x,
                    ruler_y,
                    end_x - start_x,
                    RULER_HEIGHT,
                    Color::rgba(80, 200, 120, 50),
                );
            }
        }

        // Measure numbers.
        let first_tick = (self.scroll_x / tick_width) as u64;
        let visible_ticks = ((screen_w - PIANO_KEY_WIDTH) / tick_width) as u64 + 1;
        let first_bar_tick = (first_tick / ticks_per_bar) * ticks_per_bar;

        let mut tick = first_bar_tick;
        while tick <= first_tick + visible_ticks + ticks_per_bar {
            let x = PIANO_KEY_WIDTH + tick as f32 * tick_width - self.scroll_x;
            if x >= PIANO_KEY_WIDTH && x <= screen_w && tick.is_multiple_of(ticks_per_bar) {
                let bar = tick / ticks_per_bar + 1;
                renderer.draw_text(
                    &format!("{bar}"),
                    x + 3.0,
                    ruler_y + 4.0,
                    FontSize::Small,
                    TEXT_SECONDARY,
                );
                renderer.draw_vline(x, ruler_y, ruler_y + RULER_HEIGHT, GRID_BAR);
            }
            tick += ticks_per_bar;
        }

        // Bottom border.
        renderer.draw_hline(
            PIANO_KEY_WIDTH,
            screen_w,
            ruler_y + RULER_HEIGHT - 1.0,
            GRID_BAR,
        );
    }

    pub(super) fn render_piano_roll<R: Renderer>(
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

        // Rubber-band selection rectangle overlay.
        if self.drag_select_active
            && let Some((sx, sy)) = self.drag_select_start
        {
            let (mx, my) = self.last_mouse;
            let rx = sx.min(mx).max(PIANO_KEY_WIDTH);
            let ry = sy.min(my).max(roll_y);
            let rw = (sx.max(mx) - rx).max(0.0);
            let rh = (sy.max(my) - ry).max(0.0);
            renderer.fill_rect(rx, ry, rw, rh, Color::rgba(100, 150, 255, 40));
            renderer.draw_rect(rx, ry, rw, rh, Color::rgba(100, 150, 255, 120));
        }

        // Loop region markers.
        if self.transport.loop_enabled && self.transport.loop_end > self.transport.loop_start {
            let start_x =
                PIANO_KEY_WIDTH + self.transport.loop_start as f32 * tick_width - self.scroll_x;
            let end_x =
                PIANO_KEY_WIDTH + self.transport.loop_end as f32 * tick_width - self.scroll_x;
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
            let playhead_x =
                PIANO_KEY_WIDTH + self.transport.position_ticks as f32 * tick_width - self.scroll_x;
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
    pub(super) fn render_overview<R: Renderer>(
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
                let pitch_min = LOWEST_NOTE as f32;
                let pitch_range = VISIBLE_NOTE_RANGE as f32;
                let note_h = (lane_h - 10.0) / pitch_range;
                let ny = y + lane_h
                    - 6.0
                    - (note.pitch as f32 - pitch_min).clamp(0.0, pitch_range) * note_h;

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
    pub(super) fn handle_overview_click(&mut self, _mx: f32, my: f32, content_y: f32) {
        let content_h = WINDOW_HEIGHT as f32 - content_y;
        let lane_h = content_h / 8.0;
        let relative_y = my - content_y;
        if relative_y < 0.0 {
            return;
        }
        let ch = (relative_y / lane_h) as usize;
        self.select_channel(ch);
    }

    pub(super) fn render_velocity_lane<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        lane_y: f32,
        lane_h: f32,
    ) {
        let roll_w = screen_w - PIANO_KEY_WIDTH;

        // Lane background.
        renderer.fill_rect(0.0, lane_y, PIANO_KEY_WIDTH, lane_h, Color::rgb(35, 35, 40));
        renderer.fill_rect(
            PIANO_KEY_WIDTH,
            lane_y,
            roll_w,
            lane_h,
            Color::rgb(40, 40, 46),
        );

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

    pub(super) fn render_piano_keys<R: Renderer>(
        &self,
        renderer: &mut R,
        roll_y: f32,
        roll_h: f32,
    ) {
        renderer.fill_rect(0.0, roll_y, PIANO_KEY_WIDTH, roll_h, Color::rgb(45, 45, 52));

        let half = NOTE_HEIGHT / 2.0;
        let white_border = Color::rgb(180, 180, 185);

        // Pass 1: white keys. Each white key extends into adjacent black key rows.
        for i in 0..VISIBLE_NOTE_RANGE {
            let note = LOWEST_NOTE + VISIBLE_NOTE_RANGE - 1 - i;
            if MidiTrack::is_black_key(note) {
                continue;
            }
            let y = roll_y + i as f32 * NOTE_HEIGHT - self.scroll_y;
            if y < roll_y - NOTE_HEIGHT * 2.0 || y > roll_y + roll_h + NOTE_HEIGHT {
                continue;
            }

            // Extend upward into the row above if that note is a black key.
            let top = if note < 127 && MidiTrack::is_black_key(note + 1) {
                y - half
            } else {
                y
            };
            // Extend downward into the row below if that note is a black key.
            let bottom = if note > 0 && MidiTrack::is_black_key(note - 1) {
                y + NOTE_HEIGHT + half
            } else {
                y + NOTE_HEIGHT
            };
            renderer.fill_rect(0.0, top, PIANO_KEY_WIDTH, bottom - top - 1.0, KEY_WHITE);
            // Separator line at the bottom edge of the white key.
            renderer.draw_hline(0.0, PIANO_KEY_WIDTH, bottom - 1.0, white_border);
        }

        // Pass 2: black keys drawn on top at half width.
        let black_w = PIANO_KEY_WIDTH * 0.5;
        for i in 0..VISIBLE_NOTE_RANGE {
            let note = LOWEST_NOTE + VISIBLE_NOTE_RANGE - 1 - i;
            if !MidiTrack::is_black_key(note) {
                continue;
            }
            let y = roll_y + i as f32 * NOTE_HEIGHT - self.scroll_y;
            if y < roll_y - NOTE_HEIGHT || y > roll_y + roll_h {
                continue;
            }
            renderer.fill_rect(0.0, y, black_w, NOTE_HEIGHT - 1.0, KEY_BLACK);
        }

        // Pass 3: C labels on top of everything.
        for i in 0..VISIBLE_NOTE_RANGE {
            let note = LOWEST_NOTE + VISIBLE_NOTE_RANGE - 1 - i;
            if !note.is_multiple_of(12) {
                continue;
            }
            let y = roll_y + i as f32 * NOTE_HEIGHT - self.scroll_y;
            if y < roll_y - NOTE_HEIGHT || y > roll_y + roll_h {
                continue;
            }
            let (name, octave) = MidiTrack::note_name(note);
            let label = format!("{name}{octave}");
            renderer.draw_text(&label, black_w + 4.0, y + 1.0, FontSize::Small, KEY_LABEL);
        }
    }

    pub(super) fn note_to_y(&self, pitch: u8, roll_y: f32) -> f32 {
        let relative = (LOWEST_NOTE + VISIBLE_NOTE_RANGE - 1).saturating_sub(pitch);
        roll_y + relative as f32 * NOTE_HEIGHT - self.scroll_y
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

    /// Render the synth setup page: skeuomorphic panel with parameter displays.
    pub(super) fn render_synth_setup<R: Renderer>(
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
        draw_param_bar(
            renderer,
            x1,
            py,
            col_w,
            "Pitch Fine",
            patch.vco1_pitch_fine,
            -1.0,
            1.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x1,
            py,
            col_w,
            "Level",
            patch.vco1_level,
            0.0,
            1.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x1,
            py,
            col_w,
            "Pulse Width",
            patch.vco1_pulse_width,
            0.0,
            1.0,
            accent,
        );

        // VCO 2.
        draw_section_panel(renderer, x2, y0, col_w, row1_h, "VCO 2", accent);
        py = y0 + SECTION_HEADER_H;
        draw_param_label(renderer, x2, py, col_w, "Waveform", &patch.vco2_waveform);
        py += PARAM_ROW_H;
        draw_param_label(renderer, x2, py, col_w, "Scale", &patch.vco2_scale);
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "Pitch Fine",
            patch.vco2_pitch_fine,
            -1.0,
            1.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "Level",
            patch.vco2_level,
            0.0,
            1.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "Pulse Width",
            patch.vco2_pulse_width,
            0.0,
            1.0,
            accent,
        );

        // Mix / LFO.
        let mix_h = SECTION_HEADER_H + 3.0 * PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x3, y0, col_w, mix_h, "MIX / LFO", accent);
        py = y0 + SECTION_HEADER_H;
        draw_param_bar(
            renderer,
            x3,
            py,
            col_w,
            "VCO Mix",
            patch.vco_mix,
            0.0,
            1.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x3,
            py,
            col_w,
            "Ring Mod",
            patch.ring_mod_depth,
            0.0,
            1.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x3,
            py,
            col_w,
            "LFO Rate",
            patch.lfo_rate,
            0.1,
            20.0,
            accent,
        );

        // --- Row 2: EG1, EG2, Filters ---
        let row2_y = y0 + row1_h + SETUP_ROW_GAP;
        let row2_h = SECTION_HEADER_H + 4.0 * PARAM_ROW_H + 8.0;

        // EG 1 (HPF).
        draw_section_panel(renderer, x1, row2_y, col_w, row2_h, "EG 1 (HPF)", accent);
        py = row2_y + SECTION_HEADER_H;
        draw_param_bar(
            renderer,
            x1,
            py,
            col_w,
            "Attack",
            patch.eg1_attack,
            0.0,
            5.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x1,
            py,
            col_w,
            "Decay",
            patch.eg1_decay,
            0.0,
            5.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x1,
            py,
            col_w,
            "Sustain",
            patch.eg1_sustain,
            0.0,
            1.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x1,
            py,
            col_w,
            "Release",
            patch.eg1_release,
            0.0,
            5.0,
            accent,
        );

        // EG 2 (VCA/LPF).
        draw_section_panel(
            renderer,
            x2,
            row2_y,
            col_w,
            row2_h,
            "EG 2 (VCA/LPF)",
            accent,
        );
        py = row2_y + SECTION_HEADER_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "Attack",
            patch.eg2_attack,
            0.0,
            5.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "Decay",
            patch.eg2_decay,
            0.0,
            5.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "Sustain",
            patch.eg2_sustain,
            0.0,
            1.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "Release",
            patch.eg2_release,
            0.0,
            5.0,
            accent,
        );

        // Filters (HPF + LPF stacked in column 3).
        let filter_h = SECTION_HEADER_H + 2.0 * PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x3, row2_y, col_w, filter_h, "HPF", accent);
        py = row2_y + SECTION_HEADER_H;
        draw_param_bar(
            renderer,
            x3,
            py,
            col_w,
            "Cutoff",
            patch.hpf_cutoff,
            20.0,
            20000.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x3,
            py,
            col_w,
            "Resonance",
            patch.hpf_resonance,
            0.0,
            4.0,
            accent,
        );

        let lpf_y = row2_y + filter_h + SETUP_ROW_GAP;
        let lpf_h = SECTION_HEADER_H + 3.0 * PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x3, lpf_y, col_w, lpf_h, "LPF", accent);
        py = lpf_y + SECTION_HEADER_H;
        draw_param_bar(
            renderer,
            x3,
            py,
            col_w,
            "Cutoff",
            patch.lpf_cutoff,
            20.0,
            20000.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x3,
            py,
            col_w,
            "Resonance",
            patch.lpf_resonance,
            0.0,
            2.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x3,
            py,
            col_w,
            "Saturation",
            patch.lpf_saturation,
            0.0,
            1.0,
            accent,
        );

        // --- Row 3: Voice, Velocity, Glide ---
        let row3_y = row2_y + row2_h + SETUP_ROW_GAP;

        // Voice mode.
        let voice_h = SECTION_HEADER_H + 2.0 * PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x1, row3_y, col_w, voice_h, "VOICE", accent);
        py = row3_y + SECTION_HEADER_H;
        let priority_label = if patch.voice_mode.starts_with("low") {
            "Low"
        } else {
            "High"
        };
        draw_param_label(renderer, x1, py, col_w, "Priority", priority_label);
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x1,
            py,
            col_w,
            "Voices",
            patch.voice_count as f64,
            1.0,
            10.0,
            accent,
        );

        // Velocity.
        let vel_h = SECTION_HEADER_H + 4.0 * PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x2, row3_y, col_w, vel_h, "VELOCITY", accent);
        py = row3_y + SECTION_HEADER_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "VCA Depth",
            patch.velocity_vca_depth,
            0.0,
            1.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "Filter Depth",
            patch.velocity_filter_depth,
            0.0,
            4.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "Env Depth",
            patch.velocity_envelope_depth,
            0.0,
            1.0,
            accent,
        );
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x2,
            py,
            col_w,
            "Pitch Depth",
            patch.velocity_pitch_depth,
            0.0,
            12.0,
            accent,
        );

        // Glide.
        let glide_h = SECTION_HEADER_H + 2.0 * PARAM_ROW_H + 8.0;
        draw_section_panel(renderer, x3, row3_y, col_w, glide_h, "GLIDE", accent);
        py = row3_y + SECTION_HEADER_H;
        let glide_status = if patch.glide_enabled { "ON" } else { "OFF" };
        draw_param_label(renderer, x3, py, col_w, "Enabled", glide_status);
        py += PARAM_ROW_H;
        draw_param_bar(
            renderer,
            x3,
            py,
            col_w,
            "Time",
            patch.glide_time,
            0.0,
            2.0,
            accent,
        );

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
    pub(super) fn render_mono_chain<R: Renderer>(
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
        renderer.draw_text(
            &title,
            SETUP_MARGIN + 8.0,
            y0,
            FontSize::Medium,
            TEXT_PRIMARY,
        );

        // Signal flow indicator.
        renderer.draw_text(
            "Signal: Synth -> [Mono Chain] -> Positional Stage",
            SETUP_MARGIN + 8.0,
            y0 + 22.0,
            FontSize::Small,
            TEXT_SECONDARY,
        );

        let pedals_y = y0 + 48.0;

        let sel = self
            .selected_pedal
            .and_then(|(c, idx)| if c == 0 { Some(idx) } else { None });
        draw_pedal_chain(
            renderer,
            SETUP_MARGIN,
            pedals_y,
            &chain.pre_pedals,
            accent,
            sel,
        );

        // Controls hint.
        renderer.draw_text(
            "Click + to add an effect and drag a bar to adjust it. Keys: A=Add  Del=Remove  B=Bypass",
            SETUP_MARGIN + 8.0,
            content_y + content_h - 24.0,
            FontSize::Small,
            TEXT_SECONDARY,
        );
    }

    /// Render the stereo effect chain setup page.
    pub(super) fn render_stereo_chain<R: Renderer>(
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

        // Positional stage controls.
        let is_3d = chain.position_3d_enabled;
        let is_orbital = chain.orbital_enabled;
        let pos_rows = if is_orbital {
            7
        } else if is_3d {
            4
        } else {
            2
        };
        let pos_h = SECTION_HEADER_H + pos_rows as f32 * PARAM_ROW_H + 8.0;
        draw_section_panel(
            renderer,
            SETUP_MARGIN,
            y0,
            full_w,
            pos_h,
            "POSITIONAL STAGE",
            accent,
        );
        let mut py = y0 + SECTION_HEADER_H;
        let mode_label = if is_orbital {
            "Orbital"
        } else if is_3d {
            "3D Position"
        } else {
            "Pan"
        };
        draw_param_label(
            renderer,
            SETUP_MARGIN,
            py,
            full_w,
            "Mode (click to toggle)",
            mode_label,
        );
        py += PARAM_ROW_H;
        if is_orbital {
            draw_param_bar(
                renderer,
                SETUP_MARGIN,
                py,
                full_w,
                "Semi-major",
                chain.orbital_semi_major,
                0.0,
                1.0,
                accent,
            );
            py += PARAM_ROW_H;
            draw_param_bar(
                renderer,
                SETUP_MARGIN,
                py,
                full_w,
                "Eccentricity",
                chain.orbital_eccentricity,
                0.0,
                0.99,
                accent,
            );
            py += PARAM_ROW_H;
            draw_param_bar(
                renderer,
                SETUP_MARGIN,
                py,
                full_w,
                "Inclination",
                chain.orbital_inclination,
                0.0,
                180.0,
                accent,
            );
            py += PARAM_ROW_H;
            draw_param_bar(
                renderer,
                SETUP_MARGIN,
                py,
                full_w,
                "Ascending Node",
                chain.orbital_ascending_node,
                0.0,
                360.0,
                accent,
            );
            py += PARAM_ROW_H;
            draw_param_bar(
                renderer,
                SETUP_MARGIN,
                py,
                full_w,
                "Periapsis",
                chain.orbital_periapsis,
                0.0,
                360.0,
                accent,
            );
            py += PARAM_ROW_H;
            draw_param_bar(
                renderer,
                SETUP_MARGIN,
                py,
                full_w,
                "Period (bars)",
                chain.orbital_period,
                0.25,
                64.0,
                accent,
            );
        } else if is_3d {
            draw_param_bar(
                renderer,
                SETUP_MARGIN,
                py,
                full_w,
                "Azimuth",
                chain.azimuth,
                0.0,
                360.0,
                accent,
            );
            py += PARAM_ROW_H;
            draw_param_bar(
                renderer,
                SETUP_MARGIN,
                py,
                full_w,
                "Elevation",
                chain.elevation,
                -90.0,
                90.0,
                accent,
            );
            py += PARAM_ROW_H;
            draw_param_bar(
                renderer,
                SETUP_MARGIN,
                py,
                full_w,
                "Distance",
                chain.position_distance,
                0.0,
                1.0,
                accent,
            );
        } else {
            draw_param_bar(
                renderer,
                SETUP_MARGIN,
                py,
                full_w,
                "Pan",
                chain.pan,
                -1.0,
                1.0,
                accent,
            );
        }

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
        let sel = self
            .selected_pedal
            .and_then(|(c, idx)| if c == 1 { Some(idx) } else { None });
        draw_pedal_chain(
            renderer,
            left_x,
            left_y,
            &chain.post_pedals_left,
            accent,
            sel,
        );

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
        let sel = self
            .selected_pedal
            .and_then(|(c, idx)| if c == 2 { Some(idx) } else { None });
        draw_pedal_chain(
            renderer,
            right_x,
            right_y,
            &chain.post_pedals_right,
            accent,
            sel,
        );

        // Controls hint.
        renderer.draw_text(
            "Click + to add an effect and drag a bar to adjust it. Keys: A=Add  Del=Remove  B=Bypass",
            SETUP_MARGIN + 8.0,
            content_y + content_h - 24.0,
            FontSize::Small,
            TEXT_SECONDARY,
        );
    }
}
