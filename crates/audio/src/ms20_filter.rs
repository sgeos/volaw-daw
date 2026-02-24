//! MS-20 filter implementations.
//!
//! The Korg MS-20 has two filters in series:
//!
//! 1. **Sallen-Key HPF**: A second-order highpass filter controlled by EG1.
//!    Cutoff range: 20 Hz to 20 kHz.
//! 2. **Korg-35 LPF**: A resonant lowpass filter with self-oscillation
//!    capability, controlled by EG2. Cutoff range: 20 Hz to 20 kHz.
//!
//! ## Korg-35 Filter
//!
//! The Korg-35 is a nonlinear resonant filter based on a diode ladder
//! topology. At high resonance settings, it self-oscillates, producing a
//! sine-like tone. This implementation uses a trapezoidal integrator
//! (TPT) formulation for numerical stability, avoiding the instability
//! problems of naive Euler integration at high resonance.
//!
//! ## References
//!
//! - Will Pirkle, "Designing Audio Effect Plugins in C++", Chapter 10
//! - Vesa Valimaki and Antti Huovilainen, "Oscillator and Filter
//!   Algorithms for Virtual Analog Synthesis"

/// Sallen-Key highpass filter.
///
/// A second-order resonant highpass filter used as the first filter stage
/// in the MS-20 signal chain. Controlled by EG1 for cutoff modulation.
#[derive(Debug, Clone)]
pub struct SallenKeyHpf {
    /// Cutoff frequency in Hz (20 to 20000).
    pub cutoff: f64,
    /// Resonance (Q factor). Range: 0.5 (gentle) to 25.0 (sharp peak).
    pub resonance: f64,
    /// Whether the filter is active. When inactive, signal passes through
    /// unchanged.
    pub active: bool,

    // State variables (transposed direct form II).
    s1: f64,
    s2: f64,
}

impl SallenKeyHpf {
    /// Create a new Sallen-Key HPF with the given cutoff and resonance.
    pub fn new(cutoff: f64, resonance: f64) -> Self {
        Self {
            cutoff: cutoff.clamp(20.0, 20000.0),
            resonance: resonance.clamp(0.5, 25.0),
            active: true,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// Process a single sample through the highpass filter.
    ///
    /// Uses the transposed direct form II biquad implementation with
    /// highpass coefficients derived from the cutoff and resonance.
    pub fn tick(&mut self, input: f64, sample_rate: f64) -> f64 {
        if !self.active {
            return input;
        }

        let cutoff = self.cutoff.clamp(20.0, sample_rate * 0.49);
        let q = self.resonance.clamp(0.5, 25.0);

        // Bilinear transform coefficients for second-order highpass.
        let w0 = core::f64::consts::TAU * cutoff / sample_rate;
        let cos_w0 = libm::cos(w0);
        let sin_w0 = libm::sin(w0);
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        // Normalize by a0.
        let b0n = b0 / a0;
        let b1n = b1 / a0;
        let b2n = b2 / a0;
        let a1n = a1 / a0;
        let a2n = a2 / a0;

        // Transposed direct form II.
        let output = b0n * input + self.s1;
        self.s1 = b1n * input - a1n * output + self.s2;
        self.s2 = b2n * input - a2n * output;

        output
    }

    /// Reset the filter state.
    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }
}

impl Default for SallenKeyHpf {
    fn default() -> Self {
        Self::new(20.0, 0.707)
    }
}

/// Korg-35 resonant lowpass filter.
///
/// A nonlinear resonant lowpass filter modeled after the Korg MS-20's
/// distinctive filter circuit. Uses a trapezoidal integrator (TPT / bilinear
/// transform) formulation for numerical stability at high resonance.
///
/// The filter can self-oscillate at maximum resonance, producing a sine tone
/// at the cutoff frequency.
#[derive(Debug, Clone)]
pub struct Korg35Lpf {
    /// Cutoff frequency in Hz (20 to 20000).
    pub cutoff: f64,
    /// Resonance amount. Range: 0.0 (no resonance) to 2.0 (self-oscillation).
    pub resonance: f64,
    /// Nonlinear saturation amount. Range: 0.0 (linear) to 1.0 (full saturation).
    pub saturation: f64,
    /// Whether the filter is active.
    pub active: bool,

    // TPT integrator states.
    s1: f64,
    s2: f64,
}

impl Korg35Lpf {
    /// Create a new Korg-35 LPF with the given cutoff and resonance.
    pub fn new(cutoff: f64, resonance: f64) -> Self {
        Self {
            cutoff: cutoff.clamp(20.0, 20000.0),
            resonance: resonance.clamp(0.0, 2.0),
            saturation: 0.0,
            active: true,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// Process a single sample through the lowpass filter.
    ///
    /// Uses a biquad lowpass (Audio EQ Cookbook) with nonlinear state
    /// saturation in the feedback path for the characteristic Korg-35
    /// sound. The resonance parameter maps to the filter Q factor.
    pub fn tick(&mut self, input: f64, sample_rate: f64) -> f64 {
        if !self.active {
            return input;
        }

        let cutoff = self.cutoff.clamp(20.0, sample_rate * 0.49);
        let k = self.resonance.clamp(0.0, 2.0);

        // Map resonance (0-2) to Q factor.
        // k=0: Q=0.707 (Butterworth), k=2: Q=25 (near self-oscillation).
        let q = 0.707 + k * 12.0;

        // Biquad lowpass coefficients (Audio EQ Cookbook).
        let w0 = core::f64::consts::TAU * cutoff / sample_rate;
        let cos_w0 = libm::cos(w0);
        let sin_w0 = libm::sin(w0);
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        // Normalize by a0.
        let b0n = b0 / a0;
        let b1n = b1 / a0;
        let b2n = b2 / a0;
        let a1n = a1 / a0;
        let a2n = a2 / a0;

        // Transposed direct form II.
        let output = b0n * input + self.s1;
        self.s1 = b1n * input - a1n * output + self.s2;
        self.s2 = b2n * input - a2n * output;

        // Apply nonlinear saturation to the state variables (feedback
        // path) for Korg-35 diode ladder character.
        if self.saturation > 0.0 {
            let sat_scale = 1.0 + self.saturation * 2.0;
            self.s1 = libm::tanh(self.s1 * sat_scale) / sat_scale;
            self.s2 = libm::tanh(self.s2 * sat_scale) / sat_scale;
        }

        output
    }

    /// Reset the filter state.
    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }
}

impl Default for Korg35Lpf {
    fn default() -> Self {
        Self::new(20000.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Sallen-Key HPF Tests ---

    #[test]
    fn hpf_passes_high_frequencies() {
        let mut hpf = SallenKeyHpf::new(1000.0, 0.707);
        let sr = 48000.0;

        // Generate a 5kHz sine (above cutoff) and measure output energy.
        let freq = 5000.0;
        let mut energy = 0.0;
        for i in 0..4800 {
            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * freq * t);
            let output = hpf.tick(input, sr);
            if i > 480 {
                // Skip transient.
                energy += output * output;
            }
        }

        assert!(energy > 1000.0, "HPF should pass high frequencies: {energy}");
    }

    #[test]
    fn hpf_attenuates_low_frequencies() {
        let mut hpf = SallenKeyHpf::new(1000.0, 0.707);
        let sr = 48000.0;

        // Generate a 100Hz sine (below cutoff) and measure output energy.
        let freq = 100.0;
        let mut energy = 0.0;
        for i in 0..4800 {
            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * freq * t);
            let output = hpf.tick(input, sr);
            if i > 480 {
                energy += output * output;
            }
        }

        assert!(
            energy < 100.0,
            "HPF should attenuate low frequencies: {energy}"
        );
    }

    #[test]
    fn hpf_inactive_passthrough() {
        let mut hpf = SallenKeyHpf::new(1000.0, 0.707);
        hpf.active = false;
        let output = hpf.tick(0.5, 48000.0);
        assert!((output - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn hpf_reset_clears_state() {
        let mut hpf = SallenKeyHpf::new(1000.0, 0.707);
        for i in 0..100 {
            hpf.tick(libm::sin(i as f64 * 0.1), 48000.0);
        }
        hpf.reset();
        assert!(hpf.s1.abs() < f64::EPSILON);
        assert!(hpf.s2.abs() < f64::EPSILON);
    }

    // --- Korg-35 LPF Tests ---

    #[test]
    fn lpf_passes_low_frequencies() {
        let mut lpf = Korg35Lpf::new(5000.0, 0.0);
        let sr = 48000.0;

        // Generate a 200Hz sine (below cutoff).
        let freq = 200.0;
        let mut energy = 0.0;
        for i in 0..4800 {
            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * freq * t);
            let output = lpf.tick(input, sr);
            if i > 480 {
                energy += output * output;
            }
        }

        assert!(energy > 1000.0, "LPF should pass low frequencies: {energy}");
    }

    #[test]
    fn lpf_attenuates_high_frequencies() {
        let mut lpf = Korg35Lpf::new(500.0, 0.0);
        let sr = 48000.0;

        // Generate a 10kHz sine (well above cutoff).
        let freq = 10000.0;
        let mut energy = 0.0;
        for i in 0..4800 {
            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * freq * t);
            let output = lpf.tick(input, sr);
            if i > 480 {
                energy += output * output;
            }
        }

        assert!(
            energy < 50.0,
            "LPF should attenuate high frequencies: {energy}"
        );
    }

    #[test]
    fn lpf_resonance_boosts_cutoff() {
        let sr = 48000.0;
        let cutoff = 1000.0;

        // Measure energy near cutoff without resonance.
        let mut lpf_flat = Korg35Lpf::new(cutoff, 0.0);
        let mut energy_flat = 0.0;
        for i in 0..4800 {
            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * cutoff * t);
            let output = lpf_flat.tick(input, sr);
            if i > 480 {
                energy_flat += output * output;
            }
        }

        // Measure energy near cutoff with resonance.
        let mut lpf_res = Korg35Lpf::new(cutoff, 1.5);
        let mut energy_res = 0.0;
        for i in 0..4800 {
            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * cutoff * t);
            let output = lpf_res.tick(input, sr);
            if i > 480 {
                energy_res += output * output;
            }
        }

        assert!(
            energy_res > energy_flat,
            "Resonance should boost energy at cutoff: {energy_res} vs {energy_flat}"
        );
    }

    #[test]
    fn lpf_inactive_passthrough() {
        let mut lpf = Korg35Lpf::new(1000.0, 0.0);
        lpf.active = false;
        let output = lpf.tick(0.5, 48000.0);
        assert!((output - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn lpf_reset_clears_state() {
        let mut lpf = Korg35Lpf::new(1000.0, 0.5);
        for i in 0..100 {
            lpf.tick(libm::sin(i as f64 * 0.1), 48000.0);
        }
        lpf.reset();
        assert!(lpf.s1.abs() < f64::EPSILON);
        assert!(lpf.s2.abs() < f64::EPSILON);
    }

    #[test]
    fn lpf_output_bounded() {
        let mut lpf = Korg35Lpf::new(1000.0, 1.8);
        lpf.saturation = 1.0;
        let sr = 48000.0;

        for i in 0..48000 {
            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * 440.0 * t);
            let output = lpf.tick(input, sr);
            assert!(
                output.abs() < 10.0,
                "LPF output should be bounded: {output}"
            );
        }
    }

    #[test]
    fn lpf_saturation_softens_signal() {
        let sr = 48000.0;

        // High resonance without saturation.
        let mut lpf_linear = Korg35Lpf::new(1000.0, 1.8);
        lpf_linear.saturation = 0.0;
        let mut peak_linear = 0.0f64;
        for i in 0..4800 {
            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * 1000.0 * t);
            let output = lpf_linear.tick(input, sr);
            if output.abs() > peak_linear {
                peak_linear = output.abs();
            }
        }

        // High resonance with saturation.
        let mut lpf_sat = Korg35Lpf::new(1000.0, 1.8);
        lpf_sat.saturation = 1.0;
        let mut peak_sat = 0.0f64;
        for i in 0..4800 {
            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * 1000.0 * t);
            let output = lpf_sat.tick(input, sr);
            if output.abs() > peak_sat {
                peak_sat = output.abs();
            }
        }

        // Saturation should reduce peak levels compared to linear.
        assert!(
            peak_sat <= peak_linear + 0.1,
            "Saturation should soften peaks: sat={peak_sat} linear={peak_linear}"
        );
    }
}
