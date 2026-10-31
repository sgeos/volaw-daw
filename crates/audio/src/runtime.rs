//! The engine runtime, which is the audio-thread half of the lock-free design,
//! and the traits that platform audio layers implement.
//!
//! `command_queue` returns a `CommandSender` for the control thread and a
//! `CommandReceiver` for the audio thread. The audio thread owns an
//! `EngineRunner`, which holds the `AudioEngine` and the receiver. A platform
//! crate implements `AudioOutput` by calling `AudioSource::render_f32` from its
//! device callback.
//!
//! Nothing here blocks, locks, allocates or frees on the audio thread. The
//! runner's buffers are allocated once at construction. A `Bulk` edit is
//! allocated on the control thread, and after the runner applies it, it
//! travels back through a second queue, so that the sender frees it on the
//! control thread together with any state the edit swapped out of the engine.
//! The test `tests/audio_thread_alloc.rs` counts heap use on the audio thread.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use crate::command::{EngineCommand, EngineEdit, apply_engine_command};
use crate::engine::{AudioEngine, SCRATCH_FRAMES};
use crate::spsc::{Consumer, MAX_CAPACITY, Producer, ring_buffer};

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

/// The control thread's half of `command_queue`.
///
/// `send` first frees the `Bulk` edits that the audio thread has returned, and
/// then queues the command. Spent edits wait in the return queue until the
/// next `send` or `reclaim`, or until the queue itself is dropped.
pub struct CommandSender {
    commands: Producer<EngineCommand>,
    spent: Consumer<Box<dyn EngineEdit>>,
}

impl CommandSender {
    /// Free every spent edit the audio thread has returned so far, and report
    /// how many there were. It visits at most the return queue's capacity, so
    /// an audio thread that keeps returning edits cannot hold the caller.
    pub fn reclaim(&mut self) -> usize {
        let mut freed = 0;
        for _ in 0..self.spent.capacity() {
            match self.spent.pop() {
                Some(edit) => {
                    drop(edit);
                    freed += 1;
                }
                None => break,
            }
        }
        freed
    }
}

impl CommandSink for CommandSender {
    fn send(&mut self, cmd: EngineCommand) -> Result<(), EngineCommand> {
        self.reclaim();
        self.commands.push(cmd)
    }
}

/// The audio thread's half of `command_queue`, which `EngineRunner` owns.
pub struct CommandReceiver {
    commands: Consumer<EngineCommand>,
    spent: Producer<Box<dyn EngineEdit>>,
}

impl CommandReceiver {
    /// The number of commands the queue holds at once.
    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }
}

/// Create the command queue between the control thread and the audio thread,
/// holding at least `capacity` commands, with its return queue for spent
/// `Bulk` edits.
///
/// The return queue holds two more edits than the command queue holds
/// commands, so that it cannot fill while the sender reclaims before every
/// push. The bound holds because every edit in the return queue arrived after
/// the last reclaim emptied it. Such an edit was either waiting in the command
/// queue at that reclaim, at most its capacity, or pushed by the one send that
/// followed it, or being applied while the reclaim ran, at most one. If the
/// return queue is full nonetheless, for example because a caller of
/// `reclaim` stopped sending, the runner keeps the edit and stops draining
/// until there is room, rather than free it on the audio thread.
///
/// # Panics
///
/// Panics if `capacity` exceeds `spsc::MAX_CAPACITY`, or if the return queue's
/// capacity would, which needs a capacity within two of that limit. Neither
/// is an allocation that can succeed.
pub fn command_queue(capacity: usize) -> (CommandSender, CommandReceiver) {
    // The usable capacity that `ring_buffer` rounds to, computed before
    // either queue is allocated, so that a capacity out of range panics
    // instead of requesting an allocation that cannot succeed.
    let spent_capacity = capacity
        .checked_add(1)
        .and_then(usize::checked_next_power_of_two)
        .and_then(|slots| (slots - 1).checked_add(2))
        .filter(|&c| c <= MAX_CAPACITY)
        .expect("command queue capacity leaves no room for its return queue");
    let (commands_tx, commands_rx) = ring_buffer(capacity);
    debug_assert_eq!(commands_rx.capacity() + 2, spent_capacity);
    let (spent_tx, spent_rx) = ring_buffer(spent_capacity);
    (
        CommandSender {
            commands: commands_tx,
            spent: spent_rx,
        },
        CommandReceiver {
            commands: commands_rx,
            spent: spent_tx,
        },
    )
}

/// Owns the engine on the audio thread and applies queued commands before
/// each block.
pub struct EngineRunner {
    engine: AudioEngine,
    receiver: CommandReceiver,
    /// A spent edit that the full return queue could not take. While it is
    /// held, no further command is drained.
    held: Option<Box<dyn EngineEdit>>,
    /// `f64` staging buffer, allocated once. Its length is even and nonzero,
    /// and a larger request is rendered in chunks of this length.
    staging: Vec<f64>,
}

impl EngineRunner {
    /// `block_samples` sizes the staging buffer. It is rounded up to an even
    /// number of at least two, so that every chunk holds whole frames, and
    /// capped at the engine's preallocated scratch, because a larger chunk
    /// would make the engine grow its buffers on the audio thread.
    pub fn new(engine: AudioEngine, receiver: CommandReceiver, block_samples: usize) -> Self {
        let len = block_samples
            .clamp(2, 2 * SCRATCH_FRAMES)
            .next_multiple_of(2);
        Self {
            engine,
            receiver,
            held: None,
            staging: vec![0.0; len],
        }
    }

    pub fn engine(&self) -> &AudioEngine {
        &self.engine
    }

    pub fn into_engine(self) -> AudioEngine {
        self.engine
    }

    /// Hand a spent edit back to the control thread. Returns `false`, keeping
    /// the edit in `held`, when the return queue is full.
    fn retire(&mut self, edit: Box<dyn EngineEdit>) -> bool {
        match self.receiver.spent.push(edit) {
            Ok(()) => true,
            Err(edit) => {
                self.held = Some(edit);
                false
            }
        }
    }

    /// Apply queued commands. At most one buffer's capacity is drained per
    /// call, so a producer that keeps pushing cannot hold the audio thread.
    /// Draining stops while a spent edit cannot be returned, which delays
    /// later commands but keeps their order.
    fn drain_commands(&mut self) {
        if let Some(edit) = self.held.take()
            && !self.retire(edit)
        {
            return;
        }
        for _ in 0..self.receiver.capacity() {
            match self.receiver.commands.pop() {
                Some(EngineCommand::Bulk(mut edit)) => {
                    edit.apply(&mut self.engine);
                    if !self.retire(edit) {
                        return;
                    }
                }
                // Every other variant is plain data, so dropping it frees
                // nothing.
                Some(cmd) => apply_engine_command(&mut self.engine, cmd),
                None => return,
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
    use alloc::sync::Arc;

    const SR: u32 = 48_000;

    fn runner(block: usize) -> (CommandSender, EngineRunner) {
        let (tx, rx) = command_queue(16);
        (tx, EngineRunner::new(AudioEngine::new(SR), rx, block))
    }

    #[test]
    fn staging_length_is_even_and_nonzero() {
        let cap = 2 * SCRATCH_FRAMES;
        for (requested, expected) in [
            (0, 2),
            (1, 2),
            (2, 2),
            (7, 8),
            (8, 8),
            (cap, cap),
            (cap + 1, cap),
            (usize::MAX, cap),
        ] {
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
        let cap = r.receiver.capacity();
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

    /// An edit that counts its applications and holds a marker, so that a
    /// test can tell whether the edit has been dropped.
    fn marked_edit(marker: &Arc<()>) -> EngineCommand {
        let marker = Arc::clone(marker);
        EngineCommand::Bulk(Box::new(move |engine: &mut AudioEngine| {
            let _ = &marker;
            engine.master_volume += 0.001;
        }))
    }

    #[test]
    fn spent_edits_wait_for_the_sender_to_free_them() {
        let (mut tx, mut r) = runner(64);
        let marker = Arc::new(());
        assert!(tx.send(marked_edit(&marker)).is_ok());
        let before = r.engine().master_volume;
        r.render_f32(&mut [0.0; 8]);
        assert!(r.engine().master_volume > before, "the edit was applied");
        assert_eq!(Arc::strong_count(&marker), 2, "the runner did not drop it");
        assert_eq!(tx.reclaim(), 1);
        assert_eq!(Arc::strong_count(&marker), 1, "the sender dropped it");
        assert_eq!(tx.reclaim(), 0);
    }

    #[test]
    fn return_queue_outsizes_the_command_queue_by_two() {
        for capacity in [1, 16, 1000, 1024] {
            let (tx, rx) = command_queue(capacity);
            assert!(rx.capacity() >= capacity);
            assert!(tx.spent.capacity() >= rx.capacity() + 2);
        }
    }

    #[test]
    #[should_panic(expected = "return queue")]
    fn capacity_at_the_limit_leaves_no_room_for_the_return_queue() {
        let _ = command_queue(MAX_CAPACITY);
    }

    #[test]
    fn full_return_queue_holds_the_edit_and_later_commands() {
        // Pushing past `send` skips the reclaim that keeps the return queue
        // from filling, which only a misbehaving sender could do.
        let (mut tx, mut r) = runner(64);
        let marker = Arc::new(());
        let mut rounds = 0;
        while r.held.is_none() {
            while tx.commands.push(marked_edit(&marker)).is_ok() {}
            r.render_f32(&mut [0.0; 2]);
            rounds += 1;
            assert!(rounds < 16, "the return queue should fill");
        }
        let queued = Arc::strong_count(&marker) - 1;
        let later = EngineCommand::SetMetronomeEnabled { enabled: true };
        // The command queue still holds edits, so make room first.
        assert!(tx.reclaim() > 0);
        r.render_f32(&mut [0.0; 2]);
        let later_sent = tx.commands.push(later).is_ok();
        // Every edit is still owned by a queue or the runner, none was
        // dropped on the audio side.
        assert!(Arc::strong_count(&marker) - 1 <= queued);
        // Drain everything, reclaiming between callbacks as `send` would.
        for _ in 0..16 {
            tx.reclaim();
            r.render_f32(&mut [0.0; 2]);
        }
        tx.reclaim();
        assert!(r.held.is_none());
        assert_eq!(Arc::strong_count(&marker), 1, "every edit came back");
        assert!(later_sent);
        assert!(
            r.engine().metronome.enabled,
            "the later command ran in order"
        );
    }

    #[test]
    fn held_edit_blocks_draining_until_there_is_room() {
        let (mut tx, mut r) = runner(64);
        let marker = Arc::new(());
        while r.held.is_none() {
            while tx.commands.push(marked_edit(&marker)).is_ok() {}
            r.render_f32(&mut [0.0; 2]);
        }
        // With the return queue full, a callback applies nothing.
        let volume = r.engine().master_volume;
        r.render_f32(&mut [0.0; 2]);
        assert_eq!(r.engine().master_volume, volume);
        assert!(r.held.is_some());
        // One reclaim frees the queue, and the next callback resumes.
        assert!(tx.reclaim() > 0);
        r.render_f32(&mut [0.0; 2]);
        assert!(r.engine().master_volume > volume);
    }
}
