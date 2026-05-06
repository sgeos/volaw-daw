//! Lookahead brickwall limiter for the master bus.
//!
//! Prevents the summed output from exceeding the ceiling (1.0 = 0 dBFS)
//! by detecting peaks before they arrive in the output and smoothly
//! reducing gain in advance.
//!
//! ## Algorithm
//!
//! A short delay (the "lookahead") buffers the audio. A sliding window
//! maximum over the lookahead period detects upcoming peaks. When a peak
//! would exceed the ceiling, gain is reduced immediately. When the peak
//! passes, gain recovers slowly via a one-pole release filter.
//!
//! Because the gain reduction is applied before the peak arrives at the
//! output (thanks to the delay), there is no clipping and no audible
//! distortion. The release is smooth enough to avoid pumping artifacts.
//!
//! ## Latency
//!
//! Adds 5 ms of latency (the lookahead window). The first 5 ms of output
//! after initialization will be silent while the delay buffers fill.

use alloc::collections::VecDeque;

use crate::dsp::delay_line::DelayLine;

/// Lookahead time in milliseconds.
const LOOKAHEAD_MS: f64 = 5.0;

/// Release time in milliseconds.
const RELEASE_MS: f64 = 100.0;

/// Output ceiling in linear amplitude.
const CEILING: f64 = 1.0;

/// Lookahead brickwall limiter.
///
/// Processes stereo interleaved audio in place, ensuring no sample
/// exceeds the ceiling. Uses a monotonic deque for O(1) amortized
/// sliding window peak detection.
#[derive(Debug, Clone)]
pub struct LookaheadLimiter {
    /// Output ceiling in linear amplitude.
    ceiling: f64,

    /// Lookahead window size in sample frames.
    lookahead: usize,

    /// One-pole release smoothing coefficient.
    release_coeff: f64,

    /// Lookahead delay buffer for the left channel.
    delay_left: DelayLine,

    /// Lookahead delay buffer for the right channel.
    delay_right: DelayLine,

    /// Monotonic deque for sliding window maximum.
    /// Each entry is (frame_index, peak_value). Values are strictly
    /// decreasing from front to back. The front holds the window max.
    peak_deque: VecDeque<(usize, f64)>,

    /// Monotonically increasing frame counter for deque expiry.
    frame_counter: usize,

    /// Current smoothed gain multiplier (0.0 to 1.0).
    gain: f64,
}

impl LookaheadLimiter {
    /// Create a new limiter configured for the given sample rate.
    pub fn new(sample_rate: f64) -> Self {
        let lookahead = (LOOKAHEAD_MS * 0.001 * sample_rate) as usize;
        let lookahead = lookahead.max(1);
        let release_coeff = if RELEASE_MS > 0.0 && sample_rate > 0.0 {
            libm::exp(-1.0 / (RELEASE_MS * 0.001 * sample_rate))
        } else {
            0.0
        };

        Self {
            ceiling: CEILING,
            lookahead,
            release_coeff,
            // Delay line needs lookahead + 1 slots so read_integer(lookahead)
            // reaches the oldest sample in the window.
            delay_left: DelayLine::new(lookahead + 1),
            delay_right: DelayLine::new(lookahead + 1),
            peak_deque: VecDeque::with_capacity(lookahead + 1),
            frame_counter: 0,
            gain: 1.0,
        }
    }

    /// Process a stereo interleaved buffer in place: `[L, R, L, R, ...]`.
    ///
    /// Each stereo frame is delayed by the lookahead window. Peaks are
    /// detected in advance and gain is reduced before they arrive in the
    /// output. The same gain is applied to both channels to preserve the
    /// stereo image.
    pub fn process_interleaved(&mut self, buffer: &mut [f64]) {
        let num_frames = buffer.len() / 2;

        for i in 0..num_frames {
            let left = buffer[i * 2];
            let right = buffer[i * 2 + 1];

            debug_assert!(
                left.is_finite() && right.is_finite(),
                "Limiter input non-finite at frame {i}: L={left} R={right}"
            );

            // Write current samples into delay lines.
            self.delay_left.write(left);
            self.delay_right.write(right);

            // Frame peak (stereo-linked detection).
            let peak = libm::fabs(left).max(libm::fabs(right));

            // Update monotonic deque: pop smaller values from the back.
            while let Some(&(_, v)) = self.peak_deque.back() {
                if v <= peak {
                    self.peak_deque.pop_back();
                } else {
                    break;
                }
            }
            self.peak_deque.push_back((self.frame_counter, peak));

            // Expire entries outside the lookahead window.
            while let Some(&(t, _)) = self.peak_deque.front() {
                if self.frame_counter - t >= self.lookahead {
                    self.peak_deque.pop_front();
                } else {
                    break;
                }
            }

            // Window maximum.
            let window_max = self.peak_deque.front().map(|&(_, v)| v).unwrap_or(0.0);

            // Target gain: reduce only when window max exceeds ceiling.
            let target_gain = if window_max > self.ceiling {
                self.ceiling / window_max
            } else {
                1.0
            };

            // Gain smoothing: instant attack, slow release.
            if target_gain < self.gain {
                self.gain = target_gain;
            } else {
                self.gain =
                    self.release_coeff * self.gain + (1.0 - self.release_coeff) * target_gain;
            }

            // Read delayed samples and apply gain.
            let delayed_left = self.delay_left.read_integer(self.lookahead);
            let delayed_right = self.delay_right.read_integer(self.lookahead);

            buffer[i * 2] = delayed_left * self.gain;
            buffer[i * 2 + 1] = delayed_right * self.gain;

            self.frame_counter += 1;
        }
    }

    /// Reset all internal state to initial conditions.
    pub fn reset(&mut self) {
        self.delay_left.reset();
        self.delay_right.reset();
        self.peak_deque.clear();
        self.frame_counter = 0;
        self.gain = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a stereo interleaved buffer from left and right slices.
    fn interleave(left: &[f64], right: &[f64]) -> alloc::vec::Vec<f64> {
        let len = left.len().min(right.len());
        let mut buf = alloc::vec::Vec::with_capacity(len * 2);
        for i in 0..len {
            buf.push(left[i]);
            buf.push(right[i]);
        }
        buf
    }

    /// Helper: deinterleave a stereo buffer into separate left and right.
    fn deinterleave(buf: &[f64]) -> (alloc::vec::Vec<f64>, alloc::vec::Vec<f64>) {
        let frames = buf.len() / 2;
        let mut left = alloc::vec::Vec::with_capacity(frames);
        let mut right = alloc::vec::Vec::with_capacity(frames);
        for i in 0..frames {
            left.push(buf[i * 2]);
            right.push(buf[i * 2 + 1]);
        }
        (left, right)
    }

    #[test]
    fn below_ceiling_passes_through() {
        let sr = 48000.0;
        let mut limiter = LookaheadLimiter::new(sr);
        let lookahead = limiter.lookahead;

        // Signal well below ceiling.
        let amplitude = 0.5;
        let frames = lookahead + 512;
        let left: alloc::vec::Vec<f64> = (0..frames).map(|_| amplitude).collect();
        let right: alloc::vec::Vec<f64> = (0..frames).map(|_| amplitude).collect();
        let mut buf = interleave(&left, &right);

        limiter.process_interleaved(&mut buf);
        let (out_l, out_r) = deinterleave(&buf);

        // After the lookahead fill period, output should match input.
        for i in lookahead..frames {
            assert!(
                (out_l[i] - amplitude).abs() < 0.01,
                "Left frame {i}: expected {amplitude}, got {}",
                out_l[i]
            );
            assert!(
                (out_r[i] - amplitude).abs() < 0.01,
                "Right frame {i}: expected {amplitude}, got {}",
                out_r[i]
            );
        }
    }

    #[test]
    fn above_ceiling_is_limited() {
        let sr = 48000.0;
        let mut limiter = LookaheadLimiter::new(sr);
        let lookahead = limiter.lookahead;

        // Signal at 4x ceiling.
        let amplitude = 4.0;
        let frames = lookahead + 512;
        let left: alloc::vec::Vec<f64> = (0..frames).map(|_| amplitude).collect();
        let right: alloc::vec::Vec<f64> = (0..frames).map(|_| amplitude).collect();
        let mut buf = interleave(&left, &right);

        limiter.process_interleaved(&mut buf);
        let (out_l, out_r) = deinterleave(&buf);

        // After the fill period, no sample should exceed the ceiling.
        for i in lookahead..frames {
            assert!(
                out_l[i].abs() <= 1.0 + 1e-6,
                "Left frame {i} exceeds ceiling: {}",
                out_l[i]
            );
            assert!(
                out_r[i].abs() <= 1.0 + 1e-6,
                "Right frame {i} exceeds ceiling: {}",
                out_r[i]
            );
        }
    }

    #[test]
    fn stereo_image_preserved() {
        let sr = 48000.0;
        let mut limiter = LookaheadLimiter::new(sr);
        let lookahead = limiter.lookahead;

        // Left at 3.0, right at 1.5 (2:1 ratio).
        let frames = lookahead + 512;
        let left: alloc::vec::Vec<f64> = (0..frames).map(|_| 3.0).collect();
        let right: alloc::vec::Vec<f64> = (0..frames).map(|_| 1.5).collect();
        let mut buf = interleave(&left, &right);

        limiter.process_interleaved(&mut buf);
        let (out_l, out_r) = deinterleave(&buf);

        // After fill, the left:right ratio should be preserved.
        for i in (lookahead + 100)..frames {
            if out_l[i].abs() > 1e-6 && out_r[i].abs() > 1e-6 {
                let ratio = out_l[i] / out_r[i];
                assert!(
                    (ratio - 2.0).abs() < 0.1,
                    "Frame {i}: ratio {ratio}, expected 2.0"
                );
            }
        }
    }

    #[test]
    fn no_nan_or_inf() {
        let sr = 48000.0;
        let mut limiter = LookaheadLimiter::new(sr);

        // Feed extreme values.
        let frames = 2048;
        let mut buf = alloc::vec![0.0_f64; frames * 2];
        for i in 0..frames {
            buf[i * 2] = 1000.0 * libm::sin(i as f64 * 0.1);
            buf[i * 2 + 1] = -500.0 * libm::cos(i as f64 * 0.1);
        }

        limiter.process_interleaved(&mut buf);

        for (i, sample) in buf.iter().enumerate() {
            assert!(sample.is_finite(), "Sample {i} is not finite: {sample}");
        }
    }

    #[test]
    fn gain_recovers_after_transient() {
        let sr = 48000.0;
        let mut limiter = LookaheadLimiter::new(sr);
        let lookahead = limiter.lookahead;

        // Build a signal: loud burst then quiet.
        let burst_frames = 480; // 10ms burst
        let quiet_frames = 24000; // 500ms quiet
        let total = lookahead + burst_frames + quiet_frames;
        let mut left = alloc::vec![0.0_f64; total];
        let mut right = alloc::vec![0.0_f64; total];

        // Loud burst at 5x ceiling.
        for i in 0..burst_frames {
            left[lookahead + i] = 5.0;
            right[lookahead + i] = 5.0;
        }
        // Quiet signal after burst.
        for i in 0..quiet_frames {
            left[lookahead + burst_frames + i] = 0.3;
            right[lookahead + burst_frames + i] = 0.3;
        }

        let mut buf = interleave(&left, &right);
        limiter.process_interleaved(&mut buf);
        let (out_l, _out_r) = deinterleave(&buf);

        // At the end of the quiet period, gain should have recovered
        // close to 1.0, so output should be close to 0.3.
        let tail_start = total - 4800; // last 100ms
        let tail_avg: f64 =
            out_l[tail_start..total].iter().sum::<f64>() / (total - tail_start) as f64;
        assert!(
            (tail_avg - 0.3).abs() < 0.05,
            "Gain did not recover: tail average {tail_avg}, expected ~0.3"
        );
    }

    #[test]
    fn reset_restores_initial_state() {
        let sr = 48000.0;
        let mut limiter = LookaheadLimiter::new(sr);

        // Process some loud audio.
        let mut buf = alloc::vec![5.0_f64; 2048];
        limiter.process_interleaved(&mut buf);

        // Reset and verify gain is back to 1.0.
        limiter.reset();
        assert!((limiter.gain - 1.0).abs() < f64::EPSILON);
        assert_eq!(limiter.frame_counter, 0);
        assert!(limiter.peak_deque.is_empty());
    }
}
