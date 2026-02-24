//! Virtual Korg MS-20 synthesizer voice.
//!
//! The MS-20 signal chain consists of two VCOs, a Sallen-Key highpass filter,
//! a Korg-35 lowpass filter with resonance, two envelope generators, and a VCA.
//! The MS-20 also features an external signal processor (ESP) input that allows
//! external audio to be patched into the filter and modulation paths.
//!
//! ## Signal Chain
//!
//! ```text
//! External Audio In --+
//!                     v
//! VCO1 --+--> Mixer --> HPF (Sallen-Key) --> LPF (Korg-35) --> VCA --> Out
//! VCO2 --+              ^                     ^                  ^
//!                       EG1                   EG2                EG2
//! ```
//!
//! ## Current State
//!
//! VCOs are fully implemented with all 6 waveform types, amplitude
//! quantization, scale selection, and fine pitch control. The pitch glide
//! system supports per-voice portamento with departing note undershoot and
//! arriving note overshoot.
//!
//! Filters (HPF, LPF), envelope generators (EG1, EG2), and VCA are not yet
//! implemented. The VCO mix output currently bypasses the filter and
//! envelope stages.

use crate::vco::Vco;

/// Per-voice pitch glide (portamento) with undershoot and overshoot.
///
/// When a new note is triggered, the pitch transitions smoothly from the
/// old note to the new note over the configured glide time. The departing
/// note can briefly undershoot (dip in the opposite direction) and the
/// arriving note can briefly overshoot (exceed the target before settling).
#[derive(Debug, Clone)]
pub struct PitchGlide {
    /// Whether pitch glide is enabled.
    pub enabled: bool,
    /// Glide time in seconds.
    pub time: f64,
    /// Undershoot depth as a fraction of the interval. The departing note
    /// dips this fraction of the source-to-target interval in the opposite
    /// direction before the glide begins. Range: 0.0 (none) to 1.0 (full).
    pub undershoot: f64,
    /// Overshoot depth as a fraction of the interval. The arriving note
    /// exceeds the target by this fraction of the source-to-target interval
    /// before settling. Range: 0.0 (none) to 1.0 (full).
    pub overshoot: f64,

    /// Source frequency (note we are gliding from).
    source_freq: f64,
    /// Target frequency (note we are gliding to).
    target_freq: f64,
    /// Elapsed glide time in seconds.
    elapsed: f64,
}

impl PitchGlide {
    /// Create a new pitch glide with default settings (disabled).
    pub fn new() -> Self {
        Self {
            enabled: false,
            time: 0.1,
            undershoot: 0.0,
            overshoot: 0.0,
            source_freq: 440.0,
            target_freq: 440.0,
            elapsed: 1.0,
        }
    }

    /// Set a new target note, starting the glide from the current frequency.
    pub fn set_target(&mut self, new_freq: f64) {
        if !self.enabled || self.time <= 0.0 {
            self.source_freq = new_freq;
            self.target_freq = new_freq;
            self.elapsed = self.time + 1.0;
            return;
        }
        self.source_freq = self.current_freq();
        self.target_freq = new_freq;
        self.elapsed = 0.0;
    }

    /// Advance the glide by one sample and return the current frequency.
    pub fn tick(&mut self, sample_rate: f64) -> f64 {
        if sample_rate > 0.0 {
            self.elapsed += 1.0 / sample_rate;
        }
        self.current_freq()
    }

    /// Compute the current glide frequency without advancing time.
    pub fn current_freq(&self) -> f64 {
        if !self.enabled || self.time <= 0.0 {
            return self.target_freq;
        }

        let t = if self.time > 0.0 {
            (self.elapsed / self.time).clamp(0.0, 2.0)
        } else {
            2.0
        };

        // Glide complete.
        if t >= 1.5 {
            return self.target_freq;
        }

        // Work in log2-frequency space for musical pitch interpolation.
        let src = libm::log2(self.source_freq.max(1.0));
        let tgt = libm::log2(self.target_freq.max(1.0));
        let interval = tgt - src;

        // Smoothstep for the main glide (0 to 1).
        let gt = t.min(1.0);
        let s = gt * gt * (3.0 - 2.0 * gt);
        let base = src + interval * s;

        // Undershoot: at the start, pitch dips in the opposite direction.
        // Decays exponentially over the first portion of the glide.
        let undershoot_amt = self.undershoot * interval * libm::exp(-t * 6.0);

        // Overshoot: near the end, pitch exceeds target before settling.
        // Peaks around t=1.0 and decays.
        let overshoot_amt = if t > 0.7 {
            let ot = (t - 0.7) / 0.8; // 0 at t=0.7, ~1.0 at t=1.5
            self.overshoot
                * interval
                * libm::sin(core::f64::consts::PI * ot)
                * libm::exp(-4.0 * (ot - 0.375).max(0.0))
        } else {
            0.0
        };

        libm::pow(2.0, base - undershoot_amt + overshoot_amt)
    }

    /// Reset the glide state.
    pub fn reset(&mut self) {
        self.source_freq = 440.0;
        self.target_freq = 440.0;
        self.elapsed = self.time + 1.0;
    }
}

impl Default for PitchGlide {
    fn default() -> Self {
        Self::new()
    }
}

/// Virtual Korg MS-20 synthesizer voice.
///
/// Contains two homogenized VCOs, a VCO mixer, pitch glide, and external
/// input control. Filters and envelopes are not yet implemented.
#[derive(Debug, Clone)]
pub struct Ms20Voice {
    /// Voltage-controlled oscillator 1.
    pub vco1: Vco,
    /// Voltage-controlled oscillator 2.
    pub vco2: Vco,
    /// VCO mix balance. 0.0 = all VCO1, 1.0 = all VCO2, 0.5 = equal mix.
    pub vco_mix: f64,
    /// Whether external audio input is patched into the signal chain.
    pub external_input_enabled: bool,
    /// External input mix level (0.0 to 1.0).
    pub external_input_level: f64,
    /// Current note frequency in Hz.
    pub note_freq: f64,
    /// Whether a note is currently active (gate on).
    pub gate: bool,
    /// Per-voice pitch glide (portamento).
    pub glide: PitchGlide,
}

impl Ms20Voice {
    /// Create a new MS-20 voice in its default state.
    ///
    /// Both VCOs default to triangle waveform at 8' scale. The voice is
    /// gated off with a default frequency of 440 Hz (A4).
    pub fn new() -> Self {
        Self {
            vco1: Vco::new(),
            vco2: Vco::new(),
            vco_mix: 0.5,
            external_input_enabled: false,
            external_input_level: 0.5,
            note_freq: 440.0,
            gate: false,
            glide: PitchGlide::new(),
        }
    }

    /// Trigger a new note at the given frequency.
    ///
    /// If pitch glide is enabled, the voice will smoothly transition from
    /// the current pitch to the new pitch. If glide is disabled, the pitch
    /// changes immediately.
    pub fn note_on(&mut self, freq: f64) {
        self.note_freq = freq;
        self.gate = true;
        self.glide.set_target(freq);
    }

    /// Release the current note (gate off).
    pub fn note_off(&mut self) {
        self.gate = false;
    }

    /// Process a mono audio buffer in place.
    ///
    /// When the gate is off, writes silence. When the gate is on, generates
    /// audio from both VCOs mixed according to `vco_mix`. External audio
    /// input (if enabled) is mixed in from the buffer contents before being
    /// overwritten.
    ///
    /// The buffer is treated as both input (for external audio) and output.
    pub fn process(&mut self, buffer: &mut [f64], sample_rate: u32) {
        let sr = sample_rate as f64;

        if !self.gate {
            for sample in buffer.iter_mut() {
                *sample = 0.0;
            }
            return;
        }

        for sample in buffer.iter_mut() {
            // Capture external input before overwriting.
            let external = if self.external_input_enabled {
                *sample * self.external_input_level
            } else {
                0.0
            };

            // Get current frequency from the pitch glide system.
            let freq = self.glide.tick(sr);

            // Generate VCO outputs. Save previous outputs before ticking
            // so ring modulator uses the previous tick's values.
            let vco1_prev = self.vco1.last_output();
            let vco2_prev = self.vco2.last_output();
            let vco1_out = self.vco1.tick(freq, vco2_prev, sr);
            let vco2_out = self.vco2.tick(freq, vco1_prev, sr);

            // Mix VCOs according to balance.
            let vco_mix = self.vco_mix.clamp(0.0, 1.0);
            let mixed = vco1_out * (1.0 - vco_mix) + vco2_out * vco_mix + external;

            // TODO: HPF (Sallen-Key), LPF (Korg-35), EG1, EG2, VCA
            // For now, output the raw VCO mix.
            *sample = mixed;
        }
    }

    /// Reset all internal state (VCOs, glide).
    pub fn reset(&mut self) {
        self.vco1.reset();
        self.vco2.reset();
        self.glide.reset();
        self.gate = false;
    }
}

impl Default for Ms20Voice {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vco::Waveform;

    #[test]
    fn new_voice_defaults() {
        let voice = Ms20Voice::new();
        assert!(!voice.external_input_enabled);
        assert!(!voice.gate);
        assert!((voice.note_freq - 440.0).abs() < f64::EPSILON);
        assert!((voice.vco_mix - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn gate_off_writes_silence() {
        let mut voice = Ms20Voice::new();
        voice.gate = false;
        let mut buffer = [1.0f64; 32];
        voice.process(&mut buffer, 48000);
        for sample in &buffer {
            assert!(sample.abs() < f64::EPSILON);
        }
    }

    #[test]
    fn gate_on_produces_output() {
        let mut voice = Ms20Voice::new();
        voice.note_on(440.0);
        let mut buffer = [0.0f64; 480];
        voice.process(&mut buffer, 48000);
        let has_nonzero = buffer.iter().any(|s| s.abs() > 1e-10);
        assert!(has_nonzero, "Gated voice should produce non-zero output");
    }

    #[test]
    fn note_on_note_off() {
        let mut voice = Ms20Voice::new();
        voice.note_on(440.0);
        assert!(voice.gate);
        voice.note_off();
        assert!(!voice.gate);
    }

    #[test]
    fn vco_mix_extremes() {
        // All VCO1 (mix=0): should match VCO1 output.
        let mut voice = Ms20Voice::new();
        voice.vco_mix = 0.0;
        voice.note_on(440.0);
        let mut buf = [0.0f64; 48];
        voice.process(&mut buf, 48000);
        let energy_mix0: f64 = buf.iter().map(|s| s * s).sum();
        assert!(energy_mix0 > 0.0, "VCO1-only mix should produce output");

        // All VCO2 (mix=1): should also produce output.
        let mut voice2 = Ms20Voice::new();
        voice2.vco_mix = 1.0;
        voice2.note_on(440.0);
        let mut buf2 = [0.0f64; 48];
        voice2.process(&mut buf2, 48000);
        let energy_mix1: f64 = buf2.iter().map(|s| s * s).sum();
        assert!(energy_mix1 > 0.0, "VCO2-only mix should produce output");
    }

    #[test]
    fn external_input_mixes_in() {
        let mut voice = Ms20Voice::new();
        voice.note_on(440.0);
        voice.external_input_enabled = true;
        voice.external_input_level = 1.0;

        // Process with external input = 0.5 on every sample.
        let mut buf_with = [0.5f64; 480];
        voice.process(&mut buf_with, 48000);

        // Process without external input.
        let mut voice2 = Ms20Voice::new();
        voice2.note_on(440.0);
        voice2.external_input_enabled = false;
        let mut buf_without = [0.5f64; 480];
        voice2.process(&mut buf_without, 48000);

        // The output should differ because of the external input.
        let diff: f64 = buf_with
            .iter()
            .zip(buf_without.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 0.1, "External input should change the output");
    }

    #[test]
    fn pitch_glide_disabled_instant() {
        let mut glide = PitchGlide::new();
        glide.enabled = false;
        glide.set_target(880.0);
        let freq = glide.current_freq();
        assert!(
            (freq - 880.0).abs() < 1e-6,
            "Disabled glide should jump to target: {freq}"
        );
    }

    #[test]
    fn pitch_glide_smooth_transition() {
        let mut glide = PitchGlide::new();
        glide.enabled = true;
        glide.time = 0.01; // 10ms glide
        glide.source_freq = 440.0;
        glide.target_freq = 440.0;
        glide.elapsed = glide.time + 1.0;

        // Start at 440, glide to 880.
        glide.set_target(880.0);

        let mut freqs = alloc::vec::Vec::new();
        for _ in 0..960 {
            // 20ms at 48kHz
            freqs.push(glide.tick(48000.0));
        }

        // First frequency should be near 440 (start).
        assert!(
            freqs[0] < 500.0,
            "Glide should start near source: {}",
            freqs[0]
        );

        // Last frequency should be near 880 (target).
        let last = freqs[freqs.len() - 1];
        assert!(
            (last - 880.0).abs() < 10.0,
            "Glide should end near target: {last}"
        );

        // Frequencies should generally increase.
        let increasing = freqs.windows(2).filter(|w| w[1] > w[0]).count();
        assert!(
            increasing > freqs.len() / 3,
            "Frequencies should trend upward: {increasing} increasing out of {}",
            freqs.len()
        );
    }

    #[test]
    fn pitch_glide_with_overshoot() {
        let mut glide = PitchGlide::new();
        glide.enabled = true;
        glide.time = 0.01;
        glide.overshoot = 0.5;
        glide.source_freq = 440.0;
        glide.target_freq = 440.0;
        glide.elapsed = glide.time + 1.0;

        glide.set_target(880.0);

        let mut max_freq = 0.0f64;
        for _ in 0..960 {
            let f = glide.tick(48000.0);
            if f > max_freq {
                max_freq = f;
            }
        }

        // With overshoot, the peak frequency should exceed the target.
        assert!(
            max_freq > 880.0,
            "Overshoot should cause frequency to exceed target: {max_freq}"
        );
    }

    #[test]
    fn pitch_glide_reset() {
        let mut glide = PitchGlide::new();
        glide.enabled = true;
        glide.time = 0.1;
        glide.set_target(880.0);
        for _ in 0..100 {
            glide.tick(48000.0);
        }
        glide.reset();
        let freq = glide.current_freq();
        assert!(
            (freq - 440.0).abs() < 1e-6,
            "Reset should return to default: {freq}"
        );
    }

    #[test]
    fn voice_reset_clears_state() {
        let mut voice = Ms20Voice::new();
        voice.note_on(880.0);
        let mut buf = [0.0f64; 48];
        voice.process(&mut buf, 48000);
        voice.reset();
        assert!(!voice.gate);
    }

    #[test]
    fn different_waveforms_sound_different() {
        let waveforms = [
            Waveform::Triangle,
            Waveform::Sawtooth,
            Waveform::Sine,
        ];

        let mut energies = alloc::vec::Vec::new();
        for wf in &waveforms {
            let mut voice = Ms20Voice::new();
            voice.vco1.waveform = *wf;
            voice.vco2.waveform = *wf;
            voice.note_on(440.0);
            let mut buf = [0.0f64; 480];
            voice.process(&mut buf, 48000);
            let energy: f64 = buf.iter().map(|s| s * s).sum();
            energies.push(energy);
        }

        // All waveforms should produce non-zero energy.
        for (i, e) in energies.iter().enumerate() {
            assert!(
                *e > 0.01,
                "Waveform {:?} should produce energy: {e}",
                waveforms[i]
            );
        }
    }

    #[test]
    fn default_matches_new() {
        let a = Ms20Voice::new();
        let b = Ms20Voice::default();
        assert_eq!(a.external_input_enabled, b.external_input_enabled);
        assert_eq!(a.gate, b.gate);
        assert!((a.note_freq - b.note_freq).abs() < f64::EPSILON);
    }
}
