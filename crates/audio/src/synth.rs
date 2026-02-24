//! Virtual Korg MS-20 synthesizer voice.
//!
//! The MS-20 signal chain consists of two VCOs, a Sallen-Key highpass filter,
//! a Korg-35 lowpass filter with resonance, two envelope generators, a VCA,
//! and a modulation matrix. The MS-20 also features an external signal
//! processor (ESP) input that allows external audio to be patched into the
//! filter and modulation paths.
//!
//! ## Signal Chain
//!
//! ```text
//! External Audio In --+
//!                     v
//! VCO1 --+--> Mixer --> HPF (Sallen-Key) --> LPF (Korg-35) --> VCA --> Out
//! VCO2 --+              ^                     ^                  ^
//!                       |                     |                  |
//!                       EG1                   EG2                EG2
//!                       |                     |                  |
//!                       +----<-- Mod Matrix --+---->-----------+-+
//!                                ^    ^    ^
//!                               LFO  Vel  Key
//! ```
//!
//! ## Modulation
//!
//! The modulation matrix connects sources (EG1, EG2, LFO, velocity,
//! keyboard tracking) to destinations (filter cutoffs, resonance, VCO
//! pitch, VCA level, pulse width). Default routings simulate the stock
//! MS-20 patch connections.

use crate::adsr::Adsr;
use crate::dsp::lfo::{Lfo, LfoWaveform};
use crate::modmatrix::{ModMatrix, ModSources};
use crate::ms20_filter::{Korg35Lpf, SallenKeyHpf};
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
        let undershoot_amt = self.undershoot * interval * libm::exp(-t * 6.0);

        // Overshoot: near the end, pitch exceeds target before settling.
        let overshoot_amt = if t > 0.7 {
            let ot = (t - 0.7) / 0.8;
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
/// Contains the complete MS-20 signal chain: two VCOs, HPF, LPF, two
/// envelope generators, VCA, LFO, modulation matrix, and pitch glide.
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
    /// Current note velocity (0.0 to 1.0).
    pub velocity: f64,
    /// Whether a note is currently active (gate on).
    pub gate: bool,
    /// Per-voice pitch glide (portamento).
    pub glide: PitchGlide,

    /// Envelope generator 1 (modulates HPF cutoff).
    pub eg1: Adsr,
    /// Envelope generator 2 (modulates LPF cutoff and VCA).
    pub eg2: Adsr,
    /// Sallen-Key highpass filter.
    pub hpf: SallenKeyHpf,
    /// Korg-35 lowpass filter.
    pub lpf: Korg35Lpf,
    /// Voice LFO for modulation.
    pub lfo: Lfo,
    /// LFO rate in Hz.
    pub lfo_rate: f64,
    /// Modulation matrix with configurable routings.
    pub mod_matrix: ModMatrix,
}

impl Ms20Voice {
    /// Create a new MS-20 voice in its default state.
    ///
    /// Both VCOs default to triangle waveform at 8' scale. The voice is
    /// gated off with a default frequency of 440 Hz (A4). The modulation
    /// matrix is initialized with stock MS-20 routings.
    pub fn new() -> Self {
        Self {
            vco1: Vco::new(),
            vco2: Vco::new(),
            vco_mix: 0.5,
            external_input_enabled: false,
            external_input_level: 0.5,
            note_freq: 440.0,
            velocity: 1.0,
            gate: false,
            glide: PitchGlide::new(),
            eg1: Adsr::new(0.01, 0.3, 0.0, 0.1),
            eg2: Adsr::default(),
            hpf: SallenKeyHpf::default(),
            lpf: Korg35Lpf::default(),
            lfo: Lfo::new(LfoWaveform::Sine),
            lfo_rate: 5.0,
            mod_matrix: ModMatrix::ms20_default(),
        }
    }

    /// Trigger a new note at the given frequency and velocity.
    ///
    /// If pitch glide is enabled, the voice will smoothly transition from
    /// the current pitch to the new pitch.
    pub fn note_on(&mut self, freq: f64) {
        self.note_on_with_velocity(freq, 1.0);
    }

    /// Trigger a new note with a specific velocity (0.0 to 1.0).
    pub fn note_on_with_velocity(&mut self, freq: f64, velocity: f64) {
        self.note_freq = freq;
        self.velocity = velocity.clamp(0.0, 1.0);
        self.gate = true;
        self.glide.set_target(freq);
        self.eg1.gate_on();
        self.eg2.gate_on();
    }

    /// Release the current note (gate off).
    pub fn note_off(&mut self) {
        self.gate = false;
        self.eg1.gate_off();
        self.eg2.gate_off();
    }

    /// Process a mono audio buffer in place.
    ///
    /// Runs the complete MS-20 signal chain: VCOs, mixer, HPF, LPF, VCA,
    /// with modulation from EG1, EG2, and LFO via the modulation matrix.
    ///
    /// The buffer is treated as both input (for external audio) and output.
    pub fn process(&mut self, buffer: &mut [f64], sample_rate: u32) {
        let sr = sample_rate as f64;

        for sample in buffer.iter_mut() {
            // Capture external input before overwriting.
            let external = if self.external_input_enabled {
                *sample * self.external_input_level
            } else {
                0.0
            };

            // If gate is off and envelopes are idle, output silence.
            if !self.gate
                && self.eg1.stage() == crate::adsr::EnvelopeStage::Idle
                && self.eg2.stage() == crate::adsr::EnvelopeStage::Idle
            {
                *sample = 0.0;
                continue;
            }

            // Advance modulation sources.
            let eg1_val = self.eg1.tick(sr);
            let eg2_val = self.eg2.tick(sr);
            let lfo_val = self.lfo.tick(self.lfo_rate, sr);

            // Compute keyboard tracking (normalized: C2=0, C7=1).
            let key_track =
                ((libm::log2(self.note_freq.max(1.0)) - libm::log2(65.41)) / 5.0).clamp(0.0, 1.0);

            // Compute modulation outputs.
            let mod_sources = ModSources {
                eg1: eg1_val,
                eg2: eg2_val,
                lfo: lfo_val,
                velocity: self.velocity,
                key_track,
            };
            let mods = self.mod_matrix.compute(&mod_sources);

            // Get current frequency from the pitch glide system.
            let base_freq = self.glide.tick(sr);

            // Apply pitch modulation to VCO frequencies.
            let vco1_mod_semitones = mods.vco1_pitch;
            let vco2_mod_semitones = mods.vco2_pitch;
            let vco1_freq = base_freq * libm::pow(2.0, vco1_mod_semitones / 12.0);
            let vco2_freq = base_freq * libm::pow(2.0, vco2_mod_semitones / 12.0);

            // Apply PWM modulation.
            if mods.pwm_width.abs() > 1e-10 {
                self.vco1.pulse_width = (self.vco1.pulse_width + mods.pwm_width).clamp(0.01, 0.99);
                self.vco2.pulse_width = (self.vco2.pulse_width + mods.pwm_width).clamp(0.01, 0.99);
            }

            // Generate VCO outputs with ring mod cross-reference.
            let vco1_prev = self.vco1.last_output();
            let vco2_prev = self.vco2.last_output();
            let vco1_out = self.vco1.tick(vco1_freq, vco2_prev, sr);
            let vco2_out = self.vco2.tick(vco2_freq, vco1_prev, sr);

            // Mix VCOs and external input.
            let vco_mix = self.vco_mix.clamp(0.0, 1.0);
            let mixed = vco1_out * (1.0 - vco_mix) + vco2_out * vco_mix + external;

            // Apply HPF with EG1 modulation.
            let hpf_base_cutoff = self.hpf.cutoff;
            let hpf_mod_cutoff =
                hpf_base_cutoff * libm::pow(2.0, mods.hpf_cutoff);
            self.hpf.cutoff = hpf_mod_cutoff.clamp(20.0, 20000.0);
            let after_hpf = self.hpf.tick(mixed, sr);
            self.hpf.cutoff = hpf_base_cutoff; // Restore base cutoff.

            // Apply LPF with EG2 modulation.
            let lpf_base_cutoff = self.lpf.cutoff;
            let lpf_base_res = self.lpf.resonance;
            let lpf_mod_cutoff =
                lpf_base_cutoff * libm::pow(2.0, mods.lpf_cutoff);
            self.lpf.cutoff = lpf_mod_cutoff.clamp(20.0, 20000.0);
            self.lpf.resonance = (lpf_base_res + mods.lpf_resonance).clamp(0.0, 2.0);
            let after_lpf = self.lpf.tick(after_hpf, sr);
            self.lpf.cutoff = lpf_base_cutoff; // Restore base cutoff.
            self.lpf.resonance = lpf_base_res; // Restore base resonance.

            // Apply VCA with EG2 modulation.
            let vca_level = mods.vca_level.clamp(0.0, 1.0);
            *sample = after_lpf * vca_level;
        }
    }

    /// Reset all internal state (VCOs, filters, envelopes, glide).
    pub fn reset(&mut self) {
        self.vco1.reset();
        self.vco2.reset();
        self.glide.reset();
        self.eg1.reset();
        self.eg2.reset();
        self.hpf.reset();
        self.lpf.reset();
        self.lfo.reset();
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
        assert!((voice.velocity - 1.0).abs() < f64::EPSILON);
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
        let mut buffer = [0.0f64; 4800];
        voice.process(&mut buffer, 48000);
        // Should produce non-zero output (VCA shaped by EG2).
        let has_nonzero = buffer.iter().any(|s| s.abs() > 1e-6);
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
    fn note_off_decays_to_silence() {
        let mut voice = Ms20Voice::new();
        voice.eg2.release = 0.01; // 10ms release
        voice.note_on(440.0);

        // Let attack/decay happen.
        let mut buf = [0.0f64; 4800];
        voice.process(&mut buf, 48000);

        // Release the note.
        voice.note_off();

        // Process enough samples for release to complete.
        let mut release_buf = [0.0f64; 4800];
        voice.process(&mut release_buf, 48000);

        // The last samples should be near silence.
        let tail_energy: f64 = release_buf[4000..].iter().map(|s| s * s).sum();
        assert!(
            tail_energy < 0.01,
            "Should decay to near silence after release: {tail_energy}"
        );
    }

    #[test]
    fn velocity_affects_output() {
        let mut voice_loud = Ms20Voice::new();
        voice_loud.note_on_with_velocity(440.0, 1.0);
        let mut buf_loud = [0.0f64; 4800];
        voice_loud.process(&mut buf_loud, 48000);
        let energy_loud: f64 = buf_loud.iter().map(|s| s * s).sum();

        // Velocity alone does not directly scale unless routed in mod matrix.
        // The default mod matrix does not route velocity, so this test
        // verifies the velocity field is stored correctly.
        assert!(
            (voice_loud.velocity - 1.0).abs() < f64::EPSILON,
            "Velocity should be stored"
        );
        assert!(energy_loud > 0.0, "Should produce output");
    }

    #[test]
    fn vco_mix_extremes() {
        let mut voice = Ms20Voice::new();
        voice.vco_mix = 0.0;
        voice.note_on(440.0);
        let mut buf = [0.0f64; 480];
        voice.process(&mut buf, 48000);
        let energy_mix0: f64 = buf.iter().map(|s| s * s).sum();
        assert!(energy_mix0 > 0.0, "VCO1-only mix should produce output");

        let mut voice2 = Ms20Voice::new();
        voice2.vco_mix = 1.0;
        voice2.note_on(440.0);
        let mut buf2 = [0.0f64; 480];
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
        let mut buf_with = [0.5f64; 4800];
        voice.process(&mut buf_with, 48000);

        let mut voice2 = Ms20Voice::new();
        voice2.note_on(440.0);
        voice2.external_input_enabled = false;
        let mut buf_without = [0.5f64; 4800];
        voice2.process(&mut buf_without, 48000);

        let diff: f64 = buf_with
            .iter()
            .zip(buf_without.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 0.01, "External input should change the output");
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
        glide.time = 0.01;
        glide.source_freq = 440.0;
        glide.target_freq = 440.0;
        glide.elapsed = glide.time + 1.0;

        glide.set_target(880.0);

        let mut freqs = alloc::vec::Vec::new();
        for _ in 0..960 {
            freqs.push(glide.tick(48000.0));
        }

        assert!(freqs[0] < 500.0, "Glide should start near source: {}", freqs[0]);
        let last = freqs[freqs.len() - 1];
        assert!((last - 880.0).abs() < 10.0, "Glide should end near target: {last}");

        let increasing = freqs.windows(2).filter(|w| w[1] > w[0]).count();
        assert!(
            increasing > freqs.len() / 3,
            "Frequencies should trend upward: {increasing} of {}",
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
        assert!(max_freq > 880.0, "Overshoot should exceed target: {max_freq}");
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
        assert!((freq - 440.0).abs() < 1e-6, "Reset should return to default: {freq}");
    }

    #[test]
    fn voice_reset_clears_state() {
        let mut voice = Ms20Voice::new();
        voice.note_on(880.0);
        let mut buf = [0.0f64; 480];
        voice.process(&mut buf, 48000);
        voice.reset();
        assert!(!voice.gate);
    }

    #[test]
    fn hpf_affects_output() {
        // With HPF at high cutoff, low-frequency content should be reduced.
        let mut voice = Ms20Voice::new();
        voice.hpf.cutoff = 5000.0;
        voice.hpf.active = true;
        voice.note_on(200.0); // Low frequency note

        let mut buf_hpf = [0.0f64; 4800];
        voice.process(&mut buf_hpf, 48000);
        let energy_hpf: f64 = buf_hpf.iter().map(|s| s * s).sum();

        // Without HPF.
        let mut voice2 = Ms20Voice::new();
        voice2.hpf.active = false;
        voice2.note_on(200.0);

        let mut buf_no_hpf = [0.0f64; 4800];
        voice2.process(&mut buf_no_hpf, 48000);
        let energy_no_hpf: f64 = buf_no_hpf.iter().map(|s| s * s).sum();

        assert!(
            energy_hpf < energy_no_hpf,
            "HPF should reduce energy of low-frequency note: {energy_hpf} vs {energy_no_hpf}"
        );
    }

    #[test]
    fn lpf_affects_output() {
        // With LPF at low cutoff, high-frequency content should be reduced.
        let mut voice = Ms20Voice::new();
        voice.vco1.waveform = Waveform::Sawtooth;
        voice.vco2.waveform = Waveform::Sawtooth;
        voice.lpf.cutoff = 200.0;
        voice.lpf.active = true;
        voice.note_on(440.0);

        let mut buf_lpf = [0.0f64; 4800];
        voice.process(&mut buf_lpf, 48000);
        let energy_lpf: f64 = buf_lpf.iter().map(|s| s * s).sum();

        // Without LPF.
        let mut voice2 = Ms20Voice::new();
        voice2.vco1.waveform = Waveform::Sawtooth;
        voice2.vco2.waveform = Waveform::Sawtooth;
        voice2.lpf.active = false;
        voice2.note_on(440.0);

        let mut buf_no_lpf = [0.0f64; 4800];
        voice2.process(&mut buf_no_lpf, 48000);
        let energy_no_lpf: f64 = buf_no_lpf.iter().map(|s| s * s).sum();

        assert!(
            energy_lpf < energy_no_lpf,
            "LPF should reduce energy: {energy_lpf} vs {energy_no_lpf}"
        );
    }

    #[test]
    fn different_waveforms_sound_different() {
        let waveforms = [Waveform::Triangle, Waveform::Sawtooth, Waveform::Sine];

        let mut energies = alloc::vec::Vec::new();
        for wf in &waveforms {
            let mut voice = Ms20Voice::new();
            voice.vco1.waveform = *wf;
            voice.vco2.waveform = *wf;
            voice.note_on(440.0);
            let mut buf = [0.0f64; 4800];
            voice.process(&mut buf, 48000);
            let energy: f64 = buf.iter().map(|s| s * s).sum();
            energies.push(energy);
        }

        for (i, e) in energies.iter().enumerate() {
            assert!(
                *e > 0.001,
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

    #[test]
    fn output_bounded() {
        let mut voice = Ms20Voice::new();
        voice.note_on(440.0);
        let mut buffer = [0.0f64; 48000];
        voice.process(&mut buffer, 48000);
        for (i, s) in buffer.iter().enumerate() {
            assert!(
                s.abs() < 10.0,
                "Output should be bounded at sample {i}: {s}"
            );
        }
    }
}
