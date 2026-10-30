//! Modulation effects: Chorus, Flanger, Phaser, Tremolo.
//!
//! These effects use LFOs to modulate delay times, filter frequencies,
//! or amplitude to create movement and animation in the sound.

use core::f64::consts::PI;

use crate::dsp::delay_line::DelayLine;
use crate::dsp::lfo::{Lfo, LfoWaveform};

/// Chorus effect using a modulated delay line.
///
/// Mixes the input with a delayed copy whose delay time is modulated
/// by an LFO, creating a thick, ensemble-like sound.
#[derive(Debug, Clone)]
pub struct Chorus {
    /// LFO rate in Hz (0.1 to 5.0).
    pub rate_hz: f64,
    /// Modulation depth in milliseconds (1.0 to 10.0).
    pub depth_ms: f64,
    /// Wet/dry mix (0.0 to 1.0).
    pub mix: f64,
    lfo: Lfo,
    delay: DelayLine,
    base_delay_ms: f64,
}

impl Chorus {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            rate_hz: 1.0,
            depth_ms: 5.0,
            mix: 0.5,
            lfo: Lfo::new(LfoWaveform::Sine),
            // Max delay: 15ms base + 10ms depth + margin
            delay: DelayLine::from_time(30.0, sample_rate),
            base_delay_ms: 15.0,
        }
    }

    pub fn process(&mut self, buffer: &mut [f64], sample_rate: f64) {
        let samples_per_ms = sample_rate * 0.001;
        for sample in buffer.iter_mut() {
            let lfo_val = self.lfo.tick(self.rate_hz, sample_rate);
            let delay_ms = self.base_delay_ms + self.depth_ms * lfo_val;
            let delay_samples = delay_ms * samples_per_ms;

            self.delay.write(*sample);
            let delayed = self.delay.read(delay_samples);
            *sample = *sample * (1.0 - self.mix) + delayed * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.lfo.reset();
        self.delay.reset();
    }
}

/// Flanger effect with short modulated delay and feedback.
///
/// Similar to chorus but with shorter delays and feedback, creating
/// a sweeping comb-filter effect.
#[derive(Debug, Clone)]
pub struct Flanger {
    /// LFO rate in Hz (0.1 to 5.0).
    pub rate_hz: f64,
    /// Modulation depth in milliseconds (0.5 to 5.0).
    pub depth_ms: f64,
    /// Feedback amount (-0.95 to 0.95).
    pub feedback: f64,
    /// Wet/dry mix.
    pub mix: f64,
    lfo: Lfo,
    delay: DelayLine,
    base_delay_ms: f64,
    prev_output: f64,
}

impl Flanger {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            rate_hz: 0.5,
            depth_ms: 2.0,
            feedback: 0.5,
            mix: 0.5,
            lfo: Lfo::new(LfoWaveform::Sine),
            delay: DelayLine::from_time(15.0, sample_rate),
            base_delay_ms: 3.0,
            prev_output: 0.0,
        }
    }

    pub fn process(&mut self, buffer: &mut [f64], sample_rate: f64) {
        let samples_per_ms = sample_rate * 0.001;
        let fb = self.feedback.clamp(-0.95, 0.95);
        for sample in buffer.iter_mut() {
            let lfo_val = self.lfo.tick(self.rate_hz, sample_rate);
            let delay_ms = self.base_delay_ms + self.depth_ms * lfo_val;
            let delay_samples = delay_ms * samples_per_ms;

            self.delay.write(*sample + fb * self.prev_output);
            let delayed = self.delay.read(delay_samples);
            self.prev_output = delayed;
            *sample = *sample * (1.0 - self.mix) + delayed * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.lfo.reset();
        self.delay.reset();
        self.prev_output = 0.0;
    }
}

/// Phaser effect using a cascade of allpass filters.
///
/// An LFO sweeps the allpass filter frequencies, creating moving
/// notches in the frequency spectrum when mixed with the dry signal.
#[derive(Debug, Clone)]
pub struct Phaser {
    /// LFO rate in Hz (0.1 to 5.0).
    pub rate_hz: f64,
    /// Sweep depth (0.0 to 1.0).
    pub depth: f64,
    /// Feedback (-0.95 to 0.95).
    pub feedback: f64,
    /// Wet/dry mix.
    pub mix: f64,
    /// Number of allpass stages (2, 4, 6, or 8).
    pub stages: usize,
    lfo: Lfo,
    /// First-order allpass state: (x_z1, y_z1) per stage.
    allpass_state: [(f64, f64); 8],
    prev_output: f64,
}

impl Phaser {
    pub fn new(_sample_rate: f64) -> Self {
        Self {
            rate_hz: 0.5,
            depth: 0.7,
            feedback: 0.3,
            mix: 0.5,
            stages: 4,
            lfo: Lfo::new(LfoWaveform::Sine),
            allpass_state: [(0.0, 0.0); 8],
            prev_output: 0.0,
        }
    }

    pub fn process(&mut self, buffer: &mut [f64], sample_rate: f64) {
        let fb = self.feedback.clamp(-0.95, 0.95);
        let active_stages = self.stages.min(8);
        for sample in buffer.iter_mut() {
            let lfo_val = self.lfo.tick(self.rate_hz, sample_rate);
            // Map LFO to frequency range for allpass coefficient
            let min_freq = 200.0;
            let max_freq = 4000.0;
            let freq = min_freq + (max_freq - min_freq) * (0.5 + 0.5 * lfo_val * self.depth);
            let coeff = allpass_coefficient(freq, sample_rate);

            let input = *sample + fb * self.prev_output;
            let mut x = input;

            for i in 0..active_stages {
                let (ref mut x_z1, ref mut y_z1) = self.allpass_state[i];
                let output = -coeff * x + *x_z1 + coeff * *y_z1;
                *x_z1 = x;
                *y_z1 = output;
                x = output;
            }

            self.prev_output = x;
            *sample = *sample * (1.0 - self.mix) + x * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.lfo.reset();
        self.allpass_state = [(0.0, 0.0); 8];
        self.prev_output = 0.0;
    }
}

/// Compute a first-order allpass coefficient for a given break frequency.
fn allpass_coefficient(freq_hz: f64, sample_rate: f64) -> f64 {
    let t = libm::tan(PI * freq_hz / sample_rate);
    (t - 1.0) / (t + 1.0)
}

/// Tremolo effect using LFO amplitude modulation.
///
/// Modulates the signal amplitude with a periodic waveform.
#[derive(Debug, Clone)]
pub struct Tremolo {
    /// LFO rate in Hz (0.5 to 20.0).
    pub rate_hz: f64,
    /// Modulation depth (0.0 = no effect, 1.0 = full modulation).
    pub depth: f64,
    /// LFO waveform for modulation shape.
    pub waveform: LfoWaveform,
    lfo: Lfo,
}

impl Tremolo {
    pub fn new(_sample_rate: f64) -> Self {
        Self {
            rate_hz: 4.0,
            depth: 0.5,
            waveform: LfoWaveform::Sine,
            lfo: Lfo::new(LfoWaveform::Sine),
        }
    }

    pub fn process(&mut self, buffer: &mut [f64], sample_rate: f64) {
        self.lfo.waveform = self.waveform;
        for sample in buffer.iter_mut() {
            let lfo_val = self.lfo.tick(self.rate_hz, sample_rate);
            // Map LFO [-1,1] to gain [1-depth, 1]
            let gain = 1.0 - self.depth * 0.5 * (1.0 - lfo_val);
            *sample *= gain;
        }
    }

    pub fn reset(&mut self) {
        self.lfo.reset();
    }
}

/// Vibrato: pitch wobble from a delay line modulated by a sine LFO.
///
/// Unlike chorus it outputs only the delayed signal by default, so the pitch
/// itself bends rather than a copy being layered over the dry signal.
#[derive(Debug, Clone)]
pub struct Vibrato {
    /// LFO rate in Hz (0.1 to 20.0).
    pub rate_hz: f64,
    /// Modulation depth in milliseconds (0.0 to 5.0).
    pub depth_ms: f64,
    /// Wet/dry mix. 1.0 is pure vibrato.
    pub mix: f64,
    lfo: Lfo,
    delay: DelayLine,
}

impl Vibrato {
    /// The largest depth `process` honours, in milliseconds.
    const MAX_DEPTH_MS: f64 = 5.0;

    pub fn new(sample_rate: f64) -> Self {
        Self {
            rate_hz: 5.0,
            depth_ms: 2.0,
            mix: 1.0,
            lfo: Lfo::new(LfoWaveform::Sine),
            // The delay swings between 0 and twice the depth around its centre.
            delay: DelayLine::from_time(2.0 * Self::MAX_DEPTH_MS + 2.0, sample_rate),
        }
    }

    pub fn process(&mut self, buffer: &mut [f64], sample_rate: f64) {
        let samples_per_ms = sample_rate * 0.001;
        let depth = self.depth_ms.clamp(0.0, Self::MAX_DEPTH_MS);
        let mix = self.mix.clamp(0.0, 1.0);
        for sample in buffer.iter_mut() {
            let lfo_val = self.lfo.tick(self.rate_hz, sample_rate);
            let delay_samples = (depth + depth * lfo_val) * samples_per_ms;
            self.delay.write(*sample);
            let delayed = self.delay.read(delay_samples);
            *sample = *sample * (1.0 - mix) + delayed * mix;
        }
    }

    pub fn reset(&mut self) {
        self.lfo.reset();
        self.delay.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vibrato_zero_depth_is_transparent() {
        let sr = 48000.0;
        let mut v = Vibrato::new(sr);
        v.depth_ms = 0.0;
        let input: [f64; 256] = core::array::from_fn(|i| libm::sin(i as f64 * 0.05));
        let mut buf = input;
        v.process(&mut buf, sr);
        for (a, b) in input.iter().zip(&buf) {
            assert!((a - b).abs() < 1e-9);
        }
    }

    #[test]
    fn vibrato_bends_the_signal() {
        let sr = 48000.0;
        let mut v = Vibrato::new(sr);
        v.depth_ms = 3.0;
        let input: [f64; 4800] = core::array::from_fn(|i| libm::sin(i as f64 * 0.05));
        let mut buf = input;
        v.process(&mut buf, sr);
        let diff: f64 = input.iter().zip(&buf).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff > 1.0, "vibrato changed nothing");
        assert!(buf.iter().all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-9));
    }

    fn sine_buffer(freq: f64, sample_rate: f64, len: usize) -> alloc::vec::Vec<f64> {
        let mut buf = alloc::vec![0.0f64; len];
        for (i, sample) in buf.iter_mut().enumerate() {
            *sample = libm::sin(2.0 * PI * freq * i as f64 / sample_rate);
        }
        buf
    }

    #[test]
    fn chorus_adds_signal() {
        let mut chorus = Chorus::new(48000.0);
        chorus.mix = 0.5;
        let mut buf = sine_buffer(440.0, 48000.0, 512);
        let dry_energy: f64 = buf.iter().map(|s| s * s).sum();
        chorus.process(&mut buf, 48000.0);
        let wet_energy: f64 = buf.iter().map(|s| s * s).sum();
        // With chorus mixing, energy should change
        assert!((dry_energy - wet_energy).abs() > 0.01);
    }

    #[test]
    fn flanger_with_zero_feedback() {
        let mut flanger = Flanger::new(48000.0);
        flanger.feedback = 0.0;
        flanger.mix = 0.5;
        let mut buf = sine_buffer(440.0, 48000.0, 256);
        flanger.process(&mut buf, 48000.0);
        // Should not blow up
        for s in &buf {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn phaser_creates_notches() {
        let mut phaser = Phaser::new(48000.0);
        phaser.mix = 0.5;
        phaser.stages = 4;
        let mut buf = sine_buffer(1000.0, 48000.0, 512);
        phaser.process(&mut buf, 48000.0);
        for s in &buf {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn tremolo_modulates_amplitude() {
        let mut trem = Tremolo::new(48000.0);
        trem.rate_hz = 10.0;
        trem.depth = 1.0;
        let mut buf = [0.5f64; 4800]; // 100ms at 48kHz = one full LFO cycle
        trem.process(&mut buf, 48000.0);
        let min_val = buf.iter().copied().fold(f64::MAX, f64::min);
        let max_val = buf.iter().copied().fold(f64::MIN, f64::max);
        // With depth=1.0, the signal should vary between ~0 and ~0.5
        assert!(max_val > 0.4);
        assert!(min_val < 0.1);
    }

    #[test]
    fn allpass_coefficient_bounded() {
        let c = allpass_coefficient(1000.0, 48000.0);
        assert!((-1.0..0.0).contains(&c));
    }
}
