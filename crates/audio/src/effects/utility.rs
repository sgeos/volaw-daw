//! Utility and signal management effects: EQ, Volume, Buffer.

use crate::dsp::biquad::{Biquad, BiquadType};

/// Three-band parametric equalizer.
///
/// Uses three biquad filters (low shelf, peaking mid, high shelf) for
/// frequency shaping.
#[derive(Debug, Clone)]
pub struct Eq {
    /// Low band gain in dB (-12.0 to 12.0).
    pub low_gain_db: f64,
    /// Mid band gain in dB (-12.0 to 12.0).
    pub mid_gain_db: f64,
    /// High band gain in dB (-12.0 to 12.0).
    pub high_gain_db: f64,
    /// Low band frequency in Hz.
    pub low_freq: f64,
    /// Mid band frequency in Hz.
    pub mid_freq: f64,
    /// High band frequency in Hz.
    pub high_freq: f64,
    low_filter: Biquad,
    mid_filter: Biquad,
    high_filter: Biquad,
    cached_sample_rate: f64,
}

impl Eq {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            low_gain_db: 0.0,
            mid_gain_db: 0.0,
            high_gain_db: 0.0,
            low_freq: 200.0,
            mid_freq: 1000.0,
            high_freq: 5000.0,
            low_filter: Biquad::new(
                BiquadType::LowShelf { gain_db: 0.0 },
                200.0,
                0.707,
                sample_rate,
            ),
            mid_filter: Biquad::new(
                BiquadType::Peaking { gain_db: 0.0 },
                1000.0,
                1.0,
                sample_rate,
            ),
            high_filter: Biquad::new(
                BiquadType::HighShelf { gain_db: 0.0 },
                5000.0,
                0.707,
                sample_rate,
            ),
            cached_sample_rate: sample_rate,
        }
    }

    pub fn process(&mut self, buffer: &mut [f64], sample_rate: f64) {
        self.update_coefficients(sample_rate);
        for sample in buffer.iter_mut() {
            let mut s = *sample;
            s = self.low_filter.tick(s);
            s = self.mid_filter.tick(s);
            s = self.high_filter.tick(s);
            *sample = s;
        }
    }

    fn update_coefficients(&mut self, sample_rate: f64) {
        self.low_filter.set_coefficients(
            BiquadType::LowShelf {
                gain_db: self.low_gain_db,
            },
            self.low_freq,
            0.707,
            sample_rate,
        );
        self.mid_filter.set_coefficients(
            BiquadType::Peaking {
                gain_db: self.mid_gain_db,
            },
            self.mid_freq,
            1.0,
            sample_rate,
        );
        self.high_filter.set_coefficients(
            BiquadType::HighShelf {
                gain_db: self.high_gain_db,
            },
            self.high_freq,
            0.707,
            sample_rate,
        );
        self.cached_sample_rate = sample_rate;
    }

    pub fn reset(&mut self) {
        self.low_filter.reset();
        self.mid_filter.reset();
        self.high_filter.reset();
    }
}

/// Volume / gain pedal.
///
/// Simple level control for the signal chain.
#[derive(Debug, Clone)]
pub struct Volume {
    /// Gain level (0.0 = silence, 1.0 = unity, 2.0 = +6dB).
    pub gain: f64,
}

impl Volume {
    pub fn new(_sample_rate: f64) -> Self {
        Self { gain: 1.0 }
    }

    pub fn process(&mut self, buffer: &mut [f64], _sample_rate: f64) {
        let g = self.gain;
        for sample in buffer.iter_mut() {
            *sample *= g;
        }
    }

    pub fn reset(&mut self) {
        // No state to reset.
    }
}

/// Buffer / line driver.
///
/// Provides impedance matching and a slight gain boost. In a digital
/// context, this is simply a unity or near-unity gain stage.
#[derive(Debug, Clone)]
pub struct Buffer {
    /// Gain (typically 0.9 to 1.1).
    pub gain: f64,
}

impl Buffer {
    pub fn new(_sample_rate: f64) -> Self {
        Self { gain: 1.0 }
    }

    pub fn process(&mut self, buffer: &mut [f64], _sample_rate: f64) {
        let g = self.gain;
        for sample in buffer.iter_mut() {
            *sample *= g;
        }
    }

    pub fn reset(&mut self) {
        // No state to reset.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq_flat_is_passthrough() {
        let mut eq = Eq::new(48000.0);
        eq.low_gain_db = 0.0;
        eq.mid_gain_db = 0.0;
        eq.high_gain_db = 0.0;
        let mut buf = [0.5f64; 128];
        eq.process(&mut buf, 48000.0);
        // With zero gain on all bands, output should be close to input
        // (after filter settling)
        for s in &buf[64..] {
            assert!((s - 0.5).abs() < 0.1, "Flat EQ should pass through");
        }
    }

    #[test]
    fn eq_boost_increases_level() {
        let sr = 48000.0;
        let mut eq = Eq::new(sr);
        eq.low_gain_db = 12.0;
        eq.mid_gain_db = 12.0;
        eq.high_gain_db = 12.0;
        // Feed a test signal
        let mut buf = [0.1f64; 256];
        eq.process(&mut buf, sr);
        let avg_out: f64 = buf[128..].iter().map(|s| libm::fabs(*s)).sum::<f64>() / 128.0;
        assert!(avg_out > 0.1, "Boosted EQ should increase level");
    }

    #[test]
    fn volume_scales_signal() {
        let mut vol = Volume::new(48000.0);
        vol.gain = 0.5;
        let mut buf = [1.0f64; 16];
        vol.process(&mut buf, 48000.0);
        for s in &buf {
            assert!((s - 0.5).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn volume_zero_silences() {
        let mut vol = Volume::new(48000.0);
        vol.gain = 0.0;
        let mut buf = [0.8f64; 16];
        vol.process(&mut buf, 48000.0);
        for s in &buf {
            assert!(s.abs() < f64::EPSILON);
        }
    }

    #[test]
    fn buffer_unity_gain() {
        let mut buf_pedal = Buffer::new(48000.0);
        let mut buf = [0.7f64; 16];
        buf_pedal.process(&mut buf, 48000.0);
        for s in &buf {
            assert!((s - 0.7).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn buffer_slight_boost() {
        let mut buf_pedal = Buffer::new(48000.0);
        buf_pedal.gain = 1.05;
        let mut buf = [0.5f64; 16];
        buf_pedal.process(&mut buf, 48000.0);
        for s in &buf {
            assert!((s - 0.525).abs() < 1e-5);
        }
    }
}
