//! The Overview page, which shows every channel's notes across the song.

use super::*;

impl<P: Platform, S: CommandSink> DawApp<P, S> {
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
        let content_h = self.window.height_f32() - content_y;
        let lane_h = content_h / 8.0;
        let relative_y = my - content_y;
        if relative_y < 0.0 {
            return;
        }
        let ch = (relative_y / lane_h) as usize;
        self.select_channel(ch);
    }
}
