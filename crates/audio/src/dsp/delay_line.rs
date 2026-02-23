//! Ring buffer delay line with linear interpolation.
//!
//! Used by delay effects, chorus, flanger, and reverb algorithms.
//! The delay line stores samples in a circular buffer and supports
//! reading at fractional delay positions via linear interpolation.

use alloc::vec;
use alloc::vec::Vec;

/// Circular buffer delay line.
#[derive(Debug, Clone)]
pub struct DelayLine {
    buffer: Vec<f32>,
    write_pos: usize,
}

impl DelayLine {
    /// Create a new delay line with the given maximum length in samples.
    ///
    /// The buffer is initialized to silence. A minimum size of 1 sample
    /// is enforced.
    pub fn new(max_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; max_samples.max(1)],
            write_pos: 0,
        }
    }

    /// Create a delay line sized for a given maximum time in milliseconds.
    pub fn from_time(max_ms: f32, sample_rate: f32) -> Self {
        let samples = (max_ms * 0.001 * sample_rate) as usize + 1;
        Self::new(samples)
    }

    /// Write a sample and advance the write pointer.
    #[inline]
    pub fn write(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos += 1;
        if self.write_pos >= self.buffer.len() {
            self.write_pos = 0;
        }
    }

    /// Read at a fractional delay position with linear interpolation.
    ///
    /// A delay of 0.0 returns the most recently written sample.
    /// The delay is clamped to valid range.
    #[inline]
    pub fn read(&self, delay_samples: f32) -> f32 {
        let len = self.buffer.len();
        let max_delay = (len - 1) as f32;
        let clamped = delay_samples.clamp(0.0, max_delay);
        let floor = clamped as usize;
        let frac = clamped - floor as f32;

        let idx0 = self.read_index(floor);
        if frac < f32::EPSILON {
            return self.buffer[idx0];
        }
        let idx1 = self.read_index(floor + 1);
        self.buffer[idx0] + frac * (self.buffer[idx1] - self.buffer[idx0])
    }

    /// Read at an integer delay position (no interpolation).
    #[inline]
    pub fn read_integer(&self, delay_samples: usize) -> f32 {
        let idx = self.read_index(delay_samples.min(self.buffer.len() - 1));
        self.buffer[idx]
    }

    /// Compute the buffer index for reading at a given delay.
    #[inline]
    fn read_index(&self, delay: usize) -> usize {
        let len = self.buffer.len();
        (self.write_pos + len - 1 - delay % len) % len
    }

    /// Reset the delay line to silence.
    pub fn reset(&mut self) {
        for s in self.buffer.iter_mut() {
            *s = 0.0;
        }
        self.write_pos = 0;
    }

    /// Maximum delay length in samples.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether the delay line buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_read_zero_delay() {
        let mut dl = DelayLine::new(16);
        dl.write(0.75);
        assert!((dl.read(0.0) - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn read_integer_delay() {
        let mut dl = DelayLine::new(16);
        dl.write(1.0);
        dl.write(2.0);
        dl.write(3.0);
        assert!((dl.read_integer(0) - 3.0).abs() < f32::EPSILON);
        assert!((dl.read_integer(1) - 2.0).abs() < f32::EPSILON);
        assert!((dl.read_integer(2) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fractional_delay_interpolates() {
        let mut dl = DelayLine::new(16);
        dl.write(0.0);
        dl.write(1.0);
        // Delay 0.5 should interpolate between 1.0 (delay=0) and 0.0 (delay=1)
        let val = dl.read(0.5);
        assert!((val - 0.5).abs() < 1e-5);
    }

    #[test]
    fn wraps_around() {
        let mut dl = DelayLine::new(4);
        for i in 0..8 {
            dl.write(i as f32);
        }
        // Most recent sample should be 7.0
        assert!((dl.read(0.0) - 7.0).abs() < f32::EPSILON);
        assert!((dl.read_integer(3) - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn reset_clears_buffer() {
        let mut dl = DelayLine::new(8);
        dl.write(1.0);
        dl.write(2.0);
        dl.reset();
        assert!(dl.read(0.0).abs() < f32::EPSILON);
        assert!(dl.read_integer(1).abs() < f32::EPSILON);
    }

    #[test]
    fn from_time_computes_correct_size() {
        let dl = DelayLine::from_time(100.0, 48000.0);
        // 100ms at 48kHz = 4800 samples + 1
        assert_eq!(dl.len(), 4801);
    }
}
