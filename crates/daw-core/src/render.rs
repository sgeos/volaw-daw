/// RGBA color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

// DAW color palette.
pub const BG_DARK: Color = Color::rgb(30, 30, 35);
pub const BG_PANEL: Color = Color::rgb(40, 40, 48);
pub const BG_TRACK: Color = Color::rgb(50, 50, 58);
pub const BG_SELECTED: Color = Color::rgb(60, 70, 90);
pub const KEY_WHITE: Color = Color::rgb(220, 220, 225);
pub const KEY_BLACK: Color = Color::rgb(40, 40, 45);
pub const KEY_LABEL: Color = Color::rgb(160, 160, 170);
pub const NOTE_BLOCK: Color = Color::rgb(100, 160, 255);
pub const NOTE_SELECTED: Color = Color::rgb(255, 200, 80);
pub const GRID_LINE: Color = Color::rgb(60, 60, 68);
pub const GRID_BEAT: Color = Color::rgb(80, 80, 90);
pub const GRID_BAR: Color = Color::rgb(100, 100, 115);
pub const PLAYHEAD: Color = Color::rgb(255, 80, 80);
pub const TEXT_PRIMARY: Color = Color::rgb(220, 220, 230);
pub const TEXT_SECONDARY: Color = Color::rgb(150, 150, 165);
pub const TEXT_ACCENT: Color = Color::rgb(100, 180, 255);
pub const CHANNEL_COLORS: [Color; 8] = [
    Color::rgb(255, 100, 100), // Channel 0: Red
    Color::rgb(100, 200, 100), // Channel 1: Green
    Color::rgb(100, 150, 255), // Channel 2: Blue
    Color::rgb(255, 200, 80),  // Channel 3: Yellow
    Color::rgb(200, 100, 255), // Channel 4: Purple
    Color::rgb(100, 220, 220), // Channel 5: Cyan
    Color::rgb(255, 150, 80),  // Channel 6: Orange
    Color::rgb(200, 200, 200), // Channel 7: Silver
];

/// Font size categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSize {
    Small,
    Medium,
    Large,
    Title,
}

/// Rendering abstraction for the DAW.
///
/// The DAW core describes what to draw. The platform wrapper (SDL3)
/// implements the actual drawing.
pub trait Renderer {
    /// Clear the screen with a solid color.
    fn clear(&mut self, color: Color);

    /// Present the rendered frame to the display.
    fn present(&mut self);

    /// Fill a rectangle.
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color);

    /// Draw a rectangle outline.
    fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color);

    /// Draw a horizontal line from (x1, y) to (x2, y).
    fn draw_hline(&mut self, x1: f32, x2: f32, y: f32, color: Color);

    /// Draw a vertical line from (x, y1) to (x, y2).
    fn draw_vline(&mut self, x: f32, y1: f32, y2: f32, color: Color);

    /// Draw text at the given position.
    fn draw_text(&mut self, text: &str, x: f32, y: f32, size: FontSize, color: Color);

    /// Measure the width and height of text without drawing.
    fn text_size(&mut self, text: &str, size: FontSize) -> (f32, f32);

    /// Return the screen width in pixels.
    fn screen_width(&self) -> f32;

    /// Return the screen height in pixels.
    fn screen_height(&self) -> f32;
}
