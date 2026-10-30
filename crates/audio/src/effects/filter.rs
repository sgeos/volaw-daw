//! Filter effect: a resonant multimode filter with an optional LFO sweep.

use crate::dsp::biquad::{Biquad, BiquadType};
use crate::dsp::lfo::{Lfo, LfoWaveform};

/// Samples between coefficient updates while the LFO sweeps. Recomputing
/// every sample would cost a `cos` and a `sin` per sample for no audible gain.
const SWEEP_UPDATE_INTERVAL: usize = 32;

/// Resonant lowpass, highpass or bandpass filter.
///
/// With `lfo_depth` above zero the cutoff sweeps up and down by that many
/// octaves at `lfo_rate_hz`, the classic "auto filter" sound.
#[derive(Debug, Clone)]
pub struct Filter {
    /// 0 lowpass, 1 highpass, 2 bandpass. Other values round and clamp into
    /// that range.
    pub mode: f64,
    /// Cutoff frequency in Hz (20 to 20000).
    pub cutoff_hz: f64,
    /// Resonance as Q (0.5 to 20).
    pub q: f64,
    /// Sweep rate in Hz (0 to 20).
    pub lfo_rate_hz: f64,
    /// Sweep depth in octaves (0 to 4). Zero disables the sweep.
    pub lfo_depth: f64,
    /// Wet/dry mix.
    pub mix: f64,
    biquad: Biquad,
    lfo: Lfo,
    /// Parameters the coefficients were last computed from, so that a static
    /// filter does not recompute them every block.
    cached: Option<(u8, f64, f64, f64)>,
    countdown: usize,
}

impl Filter {
    pub fn new(sample_rate: f64) -> Self {
        let mut filter = Self {
            mode: 0.0,
            cutoff_hz: 1200.0,
            q: 2.0,
            lfo_rate_hz: 0.5,
            lfo_depth: 0.0,
            mix: 1.0,
            biquad: Biquad::passthrough(),
            lfo: Lfo::new(LfoWaveform::Sine),
            cached: None,
            countdown: 0,
        };
        filter.update(filter.cutoff_hz, sample_rate);
        filter
    }

    fn mode_index(&self) -> u8 {
        libm::round(self.mode.clamp(0.0, 2.0)) as u8
    }

    /// Recompute coefficients for `cutoff`, unless nothing changed.
    fn update(&mut self, cutoff: f64, sample_rate: f64) {
        let mode = self.mode_index();
        // Keep the cutoff below Nyquist, where the biquad formulas break down.
        let cutoff = cutoff.clamp(20.0, (sample_rate * 0.45).max(20.0));
        let q = self.q.clamp(0.5, 20.0);
        let key = (mode, cutoff, q, sample_rate);
        if self.cached == Some(key) {
            return;
        }
        let kind = match mode {
            0 => BiquadType::Lowpass,
            1 => BiquadType::Highpass,
            _ => BiquadType::Bandpass,
        };
        self.biquad.set_coefficients(kind, cutoff, q, sample_rate);
        self.cached = Some(key);
    }

    pub fn process(&mut self, buffer: &mut [f64], sample_rate: f64) {
        let mix = self.mix.clamp(0.0, 1.0);
        let depth = self.lfo_depth.clamp(0.0, 4.0);
        let rate = self.lfo_rate_hz.clamp(0.0, 20.0);
        if depth == 0.0 {
            self.update(self.cutoff_hz, sample_rate);
        }
        for sample in buffer.iter_mut() {
            if depth > 0.0 {
                let lfo_val = self.lfo.tick(rate, sample_rate);
                if self.countdown == 0 {
                    let swept = self.cutoff_hz * libm::exp2(depth * lfo_val);
                    self.update(swept, sample_rate);
                    self.countdown = SWEEP_UPDATE_INTERVAL;
                }
                self.countdown -= 1;
            }
            let dry = *sample;
            let wet = self.biquad.tick(dry);
            *sample = dry * (1.0 - mix) + wet * mix;
        }
    }

    pub fn reset(&mut self) {
        self.biquad.reset();
        self.lfo.reset();
        self.countdown = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::PI;

    const SR: f64 = 48000.0;

    /// Root-mean-square level of a sine at `freq` after the filter settles.
    fn level_through(filter: &mut Filter, freq: f64) -> f64 {
        let mut buf: [f64; 9600] =
            core::array::from_fn(|i| libm::sin(2.0 * PI * freq * i as f64 / SR));
        filter.process(&mut buf, SR);
        let tail = &buf[4800..];
        libm::sqrt(tail.iter().map(|s| s * s).sum::<f64>() / tail.len() as f64)
    }

    #[test]
    fn lowpass_passes_lows_and_cuts_highs() {
        let mut low = Filter::new(SR);
        let mut high = Filter::new(SR);
        assert!(level_through(&mut low, 100.0) > 0.6);
        assert!(level_through(&mut high, 10_000.0) < 0.05);
    }

    #[test]
    fn highpass_mode_inverts_that() {
        let mut low = Filter::new(SR);
        low.mode = 1.0;
        let mut high = Filter::new(SR);
        high.mode = 1.0;
        assert!(level_through(&mut low, 100.0) < 0.1);
        assert!(level_through(&mut high, 10_000.0) > 0.6);
    }

    #[test]
    fn out_of_range_parameters_stay_finite() {
        let mut f = Filter::new(SR);
        f.mode = 99.0;
        f.cutoff_hz = 1e9;
        f.q = -3.0;
        f.lfo_depth = 50.0;
        f.lfo_rate_hz = 1e6;
        let mut buf = [0.5f64; 4096];
        f.process(&mut buf, SR);
        assert!(buf.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn sweep_changes_the_tone_over_time() {
        let mut f = Filter::new(SR);
        f.lfo_depth = 3.0;
        f.lfo_rate_hz = 2.0;
        let mut buf: [f64; 24000] =
            core::array::from_fn(|i| libm::sin(2.0 * PI * 2000.0 * i as f64 / SR));
        f.process(&mut buf, SR);
        let rms = |s: &[f64]| libm::sqrt(s.iter().map(|x| x * x).sum::<f64>() / s.len() as f64);
        let levels: [f64; 8] = core::array::from_fn(|k| rms(&buf[k * 3000..(k + 1) * 3000]));
        let min = levels.iter().copied().fold(f64::MAX, f64::min);
        let max = levels.iter().copied().fold(f64::MIN, f64::max);
        assert!(max > 2.0 * min, "sweep did not move the level: {levels:?}");
    }
}
