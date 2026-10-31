//! Input handling and editing operations of `DawApp`.

use super::*;

impl<P: Platform, S: CommandSink> DawApp<P, S> {
    pub fn handle_input(&mut self, input: DawInput) {
        // Toolbar inline editing mode: intercept inputs for text editing.
        if self.toolbar_edit.is_some() {
            match input {
                DawInput::Confirm => {
                    self.apply_toolbar_edit();
                    return;
                }
                DawInput::Cancel => {
                    self.finish_toolbar_edit();
                    return;
                }
                DawInput::TextChar(ch) => {
                    self.toolbar_edit_char(ch);
                    return;
                }
                DawInput::Backspace => {
                    self.edit_buffer.pop();
                    return;
                }
                DawInput::Click(mx, my) | DawInput::ShiftClick(mx, my) => {
                    // Apply current edit, then process click normally.
                    self.apply_toolbar_edit();
                    self.handle_click(mx, my);
                    return;
                }
                DawInput::Quit => {
                    self.apply_toolbar_edit();
                    self.running = false;
                    return;
                }
                // Suppress all other inputs while editing.
                _ => return,
            }
        }

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
                self.select_channel(ch as usize);
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
                } else if self.current_page == DawPage::PianoRoll {
                    self.clear_note_selection();
                } else if self.current_page == DawPage::Studio {
                    self.selected_pedal = None;
                } else {
                    self.current_page = DawPage::Studio;
                }
            }
            DawInput::Click(mx, my) => self.handle_click(mx, my),
            DawInput::ShiftClick(mx, my) => self.handle_shift_click(mx, my),
            DawInput::Copy => self.copy_selected_notes(),
            DawInput::Paste => self.paste_notes(),
            DawInput::MouseUp(_, _) => {
                self.flush_pending_undo();
                self.dragging_param = None;
                self.dragging_volume = None;
                self.dragging_note_edge = false;
                self.dragging_velocity = false;

                // Finalize ruler loop drag.
                if self.dragging_loop {
                    self.dragging_loop = false;
                    let tpbar = self.transport.ticks_per_bar();
                    if tpbar > 0 {
                        self.transport.loop_start = (self.transport.loop_start / tpbar) * tpbar;
                        self.transport.loop_end = (self.transport.loop_end.div_ceil(tpbar)) * tpbar;
                    }
                    if self.transport.loop_start >= self.transport.loop_end {
                        self.transport.loop_enabled = false;
                        self.status_message = Some(String::from("Loop OFF"));
                    } else {
                        self.status_message = Some(String::from("Loop ON"));
                    }
                }

                // Finalize rubber-band selection or place a note on click-release.
                if self.drag_select_active {
                    // Drag completed: keep the selected notes, clear drag state.
                    self.drag_select_start = None;
                    self.drag_select_active = false;
                } else if let Some((sx, sy)) = self.drag_select_start.take() {
                    // No drag occurred (just a click on empty space). Place a note
                    // at the original click position.
                    self.drag_select_active = false;
                    self.place_note_at(sx, sy);
                }
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
                        self.scroll_y = (self.scroll_y - 20.0).clamp(0.0, Self::max_scroll_y());
                    }
                }
            }
            DawInput::Down => {
                if self.current_page == DawPage::PianoRoll {
                    if !self.selected_notes.is_empty() {
                        self.transpose_notes(-1);
                    } else {
                        self.scroll_y = (self.scroll_y + 20.0).clamp(0.0, Self::max_scroll_y());
                    }
                }
            }
            DawInput::Scroll(delta) => {
                // The pedal picker shows every pedal at once, so it does not
                // scroll, and the page under it must not either.
                if self.pedal_picker_open {
                    return;
                }
                // Scroll over toolbar fields.
                let (mx, my) = self.last_mouse;
                if my < TOOLBAR_HEIGHT {
                    if (600.0..=720.0).contains(&mx) {
                        self.adjust_tempo(delta as f64);
                    } else if (720.0..=748.0).contains(&mx) {
                        self.adjust_time_sig_num(delta);
                    } else if (748.0..=780.0).contains(&mx) {
                        self.adjust_time_sig_den(delta);
                    }
                    return;
                }
                if self.current_page == DawPage::Studio {
                    self.studio_scroll(delta);
                } else if self.current_page == DawPage::PianoRoll {
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
                            self.status_message = Some(format!("Vel: {last_vel} ({count} notes)"));
                        }
                    } else {
                        self.scroll_y =
                            (self.scroll_y - delta as f32 * 20.0).clamp(0.0, Self::max_scroll_y());
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
            DawInput::Delete | DawInput::Backspace => {
                if self.current_page == DawPage::PianoRoll {
                    self.delete_selected_notes();
                } else if self.current_page.has_pedals() {
                    self.delete_selected_pedal();
                }
            }
            DawInput::Insert => {
                if self.current_page == DawPage::MonoChain || self.current_page == DawPage::Studio {
                    self.pedal_picker_open = true;
                    self.pedal_picker_chain = 0;
                } else if self.current_page == DawPage::StereoChain {
                    self.pedal_picker_open = true;
                    let chain = if let Some((c, _)) = self.selected_pedal {
                        c
                    } else if self.pedal_picker_chain == 2 {
                        2
                    } else {
                        1
                    };
                    self.pedal_picker_chain = chain;
                }
            }
            DawInput::ShiftLeft => {
                if self.current_page.has_pedals() {
                    self.move_selected_pedal(-1);
                }
            }
            DawInput::ShiftRight => {
                if self.current_page.has_pedals() {
                    self.move_selected_pedal(1);
                }
            }
            DawInput::ToggleBypass => {
                if self.current_page.has_pedals() {
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
            DawInput::CycleTimeSig => self.adjust_time_sig_num(1),
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
                if self.dragging_loop {
                    let tick = self.x_to_tick(mx);
                    if tick > self.transport.loop_start {
                        self.transport.loop_end = tick;
                    }
                } else if let Some(ch) = self.dragging_volume {
                    self.studio_drag_volume(ch, mx);
                } else if let Some(ref target) = self.dragging_param {
                    let value = drag_value(mx, target);
                    self.apply_drag_value(target.clone(), value);
                } else if self.dragging_note_edge {
                    self.drag_note_duration(mx);
                } else if self.dragging_velocity {
                    self.drag_velocity(my);
                } else if let Some((sx, sy)) = self.drag_select_start {
                    let dx = mx - sx;
                    let dy = my - sy;
                    if !self.drag_select_active && (dx * dx + dy * dy) > 16.0 {
                        self.drag_select_active = true;
                    }
                    if self.drag_select_active {
                        self.update_drag_selection(sx, sy, mx, my);
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle a mouse click. If over the grid area, either select an existing
    /// note or place a new one. If over the channel bar, select a channel.
    pub(super) fn handle_click(&mut self, mx: f32, my: f32) {
        let content_y = CONTENT_Y;

        // The pedal picker is modal and covers the whole window, so it takes
        // every click while open.
        if self.pedal_picker_open {
            self.handle_pedal_picker_click(mx, my);
            return;
        }

        // Toolbar click: edit song name, BPM, cycle time sig, cycle grid.
        if my < TOOLBAR_HEIGHT {
            if mx < 200.0 {
                self.start_toolbar_edit(ToolbarEdit::SongName, self.project.name.clone());
            } else if (620.0..720.0).contains(&mx) {
                self.start_toolbar_edit(ToolbarEdit::Bpm, format!("{:.0}", self.project.tempo));
            } else if (720.0..748.0).contains(&mx) {
                self.start_toolbar_edit(
                    ToolbarEdit::TimeSigNum,
                    format!("{}", self.project.beats_per_bar),
                );
            } else if (748.0..780.0).contains(&mx) {
                self.start_toolbar_edit(
                    ToolbarEdit::TimeSigDen,
                    format!("{}", self.project.beat_denominator),
                );
            } else if (780.0..870.0).contains(&mx) {
                self.cycle_grid();
            }
            return;
        }

        // Page tabs and transport buttons.
        if my < CHANNEL_BAR_Y {
            self.handle_page_tab_click(mx);
            return;
        }

        // Channel bar click (all pages).
        if my < content_y {
            let tab_w = WINDOW_WIDTH as f32 / 8.0;
            let ch = (mx / tab_w) as usize;
            if ch < 8 {
                self.select_channel(ch);
            }
            return;
        }

        // Ruler bar click (piano roll only): start loop region drag.
        if self.current_page == DawPage::PianoRoll
            && my >= content_y
            && my < content_y + RULER_HEIGHT
            && mx >= PIANO_KEY_WIDTH
        {
            let tick = self.x_to_tick(mx);
            self.transport.loop_start = tick;
            self.transport.loop_end = tick;
            self.transport.loop_enabled = true;
            self.dragging_loop = true;
            return;
        }

        // Setup page navigation bar click.
        if self.current_page.is_setup() && my >= content_y && my < content_y + NAV_BAR_HEIGHT {
            if mx < 200.0 {
                self.current_page = self.current_page.setup_left();
            } else if mx > WINDOW_WIDTH as f32 - 200.0 {
                self.current_page = self.current_page.setup_right();
            }
            return;
        }

        // Dispatch to page-specific click handling.
        match self.current_page {
            DawPage::Studio => {
                self.handle_studio_click(mx, my);
                return;
            }
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
        let roll_y = Self::roll_y();
        let roll_h = Self::roll_h();
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

    /// Save a snapshot of the current channel track to the undo stack.
    pub(super) fn push_undo(&mut self) {
        let ch = self.selected_channel;
        self.push_undo_action(UndoAction::Track {
            channel: ch,
            snapshot: self.tracks[ch].clone(),
        });
    }

    /// Push any undo action onto the stack. Clears the redo stack.
    pub(super) fn push_undo_action(&mut self, action: UndoAction) {
        if self.undo_stack.len() >= MAX_UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    /// Flush the pending parameter undo to the stack if present.
    pub(super) fn flush_pending_undo(&mut self) {
        if let Some(action) = self.pending_param_undo.take() {
            self.push_undo_action(action);
        }
    }

    /// Snapshot a pedal chain before a structural modification.
    pub(super) fn push_chain_undo(&mut self, ch: usize, chain_id: u8) {
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
    pub(super) fn capture_inverse(&self, action: &UndoAction) -> UndoAction {
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
    pub(super) fn apply_action(&mut self, action: &UndoAction) {
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
    pub(super) fn undo(&mut self) {
        self.flush_pending_undo();
        if let Some(action) = self.undo_stack.pop() {
            let redo_action = self.capture_inverse(&action);
            self.apply_action(&action);
            self.redo_stack.push(redo_action);
            self.status_message = Some(String::from("Undo"));
        }
    }

    /// Redo the last undone operation.
    pub(super) fn redo(&mut self) {
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
    pub(super) fn cycle_preset(&mut self, offset: i32) {
        let count = presets::preset_names().len();
        if count == 0 {
            return;
        }
        let current = self.preset_indices[self.selected_channel].unwrap_or(0);
        let next = if offset > 0 {
            (current + 1) % count
        } else if current == 0 {
            count - 1
        } else {
            current - 1
        };
        self.select_preset(next);
    }

    /// Cycle the snap grid subdivision through straight, triplet, and dotted values.
    /// Each base division (1/1 through 1/32) has straight, T (triplet), and D (dotted) variants.
    pub(super) fn cycle_grid(&mut self) {
        let tpb = self.project.ticks_per_beat as u64;
        let tpbar = self.transport.ticks_per_bar();
        // Base values: bar, half, quarter, eighth, sixteenth, thirty-second.
        let bases: [u64; 6] = [tpbar, tpb * 2, tpb, tpb / 2, tpb / 4, tpb / 8];
        // Build full list: for each base, straight then triplet (2/3) then dotted (3/2).
        let mut subdivisions = Vec::with_capacity(18);
        for &b in &bases {
            subdivisions.push(b);
            subdivisions.push(b * 2 / 3);
            subdivisions.push(b * 3 / 2);
        }
        let current_idx = subdivisions.iter().position(|&s| s == self.snap_ticks);
        let next_idx = match current_idx {
            Some(i) => (i + 1) % subdivisions.len(),
            None => 6, // Default to quarter note (index 6 = 1/4 straight).
        };
        self.snap_ticks = subdivisions[next_idx];
        let label = grid_label(self.snap_ticks, tpb, tpbar);
        self.status_message = Some(format!("Grid: {label}"));
    }

    /// Adjust the project tempo by a given BPM delta, clamped to 20-300 BPM.
    pub(super) fn adjust_tempo(&mut self, delta: f64) {
        self.project.tempo = (self.project.tempo + delta).clamp(1.0, 1200.0);
        self.transport.tempo = self.project.tempo;
        self.status_message = Some(format!("{:.0} BPM", self.project.tempo));
    }

    /// Enter toolbar editing mode for the given field.
    pub(super) fn start_toolbar_edit(&mut self, field: ToolbarEdit, initial: String) {
        self.toolbar_edit = Some(field);
        self.edit_buffer = initial;
    }

    /// Exit toolbar editing mode without applying changes.
    pub(super) fn finish_toolbar_edit(&mut self) {
        self.toolbar_edit = None;
        self.edit_buffer.clear();
    }

    /// Apply the current toolbar edit buffer to the relevant project field.
    pub(super) fn apply_toolbar_edit(&mut self) {
        match self.toolbar_edit {
            Some(ToolbarEdit::SongName) => {
                let name = self.edit_buffer.trim().to_string();
                if !name.is_empty() {
                    self.project.name = name;
                }
            }
            Some(ToolbarEdit::Bpm) => {
                if let Ok(bpm) = self.edit_buffer.trim().parse::<f64>() {
                    let clamped = bpm.clamp(1.0, 1200.0);
                    self.project.tempo = clamped;
                    self.transport.tempo = clamped;
                    self.status_message = Some(format!("{clamped:.0} BPM"));
                }
            }
            Some(ToolbarEdit::TimeSigNum) => {
                if let Ok(num) = self.edit_buffer.trim().parse::<u8>() {
                    let clamped = num.clamp(1, 32);
                    self.project.beats_per_bar = clamped;
                    self.transport.beats_per_bar = clamped;
                    let den = self.project.beat_denominator;
                    self.status_message = Some(format!("{clamped}/{den} Time"));
                }
            }
            Some(ToolbarEdit::TimeSigDen) => {
                if let Ok(den) = self.edit_buffer.trim().parse::<u8>() {
                    // Snap to nearest valid power of 2 (1, 2, 4, 8, 16, 32).
                    const DENOMS: [u8; 6] = [1, 2, 4, 8, 16, 32];
                    let snapped = DENOMS
                        .iter()
                        .min_by_key(|&&d| (d as i32 - den as i32).unsigned_abs())
                        .copied()
                        .unwrap_or(4);
                    self.project.beat_denominator = snapped;
                    self.transport.beat_denominator = snapped;
                    let num = self.project.beats_per_bar;
                    self.status_message = Some(format!("{num}/{snapped} Time"));
                }
            }
            None => {}
        }
        self.finish_toolbar_edit();
    }

    /// Append a character to the toolbar edit buffer with field-specific validation.
    pub(super) fn toolbar_edit_char(&mut self, ch: char) {
        match self.toolbar_edit {
            Some(ToolbarEdit::SongName) => {
                if self.edit_buffer.len() < 32
                    && (ch.is_alphanumeric() || ch == ' ' || ch == '-' || ch == '_')
                {
                    self.edit_buffer.push(ch);
                }
            }
            Some(ToolbarEdit::Bpm) => {
                if self.edit_buffer.len() < 6
                    && (ch.is_ascii_digit() || (ch == '.' && !self.edit_buffer.contains('.')))
                {
                    self.edit_buffer.push(ch);
                }
            }
            Some(ToolbarEdit::TimeSigNum) | Some(ToolbarEdit::TimeSigDen) => {
                if self.edit_buffer.len() < 2 && ch.is_ascii_digit() {
                    self.edit_buffer.push(ch);
                }
            }
            None => {}
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

    /// Toggle solo on the selected channel.
    /// When soloed, only that channel plays. Toggle again to un-solo.
    pub(super) fn toggle_solo(&mut self) {
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
    pub(super) fn toggle_metronome(&mut self) {
        self.metronome_enabled = !self.metronome_enabled;
        self.send_command(EngineCommand::SetMetronomeEnabled {
            enabled: self.metronome_enabled,
        });
        self.status_message = Some(if self.metronome_enabled {
            String::from("Metronome ON")
        } else {
            String::from("Metronome OFF")
        });
    }

    /// Adjust the time signature numerator by a delta, clamped to 1-32.
    pub(super) fn adjust_time_sig_num(&mut self, delta: i32) {
        let new_num = (self.project.beats_per_bar as i32 + delta).clamp(1, 32) as u8;
        self.project.beats_per_bar = new_num;
        self.transport.beats_per_bar = new_num;
        let den = self.project.beat_denominator;
        self.status_message = Some(format!("{new_num}/{den} Time"));
    }

    /// Adjust the time signature denominator to the next/previous power of 2.
    /// Valid values: 1, 2, 4, 8, 16, 32.
    pub(super) fn adjust_time_sig_den(&mut self, delta: i32) {
        const DENOMS: [u8; 6] = [1, 2, 4, 8, 16, 32];
        let current_idx = DENOMS
            .iter()
            .position(|&d| d == self.project.beat_denominator)
            .unwrap_or(2); // Default to 4 if not found.
        let new_idx = (current_idx as i32 + delta).clamp(0, DENOMS.len() as i32 - 1) as usize;
        let den = DENOMS[new_idx];
        self.project.beat_denominator = den;
        self.transport.beat_denominator = den;
        let num = self.project.beats_per_bar;
        self.status_message = Some(format!("{num}/{den} Time"));
    }

    /// Toggle loop mode. When enabling, auto-set the loop region to cover
    /// all notes in the current channel track.
    pub(super) fn toggle_loop(&mut self) {
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

    /// Apply all channel patches to the audio engine. Sends a Bulk edit
    /// for each channel with pre-loaded PCM samples.
    #[allow(dead_code)]
    pub(super) fn apply_all_patches(&mut self) {
        for ch in 0..8 {
            self.send_patch(ch);
        }
    }

    /// Apply all channel effect chains to the audio engine. Sends a Bulk
    /// edit for each channel to replace pedal chains and positions.
    #[allow(dead_code)]
    pub(super) fn apply_all_effects(&mut self) {
        let sr = self.sample_rate as f64;
        for ch in 0..8 {
            let cmd = engine_setup::effects_command(ch, &self.effects[ch], sr);
            self.send_command(cmd);
        }
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
        let vel_lane_y = Self::roll_y() + Self::roll_h();
        self.set_velocity_from_y(my, vel_lane_y);
    }

    /// Handle a click on the synth setup page.
    ///
    /// Tests all parameter bar regions and enum label regions. On a bar hit,
    /// starts a drag. On an enum label hit, cycles to the next value.
    pub(super) fn handle_synth_setup_click(&mut self, mx: f32, my: f32, content_y: f32) {
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
            (
                vco1_y + 2.0 * PARAM_ROW_H,
                SynthParam::Vco1PitchFine,
                -1.0,
                1.0,
            ),
            (vco1_y + 3.0 * PARAM_ROW_H, SynthParam::Vco1Level, 0.0, 1.0),
            (
                vco1_y + 4.0 * PARAM_ROW_H,
                SynthParam::Vco1PulseWidth,
                0.0,
                1.0,
            ),
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
            (
                vco2_y + 2.0 * PARAM_ROW_H,
                SynthParam::Vco2PitchFine,
                -1.0,
                1.0,
            ),
            (vco2_y + 3.0 * PARAM_ROW_H, SynthParam::Vco2Level, 0.0, 1.0),
            (
                vco2_y + 4.0 * PARAM_ROW_H,
                SynthParam::Vco2PulseWidth,
                0.0,
                1.0,
            ),
        ];
        for (py, param, min, max) in vco2_bars {
            if hit_row(mx, my, x2, py, col_w) {
                self.start_bar_drag(mx, x2 + bar_inset, bar_w, param, min, max);
                return;
            }
        }

        // Mix / LFO bars.
        let mix_y = y0 + SECTION_HEADER_H;
        let mix_bars: [(f32, SynthParam, f64, f64); 3] = [
            (mix_y, SynthParam::VcoMix, 0.0, 1.0),
            (mix_y + PARAM_ROW_H, SynthParam::RingModDepth, 0.0, 1.0),
            (mix_y + 2.0 * PARAM_ROW_H, SynthParam::LfoRate, 0.1, 20.0),
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
        let lpf_bars: [(f32, SynthParam, f64, f64); 3] = [
            (lpf_y, SynthParam::LpfCutoff, 20.0, 20000.0),
            (lpf_y + PARAM_ROW_H, SynthParam::LpfResonance, 0.0, 2.0),
            (
                lpf_y + 2.0 * PARAM_ROW_H,
                SynthParam::LpfSaturation,
                0.0,
                1.0,
            ),
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

        // Voice priority label (click to cycle high/low).
        let voice_y = row3_y + SECTION_HEADER_H;
        if hit_row(mx, my, x1, voice_y, col_w) {
            self.cycle_voice_mode();
            return;
        }
        // Voice count bar.
        let voice_count_y = voice_y + PARAM_ROW_H;
        if hit_row(mx, my, x1, voice_count_y, col_w) {
            self.start_bar_drag(mx, x1 + bar_inset, bar_w, SynthParam::VoiceCount, 1.0, 10.0);
            return;
        }

        // Velocity bars.
        let vel_y = row3_y + SECTION_HEADER_H;
        let vel_bars: [(f32, SynthParam, f64, f64); 4] = [
            (vel_y, SynthParam::VelocityVca, 0.0, 1.0),
            (vel_y + PARAM_ROW_H, SynthParam::VelocityFilter, 0.0, 4.0),
            (vel_y + 2.0 * PARAM_ROW_H, SynthParam::VelocityEnv, 0.0, 1.0),
            (
                vel_y + 3.0 * PARAM_ROW_H,
                SynthParam::VelocityPitch,
                0.0,
                12.0,
            ),
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
    pub(super) fn start_bar_drag(
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
    pub(super) fn cycle_vco1_waveform(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].vco1_waveform = next_waveform(&self.patches[ch].vco1_waveform);
        self.send_patch(ch);
    }

    /// Cycle VCO2 waveform to the next value.
    pub(super) fn cycle_vco2_waveform(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].vco2_waveform = next_waveform(&self.patches[ch].vco2_waveform);
        self.send_patch(ch);
    }

    /// Cycle VCO1 scale to the next value.
    pub(super) fn cycle_vco1_scale(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].vco1_scale = next_scale(&self.patches[ch].vco1_scale);
        self.send_patch(ch);
    }

    /// Cycle VCO2 scale to the next value.
    pub(super) fn cycle_vco2_scale(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].vco2_scale = next_scale(&self.patches[ch].vco2_scale);
        self.send_patch(ch);
    }

    /// Cycle voice mode to the next value.
    pub(super) fn cycle_voice_mode(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].voice_mode = next_voice_mode(&self.patches[ch].voice_mode);
        self.send_patch(ch);
    }

    /// Toggle glide enabled on the selected channel.
    pub(super) fn toggle_glide(&mut self) {
        let ch = self.selected_channel;
        self.patches[ch].glide_enabled = !self.patches[ch].glide_enabled;
        self.send_patch(ch);
    }

    /// Handle a click on the mono chain page (pedal parameter editing).
    pub(super) fn handle_mono_chain_click(&mut self, mx: f32, my: f32, content_y: f32) {
        let pedals_y = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP + 48.0;
        self.handle_pedal_chain_click(mx, my, pedals_y, 0, SETUP_MARGIN);
    }

    /// Handle a click on the stereo chain page (positional controls and
    /// pedal parameter editing).
    pub(super) fn handle_stereo_chain_click(&mut self, mx: f32, my: f32, content_y: f32) {
        let screen_w = WINDOW_WIDTH as f32;
        let full_w = screen_w - 2.0 * SETUP_MARGIN;
        let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;

        // Positional stage geometry (must match render_stereo_chain layout).
        let ch = self.selected_channel;
        let is_3d = self.effects[ch].position_3d_enabled;
        let is_orbital = self.effects[ch].orbital_enabled;
        let pos_rows = if is_orbital {
            7
        } else if is_3d {
            4
        } else {
            2
        };
        let pos_h = SECTION_HEADER_H + pos_rows as f32 * PARAM_ROW_H + 8.0;

        // Check clicks within the positional stage section.
        if my >= y0 && my < y0 + pos_h {
            let bar_inset = 8.0;
            let bar_w = full_w - 16.0;
            let mut py = y0 + SECTION_HEADER_H;

            let is_orbital = self.effects[ch].orbital_enabled;

            // Mode toggle row: Pan -> 3D Position -> Orbital -> Pan.
            if hit_row(mx, my, SETUP_MARGIN, py, full_w) {
                if !is_3d && !is_orbital {
                    // Pan -> 3D Position
                    self.effects[ch].position_3d_enabled = true;
                    self.effects[ch].orbital_enabled = false;
                } else if is_3d && !is_orbital {
                    // 3D Position -> Orbital
                    self.effects[ch].position_3d_enabled = true;
                    self.effects[ch].orbital_enabled = true;
                } else {
                    // Orbital -> Pan
                    self.effects[ch].position_3d_enabled = false;
                    self.effects[ch].orbital_enabled = false;
                }
                let cmd = engine_setup::position_command(ch, &self.effects[ch]);
                self.send_command(cmd);
                return;
            }
            py += PARAM_ROW_H;

            if is_orbital {
                // Orbital mode: 6 parameter bars.
                let bars: [(f32, SynthParam, f64, f64); 6] = [
                    (py, SynthParam::OrbitalSemiMajor, 0.0, 1.0),
                    (py + PARAM_ROW_H, SynthParam::OrbitalEccentricity, 0.0, 0.99),
                    (
                        py + 2.0 * PARAM_ROW_H,
                        SynthParam::OrbitalInclination,
                        0.0,
                        180.0,
                    ),
                    (
                        py + 3.0 * PARAM_ROW_H,
                        SynthParam::OrbitalAscendingNode,
                        0.0,
                        360.0,
                    ),
                    (
                        py + 4.0 * PARAM_ROW_H,
                        SynthParam::OrbitalPeriapsis,
                        0.0,
                        360.0,
                    ),
                    (
                        py + 5.0 * PARAM_ROW_H,
                        SynthParam::OrbitalPeriod,
                        0.25,
                        64.0,
                    ),
                ];
                for (bar_y, param, min, max) in bars {
                    if hit_row(mx, my, SETUP_MARGIN, bar_y, full_w) {
                        self.start_bar_drag(mx, SETUP_MARGIN + bar_inset, bar_w, param, min, max);
                        return;
                    }
                }
            } else if is_3d {
                let bars: [(f32, SynthParam, f64, f64); 3] = [
                    (py, SynthParam::Azimuth, 0.0, 360.0),
                    (py + PARAM_ROW_H, SynthParam::Elevation, -90.0, 90.0),
                    (
                        py + 2.0 * PARAM_ROW_H,
                        SynthParam::PositionDistance,
                        0.0,
                        1.0,
                    ),
                ];
                for (bar_y, param, min, max) in bars {
                    if hit_row(mx, my, SETUP_MARGIN, bar_y, full_w) {
                        self.start_bar_drag(mx, SETUP_MARGIN + bar_inset, bar_w, param, min, max);
                        return;
                    }
                }
            } else if hit_row(mx, my, SETUP_MARGIN, py, full_w) {
                self.start_bar_drag(
                    mx,
                    SETUP_MARGIN + bar_inset,
                    bar_w,
                    SynthParam::Pan,
                    -1.0,
                    1.0,
                );
                return;
            }
            return;
        }

        // Pedal chain area below positional stage.
        let chains_y = y0 + pos_h + SETUP_ROW_GAP;
        let left_header_h = 28.0;
        let pedals_y = chains_y + left_header_h + 4.0;

        let half_w = (screen_w - 3.0 * SETUP_MARGIN) / 2.0;
        let right_x = SETUP_MARGIN + half_w + SETUP_MARGIN;

        if mx < right_x {
            self.handle_pedal_chain_click(mx, my, pedals_y, 1, SETUP_MARGIN);
        } else {
            self.handle_pedal_chain_click(mx, my, pedals_y, 2, right_x);
        }
    }

    /// Handle a click within a pedal chain layout.
    ///
    /// `chain_id`: 0 = pre (mono), 1 = post-left, 2 = post-right.
    /// `start_x`: the left edge x coordinate where this chain's pedals begin.
    pub(super) fn handle_pedal_chain_click(
        &mut self,
        mx: f32,
        my: f32,
        pedals_y: f32,
        chain_id: u8,
        start_x: f32,
    ) {
        let ch = self.selected_channel;
        let pedals = match chain_id {
            0 => &self.effects[ch].pre_pedals,
            1 => &self.effects[ch].post_pedals_left,
            2 => &self.effects[ch].post_pedals_right,
            _ => return,
        };

        let bar_inset = 8.0;
        let bar_w = PEDAL_W - 16.0;

        // Scan for hit targets without mutable self access.
        enum HitKind {
            Button(usize, PedalButton),
            Header(usize, String),
            Add,
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

        if add_card(start_x, pedals_y, pedals.len()).contains(mx, my) {
            hit = Some(HitKind::Add);
        }
        for (pedal_idx, pedal) in pedals.iter().enumerate() {
            if hit.is_some() {
                break;
            }
            let px = pedal_x(start_x, pedal_idx);
            if !Rect::new(px, pedals_y, PEDAL_W, pedal_h(pedal)).contains(mx, my) {
                continue;
            }
            if pedal_led(px, pedals_y).contains(mx, my) {
                hit = Some(HitKind::Button(pedal_idx, PedalButton::Bypass));
                break;
            }
            if let Some((button, _)) = pedal_buttons(px, pedals_y, pedal)
                .into_iter()
                .find(|(_, r)| r.contains(mx, my))
            {
                hit = Some(HitKind::Button(pedal_idx, button));
                break;
            }
            let param_start_y = pedals_y + PEDAL_HEADER_H;
            for (param_idx, param) in pedal.params.iter().enumerate() {
                let py = param_start_y + param_idx as f32 * PARAM_ROW_H;
                if hit_row(mx, my, px, py, PEDAL_W) {
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
                    break;
                }
            }
            if hit.is_none() {
                // Anywhere else on the box selects it.
                hit = Some(HitKind::Header(pedal_idx, pedal.pedal_type.clone()));
            }
        }

        // Now process the hit with mutable self access.
        match hit {
            Some(HitKind::Add) => {
                self.pedal_picker_chain = chain_id;
                self.pedal_picker_open = true;
            }
            Some(HitKind::Button(pedal_idx, button)) => {
                self.selected_pedal = Some((chain_id, pedal_idx));
                match button {
                    PedalButton::Bypass => self.toggle_pedal_bypass(),
                    PedalButton::MoveLeft => self.move_selected_pedal(-1),
                    PedalButton::MoveRight => self.move_selected_pedal(1),
                    PedalButton::Remove => self.delete_selected_pedal(),
                }
            }
            Some(HitKind::Header(pedal_idx, name)) => {
                self.selected_pedal = Some((chain_id, pedal_idx));
                self.status_message = Some(format!(
                    "Selected: {} [Del=Remove, B=Bypass, Shift+Arrows=Move]",
                    pedal_display_name(&name)
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
                // Remember which chain section was clicked so 'A' targets it.
                if chain_id == 1 || chain_id == 2 {
                    self.pedal_picker_chain = chain_id;
                }
            }
        }
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

    /// Adjust a parameter by scroll delta.
    ///
    /// On synth page: if actively dragging a param, scroll fine-adjusts it.
    /// On chain pages: hit-tests the mouse position against pedal param bars.
    /// Each scroll tick nudges the value by 2% of the parameter range
    /// (in log space for frequency parameters).
    pub(super) fn scroll_param(&mut self, delta: i32) {
        // If actively dragging a synth or pedal param, fine-adjust it.
        if let Some(ref target) = self.dragging_param.clone() {
            match target {
                DragTarget::Synth {
                    param,
                    min_val,
                    max_val,
                    log_scale,
                    ..
                } => {
                    let ch = self.selected_channel;
                    let current = self.get_synth_param_value(ch, *param);
                    let new_val = scroll_step(current, *min_val, *max_val, delta, *log_scale);
                    self.set_synth_param(ch, *param, new_val);
                }
                DragTarget::Pedal {
                    chain,
                    pedal_idx,
                    param_idx,
                    min_val,
                    max_val,
                    log_scale,
                    ..
                } => {
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
        if self.current_page.has_pedals() {
            let (mx, my) = self.last_mouse;
            let content_y = CONTENT_Y;
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
            let content_y = CONTENT_Y;
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
    pub(super) fn find_synth_param_at(
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
            (
                vco1_y + 2.0 * PARAM_ROW_H,
                SynthParam::Vco1PitchFine,
                -1.0,
                1.0,
            ),
            (vco1_y + 3.0 * PARAM_ROW_H, SynthParam::Vco1Level, 0.0, 1.0),
            (
                vco1_y + 4.0 * PARAM_ROW_H,
                SynthParam::Vco1PulseWidth,
                0.0,
                1.0,
            ),
        ];
        for (py, param, min, max) in vco1_bars {
            if hit_row(mx, my, x1, py, col_w) {
                return Some((param, min, max));
            }
        }

        // VCO2 param bars.
        let vco2_y = y0 + SECTION_HEADER_H;
        let vco2_bars: [(f32, SynthParam, f64, f64); 3] = [
            (
                vco2_y + 2.0 * PARAM_ROW_H,
                SynthParam::Vco2PitchFine,
                -1.0,
                1.0,
            ),
            (vco2_y + 3.0 * PARAM_ROW_H, SynthParam::Vco2Level, 0.0, 1.0),
            (
                vco2_y + 4.0 * PARAM_ROW_H,
                SynthParam::Vco2PulseWidth,
                0.0,
                1.0,
            ),
        ];
        for (py, param, min, max) in vco2_bars {
            if hit_row(mx, my, x2, py, col_w) {
                return Some((param, min, max));
            }
        }

        // Mix / LFO bars.
        let mix_y = y0 + SECTION_HEADER_H;
        let mix_bars: [(f32, SynthParam, f64, f64); 3] = [
            (mix_y, SynthParam::VcoMix, 0.0, 1.0),
            (mix_y + PARAM_ROW_H, SynthParam::RingModDepth, 0.0, 1.0),
            (mix_y + 2.0 * PARAM_ROW_H, SynthParam::LfoRate, 0.1, 20.0),
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
        let lpf_bars: [(f32, SynthParam, f64, f64); 3] = [
            (lpf_y, SynthParam::LpfCutoff, 20.0, 20000.0),
            (lpf_y + PARAM_ROW_H, SynthParam::LpfResonance, 0.0, 2.0),
            (
                lpf_y + 2.0 * PARAM_ROW_H,
                SynthParam::LpfSaturation,
                0.0,
                1.0,
            ),
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
            (
                vel_y + 3.0 * PARAM_ROW_H,
                SynthParam::VelocityPitch,
                0.0,
                12.0,
            ),
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
    pub(super) fn find_pedal_param_at(
        &self,
        mx: f32,
        my: f32,
        content_y: f32,
    ) -> Option<(u8, usize, usize, f64, f64)> {
        let screen_w = WINDOW_WIDTH as f32;
        let pedal_w = 180.0;
        let pedal_gap = 16.0;
        let header_h = 28.0;

        if self.current_page == DawPage::Studio {
            if mx < studio::BROWSER_W {
                return None;
            }
            let (start_x, pedals_y) = self.studio_rack_origin();
            return find_pedal_param_in_chain(
                mx,
                my,
                pedals_y,
                start_x,
                &self.effects[self.selected_channel].pre_pedals,
                pedal_w,
                pedal_gap,
                header_h,
                0,
            );
        }

        if self.current_page == DawPage::MonoChain {
            let pedals_y = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP + 48.0;
            let pedals = &self.effects[self.selected_channel].pre_pedals;
            return find_pedal_param_in_chain(
                mx,
                my,
                pedals_y,
                SETUP_MARGIN,
                pedals,
                pedal_w,
                pedal_gap,
                header_h,
                0,
            );
        }

        if self.current_page == DawPage::StereoChain {
            let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;
            let ch = self.selected_channel;
            let pos_rows = if self.effects[ch].position_3d_enabled {
                4
            } else {
                2
            };
            let pos_h = SECTION_HEADER_H + pos_rows as f32 * PARAM_ROW_H + 8.0;
            let chains_y = y0 + pos_h + SETUP_ROW_GAP;
            let left_header_h = 28.0;
            let pedals_y = chains_y + left_header_h + 4.0;

            let half_w = (screen_w - 3.0 * SETUP_MARGIN) / 2.0;
            let right_x = SETUP_MARGIN + half_w + SETUP_MARGIN;

            if mx < right_x {
                return find_pedal_param_in_chain(
                    mx,
                    my,
                    pedals_y,
                    SETUP_MARGIN,
                    &self.effects[ch].post_pedals_left,
                    pedal_w,
                    pedal_gap,
                    header_h,
                    1,
                );
            } else {
                return find_pedal_param_in_chain(
                    mx,
                    my,
                    pedals_y,
                    right_x,
                    &self.effects[ch].post_pedals_right,
                    pedal_w,
                    pedal_gap,
                    header_h,
                    2,
                );
            }
        }

        None
    }

    /// Get the current value of a synth parameter from PatchConfig.
    pub(super) fn get_synth_param_value(&self, ch: usize, param: SynthParam) -> f64 {
        let patch = &self.patches[ch];
        match param {
            SynthParam::Vco1PitchFine => patch.vco1_pitch_fine,
            SynthParam::Vco1Level => patch.vco1_level,
            SynthParam::Vco1PulseWidth => patch.vco1_pulse_width,
            SynthParam::Vco2PitchFine => patch.vco2_pitch_fine,
            SynthParam::Vco2Level => patch.vco2_level,
            SynthParam::Vco2PulseWidth => patch.vco2_pulse_width,
            SynthParam::VcoMix => patch.vco_mix,
            SynthParam::RingModDepth => patch.ring_mod_depth,
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
            SynthParam::LpfSaturation => patch.lpf_saturation,
            SynthParam::VelocityVca => patch.velocity_vca_depth,
            SynthParam::VelocityFilter => patch.velocity_filter_depth,
            SynthParam::VelocityEnv => patch.velocity_envelope_depth,
            SynthParam::VelocityPitch => patch.velocity_pitch_depth,
            SynthParam::GlideTime => patch.glide_time,
            SynthParam::Pan => self.effects[ch].pan,
            SynthParam::Azimuth => self.effects[ch].azimuth,
            SynthParam::Elevation => self.effects[ch].elevation,
            SynthParam::PositionDistance => self.effects[ch].position_distance,
            SynthParam::OrbitalSemiMajor => self.effects[ch].orbital_semi_major,
            SynthParam::OrbitalEccentricity => self.effects[ch].orbital_eccentricity,
            SynthParam::OrbitalInclination => self.effects[ch].orbital_inclination,
            SynthParam::OrbitalAscendingNode => self.effects[ch].orbital_ascending_node,
            SynthParam::OrbitalPeriapsis => self.effects[ch].orbital_periapsis,
            SynthParam::OrbitalPeriod => self.effects[ch].orbital_period,
            SynthParam::VoiceCount => self.patches[ch].voice_count as f64,
        }
    }

    /// Toggle bypass on the currently selected pedal.
    pub(super) fn toggle_pedal_bypass(&mut self) {
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
    pub(super) fn delete_selected_pedal(&mut self) {
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
    pub(super) fn move_selected_pedal(&mut self, direction: i32) {
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
        if pedal_idx >= pedals.len() || new_idx < 0 || new_idx as usize >= pedals.len() {
            return;
        }
        let new_idx = new_idx as usize;
        pedals.swap(pedal_idx, new_idx);
        self.selected_pedal = Some((chain_id, new_idx));
        self.rebuild_engine_chain(ch, chain_id);
    }

    /// Add a pedal of the given type to a chain.
    pub(super) fn add_pedal_to_chain(&mut self, chain_id: u8, pedal_type: &str) {
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
    pub(super) fn rebuild_engine_chain(&mut self, ch: usize, chain_id: u8) {
        let sr = self.sample_rate as f64;
        let Some(configs) = self.effects[ch].get_chain_pedals(chain_id) else {
            return;
        };
        if let Some(cmd) = engine_setup::chain_command(ch, chain_id, configs, sr) {
            self.send_command(cmd);
        }
    }

    /// Handle a click on the pedal type picker overlay.
    pub(super) fn handle_pedal_picker_click(&mut self, mx: f32, my: f32) {
        // Click outside picker closes it.
        if !picker_rect().contains(mx, my) {
            self.pedal_picker_open = false;
            return;
        }
        if let Some(pedal_type) = picker_item_at(mx, my) {
            self.pedal_picker_open = false;
            self.add_pedal_to_chain(self.pedal_picker_chain, pedal_type);
        }
    }

    /// Apply a dragged parameter value to both PatchConfig and the audio engine.
    pub(super) fn apply_drag_value(&mut self, target: DragTarget, value: f64) {
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
    pub(super) fn set_synth_param(&mut self, ch: usize, param: SynthParam, value: f64) {
        let patch = &mut self.patches[ch];
        match param {
            SynthParam::Vco1PitchFine => patch.vco1_pitch_fine = value,
            SynthParam::Vco1Level => patch.vco1_level = value,
            SynthParam::Vco1PulseWidth => patch.vco1_pulse_width = value,
            SynthParam::Vco2PitchFine => patch.vco2_pitch_fine = value,
            SynthParam::Vco2Level => patch.vco2_level = value,
            SynthParam::Vco2PulseWidth => patch.vco2_pulse_width = value,
            SynthParam::VcoMix => patch.vco_mix = value,
            SynthParam::RingModDepth => patch.ring_mod_depth = value,
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
            SynthParam::LpfSaturation => patch.lpf_saturation = value,
            SynthParam::VelocityVca => patch.velocity_vca_depth = value,
            SynthParam::VelocityFilter => patch.velocity_filter_depth = value,
            SynthParam::VelocityEnv => patch.velocity_envelope_depth = value,
            SynthParam::VelocityPitch => patch.velocity_pitch_depth = value,
            SynthParam::GlideTime => patch.glide_time = value,
            SynthParam::VoiceCount => {
                self.patches[ch].voice_count = (libm::round(value) as u8).clamp(1, 10);
                self.send_patch(ch);
                // Re-trigger any currently active notes on this channel,
                // because set_voice_mode releases all voices when resizing
                // the voice pool.
                let retrigger: Vec<_> = self.active_notes[ch]
                    .iter()
                    .filter_map(|&idx| self.tracks[ch].notes.get(idx))
                    .map(|note| {
                        let freq = MidiTrack::note_to_freq(note.pitch);
                        let vel = note.velocity as f64 / 127.0;
                        EngineCommand::NoteOn {
                            channel: ch as u8,
                            freq,
                            velocity: vel,
                        }
                    })
                    .collect();
                for cmd in retrigger {
                    self.send_command(cmd);
                }
                return;
            }
            SynthParam::Pan
            | SynthParam::Azimuth
            | SynthParam::Elevation
            | SynthParam::PositionDistance
            | SynthParam::OrbitalSemiMajor
            | SynthParam::OrbitalEccentricity
            | SynthParam::OrbitalInclination
            | SynthParam::OrbitalAscendingNode
            | SynthParam::OrbitalPeriapsis
            | SynthParam::OrbitalPeriod => {
                match param {
                    SynthParam::Pan => self.effects[ch].pan = value,
                    SynthParam::Azimuth => self.effects[ch].azimuth = value,
                    SynthParam::Elevation => self.effects[ch].elevation = value,
                    SynthParam::PositionDistance => self.effects[ch].position_distance = value,
                    SynthParam::OrbitalSemiMajor => self.effects[ch].orbital_semi_major = value,
                    SynthParam::OrbitalEccentricity => {
                        self.effects[ch].orbital_eccentricity = value
                    }
                    SynthParam::OrbitalInclination => self.effects[ch].orbital_inclination = value,
                    SynthParam::OrbitalAscendingNode => {
                        self.effects[ch].orbital_ascending_node = value
                    }
                    SynthParam::OrbitalPeriapsis => self.effects[ch].orbital_periapsis = value,
                    SynthParam::OrbitalPeriod => self.effects[ch].orbital_period = value,
                    _ => unreachable!(),
                }
                let cmd = engine_setup::position_command(ch, &self.effects[ch]);
                self.send_command(cmd);
                return;
            }
        }
        // Sync the full patch to the engine via a Bulk edit.
        self.send_patch(ch);
    }

    /// Write a pedal parameter value to EffectChainConfig and sync to the engine.
    pub(super) fn set_pedal_param(
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
        let cmd = engine_setup::pedal_param_command(ch, chain, pedal_idx, param_name, value);
        self.send_command(cmd);
    }
}
