#![no_std]
extern crate alloc;

pub mod adsr;
pub mod channel;
pub mod command;
pub mod dsp;
pub mod effects;
pub mod engine;
pub mod limiter;
pub mod metronome;
pub mod modmatrix;
pub mod ms20_filter;
pub mod pedals;
pub mod position;
pub mod runtime;
pub mod sample;
pub mod spsc;
pub mod synth;
pub mod vco;

pub use adsr::Adsr;
pub use channel::Channel;
pub use command::EngineCommand;
pub use effects::Pedal;
pub use engine::{AudioEngine, NUM_CHANNELS};
pub use limiter::LookaheadLimiter;
pub use metronome::Metronome;
pub use modmatrix::ModMatrix;
pub use ms20_filter::{Korg35Lpf, SallenKeyHpf};
pub use pedals::PedalChain;
pub use position::PositionalStage;
pub use runtime::{AudioOutput, AudioSource, CommandSink, EngineRunner, NullSink};
pub use sample::SampleInput;
pub use synth::{Ms20Voice, SingleVoice, VoiceMode};
pub use vco::{Scale, Vco, Waveform};
