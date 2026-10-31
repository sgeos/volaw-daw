//! The Piano Roll page: the ruler, the note grid, the piano keys and the
//! velocity lane, and the note editing operations that act on them.

use super::*;

impl<P: Platform, S: CommandSink> DawApp<P, S> {
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

    /// Handle a click below the channel bar on the Piano Roll page. The ruler
    /// starts a loop region drag, the velocity lane edits velocity, the piano
    /// keys audition a note, and the grid selects a note or starts a drag.
    pub(super) fn handle_piano_roll_click(&mut self, mx: f32, my: f32, content_y: f32) {
        // Ruler bar click: start loop region drag.
        if my >= content_y && my < content_y + RULER_HEIGHT && mx >= PIANO_KEY_WIDTH {
            let tick = self.x_to_tick(mx);
            self.transport.loop_start = tick;
            self.transport.loop_end = tick;
            self.transport.loop_enabled = true;
            self.dragging_loop = true;
            return;
        }

        // Determine the velocity lane boundary.
        let roll_y = Self::roll_y();
        let roll_h = self.roll_h();
        let vel_lane_y = roll_y + roll_h;

        // Velocity lane click.
        if my >= vel_lane_y && mx >= PIANO_KEY_WIDTH {
            self.handle_velocity_lane_click(mx, my, vel_lane_y);
            return;
        }

        // Piano key click (audition note).
        if my >= roll_y && my < vel_lane_y && mx < PIANO_KEY_WIDTH {
            let pitch = self.y_to_pitch(my, roll_y);
            if (LOWEST_NOTE..LOWEST_NOTE + VISIBLE_NOTE_RANGE).contains(&pitch) {
                self.preview_note(pitch, DEFAULT_VELOCITY);
            }
            return;
        }

        // Piano roll grid click (to the right of the piano keys).
        if my >= roll_y && mx >= PIANO_KEY_WIDTH {
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

            // No existing note hit. Start a potential drag selection.
            // Note placement is deferred to MouseUp if no drag occurs.
            self.drag_select_start = Some((mx, my));
            self.drag_select_active = false;
            self.drag_select_additive = false;
        }
    }

    /// Nudge selected notes horizontally by a tick delta.
    /// Clamps so no note goes below tick 0.
    pub(super) fn nudge_notes(&mut self, delta_ticks: i64) {
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
            if nudged
                .iter()
                .any(|&(t, p)| t == note.tick && p == note.pitch)
            {
                self.selected_notes.push(i);
            }
        }
    }

    /// Transpose selected notes by a number of semitones.
    /// Clamps to MIDI range (0-127).
    pub(super) fn transpose_notes(&mut self, semitones: i8) {
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
        let unit = if amount == 12 {
            "octave"
        } else {
            "semitone(s)"
        };
        let display = if amount == 12 { 1 } else { amount };
        self.status_message = Some(format!("Transpose {dir} {display} {unit}"));
    }

    /// Duplicate selected notes, offset by one grid unit to the right.
    /// Select all notes in the current channel track.
    pub(super) fn select_all_notes(&mut self) {
        if self.current_page != DawPage::PianoRoll {
            return;
        }
        let ch = self.selected_channel;
        let count = self.tracks[ch].notes.len();
        self.selected_notes = (0..count).collect();
        self.status_message = Some(format!("Selected {count} note(s)"));
    }

    pub(super) fn duplicate_notes(&mut self) {
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
    pub(super) fn quantize_notes(&mut self) {
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

    /// Drag the selected note's right edge to adjust duration.
    ///
    /// When multiple notes are selected, the primary (last) note determines
    /// the new duration and the delta is applied to all other selected notes.
    pub(super) fn drag_note_duration(&mut self, mx: f32) {
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
    pub(super) fn handle_velocity_lane_click(&mut self, mx: f32, my: f32, vel_lane_y: f32) {
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
    pub(super) fn set_velocity_from_y(&mut self, my: f32, vel_lane_y: f32) {
        let bar_margin = 1.0_f32;
        let usable_h = VELOCITY_LANE_HEIGHT - 2.0 * bar_margin;
        // Bottom of lane = velocity 0, top of lane = velocity 127.
        let frac =
            ((vel_lane_y + VELOCITY_LANE_HEIGHT - bar_margin - my) / usable_h).clamp(0.0, 1.0);
        let velocity = libm::roundf(frac * 127.0) as u8;
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
    pub(super) fn drag_velocity(&mut self, my: f32) {
        let vel_lane_y = Self::roll_y() + self.roll_h();
        self.set_velocity_from_y(my, vel_lane_y);
    }

    /// Delete all selected notes.
    pub(super) fn delete_selected_notes(&mut self) {
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
    pub(super) fn handle_shift_click(&mut self, mx: f32, my: f32) {
        if self.current_page != DawPage::PianoRoll {
            // On non-piano-roll pages, treat Shift+Click as a regular click.
            self.handle_click(mx, my);
            return;
        }
        let roll_y = Self::roll_y();
        if my < roll_y || mx < PIANO_KEY_WIDTH {
            return;
        }
        let tw = self.tick_width();
        let track = &self.tracks[self.selected_channel];
        for (i, note) in track.notes.iter().enumerate() {
            let note_x = PIANO_KEY_WIDTH + note.tick as f32 * tw - self.scroll_x;
            let note_w = note.duration as f32 * tw;
            let note_y = self.note_to_y(note.pitch, roll_y);
            if mx >= note_x && mx <= note_x + note_w && my >= note_y && my <= note_y + NOTE_HEIGHT {
                self.toggle_note_selection(i);
                return;
            }
        }
        // No note hit. Start an additive drag selection (preserves existing selection).
        self.drag_select_start = Some((mx, my));
        self.drag_select_active = false;
        self.drag_select_additive = true;
    }

    /// Copy selected notes to the clipboard.
    pub(super) fn copy_selected_notes(&mut self) {
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
    pub(super) fn paste_notes(&mut self) {
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
}
