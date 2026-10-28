//! SDL3 and std platform layer for the DAW.
//!
//! `SdlPlatform` implements `daw_core::Platform` over the file system and the
//! monotonic clock, `SdlRenderer` implements `daw_core::Renderer` with SDL3
//! and SDL3_ttf, and `input::translate_event` maps SDL events to
//! `daw_core::DawInput`. Audio output lives in the separate `audio-sdl` crate.

pub mod input;
pub mod platform;
pub mod renderer;

pub use platform::SdlPlatform;
pub use renderer::SdlRenderer;
