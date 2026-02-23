//! Dynamic processing effects: Noise Gate, Compressor.

use crate::dsp::envelope::EnvelopeFollower;

/// Noise gate / suppressor.
///
/// Silences the signal when its amplitude falls below a threshold.
/// Uses attack and release times for smooth gating transitions.
#[derive(Debug, Clone)]
pub struct NoiseGate {
    /// Gate threshold (0.0 to 1.0).
    pub threshold: f32,
    /// Attack time in milliseconds.
    pub attack_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
    envelope: EnvelopeFollower,
    gate_level: f32,
}

impl NoiseGate {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            threshold: 0.01,
            attack_ms: 0.5,
            release_ms: 20.0,
            envelope: EnvelopeFollower::new(0.5, 20.0, sample_rate),
            gate_level: 0.0,
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        self.envelope
            .set_times(self.attack_ms, self.release_ms, sample_rate);
        for sample in buffer.iter_mut() {
            let env = self.envelope.tick(*sample);
            // Smooth gate transition
            let target = if env > self.threshold { 1.0 } else { 0.0 };
            // Simple one-pole smoothing to avoid clicks
            let coeff = if target > self.gate_level {
                0.99
            } else {
                0.995
            };
            self.gate_level = coeff * self.gate_level + (1.0 - coeff) * target;
            *sample *= self.gate_level;
        }
    }

    pub fn reset(&mut self) {
        self.envelope.reset();
        self.gate_level = 0.0;
    }
}

/// Compressor / limiter.
///
/// Reduces the dynamic range of the signal by attenuating levels
/// above a threshold. Includes makeup gain to restore perceived volume.
#[derive(Debug, Clone)]
pub struct Compressor {
    /// Threshold level (0.0 to 1.0).
    pub threshold: f32,
    /// Compression ratio (1.0 = no compression, 20.0 = heavy limiting).
    pub ratio: f32,
    /// Attack time in milliseconds.
    pub attack_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
    /// Makeup gain (linear, typically 1.0 to 4.0).
    pub makeup_gain: f32,
    envelope: EnvelopeFollower,
}

impl Compressor {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            threshold: 0.5,
            ratio: 4.0,
            attack_ms: 5.0,
            release_ms: 50.0,
            makeup_gain: 1.5,
            envelope: EnvelopeFollower::new(5.0, 50.0, sample_rate),
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        self.envelope
            .set_times(self.attack_ms, self.release_ms, sample_rate);
        for sample in buffer.iter_mut() {
            let env = self.envelope.tick(*sample);
            let gain = if env > self.threshold && self.ratio > 1.0 {
                // Compute gain reduction in linear domain
                let excess = env - self.threshold;
                let compressed_excess = excess / self.ratio;
                let compressed_level = self.threshold + compressed_excess;
                if env > 0.0 {
                    compressed_level / env
                } else {
                    1.0
                }
            } else {
                1.0
            };
            *sample *= gain * self.makeup_gain;
        }
    }

    pub fn reset(&mut self) {
        self.envelope.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_gate_silences_quiet_signal() {
        let mut gate = NoiseGate::new(48000.0);
        gate.threshold = 0.1;
        let mut buf = [0.01f32; 4800]; // Very quiet
        gate.process(&mut buf, 48000.0);
        // After settling, should be near silence
        let tail_energy: f32 = buf[2400..].iter().map(|s| s * s).sum();
        assert!(tail_energy < 0.001);
    }

    #[test]
    fn noise_gate_passes_loud_signal() {
        let mut gate = NoiseGate::new(48000.0);
        gate.threshold = 0.1;
        let mut buf = [0.5f32; 4800];
        gate.process(&mut buf, 48000.0);
        // Loud signal should pass through
        let avg: f32 = buf[2400..].iter().sum::<f32>() / 2400.0;
        assert!(avg > 0.3);
    }

    #[test]
    fn compressor_reduces_loud_peaks() {
        let mut comp = Compressor::new(48000.0);
        comp.threshold = 0.3;
        comp.ratio = 10.0;
        comp.makeup_gain = 1.0;
        let mut buf = [0.8f32; 4800];
        comp.process(&mut buf, 48000.0);
        // After settling, peaks should be reduced
        let peak = buf[2400..]
            .iter()
            .map(|s| libm::fabsf(*s))
            .fold(0.0f32, f32::max);
        assert!(peak < 0.8, "Compressor should reduce peaks, got {}", peak);
    }

    #[test]
    fn compressor_passes_quiet_signal() {
        let mut comp = Compressor::new(48000.0);
        comp.threshold = 0.5;
        comp.ratio = 4.0;
        comp.makeup_gain = 1.0;
        let mut buf = [0.1f32; 2400];
        comp.process(&mut buf, 48000.0);
        // Quiet signal should be largely unchanged
        for s in &buf[1200..] {
            assert!((s - 0.1).abs() < 0.05);
        }
    }
}
