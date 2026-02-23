//! Distortion and saturation effects.
//!
//! Four waveshaper variants with different clipping characteristics:
//! - Overdrive: smooth soft clipping via hyperbolic tangent
//! - Distortion: hard clipping at a threshold
//! - Fuzz: asymmetric extreme clipping
//! - Tape Saturation: gentle saturation curve

use crate::dsp::biquad::{Biquad, BiquadType};

/// Overdrive pedal with tanh soft clipping.
///
/// Applies gain to the input signal, then clips smoothly using tanh.
/// A one-pole lowpass tone filter shapes the output brightness.
#[derive(Debug, Clone)]
pub struct Overdrive {
    /// Input gain (1.0 to 20.0).
    pub gain: f32,
    /// Tone control: lowpass cutoff as fraction of Nyquist (0.0 to 1.0).
    pub tone: f32,
    /// Wet/dry mix (0.0 = dry, 1.0 = fully wet).
    pub mix: f32,
    tone_filter: Biquad,
    cached_sample_rate: f32,
}

impl Overdrive {
    pub fn new(sample_rate: f32) -> Self {
        let cutoff = 4000.0;
        Self {
            gain: 4.0,
            tone: 0.5,
            mix: 1.0,
            tone_filter: Biquad::new(BiquadType::Lowpass, cutoff, 0.707, sample_rate),
            cached_sample_rate: sample_rate,
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        self.update_tone(sample_rate);
        let inv_gain = 1.0 / libm::tanhf(self.gain);
        for sample in buffer.iter_mut() {
            let dry = *sample;
            let driven = libm::tanhf(self.gain * dry) * inv_gain;
            let filtered = self.tone_filter.tick(driven);
            *sample = dry * (1.0 - self.mix) + filtered * self.mix;
        }
    }

    fn update_tone(&mut self, sample_rate: f32) {
        let cutoff = 500.0 + self.tone * 7500.0;
        if (sample_rate - self.cached_sample_rate).abs() > 1.0 {
            self.tone_filter
                .set_coefficients(BiquadType::Lowpass, cutoff, 0.707, sample_rate);
            self.cached_sample_rate = sample_rate;
        }
    }

    pub fn reset(&mut self) {
        self.tone_filter.reset();
    }
}

/// Hard clipping distortion pedal.
///
/// Applies gain then clips the signal at a fixed threshold.
#[derive(Debug, Clone)]
pub struct Distortion {
    /// Input gain (1.0 to 100.0).
    pub gain: f32,
    /// Clipping threshold (0.0 to 1.0).
    pub threshold: f32,
    /// Tone control (0.0 to 1.0).
    pub tone: f32,
    /// Wet/dry mix.
    pub mix: f32,
    tone_filter: Biquad,
    cached_sample_rate: f32,
}

impl Distortion {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            gain: 10.0,
            threshold: 0.5,
            tone: 0.5,
            mix: 1.0,
            tone_filter: Biquad::new(BiquadType::Lowpass, 4000.0, 0.707, sample_rate),
            cached_sample_rate: sample_rate,
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        self.update_tone(sample_rate);
        for sample in buffer.iter_mut() {
            let dry = *sample;
            let amplified = self.gain * dry;
            let clipped = amplified.clamp(-self.threshold, self.threshold);
            let normalized = if self.threshold > 0.0 {
                clipped / self.threshold
            } else {
                0.0
            };
            let filtered = self.tone_filter.tick(normalized);
            *sample = dry * (1.0 - self.mix) + filtered * self.mix;
        }
    }

    fn update_tone(&mut self, sample_rate: f32) {
        let cutoff = 500.0 + self.tone * 7500.0;
        if (sample_rate - self.cached_sample_rate).abs() > 1.0 {
            self.tone_filter
                .set_coefficients(BiquadType::Lowpass, cutoff, 0.707, sample_rate);
            self.cached_sample_rate = sample_rate;
        }
    }

    pub fn reset(&mut self) {
        self.tone_filter.reset();
    }
}

/// Fuzz pedal with asymmetric clipping.
///
/// The positive half-cycle clips harder than the negative, creating
/// the characteristic asymmetric fuzz tone with strong even harmonics.
#[derive(Debug, Clone)]
pub struct Fuzz {
    /// Input gain (1.0 to 200.0).
    pub gain: f32,
    /// Tone control (0.0 to 1.0).
    pub tone: f32,
    /// Wet/dry mix.
    pub mix: f32,
    tone_filter: Biquad,
    cached_sample_rate: f32,
}

impl Fuzz {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            gain: 50.0,
            tone: 0.5,
            mix: 1.0,
            tone_filter: Biquad::new(BiquadType::Lowpass, 3000.0, 0.707, sample_rate),
            cached_sample_rate: sample_rate,
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        self.update_tone(sample_rate);
        for sample in buffer.iter_mut() {
            let dry = *sample;
            let driven = self.gain * dry;
            // Asymmetric clipping: positive clips at 0.3, negative at 0.7
            let clipped = if driven > 0.0 {
                libm::fminf(driven, 0.3) / 0.3
            } else {
                libm::fmaxf(driven, -0.7) / 0.7
            };
            let filtered = self.tone_filter.tick(clipped);
            *sample = dry * (1.0 - self.mix) + filtered * self.mix;
        }
    }

    fn update_tone(&mut self, sample_rate: f32) {
        let cutoff = 300.0 + self.tone * 5000.0;
        if (sample_rate - self.cached_sample_rate).abs() > 1.0 {
            self.tone_filter
                .set_coefficients(BiquadType::Lowpass, cutoff, 0.707, sample_rate);
            self.cached_sample_rate = sample_rate;
        }
    }

    pub fn reset(&mut self) {
        self.tone_filter.reset();
    }
}

/// Tape saturation with gentle soft clipping.
///
/// Emulates the smooth compression of magnetic tape recording.
/// Uses x / (1 + |x|) for a gentle saturation curve.
#[derive(Debug, Clone)]
pub struct TapeSaturation {
    /// Drive amount (1.0 to 5.0).
    pub drive: f32,
    /// Warmth: lowpass cutoff as fraction (0.0 to 1.0).
    pub warmth: f32,
    /// Wet/dry mix.
    pub mix: f32,
    warmth_filter: Biquad,
    cached_sample_rate: f32,
}

impl TapeSaturation {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            drive: 2.0,
            warmth: 0.5,
            mix: 1.0,
            warmth_filter: Biquad::new(BiquadType::Lowpass, 6000.0, 0.707, sample_rate),
            cached_sample_rate: sample_rate,
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        self.update_warmth(sample_rate);
        for sample in buffer.iter_mut() {
            let dry = *sample;
            let driven = self.drive * dry;
            let saturated = driven / (1.0 + libm::fabsf(driven));
            let filtered = self.warmth_filter.tick(saturated);
            *sample = dry * (1.0 - self.mix) + filtered * self.mix;
        }
    }

    fn update_warmth(&mut self, sample_rate: f32) {
        let cutoff = 2000.0 + (1.0 - self.warmth) * 10000.0;
        if (sample_rate - self.cached_sample_rate).abs() > 1.0 {
            self.warmth_filter
                .set_coefficients(BiquadType::Lowpass, cutoff, 0.707, sample_rate);
            self.cached_sample_rate = sample_rate;
        }
    }

    pub fn reset(&mut self) {
        self.warmth_filter.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overdrive_soft_clips() {
        let mut od = Overdrive::new(48000.0);
        od.gain = 10.0;
        od.mix = 1.0;
        let mut buffer = [0.8f32; 64];
        od.process(&mut buffer, 48000.0);
        // Output should be bounded
        for s in &buffer[16..] {
            assert!(libm::fabsf(*s) <= 1.05);
        }
    }

    #[test]
    fn distortion_hard_clips() {
        let mut dist = Distortion::new(48000.0);
        dist.gain = 50.0;
        dist.threshold = 0.5;
        dist.mix = 1.0;
        let mut buffer = [0.5f32; 64];
        dist.process(&mut buffer, 48000.0);
        for s in &buffer[16..] {
            assert!(libm::fabsf(*s) <= 1.05);
        }
    }

    #[test]
    fn fuzz_asymmetric() {
        let mut fuzz = Fuzz::new(48000.0);
        // Use moderate gain so positive clips but negative does not fully
        fuzz.gain = 5.0;
        fuzz.mix = 1.0;
        let mut pos_buf = [0.1f32; 64];
        let mut neg_buf = [-0.1f32; 64];
        fuzz.process(&mut pos_buf, 48000.0);
        fuzz.reset();
        fuzz.process(&mut neg_buf, 48000.0);
        // driven=0.5: pos clips at 0.3 -> 1.0, neg clips at -0.7 -> 0.714
        let pos_avg: f32 = pos_buf[32..].iter().map(|s| libm::fabsf(*s)).sum::<f32>() / 32.0;
        let neg_avg: f32 = neg_buf[32..].iter().map(|s| libm::fabsf(*s)).sum::<f32>() / 32.0;
        assert!(
            (pos_avg - neg_avg).abs() > 0.01,
            "Fuzz should be asymmetric: pos_avg={}, neg_avg={}",
            pos_avg,
            neg_avg
        );
    }

    #[test]
    fn tape_saturation_gentle() {
        let mut tape = TapeSaturation::new(48000.0);
        tape.drive = 3.0;
        tape.mix = 1.0;
        let mut buffer = [0.5f32; 64];
        tape.process(&mut buffer, 48000.0);
        for s in &buffer[16..] {
            assert!(libm::fabsf(*s) <= 1.0);
        }
    }

    #[test]
    fn zero_mix_is_passthrough() {
        let mut od = Overdrive::new(48000.0);
        od.mix = 0.0;
        let mut buffer = [0.3f32; 32];
        od.process(&mut buffer, 48000.0);
        for s in &buffer {
            assert!((s - 0.3).abs() < 1e-5);
        }
    }
}
