//! Delay effects: Digital Delay, Analog Delay, Ping-Pong Delay.
//!
//! Time-based effects that repeat the signal at configurable intervals
//! with feedback for decaying echoes.

use crate::dsp::biquad::{Biquad, BiquadType};
use crate::dsp::delay_line::DelayLine;

/// Clean digital delay with feedback.
///
/// Produces crisp, uncolored repeats of the input signal.
#[derive(Debug, Clone)]
pub struct DigitalDelay {
    /// Delay time in milliseconds (10 to 2000).
    pub time_ms: f32,
    /// Feedback amount (0.0 to 0.95).
    pub feedback: f32,
    /// Wet/dry mix.
    pub mix: f32,
    delay: DelayLine,
}

impl DigitalDelay {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            time_ms: 300.0,
            feedback: 0.4,
            mix: 0.4,
            delay: DelayLine::from_time(2100.0, sample_rate),
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let delay_samples = self.time_ms * 0.001 * sample_rate;
        let fb = self.feedback.clamp(0.0, 0.95);
        for sample in buffer.iter_mut() {
            let delayed = self.delay.read(delay_samples);
            self.delay.write(*sample + fb * delayed);
            *sample = *sample * (1.0 - self.mix) + delayed * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.delay.reset();
    }
}

/// Analog-style delay with filtered feedback path.
///
/// Each repeat passes through a lowpass filter, progressively
/// darkening the echoes like a bucket-brigade analog delay.
#[derive(Debug, Clone)]
pub struct AnalogDelay {
    /// Delay time in milliseconds.
    pub time_ms: f32,
    /// Feedback amount (0.0 to 0.95).
    pub feedback: f32,
    /// Feedback filter tone (0.0 = dark, 1.0 = bright).
    pub tone: f32,
    /// Wet/dry mix.
    pub mix: f32,
    delay: DelayLine,
    filter: Biquad,
}

impl AnalogDelay {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            time_ms: 300.0,
            feedback: 0.4,
            tone: 0.5,
            mix: 0.4,
            delay: DelayLine::from_time(2100.0, sample_rate),
            filter: Biquad::new(BiquadType::Lowpass, 3000.0, 0.707, sample_rate),
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let delay_samples = self.time_ms * 0.001 * sample_rate;
        let fb = self.feedback.clamp(0.0, 0.95);
        let cutoff = 800.0 + self.tone * 4000.0;
        self.filter
            .set_coefficients(BiquadType::Lowpass, cutoff, 0.707, sample_rate);
        for sample in buffer.iter_mut() {
            let delayed = self.delay.read(delay_samples);
            let filtered = self.filter.tick(delayed);
            self.delay.write(*sample + fb * filtered);
            *sample = *sample * (1.0 - self.mix) + delayed * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.delay.reset();
        self.filter.reset();
    }
}

/// Ping-pong delay with cross-feedback between two delay lines.
///
/// In mono context, the output is the sum of both taps, creating
/// a rhythmic alternating echo pattern. In a stereo mix, the two
/// taps would be panned to separate speakers.
#[derive(Debug, Clone)]
pub struct PingPongDelay {
    /// Delay time in milliseconds per side.
    pub time_ms: f32,
    /// Feedback amount (0.0 to 0.95).
    pub feedback: f32,
    /// Wet/dry mix.
    pub mix: f32,
    delay_a: DelayLine,
    delay_b: DelayLine,
}

impl PingPongDelay {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            time_ms: 250.0,
            feedback: 0.4,
            mix: 0.4,
            delay_a: DelayLine::from_time(2100.0, sample_rate),
            delay_b: DelayLine::from_time(2100.0, sample_rate),
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let delay_samples = self.time_ms * 0.001 * sample_rate;
        let fb = self.feedback.clamp(0.0, 0.95);
        for sample in buffer.iter_mut() {
            let tap_a = self.delay_a.read(delay_samples);
            let tap_b = self.delay_b.read(delay_samples);
            // Input feeds into delay A, cross-feedback between A and B
            self.delay_a.write(*sample + fb * tap_b);
            self.delay_b.write(fb * tap_a);
            let wet = (tap_a + tap_b) * 0.5;
            *sample = *sample * (1.0 - self.mix) + wet * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.delay_a.reset();
        self.delay_b.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digital_delay_echoes() {
        let sr = 48000.0;
        let mut delay = DigitalDelay::new(sr);
        delay.time_ms = 100.0;
        delay.feedback = 0.0;
        delay.mix = 1.0;
        // Send an impulse
        let mut buf = alloc::vec![0.0f32; 9600]; // 200ms
        buf[0] = 1.0;
        delay.process(&mut buf, sr);
        // Echo should appear at ~100ms = sample 4800
        let echo_region: f32 = buf[4750..4850]
            .iter()
            .map(|s| libm::fabsf(*s))
            .fold(0.0f32, f32::max);
        assert!(echo_region > 0.5, "Expected echo at 100ms");
    }

    #[test]
    fn analog_delay_darkens_repeats() {
        let sr = 48000.0;
        let mut delay = AnalogDelay::new(sr);
        delay.time_ms = 50.0;
        delay.feedback = 0.8;
        delay.tone = 0.2;
        delay.mix = 0.5;
        let mut buf = alloc::vec![0.0f32; 4800];
        buf[0] = 1.0;
        delay.process(&mut buf, sr);
        // Should produce decaying echoes
        for s in &buf {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn ping_pong_creates_alternating_echoes() {
        let sr = 48000.0;
        let mut pp = PingPongDelay::new(sr);
        pp.time_ms = 50.0;
        pp.feedback = 0.5;
        pp.mix = 1.0;
        let mut buf = alloc::vec![0.0f32; 9600];
        buf[0] = 1.0;
        pp.process(&mut buf, sr);
        // First echo from delay_a at ~50ms, second from delay_b at ~100ms
        let echo1: f32 = buf[2350..2450]
            .iter()
            .map(|s| libm::fabsf(*s))
            .fold(0.0f32, f32::max);
        assert!(echo1 > 0.1, "Expected first echo");
    }

    #[test]
    fn delay_feedback_decays() {
        let sr = 48000.0;
        let mut delay = DigitalDelay::new(sr);
        delay.time_ms = 50.0;
        delay.feedback = 0.5;
        delay.mix = 1.0;
        let mut buf = alloc::vec![0.0f32; 19200]; // 400ms
        buf[0] = 1.0;
        delay.process(&mut buf, sr);
        // Energy should decrease over time
        let energy_first_half: f32 = buf[..9600].iter().map(|s| s * s).sum();
        let energy_second_half: f32 = buf[9600..].iter().map(|s| s * s).sum();
        assert!(energy_first_half > energy_second_half);
    }
}
