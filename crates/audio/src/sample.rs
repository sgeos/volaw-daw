extern crate alloc;
use alloc::collections::VecDeque;

/// Audio sample input for a channel.
///
/// Allows recorded sound effects, voice samples, or other pre-recorded audio
/// to be fed into the channel signal chain. The sample input sits before the
/// MS-20 voice in the signal path, meaning samples can be processed through
/// the full synth and pedal chain via the external signal processor input.
///
/// ## Signal Path
///
/// ```text
/// Sample Audio --> [MS-20 Voice] --> [Pedal Chain] --> Stereo Mix
/// ```
///
/// ## Usage
///
/// Feed PCM data into the sample input using [`feed_samples`](SampleInput::feed_samples)
/// or [`feed_sample`](SampleInput::feed_sample). During rendering, one f64 value
/// is consumed per audio tick (48kHz default). If the queue is empty, the output
/// defaults to 0.0 (silence).
#[derive(Debug, Clone)]
pub struct SampleInput {
    /// Whether this sample input is actively providing audio.
    pub active: bool,
    /// Queue of f64 samples waiting to be consumed during rendering.
    queue: VecDeque<f64>,
}

impl SampleInput {
    /// Create a new inactive sample input.
    pub fn new() -> Self {
        Self {
            active: false,
            queue: VecDeque::new(),
        }
    }

    /// Feed a single f64 sample into the input queue.
    pub fn feed_sample(&mut self, sample: f64) {
        self.queue.push_back(sample);
    }

    /// Feed a slice of f64 samples into the input queue.
    pub fn feed_samples(&mut self, samples: &[f64]) {
        self.queue.extend(samples.iter().copied());
    }

    /// Return the number of queued samples available for consumption.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Clear all queued samples.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Process a mono audio buffer in place.
    ///
    /// When active, consumes one f64 from the queue per buffer position.
    /// If the queue is empty, writes 0.0 (silence). When inactive, writes
    /// 0.0 for every position.
    pub fn process(&mut self, buffer: &mut [f64]) {
        if !self.active {
            for s in buffer.iter_mut() {
                *s = 0.0;
            }
            return;
        }
        for s in buffer.iter_mut() {
            *s = self.queue.pop_front().unwrap_or(0.0);
        }
    }
}

impl Default for SampleInput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sample_input_inactive() {
        let input = SampleInput::new();
        assert!(!input.active);
        assert_eq!(input.queued(), 0);
    }

    #[test]
    fn inactive_writes_silence() {
        let mut input = SampleInput::new();
        input.feed_samples(&[1.0, 2.0, 3.0]);
        let mut buffer = [9.9f64; 4];
        input.process(&mut buffer);
        for s in &buffer {
            assert!(
                s.abs() < f64::EPSILON,
                "inactive input should write silence"
            );
        }
        // Samples should still be in queue since inactive does not consume.
        assert_eq!(input.queued(), 3);
    }

    #[test]
    fn active_consumes_from_queue() {
        let mut input = SampleInput::new();
        input.active = true;
        input.feed_samples(&[0.1, 0.2, 0.3, 0.4]);
        let mut buffer = [0.0f64; 4];
        input.process(&mut buffer);
        assert!((buffer[0] - 0.1).abs() < f64::EPSILON);
        assert!((buffer[1] - 0.2).abs() < f64::EPSILON);
        assert!((buffer[2] - 0.3).abs() < f64::EPSILON);
        assert!((buffer[3] - 0.4).abs() < f64::EPSILON);
        assert_eq!(input.queued(), 0);
    }

    #[test]
    fn active_defaults_to_silence_when_empty() {
        let mut input = SampleInput::new();
        input.active = true;
        input.feed_samples(&[0.5, 0.6]);
        let mut buffer = [9.9f64; 4];
        input.process(&mut buffer);
        assert!((buffer[0] - 0.5).abs() < f64::EPSILON);
        assert!((buffer[1] - 0.6).abs() < f64::EPSILON);
        assert!(buffer[2].abs() < f64::EPSILON, "should default to 0.0");
        assert!(buffer[3].abs() < f64::EPSILON, "should default to 0.0");
    }

    #[test]
    fn feed_sample_one_at_a_time() {
        let mut input = SampleInput::new();
        input.active = true;
        input.feed_sample(0.7);
        input.feed_sample(0.8);
        assert_eq!(input.queued(), 2);
        let mut buffer = [0.0f64; 2];
        input.process(&mut buffer);
        assert!((buffer[0] - 0.7).abs() < f64::EPSILON);
        assert!((buffer[1] - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn clear_empties_queue() {
        let mut input = SampleInput::new();
        input.feed_samples(&[1.0, 2.0, 3.0]);
        assert_eq!(input.queued(), 3);
        input.clear();
        assert_eq!(input.queued(), 0);
    }

    #[test]
    fn multiple_process_calls_drain_queue() {
        let mut input = SampleInput::new();
        input.active = true;
        input.feed_samples(&[0.1, 0.2, 0.3, 0.4]);

        let mut buf1 = [0.0f64; 2];
        input.process(&mut buf1);
        assert!((buf1[0] - 0.1).abs() < f64::EPSILON);
        assert!((buf1[1] - 0.2).abs() < f64::EPSILON);

        let mut buf2 = [0.0f64; 2];
        input.process(&mut buf2);
        assert!((buf2[0] - 0.3).abs() < f64::EPSILON);
        assert!((buf2[1] - 0.4).abs() < f64::EPSILON);

        // Queue is now empty.
        let mut buf3 = [9.9f64; 2];
        input.process(&mut buf3);
        assert!(buf3[0].abs() < f64::EPSILON);
        assert!(buf3[1].abs() < f64::EPSILON);
    }

    #[test]
    fn default_matches_new() {
        let a = SampleInput::new();
        let b = SampleInput::default();
        assert_eq!(a.active, b.active);
        assert_eq!(a.queued(), b.queued());
    }
}
