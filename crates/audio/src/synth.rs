//! Virtual Korg MS-20 synthesizer with monophonic and polyphonic voice modes.
//!
//! The MS-20 signal chain consists of two VCOs, a Sallen-Key highpass filter,
//! a Korg-35 lowpass filter with resonance, two envelope generators, a VCA,
//! and a modulation matrix. The MS-20 also features an external signal
//! processor (ESP) input that allows external audio to be patched into the
//! filter and modulation paths.
//!
//! ## Signal Chain (per voice)
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
//! ## Voice Modes
//!
//! - `High(1)`: Monophonic, highest held note sounds (stock MS-20 behavior).
//! - `Low(1)`: Monophonic, lowest held note sounds (bass synth behavior).
//! - `High(N)`: Polyphonic N voices, voice stealing drops lowest note.
//! - `Low(N)`: Polyphonic N voices, voice stealing drops highest note.
//!
//! ## Velocity Routing
//!
//! Four independent velocity depth controls scale per-voice velocity (0.0
//! to 1.0, linear) into modulation targets. All default to zero (stock
//! MS-20 behavior with no velocity sensitivity).

use alloc::vec::Vec;

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

// ---------------------------------------------------------------------------
// Voice mode and per-voice state
// ---------------------------------------------------------------------------

/// Voice mode determining note priority and polyphony.
///
/// The parameter specifies the number of simultaneous voices. For mono modes
/// (1 voice), note priority determines which held note sounds. For poly modes
/// (N voices), voice stealing drops notes based on priority when the pool is
/// exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceMode {
    /// High note priority. Mono: highest held note sounds. Poly: steal
    /// the voice playing the lowest note when the pool is full.
    High(u8),
    /// Low note priority. Mono: lowest held note sounds. Poly: steal
    /// the voice playing the highest note when the pool is full.
    Low(u8),
}

impl VoiceMode {
    /// Return the number of voices for this mode.
    pub fn voice_count(self) -> u8 {
        match self {
            VoiceMode::High(n) | VoiceMode::Low(n) => n.max(1),
        }
    }

    /// Return true if this is a monophonic mode (1 voice).
    pub fn is_mono(self) -> bool {
        self.voice_count() == 1
    }
}

impl Default for VoiceMode {
    fn default() -> Self {
        VoiceMode::High(1)
    }
}

/// A held note entry for the monophonic note stack.
#[derive(Debug, Clone, Copy)]
struct HeldNote {
    freq: f64,
    velocity: f64,
}

/// Notes the monophonic stack remembers at once. A player holds far fewer,
/// and a script or file that never releases notes cannot grow the stack past
/// this bound, because the oldest entry gives way.
const MAX_HELD_NOTES: usize = 64;

/// The monophonic note stack, in fixed storage.
///
/// `NoteOn` runs on the audio thread, so the stack must not grow a heap
/// buffer there. A `Vec` would allocate on its first push and again whenever
/// it doubled, and a clone of the synthesizer would lose any capacity
/// reserved beforehand. A full stack forgets its oldest note, which changes
/// which note sounds only after more than `MAX_HELD_NOTES` notes are held.
#[derive(Debug, Clone)]
struct HeldNotes {
    notes: [HeldNote; MAX_HELD_NOTES],
    len: usize,
}

impl HeldNotes {
    const fn new() -> Self {
        Self {
            notes: [HeldNote {
                freq: 0.0,
                velocity: 0.0,
            }; MAX_HELD_NOTES],
            len: 0,
        }
    }

    fn as_slice(&self) -> &[HeldNote] {
        &self.notes[..self.len]
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    fn push(&mut self, note: HeldNote) {
        if self.len == MAX_HELD_NOTES {
            self.notes.copy_within(1.., 0);
            self.len -= 1;
        }
        self.notes[self.len] = note;
        self.len += 1;
    }

    fn retain(&mut self, keep: impl Fn(&HeldNote) -> bool) {
        let mut kept = 0;
        for read in 0..self.len {
            let note = self.notes[read];
            if keep(&note) {
                self.notes[kept] = note;
                kept += 1;
            }
        }
        self.len = kept;
    }
}

/// Per-voice DSP state for the MS-20 synthesizer.
///
/// Each voice contains its own oscillators, envelope generators, filters,
/// and pitch glide so that polyphonic modes can maintain independent state
/// per note.
#[derive(Debug, Clone)]
pub struct SingleVoice {
    /// Voltage-controlled oscillator 1.
    pub vco1: Vco,
    /// Voltage-controlled oscillator 2.
    pub vco2: Vco,
    /// Envelope generator 1 (modulates HPF cutoff).
    pub eg1: Adsr,
    /// Envelope generator 2 (modulates LPF cutoff and VCA).
    pub eg2: Adsr,
    /// Sallen-Key highpass filter.
    pub hpf: SallenKeyHpf,
    /// Korg-35 lowpass filter.
    pub lpf: Korg35Lpf,
    /// Per-voice pitch glide (portamento).
    pub glide: PitchGlide,
    /// Current note frequency in Hz.
    pub note_freq: f64,
    /// Current note velocity (0.0 to 1.0).
    pub velocity: f64,
    /// Whether a note is currently active (gate on).
    pub gate: bool,
}

impl SingleVoice {
    /// Create a new voice in its default state (gated off, A4).
    pub fn new() -> Self {
        Self {
            vco1: Vco::new(),
            vco2: Vco::new(),
            eg1: Adsr::new(0.01, 0.3, 0.0, 0.1),
            eg2: Adsr::default(),
            hpf: SallenKeyHpf::default(),
            lpf: Korg35Lpf::default(),
            glide: PitchGlide::new(),
            note_freq: 440.0,
            velocity: 1.0,
            gate: false,
        }
    }

    /// Reset all DSP state (oscillators, filters, envelopes, glide).
    pub fn reset(&mut self) {
        self.vco1.reset();
        self.vco2.reset();
        self.eg1.reset();
        self.eg2.reset();
        self.hpf.reset();
        self.lpf.reset();
        self.glide.reset();
        self.gate = false;
    }

    /// Trigger a note on this voice with retrigger.
    fn trigger(&mut self, freq: f64, velocity: f64) {
        self.note_freq = freq;
        self.velocity = velocity.clamp(0.0, 1.0);
        self.gate = true;
        self.glide.set_target(freq);
        self.eg1.gate_on();
        self.eg2.gate_on();
    }

    /// Release this voice (gate off).
    fn release(&mut self) {
        self.gate = false;
        self.eg1.gate_off();
        self.eg2.gate_off();
    }

    /// Return true if this voice is completely idle (gate off, envelopes done).
    fn is_idle(&self) -> bool {
        !self.gate
            && self.eg1.stage() == crate::adsr::EnvelopeStage::Idle
            && self.eg2.stage() == crate::adsr::EnvelopeStage::Idle
    }
}

impl Default for SingleVoice {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Ms20Voice: multi-voice synthesizer
// ---------------------------------------------------------------------------

/// Virtual Korg MS-20 synthesizer with configurable voice modes.
///
/// Contains a pool of [`SingleVoice`] instances (1 for mono, N for poly),
/// shared state (LFO, modulation matrix, VCO mix), and voice allocation
/// logic (note priority for mono, voice stealing for poly).
#[derive(Debug, Clone)]
pub struct Ms20Voice {
    /// Voice pool. Mono modes use 1 voice, poly modes use N voices.
    pub voices: Vec<SingleVoice>,
    /// Voice mode determining note priority and polyphony.
    pub voice_mode: VoiceMode,
    /// VCO mix balance. 0.0 = all VCO1, 1.0 = all VCO2, 0.5 = equal mix.
    pub vco_mix: f64,
    /// Ring modulation depth. 0.0 = pure additive VCO mix, 1.0 = pure ring
    /// modulation (VCO1 * VCO2). Intermediate values crossfade between the
    /// two signals.
    pub ring_mod_depth: f64,
    /// Whether external audio input is patched into the signal chain.
    pub external_input_enabled: bool,
    /// External input mix level (0.0 to 1.0).
    pub external_input_level: f64,
    /// Voice LFO for modulation (shared across all voices).
    pub lfo: Lfo,
    /// LFO rate in Hz.
    pub lfo_rate: f64,
    /// Modulation matrix with configurable routings (shared across all voices).
    pub mod_matrix: ModMatrix,
    /// Velocity depth for VCA level modulation (0.0 = no effect).
    pub velocity_vca_depth: f64,
    /// Velocity depth for LPF cutoff modulation in octaves (0.0 = no effect).
    pub velocity_filter_depth: f64,
    /// Velocity depth for envelope scaling (0.0 = no effect).
    pub velocity_envelope_depth: f64,
    /// Velocity depth for VCO pitch modulation in semitones (0.0 = no effect).
    pub velocity_pitch_depth: f64,

    /// Held note stack for monophonic note priority.
    held_notes: HeldNotes,
}

impl Ms20Voice {
    /// Create a new MS-20 voice in its default state (mono high, 1 voice).
    pub fn new() -> Self {
        Self::with_voice_mode(VoiceMode::default())
    }

    /// Create a new MS-20 voice with the specified voice mode.
    pub fn with_voice_mode(mode: VoiceMode) -> Self {
        let count = mode.voice_count() as usize;
        let voices = (0..count).map(|_| SingleVoice::new()).collect();
        Self {
            voices,
            voice_mode: mode,
            vco_mix: 0.5,
            ring_mod_depth: 0.0,
            external_input_enabled: false,
            external_input_level: 0.5,
            lfo: Lfo::new(LfoWaveform::Sine),
            lfo_rate: 5.0,
            mod_matrix: ModMatrix::ms20_default(),
            velocity_vca_depth: 0.0,
            velocity_filter_depth: 0.0,
            velocity_envelope_depth: 0.0,
            velocity_pitch_depth: 0.0,
            held_notes: HeldNotes::new(),
        }
    }

    /// Change the voice mode, resizing the voice pool as needed.
    ///
    /// All active notes are released. New voices are initialized by cloning
    /// the first voice's patch parameters (VCO/filter/envelope settings).
    pub fn set_voice_mode(&mut self, mode: VoiceMode) {
        let new_count = mode.voice_count() as usize;
        self.voice_mode = mode;

        // Release all active voices.
        for voice in &mut self.voices {
            voice.release();
        }
        self.held_notes.clear();

        // Expand pool by cloning voice 0's configuration.
        if self.voices.len() < new_count {
            let template = self.voices.first().cloned().unwrap_or_default();
            while self.voices.len() < new_count {
                let mut new_voice = template.clone();
                new_voice.reset();
                self.voices.push(new_voice);
            }
        }
        self.voices.truncate(new_count);
    }

    /// Trigger a new note at the given frequency with default velocity (1.0).
    pub fn note_on(&mut self, freq: f64) {
        self.note_on_with_velocity(freq, 1.0);
    }

    /// Trigger a new note with a specific velocity (0.0 to 1.0).
    pub fn note_on_with_velocity(&mut self, freq: f64, velocity: f64) {
        let vel = velocity.clamp(0.0, 1.0);

        if self.voice_mode.is_mono() {
            self.mono_note_on(freq, vel);
        } else {
            self.poly_note_on(freq, vel);
        }
    }

    /// Release all active notes (backward-compatible no-argument form).
    pub fn note_off(&mut self) {
        self.held_notes.clear();
        for voice in &mut self.voices {
            if voice.gate {
                voice.release();
            }
        }
    }

    /// Release a specific note by frequency.
    ///
    /// In mono mode, removes the note from the held stack and switches to
    /// the next priority note (with retrigger) or releases if no notes remain.
    /// In poly mode, releases the voice playing the matching frequency.
    pub fn note_off_freq(&mut self, freq: f64) {
        if self.voice_mode.is_mono() {
            self.mono_note_off(freq);
        } else {
            self.poly_note_off(freq);
        }
    }

    /// Process a mono audio buffer in place, summing all active voices.
    ///
    /// Runs the complete MS-20 signal chain per voice: VCOs, mixer, HPF,
    /// LPF, VCA, with modulation from EG1, EG2, LFO, and velocity via the
    /// modulation matrix. The buffer is treated as both input (for external
    /// audio on voice 0) and output.
    pub fn process(&mut self, buffer: &mut [f64], sample_rate: u32) {
        let sr = sample_rate as f64;
        let vco_mix = self.vco_mix.clamp(0.0, 1.0);
        let ring_mod = self.ring_mod_depth.clamp(0.0, 1.0);

        for sample in buffer.iter_mut() {
            // Capture external input before overwriting.
            let external = if self.external_input_enabled {
                *sample * self.external_input_level
            } else {
                0.0
            };

            // Tick shared LFO (one phase advance per sample).
            let lfo_val = self.lfo.tick(self.lfo_rate, sr);

            // Sum all voice outputs.
            let mut output = 0.0;

            for (vi, voice) in self.voices.iter_mut().enumerate() {
                // Skip idle voices.
                if voice.is_idle() {
                    continue;
                }

                // Advance per-voice envelopes.
                let eg1_val = voice.eg1.tick(sr);
                let eg2_val = voice.eg2.tick(sr);

                // Scale envelope outputs by velocity envelope depth.
                let vel_env_scale = 1.0 - self.velocity_envelope_depth
                    + self.velocity_envelope_depth * voice.velocity;
                let eg1_mod = eg1_val * vel_env_scale;
                let eg2_mod = eg2_val * vel_env_scale;

                // Compute keyboard tracking (normalized: C2=0, C7=1).
                let key_track = ((libm::log2(voice.note_freq.max(1.0)) - libm::log2(65.41)) / 5.0)
                    .clamp(0.0, 1.0);

                // Compute modulation outputs.
                let mod_sources = ModSources {
                    eg1: eg1_mod,
                    eg2: eg2_mod,
                    lfo: lfo_val,
                    velocity: voice.velocity,
                    key_track,
                };
                let mods = self.mod_matrix.compute(&mod_sources);

                // Get current frequency from pitch glide.
                let base_freq = voice.glide.tick(sr);

                // Apply pitch modulation with velocity pitch depth.
                let vel_pitch = self.velocity_pitch_depth * voice.velocity;
                let vco1_mod_semitones = mods.vco1_pitch + vel_pitch;
                let vco2_mod_semitones = mods.vco2_pitch + vel_pitch;
                let vco1_freq = base_freq * libm::pow(2.0, vco1_mod_semitones / 12.0);
                let vco2_freq = base_freq * libm::pow(2.0, vco2_mod_semitones / 12.0);

                // Apply PWM modulation.
                if mods.pwm_width.abs() > 1e-10 {
                    voice.vco1.pulse_width =
                        (voice.vco1.pulse_width + mods.pwm_width).clamp(0.01, 0.99);
                    voice.vco2.pulse_width =
                        (voice.vco2.pulse_width + mods.pwm_width).clamp(0.01, 0.99);
                }

                // Generate VCO outputs independently.
                let vco1_out = voice.vco1.tick(vco1_freq, sr);
                let vco2_out = voice.vco2.tick(vco2_freq, sr);

                // Mix VCOs with optional ring modulation crossfade.
                // dry = additive blend, wet = VCO1 * VCO2.
                let dry = vco1_out * (1.0 - vco_mix) + vco2_out * vco_mix;
                let wet = vco1_out * vco2_out;
                let ext = if vi == 0 { external } else { 0.0 };
                let mixed = dry * (1.0 - ring_mod) + wet * ring_mod + ext;

                // Apply HPF with EG1 modulation.
                let hpf_base_cutoff = voice.hpf.cutoff;
                let hpf_mod_cutoff = hpf_base_cutoff * libm::pow(2.0, mods.hpf_cutoff);
                voice.hpf.cutoff = hpf_mod_cutoff.clamp(20.0, 20000.0);
                let after_hpf = voice.hpf.tick(mixed, sr);
                voice.hpf.cutoff = hpf_base_cutoff;

                // Apply LPF with EG2 modulation and velocity filter depth.
                let lpf_base_cutoff = voice.lpf.cutoff;
                let lpf_base_res = voice.lpf.resonance;
                let vel_filter_octaves = self.velocity_filter_depth * voice.velocity;
                let lpf_mod_cutoff =
                    lpf_base_cutoff * libm::pow(2.0, mods.lpf_cutoff + vel_filter_octaves);
                voice.lpf.cutoff = lpf_mod_cutoff.clamp(20.0, 20000.0);
                voice.lpf.resonance = (lpf_base_res + mods.lpf_resonance).clamp(0.0, 2.0);
                let after_lpf = voice.lpf.tick(after_hpf, sr);
                voice.lpf.cutoff = lpf_base_cutoff;
                voice.lpf.resonance = lpf_base_res;

                // Apply VCA with EG2 modulation and velocity VCA depth.
                let vca_base = mods.vca_level.clamp(0.0, 1.0);
                let vca_vel_scale =
                    1.0 - self.velocity_vca_depth + self.velocity_vca_depth * voice.velocity;
                let vca_level = (vca_base * vca_vel_scale).clamp(0.0, 1.0);
                output += after_lpf * vca_level;
            }

            debug_assert!(
                output.is_finite(),
                "Ms20Voice::process produced non-finite output: {output}"
            );
            *sample = output;
        }
    }

    /// Reset all internal state (all voices, LFO, held notes).
    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.reset();
        }
        self.lfo.reset();
        self.held_notes.clear();
    }

    // -----------------------------------------------------------------------
    // Monophonic note handling
    // -----------------------------------------------------------------------

    /// Handle note-on in monophonic mode.
    ///
    /// Pushes the note onto the held stack and selects the priority note
    /// (highest for High mode, lowest for Low mode). Always retriggers
    /// envelopes.
    fn mono_note_on(&mut self, freq: f64, velocity: f64) {
        self.held_notes.push(HeldNote { freq, velocity });
        let (sel_freq, sel_vel) = self.mono_select_note();
        self.voices[0].trigger(sel_freq, sel_vel);
    }

    /// Handle note-off in monophonic mode.
    ///
    /// Removes the note from the held stack. If other notes remain, switches
    /// to the priority note with retrigger. Otherwise releases the voice.
    fn mono_note_off(&mut self, freq: f64) {
        self.held_notes.retain(|n| (n.freq - freq).abs() > 0.01);
        if self.held_notes.is_empty() {
            self.voices[0].release();
        } else {
            let (sel_freq, sel_vel) = self.mono_select_note();
            self.voices[0].trigger(sel_freq, sel_vel);
        }
    }

    /// Select the priority note from the held stack.
    ///
    /// Returns (freq, velocity) of the highest note for High mode or the
    /// lowest note for Low mode. Panics if held_notes is empty.
    fn mono_select_note(&self) -> (f64, f64) {
        let note = match self.voice_mode {
            VoiceMode::High(_) => self.held_notes.as_slice().iter().max_by(|a, b| {
                a.freq
                    .partial_cmp(&b.freq)
                    .unwrap_or(core::cmp::Ordering::Equal)
            }),
            VoiceMode::Low(_) => self.held_notes.as_slice().iter().min_by(|a, b| {
                a.freq
                    .partial_cmp(&b.freq)
                    .unwrap_or(core::cmp::Ordering::Equal)
            }),
        };
        let n = note.expect("held_notes must not be empty when selecting");
        (n.freq, n.velocity)
    }

    // -----------------------------------------------------------------------
    // Polyphonic note handling
    // -----------------------------------------------------------------------

    /// Handle note-on in polyphonic mode.
    ///
    /// Assigns the note to a free voice, or steals the lowest-frequency
    /// voice (High mode) or highest-frequency voice (Low mode) when the
    /// pool is full. Retriggers envelopes on the assigned voice.
    fn poly_note_on(&mut self, freq: f64, velocity: f64) {
        // Try to find a free (idle or gate-off) voice.
        let free_idx = self.voices.iter().position(|v| v.is_idle());
        if let Some(idx) = free_idx {
            self.voices[idx].trigger(freq, velocity);
            return;
        }

        // Try a voice that is releasing (gate off but envelopes still active).
        let releasing_idx = self.voices.iter().position(|v| !v.gate);
        if let Some(idx) = releasing_idx {
            self.voices[idx].trigger(freq, velocity);
            return;
        }

        // All voices active: steal based on priority.
        let steal_idx = self.find_steal_target();
        if let Some(idx) = steal_idx {
            self.voices[idx].trigger(freq, velocity);
        }
    }

    /// Handle note-off in polyphonic mode.
    ///
    /// Finds the voice playing the given frequency and releases it.
    fn poly_note_off(&mut self, freq: f64) {
        if let Some(voice) = self
            .voices
            .iter_mut()
            .filter(|v| v.gate)
            .find(|v| (v.note_freq - freq).abs() < 0.01)
        {
            voice.release();
        }
    }

    /// Find the voice index to steal based on voice mode priority.
    ///
    /// High mode steals the lowest-frequency voice. Low mode steals the
    /// highest-frequency voice.
    fn find_steal_target(&self) -> Option<usize> {
        if self.voices.is_empty() {
            return None;
        }
        let mut target = 0;
        for (i, voice) in self.voices.iter().enumerate().skip(1) {
            match self.voice_mode {
                VoiceMode::High(_) => {
                    if voice.note_freq < self.voices[target].note_freq {
                        target = i;
                    }
                }
                VoiceMode::Low(_) => {
                    if voice.note_freq > self.voices[target].note_freq {
                        target = i;
                    }
                }
            }
        }
        Some(target)
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
        let synth = Ms20Voice::new();
        assert_eq!(synth.voice_mode, VoiceMode::High(1));
        assert_eq!(synth.voices.len(), 1);
        assert!(!synth.external_input_enabled);
        assert!(!synth.voices[0].gate);
        assert!((synth.voices[0].note_freq - 440.0).abs() < f64::EPSILON);
        assert!((synth.vco_mix - 0.5).abs() < f64::EPSILON);
        assert!((synth.voices[0].velocity - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gate_off_writes_silence() {
        let mut synth = Ms20Voice::new();
        let mut buffer = [1.0f64; 32];
        synth.process(&mut buffer, 48000);
        for sample in &buffer {
            assert!(sample.abs() < f64::EPSILON);
        }
    }

    #[test]
    fn gate_on_produces_output() {
        let mut synth = Ms20Voice::new();
        synth.note_on(440.0);
        let mut buffer = [0.0f64; 4800];
        synth.process(&mut buffer, 48000);
        let has_nonzero = buffer.iter().any(|s| s.abs() > 1e-6);
        assert!(has_nonzero, "Gated voice should produce non-zero output");
    }

    #[test]
    fn note_on_note_off() {
        let mut synth = Ms20Voice::new();
        synth.note_on(440.0);
        assert!(synth.voices[0].gate);
        synth.note_off();
        assert!(!synth.voices[0].gate);
    }

    #[test]
    fn note_off_decays_to_silence() {
        let mut synth = Ms20Voice::new();
        synth.voices[0].eg2.release = 0.01;
        synth.note_on(440.0);

        let mut buf = [0.0f64; 4800];
        synth.process(&mut buf, 48000);

        synth.note_off();

        let mut release_buf = [0.0f64; 4800];
        synth.process(&mut release_buf, 48000);

        let tail_energy: f64 = release_buf[4000..].iter().map(|s| s * s).sum();
        assert!(
            tail_energy < 0.01,
            "Should decay to near silence after release: {tail_energy}"
        );
    }

    #[test]
    fn velocity_stored_correctly() {
        let mut synth = Ms20Voice::new();
        synth.note_on_with_velocity(440.0, 0.75);
        assert!(
            (synth.voices[0].velocity - 0.75).abs() < f64::EPSILON,
            "Velocity should be stored"
        );
    }

    #[test]
    fn vco_mix_extremes() {
        let mut synth = Ms20Voice::new();
        synth.vco_mix = 0.0;
        synth.note_on(440.0);
        let mut buf = [0.0f64; 480];
        synth.process(&mut buf, 48000);
        let energy_mix0: f64 = buf.iter().map(|s| s * s).sum();
        assert!(energy_mix0 > 0.0, "VCO1-only mix should produce output");

        let mut synth2 = Ms20Voice::new();
        synth2.vco_mix = 1.0;
        synth2.note_on(440.0);
        let mut buf2 = [0.0f64; 480];
        synth2.process(&mut buf2, 48000);
        let energy_mix1: f64 = buf2.iter().map(|s| s * s).sum();
        assert!(energy_mix1 > 0.0, "VCO2-only mix should produce output");
    }

    #[test]
    fn external_input_mixes_in() {
        let mut synth = Ms20Voice::new();
        synth.note_on(440.0);
        synth.external_input_enabled = true;
        synth.external_input_level = 1.0;
        let mut buf_with = [0.5f64; 4800];
        synth.process(&mut buf_with, 48000);

        let mut synth2 = Ms20Voice::new();
        synth2.note_on(440.0);
        synth2.external_input_enabled = false;
        let mut buf_without = [0.5f64; 4800];
        synth2.process(&mut buf_without, 48000);

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

        assert!(
            freqs[0] < 500.0,
            "Glide should start near source: {}",
            freqs[0]
        );
        let last = freqs[freqs.len() - 1];
        assert!(
            (last - 880.0).abs() < 10.0,
            "Glide should end near target: {last}"
        );

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
        assert!(
            max_freq > 880.0,
            "Overshoot should exceed target: {max_freq}"
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
        let mut synth = Ms20Voice::new();
        synth.note_on(880.0);
        let mut buf = [0.0f64; 480];
        synth.process(&mut buf, 48000);
        synth.reset();
        assert!(!synth.voices[0].gate);
    }

    #[test]
    fn hpf_affects_output() {
        let mut synth = Ms20Voice::new();
        synth.voices[0].hpf.cutoff = 5000.0;
        synth.voices[0].hpf.active = true;
        synth.note_on(200.0);

        let mut buf_hpf = [0.0f64; 4800];
        synth.process(&mut buf_hpf, 48000);
        let energy_hpf: f64 = buf_hpf.iter().map(|s| s * s).sum();

        let mut synth2 = Ms20Voice::new();
        synth2.voices[0].hpf.active = false;
        synth2.note_on(200.0);

        let mut buf_no_hpf = [0.0f64; 4800];
        synth2.process(&mut buf_no_hpf, 48000);
        let energy_no_hpf: f64 = buf_no_hpf.iter().map(|s| s * s).sum();

        assert!(
            energy_hpf < energy_no_hpf,
            "HPF should reduce energy of low-frequency note: {energy_hpf} vs {energy_no_hpf}"
        );
    }

    #[test]
    fn lpf_affects_output() {
        let mut synth = Ms20Voice::new();
        synth.voices[0].vco1.waveform = Waveform::Sawtooth;
        synth.voices[0].vco2.waveform = Waveform::Sawtooth;
        synth.voices[0].lpf.cutoff = 200.0;
        synth.voices[0].lpf.active = true;
        synth.note_on(440.0);

        let mut buf_lpf = [0.0f64; 4800];
        synth.process(&mut buf_lpf, 48000);
        let energy_lpf: f64 = buf_lpf.iter().map(|s| s * s).sum();

        let mut synth2 = Ms20Voice::new();
        synth2.voices[0].vco1.waveform = Waveform::Sawtooth;
        synth2.voices[0].vco2.waveform = Waveform::Sawtooth;
        synth2.voices[0].lpf.active = false;
        synth2.note_on(440.0);

        let mut buf_no_lpf = [0.0f64; 4800];
        synth2.process(&mut buf_no_lpf, 48000);
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
            let mut synth = Ms20Voice::new();
            synth.voices[0].vco1.waveform = *wf;
            synth.voices[0].vco2.waveform = *wf;
            synth.note_on(440.0);
            let mut buf = [0.0f64; 4800];
            synth.process(&mut buf, 48000);
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
        assert!(!a.voices[0].gate);
        assert!(!b.voices[0].gate);
        assert!((a.voices[0].note_freq - b.voices[0].note_freq).abs() < f64::EPSILON);
    }

    #[test]
    fn output_bounded() {
        let mut synth = Ms20Voice::new();
        synth.note_on(440.0);
        let mut buffer = [0.0f64; 48000];
        synth.process(&mut buffer, 48000);
        for (i, s) in buffer.iter().enumerate() {
            assert!(
                s.abs() < 10.0,
                "Output should be bounded at sample {i}: {s}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Voice mode tests
    // -----------------------------------------------------------------------

    #[test]
    fn voice_mode_high_1_selects_highest() {
        let mut synth = Ms20Voice::with_voice_mode(VoiceMode::High(1));
        synth.note_on_with_velocity(440.0, 0.5);
        synth.note_on_with_velocity(880.0, 0.8);
        // Highest note (880) should be active.
        assert!((synth.voices[0].note_freq - 880.0).abs() < 0.01);
        assert!((synth.voices[0].velocity - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn voice_mode_low_1_selects_lowest() {
        let mut synth = Ms20Voice::with_voice_mode(VoiceMode::Low(1));
        synth.note_on_with_velocity(440.0, 0.5);
        synth.note_on_with_velocity(880.0, 0.8);
        // Lowest note (440) should be active.
        assert!((synth.voices[0].note_freq - 440.0).abs() < 0.01);
        assert!((synth.voices[0].velocity - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn mono_note_off_freq_switches_to_remaining() {
        let mut synth = Ms20Voice::with_voice_mode(VoiceMode::High(1));
        synth.note_on(440.0);
        synth.note_on(880.0);
        // 880 is active (highest).
        assert!((synth.voices[0].note_freq - 880.0).abs() < 0.01);

        // Release 880, should switch back to 440.
        synth.note_off_freq(880.0);
        assert!(synth.voices[0].gate);
        assert!((synth.voices[0].note_freq - 440.0).abs() < 0.01);

        // Release 440, should gate off.
        synth.note_off_freq(440.0);
        assert!(!synth.voices[0].gate);
    }

    #[test]
    fn poly_high_10_allocates_voices() {
        let mut synth = Ms20Voice::with_voice_mode(VoiceMode::High(10));
        assert_eq!(synth.voices.len(), 10);

        synth.note_on(440.0);
        synth.note_on(550.0);
        synth.note_on(660.0);

        let active_count = synth.voices.iter().filter(|v| v.gate).count();
        assert_eq!(active_count, 3, "Three voices should be active");
    }

    #[test]
    fn poly_high_10_steals_lowest() {
        let mut synth = Ms20Voice::with_voice_mode(VoiceMode::High(3));
        synth.note_on(440.0); // voice 0
        synth.note_on(550.0); // voice 1
        synth.note_on(660.0); // voice 2 (pool full)

        // All 3 voices active.
        assert_eq!(synth.voices.iter().filter(|v| v.gate).count(), 3);

        // New note should steal the lowest (440).
        synth.note_on(770.0);
        let freqs: Vec<f64> = synth.voices.iter().map(|v| v.note_freq).collect();
        assert!(
            !freqs.iter().any(|f| (*f - 440.0).abs() < 0.01),
            "440 Hz should have been stolen: {freqs:?}"
        );
        assert!(
            freqs.iter().any(|f| (*f - 770.0).abs() < 0.01),
            "770 Hz should be present: {freqs:?}"
        );
    }

    #[test]
    fn poly_low_3_steals_highest() {
        let mut synth = Ms20Voice::with_voice_mode(VoiceMode::Low(3));
        synth.note_on(440.0);
        synth.note_on(550.0);
        synth.note_on(660.0);

        // New note should steal the highest (660).
        synth.note_on(330.0);
        let freqs: Vec<f64> = synth.voices.iter().map(|v| v.note_freq).collect();
        assert!(
            !freqs.iter().any(|f| (*f - 660.0).abs() < 0.01),
            "660 Hz should have been stolen: {freqs:?}"
        );
        assert!(
            freqs.iter().any(|f| (*f - 330.0).abs() < 0.01),
            "330 Hz should be present: {freqs:?}"
        );
    }

    #[test]
    fn poly_note_off_freq_releases_correct_voice() {
        let mut synth = Ms20Voice::with_voice_mode(VoiceMode::High(10));
        synth.note_on(440.0);
        synth.note_on(550.0);
        synth.note_on(660.0);

        synth.note_off_freq(550.0);

        // 440 and 660 should still be active.
        let active: Vec<f64> = synth
            .voices
            .iter()
            .filter(|v| v.gate)
            .map(|v| v.note_freq)
            .collect();
        assert_eq!(active.len(), 2);
        assert!(active.iter().any(|f| (*f - 440.0).abs() < 0.01));
        assert!(active.iter().any(|f| (*f - 660.0).abs() < 0.01));
    }

    #[test]
    fn set_voice_mode_resizes_pool() {
        let mut synth = Ms20Voice::new();
        assert_eq!(synth.voices.len(), 1);

        synth.set_voice_mode(VoiceMode::High(10));
        assert_eq!(synth.voices.len(), 10);
        assert_eq!(synth.voice_mode, VoiceMode::High(10));

        synth.set_voice_mode(VoiceMode::Low(1));
        assert_eq!(synth.voices.len(), 1);
        assert_eq!(synth.voice_mode, VoiceMode::Low(1));
    }

    #[test]
    fn poly_voices_produce_output() {
        let mut synth = Ms20Voice::with_voice_mode(VoiceMode::High(3));
        synth.note_on(440.0);
        synth.note_on(550.0);
        synth.note_on(660.0);

        let mut buffer = [0.0f64; 4800];
        synth.process(&mut buffer, 48000);

        let has_nonzero = buffer.iter().any(|s| s.abs() > 1e-6);
        assert!(has_nonzero, "Polyphonic output should be non-zero");
    }

    // -----------------------------------------------------------------------
    // Velocity depth tests
    // -----------------------------------------------------------------------

    #[test]
    fn velocity_vca_depth_scales_output() {
        // With velocity_vca_depth = 1.0, a soft note should be quieter.
        let mut loud = Ms20Voice::new();
        loud.velocity_vca_depth = 1.0;
        loud.note_on_with_velocity(440.0, 1.0);
        let mut loud_buf = [0.0f64; 4800];
        loud.process(&mut loud_buf, 48000);
        let loud_energy: f64 = loud_buf.iter().map(|s| s * s).sum();

        let mut soft = Ms20Voice::new();
        soft.velocity_vca_depth = 1.0;
        soft.note_on_with_velocity(440.0, 0.3);
        let mut soft_buf = [0.0f64; 4800];
        soft.process(&mut soft_buf, 48000);
        let soft_energy: f64 = soft_buf.iter().map(|s| s * s).sum();

        assert!(
            loud_energy > soft_energy,
            "Full velocity should be louder: {loud_energy} vs {soft_energy}"
        );
    }

    #[test]
    fn velocity_vca_depth_zero_no_effect() {
        // With velocity_vca_depth = 0.0, velocity should not change output.
        let mut v1 = Ms20Voice::new();
        v1.velocity_vca_depth = 0.0;
        v1.note_on_with_velocity(440.0, 1.0);
        let mut buf1 = [0.0f64; 4800];
        v1.process(&mut buf1, 48000);
        let e1: f64 = buf1.iter().map(|s| s * s).sum();

        let mut v2 = Ms20Voice::new();
        v2.velocity_vca_depth = 0.0;
        v2.note_on_with_velocity(440.0, 0.3);
        let mut buf2 = [0.0f64; 4800];
        v2.process(&mut buf2, 48000);
        let e2: f64 = buf2.iter().map(|s| s * s).sum();

        let ratio = if e1 > e2 { e1 / e2 } else { e2 / e1 };
        assert!(
            ratio < 1.1,
            "With zero VCA depth, velocity should not affect volume: ratio {ratio}"
        );
    }

    #[test]
    fn velocity_defaults_zero() {
        let synth = Ms20Voice::new();
        assert!(synth.velocity_vca_depth.abs() < f64::EPSILON);
        assert!(synth.velocity_filter_depth.abs() < f64::EPSILON);
        assert!(synth.velocity_envelope_depth.abs() < f64::EPSILON);
        assert!(synth.velocity_pitch_depth.abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Ring modulation depth tests
    // -----------------------------------------------------------------------

    #[test]
    fn ring_mod_depth_zero_matches_additive() {
        // At ring_mod_depth = 0.0, output should be pure additive mix.
        let mut synth = Ms20Voice::new();
        synth.ring_mod_depth = 0.0;
        synth.note_on(440.0);
        let mut buf = [0.0f64; 4800];
        synth.process(&mut buf, 48000);
        let energy: f64 = buf.iter().map(|s| s * s).sum();
        assert!(
            energy > 0.001,
            "Additive mix should produce output: {energy}"
        );
    }

    #[test]
    fn ring_mod_depth_one_produces_output() {
        // At ring_mod_depth = 1.0, output is VCO1 * VCO2.
        let mut synth = Ms20Voice::new();
        synth.ring_mod_depth = 1.0;
        synth.voices[0].vco1.waveform = Waveform::Sine;
        synth.voices[0].vco2.waveform = Waveform::Sine;
        synth.note_on(440.0);
        let mut buf = [0.0f64; 4800];
        synth.process(&mut buf, 48000);
        let energy: f64 = buf.iter().map(|s| s * s).sum();
        assert!(energy > 0.001, "Ring mod should produce output: {energy}");
    }

    #[test]
    fn ring_mod_depth_changes_timbre() {
        // Ring mod at 1.0 should produce different output than at 0.0.
        let mut dry = Ms20Voice::new();
        dry.ring_mod_depth = 0.0;
        dry.voices[0].vco1.waveform = Waveform::Sine;
        dry.voices[0].vco2.waveform = Waveform::Sawtooth;
        dry.note_on(440.0);
        let mut dry_buf = [0.0f64; 4800];
        dry.process(&mut dry_buf, 48000);

        let mut wet = Ms20Voice::new();
        wet.ring_mod_depth = 1.0;
        wet.voices[0].vco1.waveform = Waveform::Sine;
        wet.voices[0].vco2.waveform = Waveform::Sawtooth;
        wet.note_on(440.0);
        let mut wet_buf = [0.0f64; 4800];
        wet.process(&mut wet_buf, 48000);

        // Compute difference between the two signals.
        let diff: f64 = dry_buf
            .iter()
            .zip(wet_buf.iter())
            .map(|(d, w)| (d - w) * (d - w))
            .sum();
        assert!(
            diff > 0.01,
            "Ring mod should change timbre vs additive mix: diff={diff}"
        );
    }

    #[test]
    fn ring_mod_default_is_zero() {
        let synth = Ms20Voice::new();
        assert!(synth.ring_mod_depth.abs() < f64::EPSILON);
    }

    /// Regression test: simulates the DAW playback pattern with default patch
    /// parameters and mod matrix. Trigger a note, hold through sustain, release,
    /// trigger again. Verify output never becomes NaN/Inf and second note is
    /// audible.
    #[test]
    fn multi_note_no_audio_death() {
        let mut synth = Ms20Voice::new();

        // Configure like the default patch (matches PatchConfig defaults).
        for voice in &mut synth.voices {
            voice.lpf.cutoff = 8000.0;
            voice.lpf.resonance = 0.707;
            voice.eg1.attack = 0.01;
            voice.eg1.decay = 0.1;
            voice.eg1.sustain = 0.8;
            voice.eg1.release = 0.2;
            voice.eg2.attack = 0.01;
            voice.eg2.decay = 0.1;
            voice.eg2.sustain = 0.8;
            voice.eg2.release = 0.2;
        }

        let sr: u32 = 48000;

        // First note: hold for 500ms (24000 samples).
        synth.note_on_with_velocity(440.0, 0.8);
        let mut buf1 = [0.0f64; 24000];
        synth.process(&mut buf1, sr);
        let energy1: f64 = buf1.iter().map(|s| s * s).sum();
        assert!(
            buf1.iter().all(|s: &f64| s.is_finite()),
            "First note produced NaN/Inf"
        );
        assert!(energy1 > 0.1, "First note should produce audible output");

        // Release and let decay for 500ms.
        synth.note_off_freq(440.0);
        let mut silence = [0.0f64; 24000];
        synth.process(&mut silence, sr);
        assert!(
            silence.iter().all(|s: &f64| s.is_finite()),
            "Release phase produced NaN/Inf"
        );

        // Second note: should also produce output.
        synth.note_on_with_velocity(550.0, 0.8);
        let mut buf2 = [0.0f64; 24000];
        synth.process(&mut buf2, sr);
        let energy2: f64 = buf2.iter().map(|s| s * s).sum();
        assert!(
            buf2.iter().all(|s: &f64| s.is_finite()),
            "Second note produced NaN/Inf"
        );
        assert!(energy2 > 0.1, "Second note should produce audible output");
    }

    #[test]
    fn full_held_stack_forgets_its_oldest_note() {
        let mut held = HeldNotes::new();
        for i in 0..=MAX_HELD_NOTES {
            held.push(HeldNote {
                freq: i as f64,
                velocity: 1.0,
            });
        }
        assert_eq!(held.as_slice().len(), MAX_HELD_NOTES);
        assert_eq!(held.as_slice()[0].freq, 1.0);
        assert_eq!(
            held.as_slice()[MAX_HELD_NOTES - 1].freq,
            MAX_HELD_NOTES as f64
        );
    }

    #[test]
    fn held_stack_retain_keeps_order() {
        let mut held = HeldNotes::new();
        for freq in [100.0, 200.0, 100.0, 300.0] {
            held.push(HeldNote {
                freq,
                velocity: 1.0,
            });
        }
        held.retain(|n| n.freq != 100.0);
        let kept: alloc::vec::Vec<f64> = held.as_slice().iter().map(|n| n.freq).collect();
        assert_eq!(kept, [200.0, 300.0]);
        held.retain(|_| false);
        assert!(held.is_empty());
    }

    #[test]
    fn mono_priority_survives_more_notes_than_the_stack_holds() {
        // Every note is held and none released. After the stack overflows,
        // the highest note held still sounds in High mode.
        let mut synth = Ms20Voice::new();
        for i in 0..(MAX_HELD_NOTES * 2) {
            synth.note_on(100.0 + i as f64);
        }
        let top = 100.0 + (MAX_HELD_NOTES * 2 - 1) as f64;
        assert_eq!(synth.voices[0].note_freq, top);
        synth.note_off_freq(top);
        assert_eq!(synth.voices[0].note_freq, top - 1.0);
    }
}
