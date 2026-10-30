#![no_std]
extern crate alloc;

pub mod app;

pub mod engine_setup;
pub mod input;
pub mod midi;
pub mod patch;
pub mod persistence;
pub mod platform;
pub mod presets;
pub mod project;
pub mod render;
pub mod script;
pub mod smf;
pub mod transport;
pub mod wav;

pub use app::DawApp;
pub use input::DawInput;
pub use midi::{AutofixStrategy, MidiNote, MidiTrack};
pub use patch::{EffectChainConfig, PatchConfig};
pub use platform::{LogLevel, MemoryPlatform, Platform};
pub use project::{ChannelConfig, DawProject, TimedEvent};
pub use render::Renderer;
pub use transport::Transport;
