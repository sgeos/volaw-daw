//! Virtual Controlled Oscillator (VCO) for the MS-20 synthesizer.
//!
//! Implements a homogenized VCO design that is a superset of both VCO1 and
//! VCO2 from the original Korg MS-20 hardware. Both VCOs in this
//! implementation are identical and support all waveform types.
//!
//! ## Waveforms
//!
//! | Waveform | Description |
//! |----------|-------------|
//! | Triangle | Bipolar triangle wave, optionally quantized |
//! | Sawtooth | Rising sawtooth wave, optionally quantized |
//! | PwSquare | Variable duty cycle pulse wave |
//! | Sine | Pure sine wave, optionally quantized |
//! | WhiteNoise | Pseudorandom noise (xorshift64 PRNG) |
//! | Pcm | Single-cycle wavetable from a PCM sample buffer |
//!
//! Ring modulation is applied at the synth mixing stage as a continuous
//! crossfade between the dry additive VCO mix and the product of both
//! VCO outputs. See `Ms20Voice::ring_mod_depth` in `synth.rs`.
//!
//! ## Amplitude Quantization
//!
//! Non-PW-Square waveforms support amplitude quantization for lo-fi effects.
//! The quantization level is specified as a bit depth:
//!
//! - `None`: Pure analog-style continuous waveform
//! - `Some(3)`: 8-level extreme lo-fi
//! - `Some(4)`: 16-level NES-style stepping
//! - `Some(8)`: 256-level subtle grit

use alloc::sync::Arc;
use alloc::vec::Vec;

/// Waveform type selection for a VCO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    /// Bipolar triangle wave (-1 to +1).
    Triangle,
    /// Rising sawtooth wave (-1 to +1).
    Sawtooth,
    /// Variable duty cycle pulse wave. Duty controlled by `pulse_width`.
    PwSquare,
    /// Pure sine wave (not on original MS-20 hardware).
    Sine,
    /// Pseudorandom white noise.
    WhiteNoise,
    /// PCM wavetable. Reads from a sample buffer using the phase accumulator
    /// with linear interpolation. The buffer represents one cycle of a waveform.
    Pcm,
}

/// Octave register (footage), inspired by organ pipe length conventions.
///
/// Each step doubles or halves the frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    /// Two octaves below base pitch (frequency * 0.25).
    Feet32,
    /// One octave below base pitch (frequency * 0.5).
    Feet16,
    /// Base pitch (frequency * 1.0).
    Feet8,
    /// One octave above base pitch (frequency * 2.0).
    Feet4,
    /// Two octaves above base pitch (frequency * 4.0).
    Feet2,
}

impl Scale {
    /// Return the frequency multiplier for this scale setting.
    pub fn multiplier(self) -> f64 {
        match self {
            Scale::Feet32 => 0.25,
            Scale::Feet16 => 0.5,
            Scale::Feet8 => 1.0,
            Scale::Feet4 => 2.0,
            Scale::Feet2 => 4.0,
        }
    }
}

/// Virtual Controlled Oscillator.
///
/// Generates audio waveforms using a phase accumulator. Supports multiple
/// waveform types, amplitude quantization, octave scaling, and fine
/// pitch adjustment.
#[derive(Debug, Clone)]
pub struct Vco {
    /// Selected waveform.
    pub waveform: Waveform,
    /// Pulse width for PW-Square mode. Range: 0.01 (narrow pulse) to
    /// 0.99 (narrow inverted pulse). Default 0.5 (50% duty = square).
    pub pulse_width: f64,
    /// Amplitude quantization bit depth. `None` for analog-style continuous
    /// output. `Some(N)` quantizes to 2^N levels. PW-Square is exempt.
    pub quantization: Option<u32>,
    /// Octave register (footage).
    pub scale: Scale,
    /// Fine pitch adjustment in semitones. Range: -12.0 to +12.0.
    pub pitch_fine: f64,
    /// VCO output level (0.0 to 1.0).
    pub level: f64,
    /// Current oscillator phase (0.0 to 1.0).
    phase: f64,
    /// Output from the most recent sample tick.
    last_output: f64,
    /// PRNG state for white noise generation.
    noise_state: u64,
    /// PCM wavetable sample buffer. Contains one cycle of a waveform as
    /// normalized f64 samples in [-1.0, 1.0]. Shared across voices via Arc.
    pcm_samples: Option<Arc<Vec<f64>>>,
}

impl Vco {
    /// Create a new VCO with default settings.
    ///
    /// Defaults to triangle waveform, 50% pulse width, no quantization,
    /// 8' scale (base pitch), zero fine tuning, full level.
    pub fn new() -> Self {
        Self {
            waveform: Waveform::Triangle,
            pulse_width: 0.5,
            quantization: None,
            scale: Scale::Feet8,
            pitch_fine: 0.0,
            level: 1.0,
            phase: 0.0,
            last_output: 0.0,
            noise_state: 0x5EED_1234_ABCD_EF01,
            pcm_samples: None,
        }
    }

    /// Set the PCM wavetable sample buffer. The buffer should contain one
    /// cycle of a waveform, normalized to [-1.0, 1.0].
    pub fn set_pcm_samples(&mut self, samples: Arc<Vec<f64>>) {
        self.pcm_samples = Some(samples);
    }

    /// Compute a single sample at the given frequency.
    ///
    /// Returns the generated sample in the range [-1.0, 1.0] scaled
    /// by `self.level`.
    pub fn tick(&mut self, base_freq: f64, sample_rate: f64) -> f64 {
        // Apply scale and fine tuning to the base frequency.
        let freq = base_freq * self.scale.multiplier() * fine_pitch_multiplier(self.pitch_fine);

        // Advance phase accumulator.
        if freq > 0.0 && sample_rate > 0.0 {
            self.phase += freq / sample_rate;
            // Wrap phase to [0, 1).
            self.phase -= libm::floor(self.phase);
        }

        // Generate raw waveform sample.
        let raw = match self.waveform {
            Waveform::Triangle => triangle(self.phase),
            Waveform::Sawtooth => sawtooth(self.phase),
            Waveform::PwSquare => pw_square(self.phase, self.pulse_width),
            Waveform::Sine => sine(self.phase),
            Waveform::WhiteNoise => white_noise(&mut self.noise_state),
            Waveform::Pcm => pcm_lookup(&self.pcm_samples, self.phase),
        };

        // Apply amplitude quantization (exempt for PW-Square).
        let quantized = match (self.waveform, self.quantization) {
            (Waveform::PwSquare, _) => raw,
            (_, Some(bits)) if bits > 0 => quantize_amplitude(raw, bits),
            _ => raw,
        };

        let output = quantized * self.level;
        self.last_output = output;
        output
    }

    /// Return the output from the most recent sample tick.
    pub fn last_output(&self) -> f64 {
        self.last_output
    }

    /// Reset the oscillator phase and internal state.
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.last_output = 0.0;
        self.noise_state = 0x5EED_1234_ABCD_EF01;
    }
}

impl Default for Vco {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the frequency multiplier for a fine pitch offset in semitones.
///
/// Uses equal temperament: multiplier = 2^(semitones / 12).
fn fine_pitch_multiplier(semitones: f64) -> f64 {
    if semitones.abs() < 1e-10 {
        return 1.0;
    }
    libm::pow(2.0, semitones / 12.0)
}

/// Triangle wave from phase (0.0 to 1.0). Output: -1.0 to +1.0.
fn triangle(phase: f64) -> f64 {
    // Rising from -1 to +1 in first half, falling from +1 to -1 in second half.
    if phase < 0.5 {
        4.0 * phase - 1.0
    } else {
        3.0 - 4.0 * phase
    }
}

/// Rising sawtooth wave from phase (0.0 to 1.0). Output: -1.0 to +1.0.
fn sawtooth(phase: f64) -> f64 {
    2.0 * phase - 1.0
}

/// Pulse wave with variable duty cycle.
///
/// `pulse_width` is the fraction of the period that the output is +1.0.
/// Values near 0.5 give a square wave; values near 0.0 or 1.0 give
/// narrow pulses.
fn pw_square(phase: f64, pulse_width: f64) -> f64 {
    let pw = pulse_width.clamp(0.01, 0.99);
    if phase < pw {
        1.0
    } else {
        -1.0
    }
}

/// Sine wave from phase (0.0 to 1.0). Output: -1.0 to +1.0.
fn sine(phase: f64) -> f64 {
    libm::sin(phase * core::f64::consts::TAU)
}

/// White noise using xorshift64 PRNG. Output: approximately -1.0 to +1.0.
fn white_noise(state: &mut u64) -> f64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    // Map u64 to [-1.0, 1.0).
    (x as f64 / (u64::MAX as f64)) * 2.0 - 1.0
}

/// Look up a sample from a PCM wavetable buffer using linear interpolation.
///
/// The phase (0.0 to 1.0) maps across the entire buffer length. Returns 0.0
/// if no buffer is loaded or the buffer is empty.
fn pcm_lookup(samples: &Option<Arc<Vec<f64>>>, phase: f64) -> f64 {
    let Some(buf) = samples else { return 0.0 };
    if buf.is_empty() {
        return 0.0;
    }
    let len = buf.len() as f64;
    let pos = phase * len;
    let idx0 = pos as usize % buf.len();
    let idx1 = (idx0 + 1) % buf.len();
    let frac = pos - libm::floor(pos);
    buf[idx0] * (1.0 - frac) + buf[idx1] * frac
}

/// Quantize a bipolar sample to a fixed number of amplitude levels.
///
/// Maps the [-1, 1] signal to `2^bits` evenly spaced discrete values.
fn quantize_amplitude(sample: f64, bits: u32) -> f64 {
    let levels = (1u64 << bits) as f64;
    let half = levels / 2.0;
    // Map [-1, 1] to [0, levels], round, map back.
    let scaled = (sample + 1.0) * half;
    let quantized = libm::round(scaled);
    quantized / half - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn triangle_wave_shape() {
        // Phase 0.0 -> -1.0
        assert!((triangle(0.0) - (-1.0)).abs() < 1e-10);
        // Phase 0.25 -> 0.0
        assert!((triangle(0.25) - 0.0).abs() < 1e-10);
        // Phase 0.5 -> +1.0
        assert!((triangle(0.5) - 1.0).abs() < 1e-10);
        // Phase 0.75 -> 0.0
        assert!((triangle(0.75) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn sawtooth_wave_shape() {
        assert!((sawtooth(0.0) - (-1.0)).abs() < 1e-10);
        assert!((sawtooth(0.5) - 0.0).abs() < 1e-10);
        // Just before wrap, approaching +1.0
        assert!((sawtooth(0.999) - 0.998).abs() < 1e-3);
    }

    #[test]
    fn pw_square_50_percent() {
        // 50% duty = standard square wave
        assert!((pw_square(0.1, 0.5) - 1.0).abs() < 1e-10);
        assert!((pw_square(0.4, 0.5) - 1.0).abs() < 1e-10);
        assert!((pw_square(0.6, 0.5) - (-1.0)).abs() < 1e-10);
        assert!((pw_square(0.9, 0.5) - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn pw_square_narrow_pulse() {
        // 10% duty = narrow pulse
        assert!((pw_square(0.05, 0.1) - 1.0).abs() < 1e-10);
        assert!((pw_square(0.15, 0.1) - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn sine_wave_shape() {
        assert!(sine(0.0).abs() < 1e-10);
        assert!((sine(0.25) - 1.0).abs() < 1e-10);
        assert!(sine(0.5).abs() < 1e-10);
        assert!((sine(0.75) - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn white_noise_bounded() {
        let mut state = 0x12345678_9ABCDEF0u64;
        for _ in 0..1000 {
            let sample = white_noise(&mut state);
            assert!((-1.0..=1.0).contains(&sample), "Noise sample out of range: {sample}");
        }
    }

    #[test]
    fn white_noise_varies() {
        let mut state = 0x12345678_9ABCDEF0u64;
        let s1 = white_noise(&mut state);
        let s2 = white_noise(&mut state);
        assert!((s1 - s2).abs() > 1e-10, "Sequential noise samples should differ");
    }

    #[test]
    fn quantize_4bit_nes_style() {
        // 4-bit = 16 levels. Triangle at phase 0.25 = 0.0, should stay 0.0.
        let q = quantize_amplitude(0.0, 4);
        assert!(q.abs() < 1e-10, "Zero should quantize to zero: {q}");

        // At extremes, should stay at extremes.
        let q_max = quantize_amplitude(1.0, 4);
        assert!((q_max - 1.0).abs() < 1e-10, "1.0 should quantize to 1.0: {q_max}");

        let q_min = quantize_amplitude(-1.0, 4);
        assert!((q_min - (-1.0)).abs() < 1e-10, "-1.0 should quantize to -1.0: {q_min}");
    }

    #[test]
    fn quantize_3bit_extreme_lofi() {
        // 3-bit = 8 levels. Check that intermediate values snap.
        let q = quantize_amplitude(0.3, 3);
        // 0.3 mapped to [0,4]: 0.3+1=1.3, *4=5.2, round=5, /4-1=0.25
        assert!((q - 0.25).abs() < 1e-10, "0.3 should quantize to 0.25: {q}");
    }

    #[test]
    fn quantization_exempt_for_pw_square() {
        let mut vco = Vco::new();
        vco.waveform = Waveform::PwSquare;
        vco.quantization = Some(3);
        // PW-Square should still produce exact +1/-1 regardless of quantization.
        let sample = vco.tick(440.0, 48000.0);
        assert!(
            (sample - 1.0).abs() < 1e-10 || (sample - (-1.0)).abs() < 1e-10,
            "PW-Square should produce +1 or -1: {sample}"
        );
    }

    #[test]
    fn scale_multipliers() {
        assert!((Scale::Feet32.multiplier() - 0.25).abs() < 1e-10);
        assert!((Scale::Feet16.multiplier() - 0.5).abs() < 1e-10);
        assert!((Scale::Feet8.multiplier() - 1.0).abs() < 1e-10);
        assert!((Scale::Feet4.multiplier() - 2.0).abs() < 1e-10);
        assert!((Scale::Feet2.multiplier() - 4.0).abs() < 1e-10);
    }

    #[test]
    fn fine_pitch_one_octave_up() {
        let m = fine_pitch_multiplier(12.0);
        assert!((m - 2.0).abs() < 1e-10, "12 semitones = 1 octave = 2x: {m}");
    }

    #[test]
    fn fine_pitch_one_octave_down() {
        let m = fine_pitch_multiplier(-12.0);
        assert!((m - 0.5).abs() < 1e-10, "-12 semitones = -1 octave = 0.5x: {m}");
    }

    #[test]
    fn fine_pitch_zero_is_unity() {
        let m = fine_pitch_multiplier(0.0);
        assert!((m - 1.0).abs() < 1e-10);
    }

    #[test]
    fn vco_produces_output() {
        let mut vco = Vco::new();
        let mut has_nonzero = false;
        for _ in 0..48 {
            let s = vco.tick(440.0, 48000.0);
            if s.abs() > 1e-10 {
                has_nonzero = true;
            }
        }
        assert!(has_nonzero, "VCO should produce non-zero output");
    }

    #[test]
    fn vco_bounded_output() {
        let mut vco = Vco::new();
        for _ in 0..4800 {
            let s = vco.tick(440.0, 48000.0);
            assert!(
                (-1.0 - 1e-10..=1.0 + 1e-10).contains(&s),
                "VCO output should be in [-1, 1]: {s}"
            );
        }
    }

    #[test]
    fn vco_level_scales_output() {
        let mut vco = Vco::new();
        vco.level = 0.5;
        for _ in 0..4800 {
            let s = vco.tick(440.0, 48000.0);
            assert!(
                (-0.5 - 1e-10..=0.5 + 1e-10).contains(&s),
                "VCO at 0.5 level should be in [-0.5, 0.5]: {s}"
            );
        }
    }

    #[test]
    fn vco_scale_changes_pitch() {
        // At 8' scale, one cycle of 440Hz takes about 109 samples at 48kHz.
        // At 4' scale (2x freq), one cycle should take about 55 samples.
        let mut vco8 = Vco::new();
        vco8.scale = Scale::Feet8;
        let mut vco4 = Vco::new();
        vco4.scale = Scale::Feet4;

        // Count zero crossings over 480 samples (10ms at 48kHz).
        let crossings_8 = count_zero_crossings(&mut vco8, 440.0, 48000.0, 480);
        let crossings_4 = count_zero_crossings(&mut vco4, 440.0, 48000.0, 480);

        // 4' should have approximately 2x the zero crossings of 8'.
        assert!(
            crossings_4 > crossings_8,
            "4' scale should have more zero crossings: {crossings_4} vs {crossings_8}"
        );
    }

    #[test]
    fn vco_reset_clears_state() {
        let mut vco = Vco::new();
        for _ in 0..100 {
            vco.tick(440.0, 48000.0);
        }
        assert!(vco.phase > 0.0);
        vco.reset();
        assert!(vco.phase.abs() < 1e-10);
        assert!(vco.last_output.abs() < 1e-10);
    }

    #[test]
    fn all_waveforms_produce_output() {
        let waveforms = [
            Waveform::Triangle,
            Waveform::Sawtooth,
            Waveform::PwSquare,
            Waveform::Sine,
            Waveform::WhiteNoise,
        ];

        for wf in &waveforms {
            let mut vco = Vco::new();
            vco.waveform = *wf;
            let mut has_nonzero = false;
            for _ in 0..480 {
                let s = vco.tick(440.0, 48000.0);
                if s.abs() > 1e-10 {
                    has_nonzero = true;
                    break;
                }
            }
            assert!(has_nonzero, "Waveform {:?} should produce non-zero output", wf);
        }
    }

    /// Helper: count positive-going zero crossings over N samples.
    fn count_zero_crossings(vco: &mut Vco, freq: f64, sr: f64, samples: usize) -> usize {
        let mut prev = 0.0f64;
        let mut crossings = 0;
        for _ in 0..samples {
            let s = vco.tick(freq, sr);
            if prev <= 0.0 && s > 0.0 {
                crossings += 1;
            }
            prev = s;
        }
        crossings
    }

    #[test]
    fn pcm_single_sample() {
        // A single-sample buffer always returns that value regardless of phase.
        let buf = Arc::new(vec![0.75]);
        let samples = Some(buf);
        let val = pcm_lookup(&samples, 0.0);
        assert!((val - 0.75).abs() < 1e-10, "Single sample lookup: {val}");
        let val2 = pcm_lookup(&samples, 0.5);
        assert!((val2 - 0.75).abs() < 1e-10, "Single sample at 0.5: {val2}");
    }

    #[test]
    fn pcm_interpolation() {
        // Two-sample buffer [0.0, 1.0]. Phase 0.25 should interpolate to 0.5.
        let buf = Arc::new(vec![0.0, 1.0]);
        let samples = Some(buf);
        let val = pcm_lookup(&samples, 0.25);
        assert!((val - 0.5).abs() < 1e-10, "Interpolation at 0.25: {val}");
        // Phase 0.0 should return exactly 0.0.
        let val0 = pcm_lookup(&samples, 0.0);
        assert!(val0.abs() < 1e-10, "Phase 0.0 lookup: {val0}");
        // Phase 0.5 should return exactly 1.0.
        let val5 = pcm_lookup(&samples, 0.5);
        assert!((val5 - 1.0).abs() < 1e-10, "Phase 0.5 lookup: {val5}");
    }

    #[test]
    fn pcm_none_is_silent() {
        // Without a sample buffer, PCM waveform should produce silence.
        let mut vco = Vco::new();
        vco.waveform = Waveform::Pcm;
        for _ in 0..48 {
            let s = vco.tick(440.0, 48000.0);
            assert!(s.abs() < 1e-10, "PCM with no buffer should be silent: {s}");
        }
    }

    #[test]
    fn pcm_produces_output() {
        // With a loaded wavetable, PCM should produce non-zero output.
        let mut vco = Vco::new();
        vco.waveform = Waveform::Pcm;
        // Create a simple sawtooth-like wavetable with 256 samples.
        let table: Vec<f64> = (0..256).map(|i| i as f64 / 128.0 - 1.0).collect();
        vco.set_pcm_samples(Arc::new(table));
        let mut has_nonzero = false;
        for _ in 0..480 {
            let s = vco.tick(440.0, 48000.0);
            if s.abs() > 1e-10 {
                has_nonzero = true;
                break;
            }
        }
        assert!(has_nonzero, "PCM with loaded wavetable should produce output");
    }

    #[test]
    fn pcm_output_bounded() {
        // PCM output should stay within [-1.0, 1.0] for a properly normalized table.
        let mut vco = Vco::new();
        vco.waveform = Waveform::Pcm;
        let table: Vec<f64> = (0..256).map(|i| i as f64 / 128.0 - 1.0).collect();
        vco.set_pcm_samples(Arc::new(table));
        for _ in 0..4800 {
            let s = vco.tick(440.0, 48000.0);
            assert!(
                (-1.0 - 1e-10..=1.0 + 1e-10).contains(&s),
                "PCM output should be in [-1, 1]: {s}"
            );
        }
    }
}
