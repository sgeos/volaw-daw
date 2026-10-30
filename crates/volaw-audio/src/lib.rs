//! Entry library for audio playback, shared by the volaw game client and the
//! volaw-daw composer.
//!
//! It wires the `no_std` `audio` core to the SDL3 platform layer in
//! `audio-sdl`, and is the single crate a host needs. The engine moves to the
//! audio thread inside an `EngineRunner`, and the host keeps an
//! `AudioSystem`, which sends `EngineCommand`s through a lock-free queue.
//!
//! ```no_run
//! use volaw_audio::{AudioConfig, AudioSystem, audio::AudioEngine, audio::EngineCommand};
//! use volaw_audio::audio::CommandSink;
//!
//! let config = AudioConfig::default();
//! let engine = AudioEngine::new(config.sample_rate);
//! match AudioSystem::start(engine, &config) {
//!     Ok(mut system) => {
//!         let note = EngineCommand::NoteOn { channel: 0, freq: 440.0, velocity: 1.0 };
//!         let _ = system.send(note);
//!     }
//!     Err(failure) => eprintln!("audio unavailable: {}", failure.message),
//! }
//! ```

pub use audio;

use audio::spsc::{Producer, ring_buffer};
use audio::{AudioEngine, AudioOutput, CommandSink, EngineCommand, EngineRunner};
use audio_sdl::SdlAudioOutput;

/// The largest `AudioConfig::command_capacity` that `start` accepts. A deeper
/// queue only delays commands, and a bound keeps a configuration value from
/// requesting an allocation that cannot succeed.
pub const MAX_COMMAND_CAPACITY: usize = 1 << 16;

/// The largest `AudioConfig::block_samples` that `start` accepts, for the same
/// reason. It is far above any device buffer seen in practice.
pub const MAX_BLOCK_SAMPLES: usize = 1 << 20;

/// Playback parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioConfig {
    /// Output sample rate in hertz. It should match the rate the engine was
    /// constructed with, which `start` checks.
    pub sample_rate: u32,
    /// Commands the queue holds before `send` reports it full.
    pub command_capacity: usize,
    /// Staging block of the audio thread, in interleaved `f32` samples.
    pub block_samples: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            command_capacity: 1024,
            block_samples: 8192,
        }
    }
}

/// Why playback did not start. The engine is returned so that the host can
/// keep it, for example to run without sound.
pub struct StartFailure {
    pub message: String,
    /// Boxed, because the engine is large and this travels in an `Err`.
    pub engine: Box<AudioEngine>,
}

impl core::fmt::Debug for StartFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StartFailure")
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

/// A running engine on the default playback device.
///
/// Dropping it stops the device first and then frees the engine, because
/// the output field is declared before the producer.
///
/// Drop or `stop` it before the host calls `SDL_Quit`. `SDL_Quit` destroys
/// every audio stream, and the drop would then destroy this one a second
/// time. The `daw` binary takes its `AudioSystem` out of the application and
/// drops it before the renderer shuts SDL down.
pub struct AudioSystem {
    output: SdlAudioOutput<EngineRunner>,
    producer: Producer<EngineCommand>,
    sample_rate: u32,
}

impl AudioSystem {
    /// Move `engine` to the audio thread and start the default device.
    pub fn start(engine: AudioEngine, config: &AudioConfig) -> Result<Self, StartFailure> {
        if engine.sample_rate != config.sample_rate {
            return Err(StartFailure {
                message: format!(
                    "engine sample rate {} differs from output sample rate {}",
                    engine.sample_rate, config.sample_rate
                ),
                engine: Box::new(engine),
            });
        }
        if config.command_capacity == 0 || config.command_capacity > MAX_COMMAND_CAPACITY {
            return Err(StartFailure {
                message: format!(
                    "command capacity {} is outside 1..={MAX_COMMAND_CAPACITY}",
                    config.command_capacity
                ),
                engine: Box::new(engine),
            });
        }
        if config.block_samples > MAX_BLOCK_SAMPLES {
            return Err(StartFailure {
                message: format!(
                    "block of {} samples exceeds {MAX_BLOCK_SAMPLES}",
                    config.block_samples
                ),
                engine: Box::new(engine),
            });
        }
        let (producer, consumer) = ring_buffer(config.command_capacity);
        let runner = EngineRunner::new(engine, consumer, config.block_samples);
        let mut output = SdlAudioOutput::new();
        match output.start(config.sample_rate, runner) {
            Ok(()) => Ok(Self {
                output,
                producer,
                sample_rate: config.sample_rate,
            }),
            Err((message, runner)) => Err(StartFailure {
                message,
                engine: Box::new(runner.into_engine()),
            }),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Stop playback and take the engine back from the audio thread.
    pub fn stop(mut self) -> AudioEngine {
        self.output
            .stop()
            .expect("an AudioSystem is running from start until stop")
            .into_engine()
    }
}

impl core::fmt::Debug for AudioSystem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AudioSystem")
            .field("sample_rate", &self.sample_rate)
            .finish_non_exhaustive()
    }
}

impl CommandSink for AudioSystem {
    fn send(&mut self, cmd: EngineCommand) -> Result<(), EngineCommand> {
        self.producer.push(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mismatched_sample_rate_returns_the_engine() {
        let engine = AudioEngine::new(44_100);
        let failure = AudioSystem::start(engine, &AudioConfig::default()).unwrap_err();
        assert!(failure.message.contains("44100"), "{}", failure.message);
        assert_eq!(failure.engine.sample_rate, 44_100);
    }

    #[test]
    fn zero_capacity_returns_the_engine() {
        let config = AudioConfig {
            command_capacity: 0,
            ..AudioConfig::default()
        };
        let failure =
            AudioSystem::start(AudioEngine::new(config.sample_rate), &config).unwrap_err();
        assert!(failure.message.contains("capacity"), "{}", failure.message);
    }

    #[test]
    fn oversized_capacity_returns_the_engine() {
        let config = AudioConfig {
            command_capacity: MAX_COMMAND_CAPACITY + 1,
            ..AudioConfig::default()
        };
        let failure =
            AudioSystem::start(AudioEngine::new(config.sample_rate), &config).unwrap_err();
        assert!(failure.message.contains("capacity"), "{}", failure.message);
        assert_eq!(failure.engine.sample_rate, config.sample_rate);
    }

    #[test]
    fn oversized_block_returns_the_engine() {
        let config = AudioConfig {
            block_samples: MAX_BLOCK_SAMPLES + 1,
            ..AudioConfig::default()
        };
        let failure =
            AudioSystem::start(AudioEngine::new(config.sample_rate), &config).unwrap_err();
        assert!(failure.message.contains("block"), "{}", failure.message);
    }

    #[test]
    fn default_config_is_within_bounds() {
        let config = AudioConfig::default();
        assert!((1..=MAX_COMMAND_CAPACITY).contains(&config.command_capacity));
        assert!(config.block_samples <= MAX_BLOCK_SAMPLES);
    }
}
