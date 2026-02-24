//! Second-order IIR biquad filter using Transposed Direct Form II.
//!
//! Coefficients are calculated using formulas from Robert Bristow-Johnson's
//! Audio EQ Cookbook. This is the standard digital filter building block for
//! audio equalization, tone controls, and frequency-selective effects.
//!
//! ## Filter Types
//!
//! - Lowpass, Highpass, Bandpass, Notch
//! - Peaking EQ (boost or cut at a center frequency)
//! - Low shelf, High shelf

use core::f64::consts::PI;

/// Filter type with associated parameters for coefficient calculation.
#[derive(Debug, Clone, Copy)]
pub enum BiquadType {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Peaking { gain_db: f64 },
    LowShelf { gain_db: f64 },
    HighShelf { gain_db: f64 },
}

/// Second-order IIR biquad filter.
#[derive(Debug, Clone)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    /// Create a new biquad filter with the given type and parameters.
    pub fn new(filter_type: BiquadType, freq_hz: f64, q: f64, sample_rate: f64) -> Self {
        let mut bq = Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        };
        bq.set_coefficients(filter_type, freq_hz, q, sample_rate);
        bq
    }

    /// Create a passthrough filter (unity gain, no filtering).
    pub fn passthrough() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Recalculate filter coefficients for new parameters.
    pub fn set_coefficients(
        &mut self,
        filter_type: BiquadType,
        freq_hz: f64,
        q: f64,
        sample_rate: f64,
    ) {
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        let cos_w0 = libm::cos(w0);
        let sin_w0 = libm::sin(w0);
        let alpha = sin_w0 / (2.0 * q);

        let (b0, b1, b2, a0, a1, a2) = match filter_type {
            BiquadType::Lowpass => {
                let b1 = 1.0 - cos_w0;
                let b0 = b1 * 0.5;
                (b0, b1, b0, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
            }
            BiquadType::Highpass => {
                let b1 = -(1.0 + cos_w0);
                let b0 = (1.0 + cos_w0) * 0.5;
                (b0, b1, b0, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
            }
            BiquadType::Bandpass => (
                alpha,
                0.0,
                -alpha,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            BiquadType::Notch => (
                1.0,
                -2.0 * cos_w0,
                1.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            BiquadType::Peaking { gain_db } => {
                let a = libm::pow(10.0, gain_db / 40.0);
                (
                    1.0 + alpha * a,
                    -2.0 * cos_w0,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cos_w0,
                    1.0 - alpha / a,
                )
            }
            BiquadType::LowShelf { gain_db } => {
                let a = libm::pow(10.0, gain_db / 40.0);
                let s = 2.0 * libm::sqrt(a) * alpha;
                let am1 = a - 1.0;
                let ap1 = a + 1.0;
                (
                    a * (ap1 - am1 * cos_w0 + s),
                    2.0 * a * (am1 - ap1 * cos_w0),
                    a * (ap1 - am1 * cos_w0 - s),
                    ap1 + am1 * cos_w0 + s,
                    -2.0 * (am1 + ap1 * cos_w0),
                    ap1 + am1 * cos_w0 - s,
                )
            }
            BiquadType::HighShelf { gain_db } => {
                let a = libm::pow(10.0, gain_db / 40.0);
                let s = 2.0 * libm::sqrt(a) * alpha;
                let am1 = a - 1.0;
                let ap1 = a + 1.0;
                (
                    a * (ap1 + am1 * cos_w0 + s),
                    -2.0 * a * (am1 + ap1 * cos_w0),
                    a * (ap1 + am1 * cos_w0 - s),
                    ap1 - am1 * cos_w0 + s,
                    2.0 * (am1 - ap1 * cos_w0),
                    ap1 - am1 * cos_w0 - s,
                )
            }
        };

        let inv_a0 = 1.0 / a0;
        self.b0 = b0 * inv_a0;
        self.b1 = b1 * inv_a0;
        self.b2 = b2 * inv_a0;
        self.a1 = a1 * inv_a0;
        self.a2 = a2 * inv_a0;
    }

    /// Process a single sample through the filter.
    #[inline]
    pub fn tick(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.z1;
        self.z1 = self.b1 * input - self.a1 * output + self.z2;
        self.z2 = self.b2 * input - self.a2 * output;
        output
    }

    /// Process a buffer of samples in place.
    pub fn process(&mut self, buffer: &mut [f64]) {
        for sample in buffer.iter_mut() {
            *sample = self.tick(*sample);
        }
    }

    /// Reset the filter state to zero.
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_preserves_signal() {
        let mut bq = Biquad::passthrough();
        let mut buffer = [0.5f64, -0.3, 0.8, 0.0];
        let original = buffer;
        bq.process(&mut buffer);
        for (a, b) in buffer.iter().zip(original.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn lowpass_attenuates_high_frequency() {
        let mut lp = Biquad::new(BiquadType::Lowpass, 100.0, 0.707, 48000.0);
        // Feed a high-frequency signal (half Nyquist)
        let mut buffer = [0.0f64; 256];
        for (i, sample) in buffer.iter_mut().enumerate() {
            *sample = if i % 2 == 0 { 1.0 } else { -1.0 };
        }
        lp.process(&mut buffer);
        // After settling, the output amplitude should be much smaller
        let peak = buffer[200..].iter().map(|s| libm::fabs(*s)).fold(0.0f64, f64::max);
        assert!(peak < 0.1, "Expected attenuation, got peak {}", peak);
    }

    #[test]
    fn reset_clears_state() {
        let mut bq = Biquad::new(BiquadType::Lowpass, 1000.0, 1.0, 48000.0);
        bq.tick(1.0);
        bq.tick(0.5);
        bq.reset();
        assert!(bq.z1.abs() < f64::EPSILON);
        assert!(bq.z2.abs() < f64::EPSILON);
    }

    #[test]
    fn bandpass_passes_center_frequency() {
        let center = 1000.0;
        let sr = 48000.0;
        let mut bp = Biquad::new(BiquadType::Bandpass, center, 1.0, sr);
        // Generate a sine wave at the center frequency
        let mut buffer = [0.0f64; 512];
        for (i, sample) in buffer.iter_mut().enumerate() {
            *sample = libm::sin(2.0 * PI * center * i as f64 / sr);
        }
        let input_rms: f64 = libm::sqrt(
            buffer.iter().map(|s| s * s).sum::<f64>() / buffer.len() as f64,
        );
        bp.process(&mut buffer);
        let output_rms: f64 = libm::sqrt(
            buffer[256..].iter().map(|s| s * s).sum::<f64>() / 256.0,
        );
        // Output should retain significant energy at center frequency
        assert!(output_rms > input_rms * 0.3, "BPF should pass center freq");
    }
}
