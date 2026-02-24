/// Audio sample input for a channel.
///
/// Allows recorded sound effects, voice samples, or other pre-recorded audio
/// to be fed into the channel signal chain. The sample input sits before the
/// MS-20 voice in the signal path, meaning samples can be processed through
/// the full synth and pedal chain.
///
/// ## Signal Path
///
/// ```text
/// Sample Audio --> [MS-20 Voice] --> [Pedal Chain] --> Stereo Mix
/// ```
///
/// ## Current State
///
/// Stub implementation. The `process` method is a no-op passthrough. Future
/// milestones will implement sample loading, playback position tracking,
/// looping, and sample rate conversion.
#[derive(Debug, Clone)]
pub struct SampleInput {
    /// Whether this sample input is actively providing audio.
    pub active: bool,
}

impl SampleInput {
    /// Create a new inactive sample input.
    pub fn new() -> Self {
        Self { active: false }
    }

    /// Process a mono audio buffer in place.
    ///
    /// Stub: no-op passthrough. When active in a future implementation,
    /// this will mix sample audio into the buffer.
    pub fn process(&mut self, _buffer: &mut [f64]) {
        // Stub: passthrough, no modification
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
    }

    #[test]
    fn process_is_passthrough() {
        let mut input = SampleInput::new();
        let mut buffer = [0.5f64; 16];
        let original = buffer;
        input.process(&mut buffer);
        for (a, b) in buffer.iter().zip(original.iter()) {
            assert!((a - b).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn default_matches_new() {
        let a = SampleInput::new();
        let b = SampleInput::default();
        assert_eq!(a.active, b.active);
    }
}
