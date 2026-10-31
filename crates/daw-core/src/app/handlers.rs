//! Input dispatch and the editing operations shared by every page of
//! `DawApp`. These are the undo stack, the toolbar, transport settings and
//! the parameter drag and scroll paths that several pages use. Each page's
//! own handlers live in that page's module.

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
                        self.scroll_y = (self.scroll_y - 20.0).clamp(0.0, self.max_scroll_y());
                    }
                }
            }
            DawInput::Down => {
                if self.current_page == DawPage::PianoRoll {
                    if !self.selected_notes.is_empty() {
                        self.transpose_notes(-1);
                    } else {
                        self.scroll_y = (self.scroll_y + 20.0).clamp(0.0, self.max_scroll_y());
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
                            (self.scroll_y - delta as f32 * 20.0).clamp(0.0, self.max_scroll_y());
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
            let tab_w = self.window.width_f32() / 8.0;
            let ch = (mx / tab_w) as usize;
            if ch < 8 {
                self.select_channel(ch);
            }
            return;
        }

        // Setup page navigation bar click.
        if self.current_page.is_setup() && my >= content_y && my < content_y + NAV_BAR_HEIGHT {
            if mx < 200.0 {
                self.current_page = self.current_page.setup_left();
            } else if mx > self.window.width_f32() - 200.0 {
                self.current_page = self.current_page.setup_right();
            }
            return;
        }

        // Dispatch to page-specific click handling.
        match self.current_page {
            DawPage::Studio => self.handle_studio_click(mx, my),
            DawPage::PianoRoll => self.handle_piano_roll_click(mx, my, content_y),
            DawPage::Overview => self.handle_overview_click(mx, my, content_y),
            DawPage::SynthSetup => self.handle_synth_setup_click(mx, my, content_y),
            DawPage::MonoChain => self.handle_mono_chain_click(mx, my, content_y),
            DawPage::StereoChain => self.handle_stereo_chain_click(mx, my, content_y),
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
        let subdivisions = grid_subdivisions(tpb, tpbar);
        let current_idx = subdivisions.iter().position(|&s| s == self.snap_ticks);
        // Subdivisions coincide at a coarse resolution, and `position` finds
        // only the first of equal values, so the cycle steps between first
        // occurrences to visit each distinct value once. At 24 ticks per beat
        // or more every value is distinct and every index is visited.
        let first_of_its_value = |j: usize| subdivisions[..j].iter().all(|&s| s != subdivisions[j]);
        let next_idx = match current_idx {
            Some(i) => (1..=subdivisions.len())
                .map(|step| (i + step) % subdivisions.len())
                .find(|&j| first_of_its_value(j))
                .unwrap_or(i),
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
}
