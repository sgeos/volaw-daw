//! Render each page of the composer to a BMP file, for checking layouts
//! without a display.
//!
//! ```sh
//! SDL_VIDEO_DRIVER=dummy cargo run -p daw --example screenshots -- tmp/shots
//! SDL_VIDEO_DRIVER=dummy cargo run -p daw --example screenshots -- tmp/shots 1280x800
//! ```
//!
//! The pages are reached by clicking at the coordinates a user would, so a
//! change of layout that moves a control also moves what this example sees.
//! It has no audio and touches no project files, because nothing is saved.

use daw_core::DawApp;
use daw_core::app::WindowSize;
use daw_core::input::DawInput;
use daw_core::platform::{LogLevel, Platform};
use daw_core::render::{Color, FontSize, Renderer};
use daw_sdl::{SdlPlatform, SdlRenderer};

const FONT_PATHS: [&str; 2] = [
    "assets/fonts/NotoSans-Regular.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
];

/// Forwards to `SdlRenderer`, and saves the frame before presenting it when
/// a path is set.
struct Capture {
    inner: SdlRenderer,
    path: Option<String>,
}

impl Renderer for Capture {
    fn clear(&mut self, color: Color) {
        self.inner.clear(color);
    }
    fn present(&mut self) {
        if let Some(path) = self.path.take()
            && let Err(e) = self.inner.save_bmp(&path)
        {
            eprintln!("{path}: {e}");
        }
        self.inner.present();
    }
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.inner.fill_rect(x, y, w, h, color);
    }
    fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.inner.draw_rect(x, y, w, h, color);
    }
    fn draw_hline(&mut self, x1: f32, x2: f32, y: f32, color: Color) {
        self.inner.draw_hline(x1, x2, y, color);
    }
    fn draw_vline(&mut self, x: f32, y1: f32, y2: f32, color: Color) {
        self.inner.draw_vline(x, y1, y2, color);
    }
    fn draw_text(&mut self, text: &str, x: f32, y: f32, size: FontSize, color: Color) {
        self.inner.draw_text(text, x, y, size, color);
    }
    fn text_size(&mut self, text: &str, size: FontSize) -> (f32, f32) {
        self.inner.text_size(text, size)
    }
    fn screen_width(&self) -> f32 {
        self.inner.screen_width()
    }
    fn screen_height(&self) -> f32 {
        self.inner.screen_height()
    }
}

/// Centre of page tab `i`, matching the page tab row layout.
fn page_tab(i: usize) -> (f32, f32) {
    (8.0 + i as f32 * 116.0 + 56.0, 46.0)
}

fn click<P: Platform, S: audio::CommandSink>(app: &mut DawApp<P, S>, (x, y): (f32, f32)) {
    app.handle_input(DawInput::MouseMove(x, y));
    app.handle_input(DawInput::Click(x, y));
    app.handle_input(DawInput::MouseUp(x, y));
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tmp/shots".into());
    let size = match std::env::args().nth(2).map(|s| WindowSize::parse(&s)) {
        None => WindowSize::DEFAULT,
        Some(Ok(size)) => size,
        Some(Err(e)) => {
            eprintln!("window size: {e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("{dir}: {e}");
        std::process::exit(1);
    }
    let platform = SdlPlatform::new();
    let Some(font) = FONT_PATHS.iter().find(|p| platform.path_exists(p)) else {
        eprintln!("no font found");
        std::process::exit(1);
    };
    let inner = match SdlRenderer::new("screenshots", size.width(), size.height(), font) {
        Ok(r) => r,
        Err(e) => {
            platform.log(LogLevel::Error, &e);
            std::process::exit(1);
        }
    };
    let mut renderer = Capture { inner, path: None };
    let mut app: DawApp<SdlPlatform, audio::NullSink> =
        DawApp::new(platform, audio::NullSink, 48_000);
    app.set_window_size(size);

    // A few notes on two tracks, placed on the piano roll by clicking.
    click(&mut app, page_tab(1));
    for (i, y) in [400.0, 380.0, 360.0, 380.0, 420.0, 340.0]
        .iter()
        .enumerate()
    {
        click(&mut app, (200.0 + i as f32 * 90.0, *y));
    }
    app.handle_input(DawInput::SelectChannel(1));
    for (i, y) in [500.0, 520.0, 480.0].iter().enumerate() {
        click(&mut app, (160.0 + i as f32 * 180.0, *y));
    }
    app.handle_input(DawInput::SelectChannel(0));

    // Studio with three effects added from the browser.
    click(&mut app, page_tab(0));
    let mut shot = |app: &mut DawApp<SdlPlatform, audio::NullSink>, name: &str| {
        renderer.path = Some(format!("{dir}/{name}.bmp"));
        app.render(&mut renderer);
    };
    shot(&mut app, "studio-empty");
    click(&mut app, (205.0, 109.0)); // Effects tab
    shot(&mut app, "studio-browser-effects");
    click(&mut app, (100.0, 195.0)); // Overdrive
    click(&mut app, (100.0, 409.0)); // Filter
    click(&mut app, (100.0, 233.0)); // Distortion
    shot(&mut app, "studio-rack");
    let add_x = 290.0 + 3.0 * 196.0 + 60.0;
    click(&mut app, (add_x, 560.0));
    shot(&mut app, "picker");
    app.handle_input(DawInput::Cancel);

    for (i, name) in [
        "piano-roll",
        "overview",
        "synth",
        "mono-chain",
        "stereo-chain",
    ]
    .iter()
    .enumerate()
    {
        click(&mut app, page_tab(i + 1));
        shot(&mut app, name);
    }
}
