//! Modulation matrix for the MS-20 synthesizer.
//!
//! Simulates the MS-20's patch bay with predefined modulation sources and
//! destinations. Each routing connects a source signal to a destination
//! parameter with a configurable depth.
//!
//! ## Predefined Sources
//!
//! | Source | Description |
//! |--------|-------------|
//! | EG1 | Envelope generator 1 output (0.0 to 1.0) |
//! | EG2 | Envelope generator 2 output (0.0 to 1.0) |
//! | LFO | Low-frequency oscillator (bipolar, -1.0 to 1.0) |
//! | Velocity | Note velocity (0.0 to 1.0) |
//! | KeyTrack | Keyboard tracking (normalized frequency) |
//!
//! ## Predefined Destinations
//!
//! | Destination | Description |
//! |-------------|-------------|
//! | HpfCutoff | Sallen-Key HPF cutoff frequency |
//! | LpfCutoff | Korg-35 LPF cutoff frequency |
//! | LpfResonance | Korg-35 LPF resonance |
//! | Vco1Pitch | VCO1 pitch (in semitones) |
//! | Vco2Pitch | VCO2 pitch (in semitones) |
//! | VcaLevel | VCA output level |
//! | PwmWidth | Pulse width for PW-Square VCOs |

use alloc::vec::Vec;

/// Modulation source identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSource {
    /// Envelope generator 1 (unipolar: 0.0 to 1.0).
    Eg1,
    /// Envelope generator 2 (unipolar: 0.0 to 1.0).
    Eg2,
    /// Low-frequency oscillator (bipolar: -1.0 to 1.0).
    Lfo,
    /// Note velocity (unipolar: 0.0 to 1.0).
    Velocity,
    /// Keyboard tracking (normalized note frequency, unipolar).
    KeyTrack,
}

/// Modulation destination identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModDest {
    /// Sallen-Key HPF cutoff frequency modulation.
    HpfCutoff,
    /// Korg-35 LPF cutoff frequency modulation.
    LpfCutoff,
    /// Korg-35 LPF resonance modulation.
    LpfResonance,
    /// VCO1 pitch modulation (in semitones).
    Vco1Pitch,
    /// VCO2 pitch modulation (in semitones).
    Vco2Pitch,
    /// VCA output level modulation.
    VcaLevel,
    /// Pulse width modulation for both VCOs.
    PwmWidth,
}

/// A single modulation routing from source to destination.
#[derive(Debug, Clone, Copy)]
pub struct ModRouting {
    /// The modulation source signal.
    pub source: ModSource,
    /// The modulation destination parameter.
    pub dest: ModDest,
    /// Modulation depth. Positive values add, negative values subtract.
    /// The range depends on the destination:
    ///   - Cutoff frequencies: depth in octaves (e.g., 2.0 = sweep 2 octaves)
    ///   - Pitch: depth in semitones
    ///   - Level/width: depth as a multiplier (0.0 to 1.0 typical)
    pub depth: f64,
}

/// Snapshot of all modulation source values for a single sample tick.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModSources {
    /// EG1 output (0.0 to 1.0).
    pub eg1: f64,
    /// EG2 output (0.0 to 1.0).
    pub eg2: f64,
    /// LFO output (-1.0 to 1.0).
    pub lfo: f64,
    /// Note velocity (0.0 to 1.0).
    pub velocity: f64,
    /// Keyboard tracking (normalized frequency, typically 0.0 to 1.0).
    pub key_track: f64,
}

impl ModSources {
    /// Read the value for a given modulation source.
    pub fn get(&self, source: ModSource) -> f64 {
        match source {
            ModSource::Eg1 => self.eg1,
            ModSource::Eg2 => self.eg2,
            ModSource::Lfo => self.lfo,
            ModSource::Velocity => self.velocity,
            ModSource::KeyTrack => self.key_track,
        }
    }
}

/// Computed modulation amounts for all destinations.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModOutputs {
    /// HPF cutoff modulation (in octaves, bipolar).
    pub hpf_cutoff: f64,
    /// LPF cutoff modulation (in octaves, bipolar).
    pub lpf_cutoff: f64,
    /// LPF resonance modulation (additive).
    pub lpf_resonance: f64,
    /// VCO1 pitch modulation (in semitones, bipolar).
    pub vco1_pitch: f64,
    /// VCO2 pitch modulation (in semitones, bipolar).
    pub vco2_pitch: f64,
    /// VCA level modulation (additive to base level, clamped to 0-1).
    pub vca_level: f64,
    /// Pulse width modulation (additive to base width).
    pub pwm_width: f64,
}

/// Modulation matrix with configurable routings.
///
/// Holds a list of source-to-destination routings. Each tick, the matrix
/// reads all source values and computes the summed modulation amount for
/// each destination.
#[derive(Debug, Clone)]
pub struct ModMatrix {
    /// Active modulation routings.
    pub routings: Vec<ModRouting>,
}

impl ModMatrix {
    /// Create a new empty modulation matrix.
    pub fn new() -> Self {
        Self {
            routings: Vec::new(),
        }
    }

    /// Create a modulation matrix with the default MS-20 routings.
    ///
    /// Default routings:
    /// - EG1 -> HPF cutoff (depth: 2 octaves)
    /// - EG2 -> LPF cutoff (depth: 4 octaves)
    /// - EG2 -> VCA level (depth: 1.0, full envelope control)
    pub fn ms20_default() -> Self {
        Self {
            routings: alloc::vec![
                ModRouting {
                    source: ModSource::Eg1,
                    dest: ModDest::HpfCutoff,
                    depth: 2.0,
                },
                ModRouting {
                    source: ModSource::Eg2,
                    dest: ModDest::LpfCutoff,
                    depth: 4.0,
                },
                ModRouting {
                    source: ModSource::Eg2,
                    dest: ModDest::VcaLevel,
                    depth: 1.0,
                },
            ],
        }
    }

    /// Add a modulation routing.
    pub fn add_routing(&mut self, routing: ModRouting) {
        self.routings.push(routing);
    }

    /// Remove all routings.
    pub fn clear(&mut self) {
        self.routings.clear();
    }

    /// Compute all modulation outputs from the current source values.
    ///
    /// For each destination, sums the contributions from all routings
    /// targeting that destination: `contribution = source_value * depth`.
    pub fn compute(&self, sources: &ModSources) -> ModOutputs {
        let mut out = ModOutputs::default();

        for routing in &self.routings {
            let value = sources.get(routing.source) * routing.depth;
            match routing.dest {
                ModDest::HpfCutoff => out.hpf_cutoff += value,
                ModDest::LpfCutoff => out.lpf_cutoff += value,
                ModDest::LpfResonance => out.lpf_resonance += value,
                ModDest::Vco1Pitch => out.vco1_pitch += value,
                ModDest::Vco2Pitch => out.vco2_pitch += value,
                ModDest::VcaLevel => out.vca_level += value,
                ModDest::PwmWidth => out.pwm_width += value,
            }
        }

        out
    }
}

impl Default for ModMatrix {
    fn default() -> Self {
        Self::ms20_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_matrix_zero_outputs() {
        let matrix = ModMatrix::new();
        let sources = ModSources::default();
        let outputs = matrix.compute(&sources);
        assert!(outputs.hpf_cutoff.abs() < f64::EPSILON);
        assert!(outputs.lpf_cutoff.abs() < f64::EPSILON);
        assert!(outputs.vca_level.abs() < f64::EPSILON);
    }

    #[test]
    fn default_matrix_eg2_vca() {
        let matrix = ModMatrix::ms20_default();
        let sources = ModSources {
            eg2: 0.8,
            ..Default::default()
        };
        let outputs = matrix.compute(&sources);
        // EG2 -> VCA at depth 1.0: 0.8 * 1.0 = 0.8
        assert!(
            (outputs.vca_level - 0.8).abs() < 1e-10,
            "VCA: {}",
            outputs.vca_level
        );
        // EG2 -> LPF cutoff at depth 4.0: 0.8 * 4.0 = 3.2 octaves
        assert!(
            (outputs.lpf_cutoff - 3.2).abs() < 1e-10,
            "LPF: {}",
            outputs.lpf_cutoff
        );
    }

    #[test]
    fn default_matrix_eg1_hpf() {
        let matrix = ModMatrix::ms20_default();
        let sources = ModSources {
            eg1: 0.5,
            ..Default::default()
        };
        let outputs = matrix.compute(&sources);
        // EG1 -> HPF cutoff at depth 2.0: 0.5 * 2.0 = 1.0 octave
        assert!(
            (outputs.hpf_cutoff - 1.0).abs() < 1e-10,
            "HPF: {}",
            outputs.hpf_cutoff
        );
    }

    #[test]
    fn multiple_routings_sum() {
        let mut matrix = ModMatrix::new();
        matrix.add_routing(ModRouting {
            source: ModSource::Eg1,
            dest: ModDest::LpfCutoff,
            depth: 2.0,
        });
        matrix.add_routing(ModRouting {
            source: ModSource::Lfo,
            dest: ModDest::LpfCutoff,
            depth: 1.0,
        });

        let sources = ModSources {
            eg1: 0.5,
            lfo: 0.3,
            ..Default::default()
        };
        let outputs = matrix.compute(&sources);
        // 0.5 * 2.0 + 0.3 * 1.0 = 1.3
        assert!(
            (outputs.lpf_cutoff - 1.3).abs() < 1e-10,
            "Sum: {}",
            outputs.lpf_cutoff
        );
    }

    #[test]
    fn negative_depth_subtracts() {
        let mut matrix = ModMatrix::new();
        matrix.add_routing(ModRouting {
            source: ModSource::Eg2,
            dest: ModDest::VcaLevel,
            depth: -0.5,
        });

        let sources = ModSources {
            eg2: 1.0,
            ..Default::default()
        };
        let outputs = matrix.compute(&sources);
        assert!(
            (outputs.vca_level - (-0.5)).abs() < 1e-10,
            "Negative: {}",
            outputs.vca_level
        );
    }

    #[test]
    fn lfo_bipolar_source() {
        let mut matrix = ModMatrix::new();
        matrix.add_routing(ModRouting {
            source: ModSource::Lfo,
            dest: ModDest::Vco1Pitch,
            depth: 2.0,
        });

        // LFO at -1.0 (bottom of sweep).
        let sources = ModSources {
            lfo: -1.0,
            ..Default::default()
        };
        let outputs = matrix.compute(&sources);
        assert!(
            (outputs.vco1_pitch - (-2.0)).abs() < 1e-10,
            "LFO negative: {}",
            outputs.vco1_pitch
        );
    }

    #[test]
    fn clear_removes_routings() {
        let mut matrix = ModMatrix::ms20_default();
        assert!(!matrix.routings.is_empty());
        matrix.clear();
        assert!(matrix.routings.is_empty());

        let sources = ModSources {
            eg2: 1.0,
            ..Default::default()
        };
        let outputs = matrix.compute(&sources);
        assert!(outputs.vca_level.abs() < f64::EPSILON);
    }

    #[test]
    fn velocity_source() {
        let mut matrix = ModMatrix::new();
        matrix.add_routing(ModRouting {
            source: ModSource::Velocity,
            dest: ModDest::VcaLevel,
            depth: 0.5,
        });

        let sources = ModSources {
            velocity: 0.8,
            ..Default::default()
        };
        let outputs = matrix.compute(&sources);
        assert!(
            (outputs.vca_level - 0.4).abs() < 1e-10,
            "Velocity: {}",
            outputs.vca_level
        );
    }

    #[test]
    fn key_track_source() {
        let mut matrix = ModMatrix::new();
        matrix.add_routing(ModRouting {
            source: ModSource::KeyTrack,
            dest: ModDest::LpfCutoff,
            depth: 1.0,
        });

        let sources = ModSources {
            key_track: 0.6,
            ..Default::default()
        };
        let outputs = matrix.compute(&sources);
        assert!(
            (outputs.lpf_cutoff - 0.6).abs() < 1e-10,
            "KeyTrack: {}",
            outputs.lpf_cutoff
        );
    }

    #[test]
    fn pwm_destination() {
        let mut matrix = ModMatrix::new();
        matrix.add_routing(ModRouting {
            source: ModSource::Lfo,
            dest: ModDest::PwmWidth,
            depth: 0.3,
        });

        let sources = ModSources {
            lfo: 0.5,
            ..Default::default()
        };
        let outputs = matrix.compute(&sources);
        assert!(
            (outputs.pwm_width - 0.15).abs() < 1e-10,
            "PWM: {}",
            outputs.pwm_width
        );
    }
}
