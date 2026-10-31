//! The window size that the layout follows.
//!
//! Every page lays out from a `WindowSize`, so a size can only be built
//! through `WindowSize::new` or `WindowSize::parse`, which reject a size
//! outside the bounds below. The bounds are a contract with the layout code
//! rather than a platform limit.
//!
//! The toolbar, the page tab row and the Studio track rows sit at fixed
//! offsets from the left, the Studio rack sits below eight fixed-height track
//! rows, and the pedal picker has a fixed size. Only what lies to the right
//! of or below those follows the size. The minimum was found by drawing every
//! page at a range of sizes and looking for shapes outside the window and
//! for text drawn over text. At 1280 by 800, with three pedals on each chain,
//! nothing is drawn outside and no overlap appears that is not also present
//! at the default size. At 1100 by 760 the toolbar status text runs into the
//! tempo and grid fields, and below 760 the Studio rack leaves the window.
//! The test `every_page_draws_inside_the_window` in `tests.rs` keeps
//! the first of those findings true.
//!
//! Known limits at the minimum. The toolbar status text has about 430 pixels
//! of room only from a width of 1420 up. Below that it starts at a fixed x
//! after the indicators, so a long message is cut off at the right edge
//! rather than drawn over them. The chain pages wrap each chain into rows
//! within its column, so a narrow window gives more rows, and a chain long
//! enough to need more rows than the height allows still runs off the bottom
//! because the chain pages do not scroll. At the minimum, each Stereo Chain
//! column fits three pedals to a row. How many rows fit depends on the
//! positional mode and on how many parameters the pedals have.
//!
//! The maximum keeps every coordinate small enough to be exact in an `f32`,
//! and covers an 8K display. It is not a promise that a platform can open a
//! window that large.

use core::fmt;

/// Width of the window when no size is given.
pub const DEFAULT_WINDOW_WIDTH: u32 = 1440;
/// Height of the window when no size is given.
pub const DEFAULT_WINDOW_HEIGHT: u32 = 900;
/// Smallest supported width.
pub const MIN_WINDOW_WIDTH: u32 = 1280;
/// Smallest supported height.
pub const MIN_WINDOW_HEIGHT: u32 = 800;
/// Largest supported width.
pub const MAX_WINDOW_WIDTH: u32 = 7680;
/// Largest supported height.
pub const MAX_WINDOW_HEIGHT: u32 = 4320;

/// A window size in pixels, within the supported bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    width: u32,
    height: u32,
}

/// Why a window size was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowSizeError {
    /// The text was not of the form `WIDTHxHEIGHT` with decimal numbers.
    Malformed,
    /// The width is outside `MIN_WINDOW_WIDTH..=MAX_WINDOW_WIDTH`.
    WidthOutOfRange(u32),
    /// The height is outside `MIN_WINDOW_HEIGHT..=MAX_WINDOW_HEIGHT`.
    HeightOutOfRange(u32),
}

impl fmt::Display for WindowSizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => write!(f, "expected WIDTHxHEIGHT, such as 1440x900"),
            Self::WidthOutOfRange(w) => write!(
                f,
                "width {w} is outside {MIN_WINDOW_WIDTH} to {MAX_WINDOW_WIDTH}"
            ),
            Self::HeightOutOfRange(h) => write!(
                f,
                "height {h} is outside {MIN_WINDOW_HEIGHT} to {MAX_WINDOW_HEIGHT}"
            ),
        }
    }
}

impl WindowSize {
    /// The size used when none is given.
    pub const DEFAULT: Self = Self {
        width: DEFAULT_WINDOW_WIDTH,
        height: DEFAULT_WINDOW_HEIGHT,
    };
    /// The smallest supported size.
    pub const MIN: Self = Self {
        width: MIN_WINDOW_WIDTH,
        height: MIN_WINDOW_HEIGHT,
    };
    /// The largest supported size.
    pub const MAX: Self = Self {
        width: MAX_WINDOW_WIDTH,
        height: MAX_WINDOW_HEIGHT,
    };

    /// A size, or the first bound it breaks, checking the width first.
    pub fn new(width: u32, height: u32) -> Result<Self, WindowSizeError> {
        if !(MIN_WINDOW_WIDTH..=MAX_WINDOW_WIDTH).contains(&width) {
            return Err(WindowSizeError::WidthOutOfRange(width));
        }
        if !(MIN_WINDOW_HEIGHT..=MAX_WINDOW_HEIGHT).contains(&height) {
            return Err(WindowSizeError::HeightOutOfRange(height));
        }
        Ok(Self { width, height })
    }

    /// Parse `WIDTHxHEIGHT`, such as `1440x900`, then check the bounds.
    /// Only ASCII digits are accepted on each side, so a sign, a space or a
    /// second `x` is malformed rather than silently read.
    pub fn parse(text: &str) -> Result<Self, WindowSizeError> {
        let (w, h) = text.split_once('x').ok_or(WindowSizeError::Malformed)?;
        Self::new(parse_dimension(w)?, parse_dimension(h)?)
    }

    /// Width in pixels.
    pub fn width(self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(self) -> u32 {
        self.height
    }

    /// Width as a layout coordinate. Exact, because the width is bounded.
    pub(crate) fn width_f32(self) -> f32 {
        self.width as f32
    }

    /// Height as a layout coordinate. Exact, because the height is bounded.
    pub(crate) fn height_f32(self) -> f32 {
        self.height as f32
    }
}

/// One side of `WIDTHxHEIGHT`. At most ten digits are read, which bounds the
/// work and still reaches every `u32`, so an overflow is a parse error.
fn parse_dimension(text: &str) -> Result<u32, WindowSizeError> {
    if text.is_empty() || text.len() > 10 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(WindowSizeError::Malformed);
    }
    text.parse().map_err(|_| WindowSizeError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_ordered_around_the_default() {
        let (min, def, max) = (WindowSize::MIN, WindowSize::DEFAULT, WindowSize::MAX);
        assert!(min.width() <= def.width() && def.width() <= max.width());
        assert!(min.height() <= def.height() && def.height() <= max.height());
        assert_eq!(WindowSize::new(def.width(), def.height()), Ok(def));
    }

    #[test]
    fn every_bound_is_inclusive() {
        for size in [WindowSize::MIN, WindowSize::MAX] {
            assert_eq!(WindowSize::new(size.width(), size.height()), Ok(size));
        }
    }

    #[test]
    fn sizes_just_outside_the_bounds_are_refused() {
        let (min, max) = (WindowSize::MIN, WindowSize::MAX);
        let w = min.width() - 1;
        assert_eq!(
            WindowSize::new(w, min.height()),
            Err(WindowSizeError::WidthOutOfRange(w))
        );
        let w = max.width() + 1;
        assert_eq!(
            WindowSize::new(w, max.height()),
            Err(WindowSizeError::WidthOutOfRange(w))
        );
        let h = min.height() - 1;
        assert_eq!(
            WindowSize::new(min.width(), h),
            Err(WindowSizeError::HeightOutOfRange(h))
        );
        let h = max.height() + 1;
        assert_eq!(
            WindowSize::new(max.width(), h),
            Err(WindowSizeError::HeightOutOfRange(h))
        );
        assert_eq!(
            WindowSize::new(0, 0),
            Err(WindowSizeError::WidthOutOfRange(0))
        );
    }

    #[test]
    fn parse_reads_width_then_height() {
        let size = WindowSize::new(1600, 1000).unwrap();
        assert_eq!(WindowSize::parse("1600x1000"), Ok(size));
        let def = WindowSize::DEFAULT;
        let text = alloc::format!("{}x{}", def.width(), def.height());
        assert_eq!(WindowSize::parse(&text), Ok(def));
    }

    #[test]
    fn parse_refuses_malformed_text() {
        for text in [
            "",
            "x",
            "1600",
            "1600x",
            "x1000",
            "1600X1000",
            "1600 x 1000",
            "+1600x1000",
            "-1600x1000",
            "1600x1000x2",
            "1600.0x1000",
            "0x16x1000",
            "１６００x1000",
            "99999999999x1000",
            "4294967296x1000",
        ] {
            assert_eq!(
                WindowSize::parse(text),
                Err(WindowSizeError::Malformed),
                "{text:?}"
            );
        }
    }

    #[test]
    fn parse_checks_bounds_after_reading() {
        assert_eq!(
            WindowSize::parse("640x480"),
            Err(WindowSizeError::WidthOutOfRange(640))
        );
        assert_eq!(
            WindowSize::parse("1600x100"),
            Err(WindowSizeError::HeightOutOfRange(100))
        );
        assert_eq!(
            WindowSize::parse("4294967295x1000"),
            Err(WindowSizeError::WidthOutOfRange(u32::MAX))
        );
    }

    #[test]
    fn errors_name_the_bound() {
        let text = alloc::format!("{}", WindowSizeError::WidthOutOfRange(5));
        assert!(
            text.contains(&alloc::format!("{MIN_WINDOW_WIDTH}")),
            "{text}"
        );
        assert!(
            text.contains(&alloc::format!("{MAX_WINDOW_WIDTH}")),
            "{text}"
        );
    }
}
