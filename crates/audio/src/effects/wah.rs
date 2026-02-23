//! Wah and envelope filter effects.
//!
//! - Auto-Wah: envelope-controlled bandpass filter
//! - Wah Manual: position-controlled bandpass filter

use crate::dsp::biquad::{Biquad, BiquadType};
use crate::dsp::envelope::EnvelopeFollower;

/// Auto-wah / envelope filter pedal.
///
/// The input signal amplitude controls the center frequency of a
/// bandpass filter, creating a dynamic "wah" effect that responds
/// to playing dynamics.
#[derive(Debug, Clone)]
pub struct AutoWah {
    /// Envelope sensitivity (0.0 to 1.0).
    pub sensitivity: f32,
    /// Minimum filter frequency in Hz.
    pub min_freq: f32,
    /// Maximum filter frequency in Hz.
    pub max_freq: f32,
    /// Bandpass Q factor.
    pub q: f32,
    /// Wet/dry mix.
    pub mix: f32,
    envelope: EnvelopeFollower,
    filter: Biquad,
}

impl AutoWah {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sensitivity: 0.7,
            min_freq: 200.0,
            max_freq: 2500.0,
            q: 3.0,
            mix: 1.0,
            envelope: EnvelopeFollower::new(5.0, 50.0, sample_rate),
            filter: Biquad::new(BiquadType::Bandpass, 500.0, 3.0, sample_rate),
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        for sample in buffer.iter_mut() {
            let dry = *sample;
            let env = self.envelope.tick(dry);
            let freq =
                self.min_freq + (self.max_freq - self.min_freq) * (env * self.sensitivity).min(1.0);
            self.filter
                .set_coefficients(BiquadType::Bandpass, freq, self.q, sample_rate);
            let wet = self.filter.tick(dry);
            *sample = dry * (1.0 - self.mix) + wet * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.envelope.reset();
        self.filter.reset();
    }
}

/// Manual wah pedal with position control.
///
/// A bandpass filter whose center frequency is controlled by a
/// position parameter, emulating a physical wah pedal rocker.
#[derive(Debug, Clone)]
pub struct WahManual {
    /// Pedal position (0.0 = heel down, 1.0 = toe down).
    pub position: f32,
    /// Minimum filter frequency in Hz.
    pub min_freq: f32,
    /// Maximum filter frequency in Hz.
    pub max_freq: f32,
    /// Bandpass Q factor.
    pub q: f32,
    /// Wet/dry mix.
    pub mix: f32,
    filter: Biquad,
}

impl WahManual {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            position: 0.5,
            min_freq: 200.0,
            max_freq: 2500.0,
            q: 3.0,
            mix: 1.0,
            filter: Biquad::new(BiquadType::Bandpass, 1000.0, 3.0, sample_rate),
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let freq = self.min_freq + (self.max_freq - self.min_freq) * self.position;
        self.filter
            .set_coefficients(BiquadType::Bandpass, freq, self.q, sample_rate);
        for sample in buffer.iter_mut() {
            let dry = *sample;
            let wet = self.filter.tick(dry);
            *sample = dry * (1.0 - self.mix) + wet * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.filter.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f32::consts::PI;

    #[test]
    fn auto_wah_responds_to_dynamics() {
        let mut wah = AutoWah::new(48000.0);
        wah.sensitivity = 1.0;
        wah.mix = 1.0;
        // Feed loud signal
        let mut loud = [0.8f32; 128];
        wah.process(&mut loud, 48000.0);
        wah.reset();
        // Feed quiet signal
        let mut quiet = [0.05f32; 128];
        wah.process(&mut quiet, 48000.0);
        // Loud signal should produce different output due to envelope tracking
        let loud_avg: f32 = loud[64..].iter().map(|s| libm::fabsf(*s)).sum::<f32>() / 64.0;
        let quiet_avg: f32 = quiet[64..].iter().map(|s| libm::fabsf(*s)).sum::<f32>() / 64.0;
        assert!(loud_avg > quiet_avg);
    }

    #[test]
    fn manual_wah_position_affects_tone() {
        let sr = 48000.0;
        let freq = 1000.0;
        // Generate test tone
        let mut buf_heel = [0.0f32; 256];
        let mut buf_toe = [0.0f32; 256];
        for i in 0..256 {
            let val = libm::sinf(2.0 * PI * freq * i as f32 / sr);
            buf_heel[i] = val;
            buf_toe[i] = val;
        }
        let mut wah_heel = WahManual::new(sr);
        wah_heel.position = 0.0;
        wah_heel.mix = 1.0;
        wah_heel.process(&mut buf_heel, sr);

        let mut wah_toe = WahManual::new(sr);
        wah_toe.position = 1.0;
        wah_toe.mix = 1.0;
        wah_toe.process(&mut buf_toe, sr);

        // Different positions should produce different amplitudes for same tone
        let rms_heel: f32 =
            libm::sqrtf(buf_heel[128..].iter().map(|s| s * s).sum::<f32>() / 128.0);
        let rms_toe: f32 =
            libm::sqrtf(buf_toe[128..].iter().map(|s| s * s).sum::<f32>() / 128.0);
        assert!(
            (rms_heel - rms_toe).abs() > 0.001,
            "Different positions should produce different tones"
        );
    }
}
