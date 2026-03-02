//! Lock-free single-producer single-consumer (SPSC) ring buffer.
//!
//! Provides a wait-free, allocation-free communication channel between
//! the UI thread (producer) and the audio callback thread (consumer).
//! The ring buffer uses power-of-two capacity for efficient modular
//! indexing via bitmask.
//!
//! ## Memory Ordering
//!
//! The producer stores `head` with `Release` after writing a slot,
//! ensuring the written data is visible to the consumer before the
//! index update. The consumer loads `head` with `Acquire` to see
//! the producer's latest writes. The symmetric pattern applies to
//! the consumer's `tail` updates.
//!
//! ## References
//!
//! - Lamport, "Proving the Correctness of Multiprocess Programs" (1977)
//! - Vyukov, "Bounded MPMC queue" (adapted for SPSC)

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Cache-line-aligned wrapper to prevent false sharing between
/// the head and tail atomic counters.
#[repr(align(64))]
struct CacheAligned<T>(T);

/// Shared ring buffer backing storage.
struct Shared<T> {
    buffer: Box<[UnsafeCell<MaybeUninit<T>>]>,
    head: CacheAligned<AtomicUsize>,
    tail: CacheAligned<AtomicUsize>,
    mask: usize,
}

// Safety: The SPSC protocol ensures that the producer exclusively
// writes to buffer[head] and the consumer exclusively reads from
// buffer[tail]. The atomic head/tail indices with Acquire/Release
// ordering provide the necessary synchronization.
unsafe impl<T: Send> Sync for Shared<T> {}

/// Write half of the SPSC ring buffer. Held by the UI thread.
///
/// Not `Sync` because only one thread may produce.
pub struct Producer<T> {
    shared: Arc<Shared<T>>,
    cached_head: usize,
}

// Safety: Producer can be moved to another thread (Send) but not
// shared across threads (not Sync).
unsafe impl<T: Send> Send for Producer<T> {}

/// Read half of the SPSC ring buffer. Held by the audio callback thread.
///
/// Not `Sync` because only one thread may consume.
pub struct Consumer<T> {
    shared: Arc<Shared<T>>,
    cached_tail: usize,
}

// Safety: Consumer can be moved to another thread (Send) but not
// shared across threads (not Sync).
unsafe impl<T: Send> Send for Consumer<T> {}

impl<T> Producer<T> {
    /// Attempt to enqueue a value. Returns `Err(value)` if the
    /// buffer is full.
    pub fn push(&mut self, value: T) -> Result<(), T> {
        let head = self.cached_head;
        let next_head = (head + 1) & self.shared.mask;

        // Load tail with Acquire to see the consumer's latest reads.
        let tail = self.shared.tail.0.load(Ordering::Acquire);
        if next_head == tail {
            return Err(value);
        }

        // Write the value into the slot.
        unsafe {
            (*self.shared.buffer[head].get()).write(value);
        }

        // Publish the write by advancing head with Release ordering.
        self.shared.head.0.store(next_head, Ordering::Release);
        self.cached_head = next_head;
        Ok(())
    }
}

impl<T> Consumer<T> {
    /// Attempt to dequeue a value. Returns `None` if the buffer is empty.
    pub fn pop(&mut self) -> Option<T> {
        let tail = self.cached_tail;

        // Load head with Acquire to see the producer's latest writes.
        let head = self.shared.head.0.load(Ordering::Acquire);
        if tail == head {
            return None;
        }

        // Read the value from the slot.
        let value = unsafe { (*self.shared.buffer[tail].get()).assume_init_read() };

        // Publish the read by advancing tail with Release ordering.
        let next_tail = (tail + 1) & self.shared.mask;
        self.shared.tail.0.store(next_tail, Ordering::Release);
        self.cached_tail = next_tail;
        Some(value)
    }
}

/// Create a producer-consumer pair with the given capacity.
///
/// The capacity is rounded up to the next power of two. One slot is
/// reserved internally to distinguish full from empty, so the usable
/// capacity is `next_power_of_two(capacity + 1) - 1`.
pub fn ring_buffer<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    // Add 1 to requested capacity because one slot is sacrificed
    // for full/empty disambiguation.
    let cap = (capacity + 1).next_power_of_two();
    let buffer: Vec<UnsafeCell<MaybeUninit<T>>> = (0..cap)
        .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
        .collect();

    let shared = Arc::new(Shared {
        buffer: buffer.into_boxed_slice(),
        head: CacheAligned(AtomicUsize::new(0)),
        tail: CacheAligned(AtomicUsize::new(0)),
        mask: cap - 1,
    });

    let producer = Producer {
        shared: Arc::clone(&shared),
        cached_head: 0,
    };
    let consumer = Consumer {
        shared,
        cached_tail: 0,
    };
    (producer, consumer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_single() {
        let (mut tx, mut rx) = ring_buffer::<u32>(4);
        assert!(rx.pop().is_none());
        assert!(tx.push(42).is_ok());
        assert_eq!(rx.pop(), Some(42));
        assert!(rx.pop().is_none());
    }

    #[test]
    fn push_pop_fifo_order() {
        let (mut tx, mut rx) = ring_buffer::<u32>(8);
        for i in 0..5 {
            assert!(tx.push(i).is_ok());
        }
        for i in 0..5 {
            assert_eq!(rx.pop(), Some(i));
        }
    }

    #[test]
    fn full_buffer_returns_err() {
        let (mut tx, mut _rx) = ring_buffer::<u32>(2);
        assert!(tx.push(1).is_ok());
        assert!(tx.push(2).is_ok());
        // Buffer should now be full (capacity rounded to 4, minus 1 = 3 usable).
        // We pushed 2, may have room for 1 more depending on rounding.
        // Keep pushing until we get Err.
        let mut count = 2;
        while tx.push(0).is_ok() {
            count += 1;
            assert!(count <= 10, "ring buffer should have bounded capacity");
        }
    }

    #[test]
    fn wrap_around() {
        let (mut tx, mut rx) = ring_buffer::<u32>(4);
        // Fill and drain multiple times to exercise wrap-around.
        for round in 0..3 {
            for i in 0..3 {
                assert!(tx.push(round * 10 + i).is_ok());
            }
            for i in 0..3 {
                assert_eq!(rx.pop(), Some(round * 10 + i));
            }
        }
    }

    #[test]
    fn threaded_transfer() {
        let (mut tx, mut rx) = ring_buffer::<u64>(1024);
        let count = 10_000u64;

        let producer = std::thread::spawn(move || {
            for i in 0..count {
                while tx.push(i).is_err() {
                    std::thread::yield_now();
                }
            }
        });

        let consumer = std::thread::spawn(move || {
            let mut received = 0u64;
            let mut expected = 0u64;
            while expected < count {
                if let Some(val) = rx.pop() {
                    assert_eq!(val, expected, "out of order at {expected}");
                    expected += 1;
                    received += 1;
                } else {
                    std::thread::yield_now();
                }
            }
            received
        });

        producer.join().unwrap();
        let received = consumer.join().unwrap();
        assert_eq!(received, count);
    }

    #[test]
    fn non_copy_type() {
        let (mut tx, mut rx) = ring_buffer::<String>(4);
        assert!(tx.push("hello".to_string()).is_ok());
        assert!(tx.push("world".to_string()).is_ok());
        assert_eq!(rx.pop(), Some("hello".to_string()));
        assert_eq!(rx.pop(), Some("world".to_string()));
    }

    #[test]
    fn boxed_closure() {
        let (mut tx, mut rx) =
            ring_buffer::<Box<dyn FnOnce() -> i32 + Send>>(4);
        assert!(tx.push(Box::new(|| 42)).is_ok());
        let f = rx.pop().unwrap();
        assert_eq!(f(), 42);
    }
}
