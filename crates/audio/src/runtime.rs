//! The engine runtime, which is the audio-thread half of the lock-free design,
//! and the traits that platform audio layers implement.
//!
//! The control thread owns a `spsc::Producer<EngineCommand>`. The audio thread
//! owns an `EngineRunner`, which holds the `AudioEngine` and the matching
//! consumer. A platform crate implements `AudioOutput` by calling
//! `AudioSource::render_f32` from its device callback. Nothing here blocks or
//! locks on the audio thread, and the runner's own buffers are allocated once
//! at construction.
//!
//! One exception to allocation-free rendering is known technical debt. An
//! `EngineCommand::Bulk` closure runs on the audio thread and may allocate,
//! and its box is freed there after it runs. The DAW sends `Bulk` only on cold
//! paths such as applying a patch or rebuilding a pedal chain, where a rare
//! glitch is accepted. Removing it needs a fixed-size command for each such
//! change, or a return queue that carries the spent box back to the control
//! thread to be freed.

use alloc::vec;
use alloc::vec::Vec;

use crate::command::{EngineCommand, apply_engine_command};
use crate::engine::AudioEngine;
use crate::spsc::{Consumer, Producer};

/// Something that fills interleaved stereo `f32` buffers on an audio thread.
///
/// Implementations must not block or allocate in `render_f32`, because it runs
/// in a real-time callback. The buffer length is even, one left and one right
/// sample per frame. An odd trailing sample, which a conforming platform never
/// passes, may be left untouched.
pub trait AudioSource: Send + 'static {
    fn render_f32(&mut self, out: &mut [f32]);
}

/// A platform audio device that pulls from an `AudioSource`.
///
/// `start` moves the source to the audio thread. On failure the source comes
/// back with the error, so the caller keeps its engine. `stop` halts the
/// device, waits until no callback can run, and returns the source.
pub trait AudioOutput<S: AudioSource> {
    type Error;

    fn start(&mut self, sample_rate: u32, source: S) -> Result<(), (Self::Error, S)>;

    fn stop(&mut self) -> Option<S>;

    fn is_running(&self) -> bool;
}

/// A destination for engine commands on the control thread.
///
/// A full or absent audio thread returns the command in `Err`, and the caller
/// decides whether to log, retry or drop it.
pub trait CommandSink {
    fn send(&mut self, cmd: EngineCommand) -> Result<(), EngineCommand>;
}

impl CommandSink for Producer<EngineCommand> {
    fn send(&mut self, cmd: EngineCommand) -> Result<(), EngineCommand> {
        self.push(cmd)
    }
}

/// A sink that discards every command. It stands in for the audio thread when
/// no device could be opened, and in tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl CommandSink for NullSink {
    fn send(&mut self, _cmd: EngineCommand) -> Result<(), EngineCommand> {
        Ok(())
    }
}

/// An optional sink discards commands while it is `None`, like `NullSink`.
/// A host uses it to run before, or without, an audio device.
impl<T: CommandSink> CommandSink for Option<T> {
    fn send(&mut self, cmd: EngineCommand) -> Result<(), EngineCommand> {
        match self {
            Some(sink) => sink.send(cmd),
            None => Ok(()),
        }
    }
}

/// Owns the engine on the audio thread and applies queued commands before
/// each block.
pub struct EngineRunner {
    engine: AudioEngine,
    consumer: Consumer<EngineCommand>,
    /// `f64` staging buffer, allocated once. Its length is even and nonzero,
    /// and a larger request is rendered in chunks of this length.
    staging: Vec<f64>,
}

impl EngineRunner {
    /// `block_samples` sizes the staging buffer. It is rounded up to an even
    /// number of at least two, so that every chunk holds whole frames.
    pub fn new(
        engine: AudioEngine,
        consumer: Consumer<EngineCommand>,
        block_samples: usize,
    ) -> Self {
        let len = block_samples.max(2).next_multiple_of(2);
        Self {
            engine,
            consumer,
            staging: vec![0.0; len],
        }
    }

    pub fn engine(&self) -> &AudioEngine {
        &self.engine
    }

    pub fn into_engine(self) -> AudioEngine {
        self.engine
    }

    /// Apply queued commands. At most one buffer's capacity is drained per
    /// call, so a producer that keeps pushing cannot hold the audio thread.
    fn drain_commands(&mut self) {
        for _ in 0..self.consumer.capacity() {
            match self.consumer.pop() {
                Some(cmd) => apply_engine_command(&mut self.engine, cmd),
                None => break,
            }
        }
    }
}

impl AudioSource for EngineRunner {
    fn render_f32(&mut self, out: &mut [f32]) {
        self.drain_commands();
        let frames_len = out.len() & !1;
        for chunk in out[..frames_len].chunks_mut(self.staging.len()) {
            let staging = &mut self.staging[..chunk.len()];
            self.engine.render_samples(staging);
            for (o, &s) in chunk.iter_mut().zip(staging.iter()) {
                *o = s as f32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spsc::ring_buffer;

    const SR: u32 = 48_000;

    fn runner(block: usize) -> (Producer<EngineCommand>, EngineRunner) {
        let (tx, rx) = ring_buffer(16);
        (tx, EngineRunner::new(AudioEngine::new(SR), rx, block))
    }

    #[test]
    fn staging_length_is_even_and_nonzero() {
        for (requested, expected) in [(0, 2), (1, 2), (2, 2), (7, 8), (8, 8)] {
            let (_tx, r) = runner(requested);
            assert_eq!(r.staging.len(), expected, "requested {requested}");
        }
    }

    #[test]
    fn commands_are_applied_before_rendering() {
        let (mut tx, mut r) = runner(64);
        assert!(!r.engine().metronome.enabled);
        assert!(
            tx.send(EngineCommand::SetMetronomeEnabled { enabled: true })
                .is_ok()
        );
        r.render_f32(&mut [0.0; 8]);
        assert!(r.engine().metronome.enabled);
    }

    #[test]
    fn chunked_rendering_matches_a_single_block() {
        // Two engines in the same state, one rendering through a small
        // staging buffer, must produce the same samples.
        let (mut tx_a, mut a) = runner(4096);
        let (mut tx_b, mut b) = runner(6);
        for tx in [&mut tx_a, &mut tx_b] {
            let note = EngineCommand::NoteOn {
                channel: 0,
                freq: 220.0,
                velocity: 1.0,
            };
            assert!(tx.send(note).is_ok());
        }
        let mut out_a = [0.0f32; 512];
        let mut out_b = [0.0f32; 512];
        a.render_f32(&mut out_a);
        b.render_f32(&mut out_b);
        assert!(
            out_a.iter().any(|&s| s != 0.0),
            "the note should be audible"
        );
        assert_eq!(out_a, out_b);
    }

    #[test]
    fn odd_trailing_sample_is_untouched() {
        let (mut tx, mut r) = runner(64);
        assert!(
            tx.send(EngineCommand::NoteOn {
                channel: 0,
                freq: 220.0,
                velocity: 1.0
            })
            .is_ok()
        );
        let mut out = [7.0f32; 257];
        r.render_f32(&mut out);
        assert_eq!(out[256], 7.0);
    }

    #[test]
    fn drain_is_bounded_by_capacity() {
        let (mut tx, mut r) = runner(64);
        let cap = r.consumer.capacity();
        for _ in 0..cap {
            assert!(
                tx.send(EngineCommand::SetMetronomeEnabled { enabled: true })
                    .is_ok()
            );
        }
        assert!(
            tx.send(EngineCommand::SetMetronomeEnabled { enabled: true })
                .is_err()
        );
        r.render_f32(&mut [0.0; 2]);
        assert!(
            tx.send(EngineCommand::SetMetronomeEnabled { enabled: true })
                .is_ok()
        );
    }

    #[test]
    fn null_sink_accepts_everything() {
        let mut sink = NullSink;
        assert!(sink.send(EngineCommand::NoteOff { channel: 200 }).is_ok());
    }

    #[test]
    fn optional_sink_forwards_or_discards() {
        let (tx, mut rx) = ring_buffer(4);
        let mut sink = Some(tx);
        assert!(sink.send(EngineCommand::NoteOff { channel: 1 }).is_ok());
        assert!(matches!(
            rx.pop(),
            Some(EngineCommand::NoteOff { channel: 1 })
        ));
        let mut none: Option<Producer<EngineCommand>> = None;
        assert!(none.send(EngineCommand::NoteOff { channel: 1 }).is_ok());
    }

    #[test]
    fn out_of_range_channel_is_ignored() {
        let (mut tx, mut r) = runner(64);
        assert!(
            tx.send(EngineCommand::NoteOn {
                channel: 200,
                freq: 1.0,
                velocity: 1.0
            })
            .is_ok()
        );
        r.render_f32(&mut [0.0; 8]);
    }
}
