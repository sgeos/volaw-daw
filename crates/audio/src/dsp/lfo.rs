//! Low-frequency oscillator for modulation effects.
//!
//! Generates periodic waveforms at sub-audio rates for controlling
//! effect parameters like delay time (chorus, flanger), filter
//! frequency (phaser, auto-wah), and amplitude (tremolo, auto-pan).

use core::f32::consts::PI;

/// LFO waveform shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoWaveform {
    Sine,
    Triangle,
    Square,
    Sawtooth,
}

/// Low-frequency oscillator with configurable waveform.
#[derive(Debug, Clone)]
pub struct Lfo {
    phase: f32,
    pub waveform: LfoWaveform,
}

impl Lfo {
    /// Create a new LFO with the given waveform, starting at phase 0.
    pub fn new(waveform: LfoWaveform) -> Self {
        Self {
            phase: 0.0,
            waveform,
        }
    }

    /// Advance by one sample and return the current value in [-1.0, 1.0].
    #[inline]
    pub fn tick(&mut self, rate_hz: f32, sample_rate: f32) -> f32 {
        let value = self.evaluate();
        self.phase += rate_hz / sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        value
    }

    /// Return the current value without advancing phase.
    fn evaluate(&self) -> f32 {
        match self.waveform {
            LfoWaveform::Sine => libm::sinf(2.0 * PI * self.phase),
            LfoWaveform::Triangle => {
                if self.phase < 0.25 {
                    4.0 * self.phase
                } else if self.phase < 0.75 {
                    2.0 - 4.0 * self.phase
                } else {
                    -4.0 + 4.0 * self.phase
                }
            }
            LfoWaveform::Square => {
                if self.phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LfoWaveform::Sawtooth => 2.0 * self.phase - 1.0,
        }
    }

    /// Reset phase to zero.
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_starts_at_zero() {
        let mut lfo = Lfo::new(LfoWaveform::Sine);
        let val = lfo.tick(1.0, 48000.0);
        assert!(val.abs() < 1e-5);
    }

    #[test]
    fn triangle_starts_at_zero() {
        let mut lfo = Lfo::new(LfoWaveform::Triangle);
        let val = lfo.tick(1.0, 48000.0);
        assert!(val.abs() < 1e-5);
    }

    #[test]
    fn square_starts_positive() {
        let mut lfo = Lfo::new(LfoWaveform::Square);
        let val = lfo.tick(1.0, 48000.0);
        assert!((val - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn sawtooth_starts_negative() {
        let mut lfo = Lfo::new(LfoWaveform::Sawtooth);
        let val = lfo.tick(1.0, 48000.0);
        assert!((val - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn lfo_wraps_phase() {
        let mut lfo = Lfo::new(LfoWaveform::Sine);
        // Run for more than one full cycle
        for _ in 0..96000 {
            lfo.tick(1.0, 48000.0);
        }
        // Phase should have wrapped and still produce valid output
        let val = lfo.tick(1.0, 48000.0);
        assert!((-1.0..=1.0).contains(&val));
    }

    #[test]
    fn reset_returns_to_start() {
        let mut lfo = Lfo::new(LfoWaveform::Sine);
        for _ in 0..100 {
            lfo.tick(10.0, 48000.0);
        }
        lfo.reset();
        let val = lfo.tick(1.0, 48000.0);
        assert!(val.abs() < 1e-5);
    }
}
