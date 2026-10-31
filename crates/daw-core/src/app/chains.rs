//! The Mono Chain and Stereo Chain pages, the pedal operations they share
//! with the Studio rack, and the pedal picker overlay.

use super::*;

/// Height of the header above each chain column on Stereo Chain.
const CHAIN_HEADER_H: f32 = 28.0;

/// The two chain columns of Stereo Chain, each `width` wide. Each chain
/// wraps within its own column, so neither runs into the other.
pub(super) struct ChainColumns {
    pub(super) header_y: f32,
    pub(super) pedals_y: f32,
    pub(super) left_x: f32,
    pub(super) right_x: f32,
    pub(super) width: f32,
}

impl<P: Platform, S: CommandSink> DawApp<P, S> {
    /// Render the pedal type picker overlay, a grid of every pedal by
    /// category with a one-line description of each.
    pub(super) fn render_pedal_picker<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        screen_h: f32,
    ) {
        renderer.fill_rect(0.0, 0.0, screen_w, screen_h, Color::rgba(0, 0, 0, 180));

        let p = picker_rect(self.window);
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
            let c = picker_category_rect(self.window, cat);
            renderer.fill_rect(c.x, c.y, c.w, c.h, BG_DARK);
            renderer.draw_text(category, c.x + 8.0, c.y + 4.0, FontSize::Small, accent);
            for (item, pedal_type) in types.iter().enumerate() {
                let r = picker_item_rect(self.window, cat, item);
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
            screen_w - 2.0 * SETUP_MARGIN,
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
        let pos_h = self.positional_stage_h();
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

        let cols = self.stereo_chain_columns(content_y);
        let (chains_y, half_w) = (cols.header_y, cols.width);

        // Left channel chain.
        let left_x = cols.left_x;
        renderer.fill_rect(left_x, chains_y, half_w, CHAIN_HEADER_H, colors::PANEL_BG);
        renderer.fill_rect(left_x, chains_y, half_w, 2.0, accent);
        renderer.draw_text(
            "LEFT CHANNEL",
            left_x + 8.0,
            chains_y + 6.0,
            FontSize::Medium,
            TEXT_PRIMARY,
        );
        let sel = self
            .selected_pedal
            .and_then(|(c, idx)| if c == 1 { Some(idx) } else { None });
        draw_pedal_chain(
            renderer,
            left_x,
            cols.pedals_y,
            half_w,
            &chain.post_pedals_left,
            accent,
            sel,
        );

        // Right channel chain.
        let right_x = cols.right_x;
        renderer.fill_rect(right_x, chains_y, half_w, CHAIN_HEADER_H, colors::PANEL_BG);
        renderer.fill_rect(right_x, chains_y, half_w, 2.0, accent);
        renderer.draw_text(
            "RIGHT CHANNEL",
            right_x + 8.0,
            chains_y + 6.0,
            FontSize::Medium,
            TEXT_PRIMARY,
        );
        let sel = self
            .selected_pedal
            .and_then(|(c, idx)| if c == 2 { Some(idx) } else { None });
        draw_pedal_chain(
            renderer,
            right_x,
            cols.pedals_y,
            half_w,
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

    /// Handle a click on the mono chain page (pedal parameter editing).
    pub(super) fn handle_mono_chain_click(&mut self, mx: f32, my: f32, content_y: f32) {
        let pedals_y = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP + 48.0;
        let width = self.window.width_f32() - 2.0 * SETUP_MARGIN;
        self.handle_pedal_chain_click(mx, my, pedals_y, 0, SETUP_MARGIN, width);
    }

    /// Handle a click on the stereo chain page (positional controls and
    /// pedal parameter editing).
    pub(super) fn handle_stereo_chain_click(&mut self, mx: f32, my: f32, content_y: f32) {
        let screen_w = self.window.width_f32();
        let full_w = screen_w - 2.0 * SETUP_MARGIN;
        let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;

        // Positional stage geometry (must match render_stereo_chain layout).
        let ch = self.selected_channel;
        let is_3d = self.effects[ch].position_3d_enabled;
        let pos_h = self.positional_stage_h();

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
        let cols = self.stereo_chain_columns(content_y);
        if mx < cols.right_x {
            self.handle_pedal_chain_click(mx, my, cols.pedals_y, 1, cols.left_x, cols.width);
        } else {
            self.handle_pedal_chain_click(mx, my, cols.pedals_y, 2, cols.right_x, cols.width);
        }
    }

    /// Handle a click within a pedal chain layout.
    ///
    /// `chain_id`: 0 = pre (mono), 1 = post-left, 2 = post-right.
    /// `start_x`: the left edge x coordinate where this chain's pedals begin.
    /// `width`: the width the chain wraps within, as `chain_slot` lays it out.
    pub(super) fn handle_pedal_chain_click(
        &mut self,
        mx: f32,
        my: f32,
        pedals_y: f32,
        chain_id: u8,
        start_x: f32,
        width: f32,
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

        if chain_add_card(start_x, pedals_y, width, pedals).contains(mx, my) {
            hit = Some(HitKind::Add);
        }
        for (pedal_idx, pedal) in pedals.iter().enumerate() {
            if hit.is_some() {
                break;
            }
            let (px, py) = chain_slot(start_x, pedals_y, width, pedals, pedal_idx);
            if !Rect::new(px, py, PEDAL_W, pedal_h(pedal)).contains(mx, my) {
                continue;
            }
            if pedal_led(px, py).contains(mx, my) {
                hit = Some(HitKind::Button(pedal_idx, PedalButton::Bypass));
                break;
            }
            if let Some((button, _)) = pedal_buttons(px, py, pedal)
                .into_iter()
                .find(|(_, r)| r.contains(mx, my))
            {
                hit = Some(HitKind::Button(pedal_idx, button));
                break;
            }
            let param_start_y = py + PEDAL_HEADER_H;
            for (param_idx, param) in pedal.params.iter().enumerate() {
                let row_y = param_start_y + param_idx as f32 * PARAM_ROW_H;
                if hit_row(mx, my, px, row_y, PEDAL_W) {
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

    /// Find which pedal parameter bar is at the given mouse position.
    pub(super) fn find_pedal_param_at(
        &self,
        mx: f32,
        my: f32,
        content_y: f32,
    ) -> Option<(u8, usize, usize, f64, f64)> {
        let ch = self.selected_channel;
        match self.current_page {
            DawPage::Studio => {
                if mx < studio::BROWSER_W {
                    return None;
                }
                let (start_x, pedals_y) = self.studio_rack_origin();
                let pedals = &self.effects[ch].pre_pedals;
                find_pedal_param_in_chain(mx, my, pedals_y, start_x, f32::INFINITY, pedals, 0)
            }
            DawPage::MonoChain => {
                let pedals_y = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP + 48.0;
                let width = self.window.width_f32() - 2.0 * SETUP_MARGIN;
                let pedals = &self.effects[ch].pre_pedals;
                find_pedal_param_in_chain(mx, my, pedals_y, SETUP_MARGIN, width, pedals, 0)
            }
            DawPage::StereoChain => {
                let cols = self.stereo_chain_columns(content_y);
                let (x, pedals, chain_id) = if mx < cols.right_x {
                    (cols.left_x, &self.effects[ch].post_pedals_left, 1)
                } else {
                    (cols.right_x, &self.effects[ch].post_pedals_right, 2)
                };
                find_pedal_param_in_chain(mx, my, cols.pedals_y, x, cols.width, pedals, chain_id)
            }
            _ => None,
        }
    }

    /// Height of the positional stage panel at the top of Stereo Chain, which
    /// shows more rows in 3D and orbital modes.
    fn positional_stage_h(&self) -> f32 {
        let chain = &self.effects[self.selected_channel];
        let rows = if chain.orbital_enabled {
            7.0
        } else if chain.position_3d_enabled {
            4.0
        } else {
            2.0
        };
        SECTION_HEADER_H + rows * PARAM_ROW_H + 8.0
    }

    /// Where Stereo Chain puts its two chain columns below the positional
    /// stage. Drawing, clicks and parameter lookup all read this. Parameter
    /// lookup once left out the orbital rows, and so in orbital mode looked
    /// for pedal bars above where they were drawn.
    pub(super) fn stereo_chain_columns(&self, content_y: f32) -> ChainColumns {
        let y0 = content_y + NAV_BAR_HEIGHT + SETUP_ROW_GAP;
        let header_y = y0 + self.positional_stage_h() + SETUP_ROW_GAP;
        let width = (self.window.width_f32() - 3.0 * SETUP_MARGIN) / 2.0;
        ChainColumns {
            header_y,
            pedals_y: header_y + CHAIN_HEADER_H + 4.0,
            left_x: SETUP_MARGIN,
            right_x: SETUP_MARGIN + width + SETUP_MARGIN,
            width,
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
        if !picker_rect(self.window).contains(mx, my) {
            self.pedal_picker_open = false;
            return;
        }
        if let Some(pedal_type) = picker_item_at(self.window, mx, my) {
            self.pedal_picker_open = false;
            self.add_pedal_to_chain(self.pedal_picker_chain, pedal_type);
        }
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
