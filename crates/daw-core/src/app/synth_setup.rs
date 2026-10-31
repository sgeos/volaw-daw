//! The Synth Setup page, which edits the synthesizer patch of the selected
//! channel, and the synthesizer parameter mapping behind it.

use super::*;

impl<P: Platform, S: CommandSink> DawApp<P, S> {
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
        let row3_y = synth_row3_y(row2_y);

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

    /// Handle a click on the synth setup page.
    ///
    /// Tests all parameter bar regions and enum label regions. On a bar hit,
    /// starts a drag. On an enum label hit, cycles to the next value.
    pub(super) fn handle_synth_setup_click(&mut self, mx: f32, my: f32, content_y: f32) {
        let screen_w = self.window.width_f32();
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
        let row3_y = synth_row3_y(row2_y);

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

    /// Find which synth parameter bar is at the given mouse position.
    ///
    /// Returns (SynthParam, min_val, max_val) if the cursor is over a bar.
    pub(super) fn find_synth_param_at(
        &self,
        mx: f32,
        my: f32,
        content_y: f32,
    ) -> Option<(SynthParam, f64, f64)> {
        let screen_w = self.window.width_f32();
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
        let row3_y = synth_row3_y(row2_y);

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
}

/// Top of row 3 of the Synth Setup page, below row 2 at `row2_y`.
///
/// Column 3 of row 2 stacks the HPF and LPF panels, which together are taller
/// than the envelope panels beside them, so row 3 starts below the taller of
/// the two. Row 3 once started below the envelope panels alone, and the GLIDE
/// panel covered the LPF Resonance and Saturation rows, which also made a
/// click on the GLIDE Enabled row drag LPF Saturation instead. Drawing and
/// both hit tests read this one figure, so they cannot disagree again.
fn synth_row3_y(row2_y: f32) -> f32 {
    let eg_h = SECTION_HEADER_H + 4.0 * PARAM_ROW_H + 8.0;
    let hpf_h = SECTION_HEADER_H + 2.0 * PARAM_ROW_H + 8.0;
    let lpf_h = SECTION_HEADER_H + 3.0 * PARAM_ROW_H + 8.0;
    let column3_h = hpf_h + SETUP_ROW_GAP + lpf_h;
    row2_y + eg_h.max(column3_h) + SETUP_ROW_GAP
}
