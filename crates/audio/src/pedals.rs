/// Virtual guitar pedal chain.
///
/// A configurable chain of audio effect pedals that processes the output
/// of the MS-20 voice. The simplest use case is sending the MS-20 audio
/// through the pedal chain for additional coloring and effects.
///
/// ## Signal Path
///
/// ```text
/// [MS-20 Voice] --> Pedal 1 --> Pedal 2 --> ... --> Pedal N --> Stereo Mix
/// ```
///
/// ## Planned Effects
///
/// Future milestones will implement common guitar pedal effects:
/// - Overdrive / Distortion
/// - Delay (analog-style)
/// - Chorus / Flanger
/// - Reverb
/// - Phaser
/// - Tremolo
/// - Compressor
///
/// ## Current State
///
/// Stub implementation. The `process` method is a no-op passthrough.
#[derive(Debug, Clone)]
pub struct PedalChain {
    /// Whether the pedal chain is active in the signal path.
    pub active: bool,
}

impl PedalChain {
    /// Create a new inactive pedal chain.
    pub fn new() -> Self {
        Self { active: false }
    }

    /// Process a mono audio buffer in place.
    ///
    /// Stub: no-op passthrough. When active in a future implementation,
    /// this will apply the chain of effects sequentially.
    pub fn process(&mut self, _buffer: &mut [f32]) {
        // Stub: passthrough, no modification
    }
}

impl Default for PedalChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_pedal_chain_inactive() {
        let chain = PedalChain::new();
        assert!(!chain.active);
    }

    #[test]
    fn process_is_passthrough() {
        let mut chain = PedalChain::new();
        let mut buffer = [0.75f32; 16];
        let original = buffer;
        chain.process(&mut buffer);
        for (a, b) in buffer.iter().zip(original.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn default_matches_new() {
        let a = PedalChain::new();
        let b = PedalChain::default();
        assert_eq!(a.active, b.active);
    }
}
