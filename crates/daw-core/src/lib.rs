#![no_std]
extern crate alloc;

pub mod input;
pub mod midi;
pub mod patch;
pub mod platform;
pub mod project;
pub mod render;
pub mod transport;

pub use input::DawInput;
pub use midi::{AutofixStrategy, MidiNote, MidiTrack};
pub use patch::{EffectChainConfig, PatchConfig};
pub use platform::Platform;
pub use project::{ChannelConfig, DawProject, TimedEvent};
pub use render::Renderer;
pub use transport::Transport;
