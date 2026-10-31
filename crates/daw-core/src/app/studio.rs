//! The Studio page and the page tab row.
//!
//! The Studio page is the home page, laid out after browser-based
//! workstations. A browser on the left lists instruments and effects. The
//! track list fills the top right, one row per channel with mute, solo,
//! volume and a whole-song note preview. The effects rack of the selected
//! track fills the bottom right. Everything is reachable with the mouse, so
//! no keyboard shortcut needs to be known.
//!
//! Layout is computed by the small geometry functions here, and both
//! drawing and hit testing call them, so the two cannot disagree.

use super::*;

/// Width of the browser panel on the left.
pub(super) const BROWSER_W: f32 = 280.0;
/// Height of the "Tracks" heading above the track list.
const SECTION_H: f32 = 26.0;
const TRACK_ROW_H: f32 = 46.0;
/// Top of the first track row.
const TRACKS_Y: f32 = CONTENT_Y + SECTION_H;
/// Top of the effects rack heading.
const RACK_Y: f32 = TRACKS_Y + 8.0 * TRACK_ROW_H + 8.0;
const RACK_HEADER_H: f32 = 34.0;
/// Left edge of the track list and rack.
const MAIN_X: f32 = BROWSER_W + 10.0;
const BROWSER_TAB_H: f32 = 26.0;
/// Top of the line of help under the browser tabs.
const BROWSER_HINT_Y: f32 = CONTENT_Y + 8.0 + BROWSER_TAB_H + 6.0;
/// Top of the browser list.
const BROWSER_LIST_Y: f32 = BROWSER_HINT_Y + 24.0;
const PRESET_ITEM_H: f32 = 26.0;
const CATEGORY_ITEM_H: f32 = 24.0;
const EFFECT_ITEM_H: f32 = 38.0;
const SCROLL_STEP: f32 = 30.0;
/// Width of one page tab.
const PAGE_TAB_W: f32 = 112.0;
/// Width of one transport button on the page tab row.
const TRANSPORT_W: f32 = 70.0;

/// Which list the browser shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BrowserTab {
    Instruments,
    Effects,
}

/// One row of the browser list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BrowserEntry {
    Preset(usize),
    Category(&'static str),
    Effect(&'static str),
}

/// A button on the page tab row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TransportButton {
    Play,
    Stop,
    Save,
}

/// A clickable element of a track row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrackPart {
    Mute,
    Solo,
    Volume,
    Notes,
    Row,
}

/// Tab `i` of the page tab row.
fn page_tab_rect(i: usize) -> Rect {
    Rect::new(
        8.0 + i as f32 * (PAGE_TAB_W + 4.0),
        TOOLBAR_HEIGHT + 3.0,
        PAGE_TAB_W,
        PAGE_TAB_HEIGHT - 6.0,
    )
}

/// The transport buttons at the right of the page tab row.
fn transport_buttons(window: WindowSize) -> [(TransportButton, Rect); 3] {
    let right = window.width_f32() - 8.0;
    let rect = |k: f32| {
        Rect::new(
            right - k * (TRANSPORT_W + 6.0),
            TOOLBAR_HEIGHT + 3.0,
            TRANSPORT_W,
            PAGE_TAB_HEIGHT - 6.0,
        )
    };
    [
        (TransportButton::Play, rect(3.0)),
        (TransportButton::Stop, rect(2.0)),
        (TransportButton::Save, rect(1.0)),
    ]
}

fn browser_tab_rect(tab: BrowserTab) -> Rect {
    let x = match tab {
        BrowserTab::Instruments => 8.0,
        BrowserTab::Effects => 8.0 + (BROWSER_W - 16.0) / 2.0,
    };
    Rect::new(
        x,
        CONTENT_Y + 8.0,
        (BROWSER_W - 16.0) / 2.0 - 2.0,
        BROWSER_TAB_H,
    )
}

/// The rows of the browser list and their heights, unscrolled.
fn browser_entries(tab: BrowserTab) -> Vec<(BrowserEntry, f32)> {
    match tab {
        BrowserTab::Instruments => (0..presets::preset_names().len())
            .map(|i| (BrowserEntry::Preset(i), PRESET_ITEM_H))
            .collect(),
        BrowserTab::Effects => PEDAL_TYPES
            .iter()
            .flat_map(|(category, types)| {
                core::iter::once((BrowserEntry::Category(category), CATEGORY_ITEM_H)).chain(
                    types
                        .iter()
                        .map(|t| (BrowserEntry::Effect(t), EFFECT_ITEM_H)),
                )
            })
            .collect(),
    }
}

/// The largest browser scroll offset for `tab`.
fn browser_max_scroll(tab: BrowserTab, window: WindowSize) -> f32 {
    let total: f32 = browser_entries(tab).iter().map(|(_, h)| h).sum();
    let visible = window.height_f32() - BROWSER_LIST_Y;
    (total - visible).max(0.0)
}

/// Top of track row `ch`.
fn track_row_y(ch: usize) -> f32 {
    TRACKS_Y + ch as f32 * TRACK_ROW_H
}

/// The clickable parts of track row `ch`.
fn track_parts(ch: usize) -> [(TrackPart, Rect); 4] {
    let y = track_row_y(ch);
    [
        (
            TrackPart::Mute,
            Rect::new(MAIN_X + 200.0, y + 12.0, 26.0, 22.0),
        ),
        (
            TrackPart::Solo,
            Rect::new(MAIN_X + 230.0, y + 12.0, 26.0, 22.0),
        ),
        (
            TrackPart::Volume,
            Rect::new(MAIN_X + 300.0, y + 6.0, VOLUME_W, TRACK_ROW_H - 12.0),
        ),
        (
            TrackPart::Notes,
            Rect::new(MAIN_X + 440.0, y + 12.0, 60.0, 22.0),
        ),
    ]
}

/// Width of the volume slider.
const VOLUME_W: f32 = 120.0;
/// Left edge of the note preview in a track row.
const PREVIEW_X: f32 = MAIN_X + 512.0;

/// Buttons on the rack heading that open the detailed pages.
fn rack_buttons(window: WindowSize) -> [(DawPage, &'static str, Rect); 2] {
    let right = window.width_f32() - 10.0;
    [
        (
            DawPage::SynthSetup,
            "Synth settings",
            Rect::new(right - 270.0, RACK_Y + 5.0, 128.0, 24.0),
        ),
        (
            DawPage::StereoChain,
            "Stereo effects",
            Rect::new(right - 136.0, RACK_Y + 5.0, 128.0, 24.0),
        ),
    ]
}

/// Draw a flat button with a centred-ish label.
fn draw_button<R: Renderer>(renderer: &mut R, r: Rect, label: &str, active: Option<Color>) {
    let fill = match active {
        Some(c) => Color::rgba(c.r, c.g, c.b, 90),
        None => colors::NAV_BUTTON,
    };
    renderer.fill_rect(r.x, r.y, r.w, r.h, fill);
    renderer.draw_rect(r.x, r.y, r.w, r.h, active.unwrap_or(colors::PANEL_BORDER));
    renderer.draw_text(label, r.x + 8.0, r.y + 3.0, FontSize::Small, TEXT_PRIMARY);
}

impl<P: Platform, S: CommandSink> DawApp<P, S> {
    /// Select a channel. The pedal selection belongs to the old channel's
    /// chains, so it is cleared with the note selection.
    pub(super) fn select_channel(&mut self, ch: usize) {
        if ch < NUM_CHANNELS {
            self.selected_channel = ch;
            self.selected_pedal = None;
            self.rack_scroll = 0.0;
            self.clear_note_selection();
        }
    }

    /// Apply preset `idx` of `presets::preset_names()` to the selected channel.
    pub(super) fn select_preset(&mut self, idx: usize) {
        let Some(&name) = presets::preset_names().get(idx) else {
            return;
        };
        let ch = self.selected_channel;
        if let Some(patch) = presets::preset(name) {
            self.patches[ch] = patch;
            self.preset_indices[ch] = Some(idx);
            self.project.channels[ch].name = String::from(name);
            self.send_patch(ch);
            self.status_message = Some(format!("Ch{}: {}", ch + 1, name));
        }
    }

    /// Left edge and top of the pedals in the Studio rack.
    pub(super) fn studio_rack_origin(&self) -> (f32, f32) {
        (MAIN_X - self.rack_scroll, RACK_Y + RACK_HEADER_H + 4.0)
    }

    fn rack_max_scroll(&self) -> f32 {
        let count = self.effects[self.selected_channel].pre_pedals.len();
        let content = pedal_x(0.0, count) + ADD_CARD_W;
        let visible = self.window.width_f32() - MAIN_X - 10.0;
        (content - visible).max(0.0)
    }

    // --- Page tab row ---

    pub(super) fn render_page_tabs<R: Renderer>(&self, renderer: &mut R, screen_w: f32) {
        renderer.fill_rect(
            0.0,
            TOOLBAR_HEIGHT,
            screen_w,
            PAGE_TAB_HEIGHT,
            Color::rgb(34, 34, 40),
        );
        for (i, page) in DawPage::ALL.iter().enumerate() {
            let active = (*page == self.current_page).then_some(TEXT_ACCENT);
            draw_button(renderer, page_tab_rect(i), page.name(), active);
        }
        for (button, r) in transport_buttons(self.window) {
            let (label, active) = match button {
                TransportButton::Play if self.transport.playing => ("Pause", Some(LOOP_MARKER)),
                TransportButton::Play => ("Play", None),
                TransportButton::Stop => ("Stop", None),
                TransportButton::Save => ("Save", None),
            };
            draw_button(renderer, r, label, active);
        }
    }

    pub(super) fn handle_page_tab_click(&mut self, mx: f32) {
        let my = TOOLBAR_HEIGHT + PAGE_TAB_HEIGHT / 2.0;
        if let Some(i) = (0..DawPage::ALL.len()).find(|&i| page_tab_rect(i).contains(mx, my)) {
            self.current_page = DawPage::ALL[i];
            return;
        }
        if let Some((button, _)) = transport_buttons(self.window)
            .into_iter()
            .find(|(_, r)| r.contains(mx, my))
        {
            match button {
                TransportButton::Play => self.handle_input(DawInput::TogglePlay),
                TransportButton::Stop => self.handle_input(DawInput::Stop),
                TransportButton::Save => self.save(),
            }
        }
    }

    // --- Studio page ---

    pub(super) fn render_studio<R: Renderer>(
        &self,
        renderer: &mut R,
        screen_w: f32,
        content_y: f32,
        content_h: f32,
    ) {
        renderer.fill_rect(0.0, content_y, screen_w, content_h, colors::SECTION_BG);
        self.render_studio_tracks(renderer, screen_w);
        self.render_studio_rack(renderer, screen_w);
        // The browser is drawn last, so that a rack scrolled left slides
        // under it rather than over it.
        self.render_studio_browser(renderer, content_y, content_h);
    }

    fn render_studio_browser<R: Renderer>(&self, renderer: &mut R, content_y: f32, content_h: f32) {
        renderer.fill_rect(0.0, content_y, BROWSER_W, content_h, BG_PANEL);
        renderer.draw_vline(
            BROWSER_W,
            content_y,
            content_y + content_h,
            colors::PANEL_BORDER,
        );
        for (tab, label) in [
            (BrowserTab::Instruments, "Instruments"),
            (BrowserTab::Effects, "Effects"),
        ] {
            let active = (tab == self.browser_tab).then_some(TEXT_ACCENT);
            draw_button(renderer, browser_tab_rect(tab), label, active);
        }

        let accent = CHANNEL_COLORS[self.selected_channel];
        let current_preset = self.preset_indices[self.selected_channel];
        let names = presets::preset_names();
        let bottom = content_y + content_h;
        let mut y = BROWSER_LIST_Y - self.browser_scroll;
        for (entry, h) in browser_entries(self.browser_tab) {
            if y + h > BROWSER_LIST_Y && y < bottom {
                match entry {
                    BrowserEntry::Preset(i) => {
                        if current_preset == Some(i) {
                            renderer.fill_rect(
                                6.0,
                                y,
                                BROWSER_W - 12.0,
                                h - 2.0,
                                Color::rgba(accent.r, accent.g, accent.b, 70),
                            );
                        }
                        let name = names.get(i).copied().unwrap_or("");
                        renderer.draw_text(name, 14.0, y + 5.0, FontSize::Small, TEXT_PRIMARY);
                    }
                    BrowserEntry::Category(name) => {
                        renderer.draw_text(name, 10.0, y + 5.0, FontSize::Small, accent);
                    }
                    BrowserEntry::Effect(name) => {
                        renderer.fill_rect(10.0, y, BROWSER_W - 20.0, h - 4.0, colors::NAV_BUTTON);
                        renderer.draw_text(
                            &pedal_display_name(name),
                            18.0,
                            y + 2.0,
                            FontSize::Small,
                            TEXT_PRIMARY,
                        );
                        if let Some(text) = pedal_description(name) {
                            renderer.draw_text(
                                text,
                                18.0,
                                y + 18.0,
                                FontSize::Small,
                                TEXT_SECONDARY,
                            );
                        }
                    }
                }
            }
            y += h;
        }
        // Repaint the tab strip over any list row that scrolled under it.
        renderer.fill_rect(
            0.0,
            content_y,
            BROWSER_W,
            BROWSER_LIST_Y - content_y - 4.0,
            BG_PANEL,
        );
        for (tab, label) in [
            (BrowserTab::Instruments, "Instruments"),
            (BrowserTab::Effects, "Effects"),
        ] {
            let active = (tab == self.browser_tab).then_some(TEXT_ACCENT);
            draw_button(renderer, browser_tab_rect(tab), label, active);
        }
        let hint = match self.browser_tab {
            BrowserTab::Instruments => "Click to play it on this track",
            BrowserTab::Effects => "Click to add it to this track",
        };
        renderer.draw_text(hint, 10.0, BROWSER_HINT_Y, FontSize::Small, TEXT_SECONDARY);
    }

    fn render_studio_tracks<R: Renderer>(&self, renderer: &mut R, screen_w: f32) {
        renderer.draw_text(
            "Tracks",
            MAIN_X,
            CONTENT_Y + 5.0,
            FontSize::Medium,
            TEXT_PRIMARY,
        );

        // One time scale for every row, long enough for the longest track.
        let ticks_per_bar = self.transport.ticks_per_bar().max(1);
        let song_ticks = self
            .tracks
            .iter()
            .flat_map(|t| t.notes.iter().map(|n| n.tick + n.duration))
            .max()
            .unwrap_or(0)
            .max(4 * ticks_per_bar);
        let preview_w = (screen_w - 10.0 - PREVIEW_X).max(1.0);
        let px_per_tick = preview_w / song_ticks as f32;

        for (ch, &color) in CHANNEL_COLORS.iter().enumerate() {
            let y = track_row_y(ch);
            let selected = ch == self.selected_channel;
            let bg = if selected {
                Color::rgba(color.r, color.g, color.b, 40)
            } else {
                BG_TRACK
            };
            renderer.fill_rect(MAIN_X, y, screen_w - MAIN_X - 10.0, TRACK_ROW_H - 3.0, bg);
            if selected {
                renderer.draw_rect(
                    MAIN_X,
                    y,
                    screen_w - MAIN_X - 10.0,
                    TRACK_ROW_H - 3.0,
                    color,
                );
            }
            renderer.fill_rect(MAIN_X + 4.0, y + 4.0, 6.0, TRACK_ROW_H - 11.0, color);

            let conf = &self.project.channels[ch];
            let name = if conf.name.is_empty() {
                format!("Track {}", ch + 1)
            } else {
                format!("{}. {}", ch + 1, conf.name)
            };
            renderer.draw_text(&name, MAIN_X + 16.0, y + 5.0, FontSize::Small, color);
            let count = self.tracks[ch].notes.len();
            renderer.draw_text(
                &format!("{count} notes"),
                MAIN_X + 16.0,
                y + 23.0,
                FontSize::Small,
                TEXT_SECONDARY,
            );

            for (part, r) in track_parts(ch) {
                match part {
                    TrackPart::Mute => {
                        let on = conf.muted.then_some(Color::rgb(255, 80, 80));
                        draw_button(renderer, r, "M", on);
                    }
                    TrackPart::Solo => {
                        let on =
                            (self.solo_channel == Some(ch)).then_some(Color::rgb(255, 200, 80));
                        draw_button(renderer, r, "S", on);
                    }
                    TrackPart::Volume => {
                        renderer.draw_text(
                            "Vol",
                            r.x - 30.0,
                            y + 14.0,
                            FontSize::Small,
                            TEXT_SECONDARY,
                        );
                        let bar_y = r.y + r.h / 2.0 - 3.0;
                        renderer.fill_rect(r.x, bar_y, r.w, 6.0, colors::KNOB_TRACK);
                        let frac = conf.volume.clamp(0.0, 1.0) as f32;
                        renderer.fill_rect(r.x, bar_y, r.w * frac, 6.0, color);
                        renderer.fill_rect(
                            r.x + r.w * frac - 2.0,
                            bar_y - 4.0,
                            4.0,
                            14.0,
                            TEXT_PRIMARY,
                        );
                    }
                    TrackPart::Notes => draw_button(renderer, r, "Notes", None),
                    TrackPart::Row => {}
                }
            }

            // Whole-song note preview.
            let top = y + 4.0;
            let h = TRACK_ROW_H - 11.0;
            renderer.fill_rect(PREVIEW_X, top, preview_w, h, BG_DARK);
            for note in &self.tracks[ch].notes {
                let nx = PREVIEW_X + note.tick as f32 * px_per_tick;
                let nw = (note.duration as f32 * px_per_tick).max(1.0);
                let ny = top + h - 2.0 - (note.pitch as f32 / 127.0) * (h - 4.0);
                renderer.fill_rect(nx, ny - 1.0, nw, 2.0, color);
            }
            let play_x = PREVIEW_X + self.transport.position_ticks as f32 * px_per_tick;
            if play_x <= PREVIEW_X + preview_w {
                renderer.fill_rect(play_x, top, 1.0, h, PLAYHEAD);
            }
        }
    }

    fn render_studio_rack<R: Renderer>(&self, renderer: &mut R, screen_w: f32) {
        let ch = self.selected_channel;
        let accent = CHANNEL_COLORS[ch];
        renderer.fill_rect(
            MAIN_X,
            RACK_Y,
            screen_w - MAIN_X - 10.0,
            RACK_HEADER_H,
            colors::PANEL_BG,
        );
        renderer.fill_rect(MAIN_X, RACK_Y, screen_w - MAIN_X - 10.0, 2.0, accent);
        let name = &self.project.channels[ch].name;
        let title = format!(
            "Track {} effects{}{}   (sound flows left to right)",
            ch + 1,
            if name.is_empty() { "" } else { ": " },
            name
        );
        renderer.draw_text(
            &title,
            MAIN_X + 8.0,
            RACK_Y + 8.0,
            FontSize::Medium,
            TEXT_PRIMARY,
        );
        for (_, label, r) in rack_buttons(self.window) {
            draw_button(renderer, r, label, None);
        }

        let (start_x, pedals_y) = self.studio_rack_origin();
        let sel = self
            .selected_pedal
            .and_then(|(c, idx)| (c == 0).then_some(idx));
        draw_pedal_chain(
            renderer,
            start_x,
            pedals_y,
            // The rack scrolls sideways, so its chain never wraps.
            f32::INFINITY,
            &self.effects[ch].pre_pedals,
            accent,
            sel,
        );
    }

    pub(super) fn handle_studio_click(&mut self, mx: f32, my: f32) {
        if mx < BROWSER_W {
            self.handle_browser_click(mx, my);
            return;
        }
        if my >= RACK_Y {
            if let Some((page, _, _)) = rack_buttons(self.window)
                .into_iter()
                .find(|(_, _, r)| r.contains(mx, my))
            {
                self.current_page = page;
                return;
            }
            if my >= RACK_Y + RACK_HEADER_H {
                let (start_x, pedals_y) = self.studio_rack_origin();
                self.handle_pedal_chain_click(mx, my, pedals_y, 0, start_x, f32::INFINITY);
            }
            return;
        }
        if my < TRACKS_Y {
            return;
        }
        let ch = ((my - TRACKS_Y) / TRACK_ROW_H) as usize;
        if ch >= NUM_CHANNELS {
            return;
        }
        let part = track_parts(ch)
            .into_iter()
            .find(|(_, r)| r.contains(mx, my))
            .map_or(TrackPart::Row, |(p, _)| p);
        self.select_channel(ch);
        match part {
            TrackPart::Mute => {
                self.project.channels[ch].muted = !self.project.channels[ch].muted;
                self.sync_engine_channels();
            }
            TrackPart::Solo => self.toggle_solo(),
            TrackPart::Volume => {
                self.dragging_volume = Some(ch);
                self.studio_drag_volume(ch, mx);
            }
            TrackPart::Notes => self.current_page = DawPage::PianoRoll,
            TrackPart::Row => {}
        }
    }

    fn handle_browser_click(&mut self, mx: f32, my: f32) {
        for tab in [BrowserTab::Instruments, BrowserTab::Effects] {
            if browser_tab_rect(tab).contains(mx, my) {
                if self.browser_tab != tab {
                    self.browser_tab = tab;
                    self.browser_scroll = 0.0;
                }
                return;
            }
        }
        if my < BROWSER_LIST_Y {
            return;
        }
        let mut y = BROWSER_LIST_Y - self.browser_scroll;
        for (entry, h) in browser_entries(self.browser_tab) {
            if my >= y && my < y + h {
                match entry {
                    BrowserEntry::Preset(i) => self.select_preset(i),
                    BrowserEntry::Effect(name) => self.add_pedal_to_chain(0, name),
                    BrowserEntry::Category(_) => {}
                }
                return;
            }
            y += h;
        }
    }

    /// Set the volume of `ch` from a mouse x over its slider.
    pub(super) fn studio_drag_volume(&mut self, ch: usize, mx: f32) {
        let Some(&(_, r)) = track_parts(ch)
            .iter()
            .find(|(p, _)| *p == TrackPart::Volume)
        else {
            return;
        };
        let volume = ((mx - r.x) / r.w).clamp(0.0, 1.0) as f64;
        self.project.channels[ch].volume = volume;
        self.sync_engine_channels();
        self.status_message = Some(format!("Track {} volume {:.0}%", ch + 1, volume * 100.0));
    }

    /// Mouse wheel on the Studio page. Over a pedal parameter it adjusts
    /// the parameter, over the browser or the rack it scrolls them, and over
    /// a volume slider it nudges the volume.
    pub(super) fn studio_scroll(&mut self, delta: i32) {
        let (mx, my) = self.last_mouse;
        if mx < BROWSER_W {
            let max = browser_max_scroll(self.browser_tab, self.window);
            self.browser_scroll =
                (self.browser_scroll - delta as f32 * SCROLL_STEP).clamp(0.0, max);
        } else if my >= RACK_Y {
            if self.find_pedal_param_at(mx, my, CONTENT_Y).is_some() {
                self.scroll_param(delta);
            } else {
                let max = self.rack_max_scroll();
                self.rack_scroll = (self.rack_scroll - delta as f32 * SCROLL_STEP).clamp(0.0, max);
            }
        } else if my >= TRACKS_Y {
            let ch = ((my - TRACKS_Y) / TRACK_ROW_H) as usize;
            if ch < NUM_CHANNELS {
                let volume =
                    (self.project.channels[ch].volume + delta as f64 * 0.05).clamp(0.0, 1.0);
                self.project.channels[ch].volume = volume;
                self.sync_engine_channels();
                self.status_message =
                    Some(format!("Track {} volume {:.0}%", ch + 1, volume * 100.0));
            }
        }
    }

    /// Clamp the browser and rack scroll offsets to their current ranges,
    /// which shrink when the window grows.
    pub(super) fn clamp_studio_scroll(&mut self) {
        let max = browser_max_scroll(self.browser_tab, self.window);
        self.browser_scroll = self.browser_scroll.clamp(0.0, max);
        self.rack_scroll = self.rack_scroll.clamp(0.0, self.rack_max_scroll());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::MemoryPlatform;
    use audio::NullSink;

    fn app() -> DawApp<MemoryPlatform, NullSink> {
        DawApp::new(MemoryPlatform::new(), NullSink, 48_000)
    }

    fn click(app: &mut DawApp<MemoryPlatform, NullSink>, r: Rect) {
        let (x, y) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        app.handle_input(DawInput::MouseMove(x, y));
        app.handle_input(DawInput::Click(x, y));
        app.handle_input(DawInput::MouseUp(x, y));
    }

    /// Centre of browser entry `target`, scrolling the browser to it first.
    fn browser_entry_rect(
        app: &mut DawApp<MemoryPlatform, NullSink>,
        target: BrowserEntry,
    ) -> Rect {
        let mut y = BROWSER_LIST_Y;
        for (entry, h) in browser_entries(app.browser_tab) {
            if entry == target {
                let scroll =
                    (y - BROWSER_LIST_Y).min(browser_max_scroll(app.browser_tab, app.window));
                app.browser_scroll = scroll;
                return Rect::new(20.0, y - scroll, 100.0, h - 4.0);
            }
            y += h;
        }
        panic!("{target:?} is not in the browser");
    }

    #[test]
    fn studio_is_the_first_page() {
        let app = app();
        assert_eq!(app.current_page, DawPage::Studio);
        assert_eq!(DawPage::ALL[0], DawPage::Studio);
    }

    #[test]
    fn page_tabs_reach_every_page() {
        let mut app = app();
        for (i, page) in DawPage::ALL.iter().enumerate() {
            click(&mut app, page_tab_rect(i));
            assert_eq!(app.current_page, *page);
        }
    }

    #[test]
    fn next_and_prev_visit_every_page_once() {
        let mut page = DawPage::Studio;
        for _ in 0..DawPage::ALL.len() {
            assert_eq!(page.next().prev(), page);
            page = page.next();
        }
        assert_eq!(page, DawPage::Studio);
    }

    #[test]
    fn play_and_stop_buttons_drive_the_transport() {
        let mut app = app();
        let [(_, play), (_, stop), _] = transport_buttons(app.window);
        click(&mut app, play);
        assert!(app.transport.playing);
        click(&mut app, play);
        assert!(!app.transport.playing);
        click(&mut app, play);
        click(&mut app, stop);
        assert!(!app.transport.playing);
        assert_eq!(app.transport.position_ticks, 0);
    }

    #[test]
    fn every_effect_in_the_browser_adds_its_pedal() {
        let mut app = app();
        click(&mut app, browser_tab_rect(BrowserTab::Effects));
        assert_eq!(app.browser_tab, BrowserTab::Effects);
        for (_, types) in PEDAL_TYPES {
            for name in *types {
                let before = app.effects[0].pre_pedals.len();
                let r = browser_entry_rect(&mut app, BrowserEntry::Effect(name));
                click(&mut app, r);
                let chain = &app.effects[0].pre_pedals;
                assert_eq!(chain.len(), before + 1, "{name}");
                assert_eq!(chain.last().unwrap().pedal_type, *name);
            }
        }
    }

    #[test]
    fn instrument_click_applies_the_preset() {
        let mut app = app();
        app.handle_input(DawInput::SelectChannel(2));
        let names = presets::preset_names();
        let idx = names.len() - 1;
        let r = browser_entry_rect(&mut app, BrowserEntry::Preset(idx));
        click(&mut app, r);
        assert_eq!(app.preset_indices[2], Some(idx));
        assert_eq!(app.project.channels[2].name, names[idx]);
    }

    #[test]
    fn rack_buttons_edit_the_chain() {
        let mut app = app();
        app.add_pedal_to_chain(0, "Overdrive");
        app.add_pedal_to_chain(0, "Bitcrusher");
        let (start_x, pedals_y) = app.studio_rack_origin();

        // Bypass the second pedal with its footer button.
        let second = &app.effects[0].pre_pedals[1];
        let bypass = pedal_buttons(pedal_x(start_x, 1), pedals_y, second)[2].1;
        click(&mut app, bypass);
        assert!(app.effects[0].pre_pedals[1].bypassed);

        // Move it left.
        let second = &app.effects[0].pre_pedals[1];
        let left = pedal_buttons(pedal_x(start_x, 1), pedals_y, second)[0].1;
        click(&mut app, left);
        assert_eq!(app.effects[0].pre_pedals[0].pedal_type, "Bitcrusher");

        // Remove the first.
        let first = &app.effects[0].pre_pedals[0];
        let remove = pedal_buttons(pedal_x(start_x, 0), pedals_y, first)[3].1;
        click(&mut app, remove);
        let types: Vec<_> = app.effects[0]
            .pre_pedals
            .iter()
            .map(|p| p.pedal_type.as_str())
            .collect();
        assert_eq!(types, ["Overdrive"]);
    }

    #[test]
    fn add_card_opens_the_picker_and_the_picker_adds() {
        let mut app = app();
        let (start_x, pedals_y) = app.studio_rack_origin();
        click(
            &mut app,
            chain_add_card(start_x, pedals_y, f32::INFINITY, &[]),
        );
        assert!(app.pedal_picker_open);
        assert_eq!(app.pedal_picker_chain, 0);
        let (cat, item) = PEDAL_TYPES
            .iter()
            .enumerate()
            .find_map(|(c, (_, t))| {
                t.iter()
                    .position(|n| *n == "PitchCorrector")
                    .map(|i| (c, i))
            })
            .unwrap();
        let item = picker_item_rect(app.window, cat, item);
        click(&mut app, item);
        assert!(!app.pedal_picker_open);
        assert_eq!(app.effects[0].pre_pedals[0].pedal_type, "PitchCorrector");
    }

    #[test]
    fn picker_click_outside_closes_without_adding() {
        let mut app = app();
        app.pedal_picker_open = true;
        click(&mut app, Rect::new(2.0, 2.0, 2.0, 2.0));
        assert!(!app.pedal_picker_open);
        assert!(app.effects[0].pre_pedals.is_empty());
        // The click did not reach the toolbar under the overlay either.
        assert!(!app.wants_text_input());
    }

    #[test]
    fn every_picker_item_is_inside_the_picker_and_distinct() {
        for window in [WindowSize::MIN, WindowSize::DEFAULT, WindowSize::MAX] {
            let p = picker_rect(window);
            let mut seen = Vec::new();
            for (cat, (_, types)) in PEDAL_TYPES.iter().enumerate() {
                for (item, name) in types.iter().enumerate() {
                    let r = picker_item_rect(window, cat, item);
                    assert!(
                        p.contains(r.x, r.y) && p.contains(r.x + r.w, r.y + r.h),
                        "{name}"
                    );
                    assert_eq!(picker_item_at(window, r.x + 1.0, r.y + 1.0), Some(*name));
                    seen.push(*name);
                }
            }
            let total: usize = PEDAL_TYPES.iter().map(|(_, t)| t.len()).sum();
            assert_eq!(seen.len(), total);
        }
    }

    #[test]
    fn mute_solo_and_volume_controls() {
        let mut app = app();
        let [(_, mute), (_, solo), (_, volume), (_, notes)] = track_parts(3);
        click(&mut app, mute);
        assert!(app.project.channels[3].muted);
        assert_eq!(app.selected_channel, 3);
        click(&mut app, solo);
        assert_eq!(app.solo_channel, Some(3));

        // Drag the volume slider from its left end to three quarters.
        let y = volume.y + volume.h / 2.0;
        app.handle_input(DawInput::Click(volume.x, y));
        assert_eq!(app.project.channels[3].volume, 0.0);
        app.handle_input(DawInput::MouseMove(volume.x + volume.w * 0.75, y));
        assert!((app.project.channels[3].volume - 0.75).abs() < 1e-6);
        app.handle_input(DawInput::MouseUp(volume.x + volume.w * 0.75, y));
        // After release, movement no longer changes it.
        app.handle_input(DawInput::MouseMove(volume.x + volume.w, y));
        assert!((app.project.channels[3].volume - 0.75).abs() < 1e-6);

        click(&mut app, notes);
        assert_eq!(app.current_page, DawPage::PianoRoll);
    }

    #[test]
    fn switching_track_clears_the_pedal_selection() {
        let mut app = app();
        app.add_pedal_to_chain(0, "Fuzz");
        app.add_pedal_to_chain(0, "Chorus");
        assert_eq!(app.selected_pedal, Some((0, 1)));
        app.handle_input(DawInput::SelectChannel(1));
        assert_eq!(app.selected_pedal, None);
        // Moving with no selection is a no-op, not a panic.
        app.handle_input(DawInput::ShiftLeft);
        app.handle_input(DawInput::ShiftRight);
    }

    #[test]
    fn browser_and_rack_scroll_stay_in_range() {
        let mut app = app();
        app.handle_input(DawInput::MouseMove(20.0, 400.0));
        for _ in 0..200 {
            app.handle_input(DawInput::Scroll(-1));
        }
        assert_eq!(
            app.browser_scroll,
            browser_max_scroll(BrowserTab::Instruments, app.window)
        );
        for _ in 0..200 {
            app.handle_input(DawInput::Scroll(1));
        }
        assert_eq!(app.browser_scroll, 0.0);

        for _ in 0..12 {
            app.add_pedal_to_chain(0, "Volume");
        }
        app.handle_input(DawInput::MouseMove(app.window.width_f32() - 20.0, 880.0));
        for _ in 0..500 {
            app.handle_input(DawInput::Scroll(-1));
        }
        assert!(app.rack_scroll > 0.0);
        assert_eq!(app.rack_scroll, app.rack_max_scroll());
    }

    #[test]
    fn growing_the_window_clamps_scroll_offsets() {
        let mut state = app();
        state.set_window_size(WindowSize::MIN);
        for _ in 0..12 {
            state.add_pedal_to_chain(0, "Chorus");
        }
        state.scroll_y = state.max_scroll_y();
        state.browser_scroll = browser_max_scroll(state.browser_tab, state.window);
        state.rack_scroll = state.rack_max_scroll();
        assert!(state.scroll_y > 0.0 && state.browser_scroll > 0.0 && state.rack_scroll > 0.0);

        state.set_window_size(WindowSize::MAX);
        assert_eq!(state.scroll_y, state.max_scroll_y());
        assert_eq!(
            state.browser_scroll,
            browser_max_scroll(state.browser_tab, state.window)
        );
        assert_eq!(state.rack_scroll, state.rack_max_scroll());

        // Shrinking keeps offsets that are still in range.
        state.set_window_size(WindowSize::MIN);
        assert!(state.scroll_y <= state.max_scroll_y());
    }

    #[test]
    fn transport_buttons_follow_the_right_edge() {
        let margin = |size: WindowSize| {
            let [_, _, (_, save)] = transport_buttons(size);
            size.width_f32() - (save.x + save.w)
        };
        let default_margin = margin(WindowSize::DEFAULT);
        assert!(default_margin > 0.0);
        for size in [WindowSize::MIN, WindowSize::DEFAULT, WindowSize::MAX] {
            let mut app = app();
            app.set_window_size(size);
            assert_eq!(margin(size), default_margin, "{size:?}");
            let [(_, play), _, _] = transport_buttons(size);
            click(&mut app, play);
            assert!(app.transport.playing, "{size:?}");
        }
    }
}
