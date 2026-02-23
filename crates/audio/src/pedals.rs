//! Virtual guitar pedal chain.
//!
//! A configurable chain of audio effect pedals that processes the output
//! of the MS-20 voice. Pedals are processed in order, with each pedal
//! receiving the output of the previous one.
//!
//! ## Signal Path
//!
//! ```text
//! [MS-20 Voice] --> Pedal 1 --> Pedal 2 --> ... --> Pedal N --> Stereo Mix
//! ```
//!
//! ## Supported Effects (25 types)
//!
//! | Category | Effects |
//! |----------|---------|
//! | Distortion / Saturation | Overdrive, Distortion, Fuzz, Tape Saturation |
//! | Dynamic / Modulation | Auto-Wah, Manual Wah, Chorus, Flanger, Phaser, Tremolo |
//! | Time-Based | Digital Delay, Analog Delay, Ping-Pong Delay, Plate/Hall/Room Reverb |
//! | Special FX | Bitcrusher, Ring Modulator, Pitch Shifter, Auto-Pan |
//! | Utility | Noise Gate, Compressor, EQ, Volume, Buffer |

use alloc::vec::Vec;

use crate::effects::Pedal;

/// Ordered chain of effect pedals.
#[derive(Debug, Clone)]
pub struct PedalChain {
    /// Whether the pedal chain is active in the signal path.
    /// When inactive, process is a no-op passthrough.
    pub active: bool,
    /// The ordered sequence of effect pedals.
    pub pedals: Vec<Pedal>,
}

impl PedalChain {
    /// Create a new empty pedal chain (inactive by default).
    pub fn new() -> Self {
        Self {
            active: false,
            pedals: Vec::new(),
        }
    }

    /// Append a pedal to the end of the chain.
    pub fn push(&mut self, pedal: Pedal) {
        self.pedals.push(pedal);
    }

    /// Remove all pedals from the chain.
    pub fn clear(&mut self) {
        self.pedals.clear();
    }

    /// Number of pedals in the chain.
    pub fn len(&self) -> usize {
        self.pedals.len()
    }

    /// Whether the chain contains no pedals.
    pub fn is_empty(&self) -> bool {
        self.pedals.is_empty()
    }

    /// Process a mono audio buffer through all pedals in order.
    ///
    /// When inactive or empty, the buffer is left unchanged (passthrough).
    pub fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.active || self.pedals.is_empty() {
            return;
        }
        for pedal in &mut self.pedals {
            pedal.process(buffer, sample_rate);
        }
    }

    /// Reset all pedal state (clear delay lines, filter memories, etc.).
    pub fn reset(&mut self) {
        for pedal in &mut self.pedals {
            pedal.reset();
        }
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
    use crate::effects::{self, Volume};

    #[test]
    fn new_pedal_chain_inactive() {
        let chain = PedalChain::new();
        assert!(!chain.active);
        assert!(chain.is_empty());
    }

    #[test]
    fn inactive_chain_is_passthrough() {
        let mut chain = PedalChain::new();
        chain.push(Pedal::Volume(Volume::new(48000.0)));
        // Chain has pedals but is inactive
        let mut buffer = [0.75f32; 16];
        let original = buffer;
        chain.process(&mut buffer, 48000);
        for (a, b) in buffer.iter().zip(original.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn active_empty_chain_is_passthrough() {
        let mut chain = PedalChain::new();
        chain.active = true;
        let mut buffer = [0.5f32; 16];
        let original = buffer;
        chain.process(&mut buffer, 48000);
        for (a, b) in buffer.iter().zip(original.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn active_chain_applies_effects() {
        let mut chain = PedalChain::new();
        chain.active = true;
        let mut vol = effects::Volume::new(48000.0);
        vol.gain = 0.5;
        chain.push(Pedal::Volume(vol));

        let mut buffer = [1.0f32; 16];
        chain.process(&mut buffer, 48000);
        for s in &buffer {
            assert!((s - 0.5).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn chain_processes_in_order() {
        let mut chain = PedalChain::new();
        chain.active = true;
        // First pedal: halve the signal
        let mut vol1 = effects::Volume::new(48000.0);
        vol1.gain = 0.5;
        chain.push(Pedal::Volume(vol1));
        // Second pedal: double the signal
        let mut vol2 = effects::Volume::new(48000.0);
        vol2.gain = 2.0;
        chain.push(Pedal::Volume(vol2));
        // Net effect should be unity
        let mut buffer = [0.6f32; 16];
        chain.process(&mut buffer, 48000);
        for s in &buffer {
            assert!((s - 0.6).abs() < 1e-5);
        }
    }

    #[test]
    fn push_and_clear() {
        let mut chain = PedalChain::new();
        chain.push(Pedal::Volume(effects::Volume::new(48000.0)));
        chain.push(Pedal::Buffer(effects::Buffer::new(48000.0)));
        assert_eq!(chain.len(), 2);
        chain.clear();
        assert!(chain.is_empty());
    }

    #[test]
    fn default_matches_new() {
        let a = PedalChain::new();
        let b = PedalChain::default();
        assert_eq!(a.active, b.active);
        assert_eq!(a.pedals.len(), b.pedals.len());
    }
}
