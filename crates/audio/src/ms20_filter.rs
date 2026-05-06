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
//! The Korg-35 is a two-pole (12 dB/oct) lowpass filter built from two
//! cascaded first-order lowpass sections with a nonlinear feedback path.
//! This implementation uses a Topology-Preserving Transform (TPT) with
//! one-pole integrators, preserving the analog circuit structure in the
//! digital domain. Diode nonlinearity in the feedback path is modeled
//! with tanh saturation, controlled by a user-facing saturation parameter.
//! At high resonance settings (K near 2.0), the filter produces strong
//! resonant ringing near the cutoff frequency. In the real circuit,
//! diode nonlinearity sustains this ringing into true self-oscillation.
//!
//! ## References
//!
//! - Will Pirkle, "Designing Audio Effect Plugins in C++", 2nd ed., Ch. 10
//! - Vadim Zavalishin, "The Art of VA Filter Design"
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
/// Two cascaded first-order TPT lowpass sections with nonlinear resonance
/// feedback, modeled after the Korg MS-20 filter circuit. The feedback
/// path includes optional tanh saturation to emulate diode nonlinearity.
///
/// At resonance near 2.0, the filter produces strong resonant ringing.
/// With saturation enabled, the nonlinear feedback approaches the
/// behavior of the real circuit's diode-driven self-oscillation.
#[derive(Debug, Clone)]
pub struct Korg35Lpf {
    /// Cutoff frequency in Hz (20 to 20000).
    pub cutoff: f64,
    /// Resonance amount. Range: 0.0 (no resonance) to 2.0 (maximum ringing).
    pub resonance: f64,
    /// Nonlinear saturation amount. Range: 0.0 (linear) to 1.0 (full saturation).
    pub saturation: f64,
    /// Whether the filter is active.
    pub active: bool,

    // One-pole TPT integrator states (one per cascade stage).
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
    /// Uses two cascaded TPT one-pole lowpass integrators with resolved
    /// feedback. The feedback loop (resonance) is solved algebraically in
    /// the current sample to avoid the one-sample delay that would make the
    /// open-loop formulation unstable at high cutoff frequencies.
    ///
    /// The resolved form computes:
    ///   y2 = (a^2 * input + a*b*s1 + b*s2) / (1 + a^2*K)
    ///   y1 = a * (input - K*y2) + b*s1
    /// where a = g/(1+g), b = 1-a, K is the resonance feedback gain, and
    /// s1, s2 are the trapezoidal integrator states.
    ///
    /// With saturation > 0, the feedback signal passes through tanh
    /// nonlinearity to emulate diode clipping in the resonance path. In
    /// this case an iterative approximation is used (one Newton step).
    pub fn tick(&mut self, input: f64, sample_rate: f64) -> f64 {
        if !self.active {
            return input;
        }

        let cutoff = self.cutoff.clamp(20.0, sample_rate * 0.49);
        let k = self.resonance.clamp(0.0, 2.0);

        // TPT integrator coefficient.
        let g = libm::tan(core::f64::consts::PI * cutoff / sample_rate);
        let alpha = g / (1.0 + g);
        let beta = 1.0 - alpha;

        if self.saturation > 0.0 {
            // Nonlinear feedback: use the linear solution as initial guess,
            // then refine with one Newton-Raphson step for the tanh
            // nonlinearity.
            let drive = 1.0 + self.saturation * 3.0;
            let denom = 1.0 + alpha * alpha * k;

            // Linear solution (initial guess).
            let y2_lin = (alpha * alpha * input + alpha * beta * self.s1 + beta * self.s2) / denom;

            // Newton step: f(y2) = y2 - [a^2*(input - k*tanh(y2*drive)/drive) + a*b*s1 + b*s2]
            // f'(y2) = 1 + a^2*k*sech^2(y2*drive)
            let t = libm::tanh(y2_lin * drive);
            let sech2 = 1.0 - t * t;
            let fb_nl = t / drive;
            let f_val = y2_lin
                - (alpha * alpha * (input - k * fb_nl) + alpha * beta * self.s1 + beta * self.s2);
            let f_deriv = 1.0 + alpha * alpha * k * sech2;
            let y2 = y2_lin - f_val / f_deriv;

            let y1 = alpha * (input - k * libm::tanh(y2 * drive) / drive) + beta * self.s1;

            self.s1 = 2.0 * y1 - self.s1;
            self.s2 = 2.0 * y2 - self.s2;
            y2
        } else {
            // Linear feedback: exact algebraic resolution.
            let denom = 1.0 + alpha * alpha * k;
            let y2 = (alpha * alpha * input + alpha * beta * self.s1 + beta * self.s2) / denom;
            let y1 = alpha * (input - k * y2) + beta * self.s1;

            self.s1 = 2.0 * y1 - self.s1;
            self.s2 = 2.0 * y2 - self.s2;
            y2
        }
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

        assert!(
            energy > 1000.0,
            "HPF should pass high frequencies: {energy}"
        );
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
        let low_freq = 100.0;

        // Helper: measure steady-state peak amplitude of a sine through the
        // filter. Runs 200ms, measures the last 100ms to avoid transient.
        fn measure_peak(cutoff_hz: f64, resonance: f64, freq: f64, sr: f64) -> f64 {
            let mut lpf = Korg35Lpf::new(cutoff_hz, resonance);
            let mut peak = 0.0f64;
            for i in 0..9600 {
                let t = i as f64 / sr;
                let input = libm::sin(core::f64::consts::TAU * freq * t);
                let output = lpf.tick(input, sr);
                if i > 4800 {
                    peak = peak.max(output.abs());
                }
            }
            peak
        }

        // Without resonance: gain ratio at cutoff vs passband.
        // The Korg-35 at K=0 is two cascaded one-pole LPFs. The passband
        // gain is unity and the gain at cutoff is -6 dB (0.5).
        let gain_low_flat = measure_peak(cutoff, 0.0, low_freq, sr);
        let gain_cut_flat = measure_peak(cutoff, 0.0, cutoff, sr);
        let ratio_flat = gain_cut_flat / gain_low_flat;

        // With resonance: the passband gain drops (DC gain = 1/(1+K)) but
        // the resonance peak near cutoff creates selective boost. The ratio
        // of cutoff gain to passband gain should increase.
        let gain_low_res = measure_peak(cutoff, 1.5, low_freq, sr);
        let gain_cut_res = measure_peak(cutoff, 1.5, cutoff, sr);
        let ratio_res = gain_cut_res / gain_low_res;

        assert!(
            ratio_res > ratio_flat,
            "Resonance should boost cutoff relative to passband: \
             ratio_res={ratio_res:.4} ratio_flat={ratio_flat:.4}"
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

    #[test]
    fn lpf_high_resonance_rings() {
        // In the Korg-35 two-pole cascade with output feedback, the damping
        // ratio is 1/sqrt(1+K). At K=0 the system is overdamped (two real
        // poles) and the impulse response is non-negative. At K=2 the system
        // is underdamped (zeta ~= 0.577) and the impulse response oscillates.
        // True self-oscillation in the real circuit comes from diode
        // nonlinearity, not the linear topology.
        let sr = 48000.0;
        let cutoff = 1000.0;

        // At K=0: impulse response of two cascaded one-pole LPFs is
        // t * exp(-t/tau), which is always non-negative.
        let mut lpf = Korg35Lpf::new(cutoff, 0.0);
        let _ = lpf.tick(1.0, sr);
        let mut has_negative_flat = false;
        for _ in 0..480 {
            if lpf.tick(0.0, sr) < -1e-15 {
                has_negative_flat = true;
            }
        }

        // At K=2.0: underdamped response should oscillate (go negative).
        let mut lpf = Korg35Lpf::new(cutoff, 2.0);
        let _ = lpf.tick(1.0, sr);
        let mut has_negative_res = false;
        for _ in 0..480 {
            if lpf.tick(0.0, sr) < -1e-15 {
                has_negative_res = true;
            }
        }

        assert!(
            !has_negative_flat,
            "No-resonance impulse response should be non-negative"
        );
        assert!(
            has_negative_res,
            "High-resonance impulse response should ring (go negative)"
        );
    }

    /// Verify resolved feedback keeps the filter stable when cutoff is
    /// modulated rapidly by an envelope (the real-world DAW scenario).
    #[test]
    fn lpf_stable_with_modulated_cutoff() {
        let sr = 48000.0;
        let mut lpf = Korg35Lpf::new(8000.0, 0.707);

        // Simulate an ADSR envelope modulating cutoff by 4 octaves.
        // Attack: 0->1 in 480 samples, decay: 1->0.8 in 4800, sustain: 0.8.
        for i in 0..48000 {
            let eg2 = if i < 480 {
                i as f64 / 480.0
            } else if i < 5280 {
                1.0 - 0.2 * ((i - 480) as f64 / 4800.0)
            } else {
                0.8
            };

            let mod_cutoff = 8000.0 * libm::pow(2.0, 4.0 * eg2);
            lpf.cutoff = mod_cutoff.clamp(20.0, 20000.0);

            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * 440.0 * t);
            let output = lpf.tick(input, sr);

            assert!(
                output.is_finite() && output.abs() < 100.0,
                "LPF unstable at sample {i}: output={output}, cutoff={}",
                lpf.cutoff,
            );
        }
    }

    #[test]
    fn lpf_tpt_stable_near_nyquist() {
        // Verify the TPT filter remains stable with cutoff near Nyquist.
        // The tan() mapping naturally warps toward infinity at Nyquist, so
        // the filter must be clamped and remain bounded.
        let sr = 48000.0;
        let cutoff = sr * 0.49; // Near Nyquist.
        let mut lpf = Korg35Lpf::new(cutoff, 1.8);
        lpf.saturation = 0.5;

        for i in 0..48000 {
            let t = i as f64 / sr;
            let input = libm::sin(core::f64::consts::TAU * 440.0 * t);
            let output = lpf.tick(input, sr);
            assert!(
                output.is_finite() && output.abs() < 100.0,
                "Filter should be stable near Nyquist: output={output} at sample {i}"
            );
        }
    }
}
