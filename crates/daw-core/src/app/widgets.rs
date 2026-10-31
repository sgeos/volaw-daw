//! Stateless drawing and parameter-mapping helpers shared by the pages.

use super::*;

// --- Setup page rendering helpers (standalone functions) ---

/// Draw a section panel with a colored accent bar and title.
pub(super) fn draw_section_panel<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    title: &str,
    accent: Color,
) {
    renderer.fill_rect(x, y, w, h, colors::PANEL_BG);
    renderer.fill_rect(x, y, w, 2.0, accent);
    renderer.draw_rect(x, y, w, h, colors::PANEL_BORDER);
    renderer.draw_text(title, x + 8.0, y + 5.0, FontSize::Small, TEXT_PRIMARY);
}

/// Return a human-readable label for a grid subdivision.
/// Supports straight, triplet (T), and dotted (D) variants.
pub(super) fn grid_label(snap_ticks: u64, ticks_per_beat: u64, ticks_per_bar: u64) -> &'static str {
    // Straight values.
    if snap_ticks == ticks_per_bar {
        return "1/1";
    }
    if snap_ticks == ticks_per_beat * 2 {
        return "1/2";
    }
    if snap_ticks == ticks_per_beat {
        return "1/4";
    }
    if snap_ticks == ticks_per_beat / 2 {
        return "1/8";
    }
    if snap_ticks == ticks_per_beat / 4 {
        return "1/16";
    }
    if snap_ticks == ticks_per_beat / 8 {
        return "1/32";
    }
    // Triplet values (base * 2/3).
    if snap_ticks == ticks_per_bar * 2 / 3 {
        return "1/1T";
    }
    if snap_ticks == ticks_per_beat * 2 * 2 / 3 {
        return "1/2T";
    }
    if snap_ticks == ticks_per_beat * 2 / 3 {
        return "1/4T";
    }
    if snap_ticks == ticks_per_beat / 2 * 2 / 3 {
        return "1/8T";
    }
    if snap_ticks == ticks_per_beat / 4 * 2 / 3 {
        return "1/16T";
    }
    if snap_ticks == ticks_per_beat / 8 * 2 / 3 {
        return "1/32T";
    }
    // Dotted values (base * 3/2).
    if snap_ticks == ticks_per_bar * 3 / 2 {
        return "1/1D";
    }
    if snap_ticks == ticks_per_beat * 2 * 3 / 2 {
        return "1/2D";
    }
    if snap_ticks == ticks_per_beat * 3 / 2 {
        return "1/4D";
    }
    if snap_ticks == ticks_per_beat / 2 * 3 / 2 {
        return "1/8D";
    }
    if snap_ticks == ticks_per_beat / 4 * 3 / 2 {
        return "1/16D";
    }
    if snap_ticks == ticks_per_beat / 8 * 3 / 2 {
        return "1/32D";
    }
    "Custom"
}

pub(super) fn is_log_label(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower == "cutoff"
        || lower.ends_with("_freq")
        || lower.ends_with("_hz")
        || lower == "frequency_hz"
        || lower == "min_freq"
        || lower == "max_freq"
        || lower == "low_freq"
        || lower == "mid_freq"
        || lower == "high_freq"
}

/// Draw a parameter with a label, numeric value, and horizontal bar indicator.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_param_bar<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    w: f32,
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    accent: Color,
) {
    let log_scale = is_log_label(label) && min > 0.0;
    let display = param_value_text(label, value, log_scale);
    renderer.draw_text(
        &param_label(label),
        x + 8.0,
        y,
        FontSize::Small,
        TEXT_SECONDARY,
    );
    renderer.draw_text(&display, x + w - 64.0, y, FontSize::Small, TEXT_PRIMARY);
    let bar_y = y + 16.0;
    let bar_w = w - 16.0;
    renderer.fill_rect(x + 8.0, bar_y, bar_w, 5.0, colors::KNOB_TRACK);
    let frac = param_fraction(value, min, max, log_scale);
    if frac > 0.0 {
        renderer.fill_rect(x + 8.0, bar_y, bar_w * frac, 5.0, accent);
    }
}

/// Draw a parameter with a label and a text value in a display box.
pub(super) fn draw_param_label<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    w: f32,
    label: &str,
    value: &str,
) {
    renderer.draw_text(label, x + 8.0, y, FontSize::Small, TEXT_SECONDARY);
    let box_x = x + w * 0.55;
    let box_w = w * 0.4;
    renderer.fill_rect(box_x, y, box_w, 18.0, Color::rgb(28, 28, 34));
    renderer.draw_rect(box_x, y, box_w, 18.0, Color::rgb(55, 55, 65));
    renderer.draw_text(value, box_x + 6.0, y + 1.0, FontSize::Small, TEXT_ACCENT);
}

/// Width of one pedal box.
pub(super) const PEDAL_W: f32 = 180.0;
/// Horizontal gap between pedal boxes.
pub(super) const PEDAL_GAP: f32 = 16.0;
/// Height of the pedal header, above the first parameter row.
pub(super) const PEDAL_HEADER_H: f32 = 28.0;
/// Height of the button row at the foot of a pedal box.
pub(super) const PEDAL_FOOTER_H: f32 = 24.0;
/// Size of the "Add effect" card at the end of a chain.
pub(super) const ADD_CARD_W: f32 = 120.0;
pub(super) const ADD_CARD_H: f32 = 64.0;

/// A clickable control on a pedal box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PedalButton {
    MoveLeft,
    MoveRight,
    Bypass,
    Remove,
}

/// An axis-aligned rectangle in screen pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub(super) const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub(super) fn contains(&self, mx: f32, my: f32) -> bool {
        mx >= self.x && mx < self.x + self.w && my >= self.y && my < self.y + self.h
    }
}

/// Left edge of pedal `idx` in a chain that starts at `start_x`.
pub(super) fn pedal_x(start_x: f32, idx: usize) -> f32 {
    start_x + idx as f32 * (PEDAL_W + PEDAL_GAP)
}

/// Height of the parameter area and header of a pedal, without the footer.
pub(super) fn pedal_body_h(pedal: &PedalConfig) -> f32 {
    PEDAL_HEADER_H + pedal.params.len().max(1) as f32 * PARAM_ROW_H + 4.0
}

/// Full height of a pedal box.
pub(super) fn pedal_h(pedal: &PedalConfig) -> f32 {
    pedal_body_h(pedal) + PEDAL_FOOTER_H
}

/// The footer buttons of a pedal box at (`px`, `py`). Drawing and hit
/// testing both use this, so they cannot disagree.
pub(super) fn pedal_buttons(px: f32, py: f32, pedal: &PedalConfig) -> [(PedalButton, Rect); 4] {
    let fy = py + pedal_body_h(pedal) + 2.0;
    [
        (PedalButton::MoveLeft, Rect::new(px + 6.0, fy, 24.0, 18.0)),
        (PedalButton::MoveRight, Rect::new(px + 34.0, fy, 24.0, 18.0)),
        (PedalButton::Bypass, Rect::new(px + 64.0, fy, 44.0, 18.0)),
        (
            PedalButton::Remove,
            Rect::new(px + PEDAL_W - 66.0, fy, 60.0, 18.0),
        ),
    ]
}

/// The bypass light in the header of a pedal box.
pub(super) fn pedal_led(px: f32, py: f32) -> Rect {
    Rect::new(px + PEDAL_W - 20.0, py + 4.0, 16.0, 16.0)
}

/// Slots in one row of a chain `width` wide. Each slot holds a pedal and the
/// connection arrow after it, so a full row never reaches past `width`. A
/// chain narrower than one slot still gets one slot per row and overflows to
/// the right. An infinite width saturates the conversion, which gives a
/// single row, as the sideways scrolling Studio rack needs.
pub(super) fn pedals_per_row(width: f32) -> usize {
    ((width / (PEDAL_W + PEDAL_GAP)) as usize).max(1)
}

/// Top left corner of slot `idx` in a chain that starts at (`start_x`, `y`)
/// and wraps into rows no wider than `width`. Slot `pedals.len()` holds the
/// "Add effect" card. A row is as tall as its tallest pedal, so a tall pedal
/// never runs into the row below.
///
/// The chain pages once laid every chain out in a single row, so on Stereo
/// Chain the left chain ran into the right one from its third pedal on.
/// Drawing and both hit tests use this, so they cannot disagree.
pub(super) fn chain_slot(
    start_x: f32,
    y: f32,
    width: f32,
    pedals: &[PedalConfig],
    idx: usize,
) -> (f32, f32) {
    let per_row = pedals_per_row(width);
    let row = idx / per_row;
    let mut row_y = y;
    // Every row above slot `idx` is full, so each slice is in bounds, and the
    // loop runs once per row.
    for first in (0..row).map(|r| r * per_row) {
        let last = (first + per_row).min(pedals.len());
        let tallest = pedals[first..last].iter().map(pedal_h).fold(0.0, f32::max);
        row_y += tallest + PEDAL_GAP;
    }
    (pedal_x(start_x, idx % per_row), row_y)
}

/// The "Add effect" card at the end of a chain laid out by `chain_slot`.
pub(super) fn chain_add_card(start_x: f32, y: f32, width: f32, pedals: &[PedalConfig]) -> Rect {
    let (x, y) = chain_slot(start_x, y, width, pedals, pedals.len());
    Rect::new(x, y, ADD_CARD_W, ADD_CARD_H)
}

/// A pedal type name such as `PitchCorrector` with spaces between words.
pub(super) fn pedal_display_name(pedal_type: &str) -> String {
    let mut out = String::with_capacity(pedal_type.len() + 4);
    let mut prev_lower = false;
    for c in pedal_type.chars() {
        if c.is_ascii_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = c.is_ascii_lowercase();
        out.push(c);
    }
    out
}

/// Draw a horizontal chain of pedal boxes with connection lines, followed by
/// an "Add effect" card.
///
/// `selected_idx` is the index of the selected pedal within this chain, if any.
/// The chain wraps into rows no wider than `width`, as `chain_slot` places them.
pub(super) fn draw_pedal_chain<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    width: f32,
    pedals: &[PedalConfig],
    accent: Color,
    selected_idx: Option<usize>,
) {
    let per_row = pedals_per_row(width);
    for (i, pedal) in pedals.iter().enumerate() {
        let (px, py) = chain_slot(x, y, width, pedals, i);
        let h = pedal_h(pedal);
        draw_pedal_box(renderer, px, py, h, pedal, accent, selected_idx == Some(i));

        // Connection line to the next pedal, or to the add card, when that
        // is on the same row. A line at the end of a row would point at
        // nothing.
        if (i + 1) % per_row != 0 {
            let line_y = py + PEDAL_HEADER_H / 2.0;
            let x1 = px + PEDAL_W;
            let x2 = x1 + PEDAL_GAP;
            renderer.draw_hline(x1, x2, line_y, accent);
            renderer.draw_hline(x2 - 8.0, x2 - 4.0, line_y - 2.0, accent);
            renderer.draw_hline(x2 - 8.0, x2 - 4.0, line_y + 2.0, accent);
        }
    }

    let card = chain_add_card(x, y, width, pedals);
    renderer.fill_rect(card.x, card.y, card.w, card.h, colors::NAV_BUTTON);
    renderer.draw_rect(card.x, card.y, card.w, card.h, accent);
    renderer.draw_text("+", card.x + 52.0, card.y + 6.0, FontSize::Large, accent);
    renderer.draw_text(
        "Add effect",
        card.x + 22.0,
        card.y + 40.0,
        FontSize::Small,
        TEXT_PRIMARY,
    );
}

/// Draw a single pedal box with its header, parameter bars and buttons.
pub(super) fn draw_pedal_box<R: Renderer>(
    renderer: &mut R,
    x: f32,
    y: f32,
    h: f32,
    pedal: &PedalConfig,
    accent: Color,
    is_selected: bool,
) {
    let w = PEDAL_W;
    // Bypassed pedals are drawn dimmed, so that the state shows at a glance.
    let body = if pedal.bypassed {
        Color::rgb(40, 38, 36)
    } else {
        colors::PEDAL_BG
    };
    renderer.fill_rect(x, y, w, h, body);
    let header_color = if is_selected {
        accent
    } else {
        colors::PEDAL_HEADER
    };
    renderer.fill_rect(x, y, w, 24.0, header_color);
    let header_text_color = if is_selected { BG_DARK } else { TEXT_PRIMARY };
    renderer.draw_text(
        &pedal_display_name(&pedal.pedal_type),
        x + 6.0,
        y + 4.0,
        FontSize::Small,
        header_text_color,
    );
    let border_color = if is_selected { TEXT_ACCENT } else { accent };
    renderer.draw_rect(x, y, w, h, border_color);
    if is_selected {
        renderer.draw_rect(x - 1.0, y - 1.0, w + 2.0, h + 2.0, border_color);
    }

    // Light: green when active, red when bypassed.
    let led = pedal_led(x, y);
    let led_color = if pedal.bypassed {
        Color::rgb(220, 60, 60)
    } else {
        Color::rgb(80, 220, 80)
    };
    renderer.fill_rect(led.x + 4.0, led.y + 4.0, 8.0, 8.0, led_color);

    let mut py = y + PEDAL_HEADER_H;
    for param in &pedal.params {
        let (min, max) = pedal_param_range(&param.name);
        draw_param_bar(
            renderer,
            x,
            py,
            w,
            &param.name,
            param.value,
            min,
            max,
            accent,
        );
        py += PARAM_ROW_H;
    }

    for (button, r) in pedal_buttons(x, y, pedal) {
        let label = match button {
            PedalButton::MoveLeft => "<",
            PedalButton::MoveRight => ">",
            PedalButton::Bypass if pedal.bypassed => "OFF",
            PedalButton::Bypass => "ON",
            PedalButton::Remove => "Remove",
        };
        renderer.fill_rect(r.x, r.y, r.w, r.h, colors::NAV_BUTTON);
        renderer.draw_rect(r.x, r.y, r.w, r.h, colors::PANEL_BORDER);
        renderer.draw_text(label, r.x + 6.0, r.y + 1.0, FontSize::Small, TEXT_PRIMARY);
    }
}

/// Find a pedal parameter bar at the given mouse position within a single
/// chain laid out by `chain_slot`.
pub(super) fn find_pedal_param_in_chain(
    mx: f32,
    my: f32,
    pedals_y: f32,
    start_x: f32,
    width: f32,
    pedals: &[PedalConfig],
    chain_id: u8,
) -> Option<(u8, usize, usize, f64, f64)> {
    for (pedal_idx, pedal) in pedals.iter().enumerate() {
        let (px, top) = chain_slot(start_x, pedals_y, width, pedals, pedal_idx);
        let param_start_y = top + PEDAL_HEADER_H;
        for (param_idx, param) in pedal.params.iter().enumerate() {
            let py = param_start_y + param_idx as f32 * PARAM_ROW_H;
            if hit_row(mx, my, px, py, PEDAL_W) {
                let (min, max) = pedal_param_range(&param.name);
                return Some((chain_id, pedal_idx, param_idx, min, max));
            }
        }
    }
    None
}

/// Map pedal parameter names to reasonable display ranges.
pub(super) fn pedal_param_range(name: &str) -> (f64, f64) {
    match name {
        "mix" | "tone" | "depth" | "sensitivity" | "warmth" | "position" | "threshold"
        | "damping" | "decay" | "rate_factor" => (0.0, 1.0),
        "gain" => (0.0, 20.0),
        "drive" => (1.0, 5.0),
        "feedback" => (-1.0, 1.0),
        "rate_hz" => (0.1, 20.0),
        "time_ms" | "attack_ms" | "release_ms" => (0.0, 2000.0),
        // Chorus, flanger and vibrato depths are a few milliseconds. The
        // former shared range of 2000 ms left them in the first pixel.
        "depth_ms" => (0.0, 10.0),
        "fold" => (1.0, 10.0),
        // The engine runs at most 8 allpass stages. This had no entry, so the
        // bar spanned 0 to 1 and any drag dropped the phaser to one stage.
        "stages" => (1.0, 8.0),
        "mode" | "scale" => (0.0, 2.0),
        "key" => (0.0, 11.0),
        "lfo_rate_hz" => (0.05, 20.0),
        "lfo_depth" => (0.0, 4.0),
        "cutoff_hz" => (20.0, 20000.0),
        "ratio" => (1.0, 20.0),
        "semitones" => (-24.0, 24.0),
        "bit_depth" => (1.0, 32.0),
        "frequency_hz" | "min_freq" | "max_freq" | "low_freq" | "mid_freq" | "high_freq" => {
            (20.0, 20000.0)
        }
        "q" => (0.1, 20.0),
        "low_gain_db" | "mid_gain_db" | "high_gain_db" => (-12.0, 12.0),
        "makeup_gain" => (0.0, 4.0),
        _ => (0.0, 1.0),
    }
}

/// Test whether a mouse click at (mx, my) falls within a parameter row.
///
/// A row occupies the rectangle `[col_x, row_y, col_x + col_w, row_y + PARAM_ROW_H]`.
pub(super) fn hit_row(mx: f32, my: f32, col_x: f32, row_y: f32, col_w: f32) -> bool {
    mx >= col_x && mx < col_x + col_w && my >= row_y && my < row_y + PARAM_ROW_H
}

/// Cycle to the next waveform name in a fixed list.
pub(super) fn next_waveform(current: &str) -> String {
    const WAVEFORMS: &[&str] = &["sawtooth", "triangle", "square", "sine", "noise", "pcm"];
    let idx = WAVEFORMS.iter().position(|w| *w == current).unwrap_or(0);
    String::from(WAVEFORMS[(idx + 1) % WAVEFORMS.len()])
}

/// Cycle to the next organ pipe scale in a fixed list.
pub(super) fn next_scale(current: &str) -> String {
    const SCALES: &[&str] = &["32'", "16'", "8'", "4'", "2'"];
    let idx = SCALES.iter().position(|s| *s == current).unwrap_or(2);
    String::from(SCALES[(idx + 1) % SCALES.len()])
}

/// Cycle to the next voice mode in a fixed list.
/// Toggle voice mode priority between High and Low.
pub(super) fn next_voice_mode(current: &str) -> String {
    if current.starts_with("low") {
        String::from("high_1")
    } else {
        String::from("low_1")
    }
}

/// Check whether a synth parameter should use logarithmic display mapping.
pub(super) fn is_log_synth_param(param: SynthParam) -> bool {
    matches!(param, SynthParam::HpfCutoff | SynthParam::LpfCutoff)
}

/// Check whether a pedal parameter name should use logarithmic display mapping.
pub(super) fn is_log_pedal_param(name: &str) -> bool {
    matches!(
        name,
        "frequency_hz"
            | "min_freq"
            | "max_freq"
            | "low_freq"
            | "mid_freq"
            | "high_freq"
            | "cutoff_hz"
            | "lfo_rate_hz"
    )
}

/// Names shown for the steps of a stepped pedal parameter, indexed by the
/// rounded value, or `None` for a continuous parameter.
pub(super) fn stepped_param_names(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "mode" => Some(&["Lowpass", "Highpass", "Bandpass"]),
        "scale" => Some(&["Chromatic", "Major", "Minor"]),
        "key" => Some(&[
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ]),
        _ => None,
    }
}

/// Text shown for a parameter value. Stepped parameters show the name of the
/// step, which is how the engine rounds them.
pub(super) fn param_value_text(name: &str, value: f64, log_scale: bool) -> String {
    if let Some(steps) = stepped_param_names(name) {
        let last = steps.len() - 1;
        let idx = if value.is_finite() {
            (libm::round(value).max(0.0) as usize).min(last)
        } else {
            0
        };
        return String::from(steps[idx]);
    }
    if log_scale && value < 100.0 {
        // Sweep and modulation rates are fractions of a hertz.
        format!("{value:.1} Hz")
    } else if log_scale {
        format!("{value:.0} Hz")
    } else {
        format!("{value:.2}")
    }
}

/// A readable label for a parameter name such as `cutoff_hz`.
pub(super) fn param_label(name: &str) -> String {
    let known = match name {
        "cutoff_hz" => Some("cutoff"),
        "q" => Some("resonance"),
        "lfo_rate_hz" => Some("sweep rate"),
        "lfo_depth" => Some("sweep depth"),
        "rate_hz" => Some("rate"),
        "depth_ms" => Some("depth"),
        "time_ms" => Some("time"),
        "attack_ms" => Some("attack"),
        "release_ms" => Some("release"),
        "frequency_hz" => Some("frequency"),
        "rate_factor" => Some("sample rate"),
        "bit_depth" => Some("bits"),
        _ => None,
    };
    match known {
        Some(label) => String::from(label),
        None => name.replace('_', " "),
    }
}

/// Check whether a DragTarget uses logarithmic mapping.
pub(super) fn is_log_target(target: &DragTarget) -> bool {
    match target {
        DragTarget::Synth { log_scale, .. } => *log_scale,
        DragTarget::Pedal { log_scale, .. } => *log_scale,
    }
}

/// Compute a display fraction for a parameter value within a range.
///
/// Uses logarithmic mapping when `log_scale` is true, for perceptually
/// uniform distribution of frequency parameters.
pub(super) fn param_fraction(value: f64, min: f64, max: f64, log_scale: bool) -> f32 {
    if max <= min {
        return 0.0;
    }
    if log_scale && min > 0.0 {
        let log_min = libm::log(min);
        let log_max = libm::log(max);
        ((libm::log(value.clamp(min, max)) - log_min) / (log_max - log_min)) as f32
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0) as f32
    }
}

/// Compute a parameter value from a fraction, using log mapping when appropriate.
pub(super) fn value_from_fraction(frac: f64, min: f64, max: f64, log_scale: bool) -> f64 {
    if log_scale && min > 0.0 {
        let log_min = libm::log(min);
        let log_max = libm::log(max);
        libm::exp(log_min + frac * (log_max - log_min))
    } else {
        min + frac * (max - min)
    }
}

/// Compute the new value after a scroll step, using log mapping for frequencies.
pub(super) fn scroll_step(current: f64, min: f64, max: f64, delta: i32, log_scale: bool) -> f64 {
    if log_scale && min > 0.0 {
        // Step in log space: each tick is 2% of the log range.
        let log_min = libm::log(min);
        let log_max = libm::log(max);
        let log_current = libm::log(current.clamp(min, max));
        let step = (log_max - log_min) * 0.02 * delta as f64;
        libm::exp(log_current + step).clamp(min, max)
    } else {
        let step = (max - min) * 0.02 * delta as f64;
        (current + step).clamp(min, max)
    }
}

/// Compute a parameter value from a mouse x position within a drag target bar.
pub(super) fn drag_value(mx: f32, target: &DragTarget) -> f64 {
    let (bar_x, bar_w, min_val, max_val) = match target {
        DragTarget::Synth {
            bar_x,
            bar_w,
            min_val,
            max_val,
            ..
        } => (*bar_x, *bar_w, *min_val, *max_val),
        DragTarget::Pedal {
            bar_x,
            bar_w,
            min_val,
            max_val,
            ..
        } => (*bar_x, *bar_w, *min_val, *max_val),
    };
    let frac = ((mx - bar_x) / bar_w).clamp(0.0, 1.0) as f64;
    let log_scale = is_log_target(target);
    value_from_fraction(frac, min_val, max_val, log_scale)
}

/// Size of the pedal picker overlay.
pub(super) const PICKER_W: f32 = 1060.0;
pub(super) const PICKER_H: f32 = 530.0;
const PICKER_COLUMNS: usize = 5;
const PICKER_COL_W: f32 = 208.0;
const PICKER_TITLE_H: f32 = 44.0;
const PICKER_CATEGORY_H: f32 = 26.0;
pub(super) const PICKER_ITEM_H: f32 = 40.0;

/// The pedal picker overlay, centred in the window.
pub(super) fn picker_rect(window: WindowSize) -> Rect {
    Rect::new(
        (window.width_f32() - PICKER_W) / 2.0,
        (window.height_f32() - PICKER_H) / 2.0,
        PICKER_W,
        PICKER_H,
    )
}

/// Height of one row of categories, sized for the largest category.
fn picker_row_h() -> f32 {
    let most = PEDAL_TYPES
        .iter()
        .map(|(_, types)| types.len())
        .max()
        .unwrap_or(0);
    PICKER_CATEGORY_H + most as f32 * PICKER_ITEM_H + 10.0
}

/// Header of category `cat` in the picker grid.
pub(super) fn picker_category_rect(window: WindowSize, cat: usize) -> Rect {
    let p = picker_rect(window);
    let col = cat % PICKER_COLUMNS;
    let row = cat / PICKER_COLUMNS;
    Rect::new(
        p.x + 10.0 + col as f32 * PICKER_COL_W,
        p.y + PICKER_TITLE_H + row as f32 * picker_row_h(),
        PICKER_COL_W - 8.0,
        PICKER_CATEGORY_H - 2.0,
    )
}

/// Item `item` of category `cat` in the picker grid.
pub(super) fn picker_item_rect(window: WindowSize, cat: usize, item: usize) -> Rect {
    let c = picker_category_rect(window, cat);
    Rect::new(
        c.x,
        c.y + PICKER_CATEGORY_H + item as f32 * PICKER_ITEM_H,
        c.w,
        PICKER_ITEM_H - 4.0,
    )
}

/// The pedal type under the mouse in the picker, if any.
pub(super) fn picker_item_at(window: WindowSize, mx: f32, my: f32) -> Option<&'static str> {
    PEDAL_TYPES
        .iter()
        .enumerate()
        .find_map(|(cat, (_, types))| {
            types
                .iter()
                .enumerate()
                .find(|(item, _)| picker_item_rect(window, cat, *item).contains(mx, my))
                .map(|(_, name)| *name)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepped_parameters_show_names_and_clamp() {
        assert_eq!(param_value_text("mode", 1.0, false), "Highpass");
        assert_eq!(param_value_text("mode", 1.6, false), "Bandpass");
        assert_eq!(param_value_text("mode", 9.0, false), "Bandpass");
        assert_eq!(param_value_text("mode", -3.0, false), "Lowpass");
        assert_eq!(param_value_text("key", f64::NAN, false), "C");
        assert_eq!(param_value_text("key", 11.0, false), "B");
        assert_eq!(param_value_text("scale", 2.0, false), "Minor");
    }

    #[test]
    fn hertz_values_keep_useful_precision() {
        assert_eq!(param_value_text("lfo_rate_hz", 0.5, true), "0.5 Hz");
        assert_eq!(param_value_text("cutoff_hz", 1200.0, true), "1200 Hz");
        assert_eq!(param_value_text("gain", 2.0, false), "2.00");
    }

    #[test]
    fn display_names_split_words() {
        assert_eq!(pedal_display_name("PitchCorrector"), "Pitch Corrector");
        assert_eq!(pedal_display_name("Eq"), "Eq");
        assert_eq!(param_label("cutoff_hz"), "cutoff");
        assert_eq!(param_label("bit_depth"), "bits");
        assert_eq!(param_label("makeup_gain"), "makeup gain");
    }

    #[test]
    fn every_pedal_parameter_has_a_usable_range() {
        for (_, types) in PEDAL_TYPES {
            for pedal_type in *types {
                for param in default_pedal_config(pedal_type).unwrap().params {
                    let (min, max) = pedal_param_range(&param.name);
                    assert!(min < max, "{pedal_type}.{}", param.name);
                    assert!(
                        (min..=max).contains(&param.value),
                        "{pedal_type}.{} default {} outside {min}..={max}",
                        param.name,
                        param.value
                    );
                    if is_log_pedal_param(&param.name) {
                        assert!(min > 0.0, "{pedal_type}.{} is log from zero", param.name);
                    }
                }
            }
        }
    }

    #[test]
    fn pedal_buttons_do_not_overlap_each_other_or_the_light() {
        let pedal = default_pedal_config("Filter").unwrap();
        let buttons = pedal_buttons(100.0, 200.0, &pedal);
        let box_rect = Rect::new(100.0, 200.0, PEDAL_W, pedal_h(&pedal));
        for (i, (_, a)) in buttons.iter().enumerate() {
            assert!(
                box_rect.contains(a.x, a.y) && box_rect.contains(a.x + a.w - 1.0, a.y + a.h - 1.0)
            );
            for (_, b) in &buttons[i + 1..] {
                let overlap =
                    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!overlap, "{a:?} overlaps {b:?}");
            }
        }
        assert_eq!(pedal_led(100.0, 200.0).y, 204.0);
    }
}
