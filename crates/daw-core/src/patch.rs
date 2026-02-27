extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Patch configuration for one MS-20 voice channel.
///
/// This maps directly to the audio engine's Ms20Voice parameters.
/// Serialized as TOML in the patches/ folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchConfig {
    pub name: String,

    // VCO1
    pub vco1_waveform: String,
    #[serde(default = "default_scale")]
    pub vco1_scale: String,
    #[serde(default)]
    pub vco1_pitch_fine: f64,
    #[serde(default = "default_level")]
    pub vco1_level: f64,
    #[serde(default = "default_pulse_width")]
    pub vco1_pulse_width: f64,
    #[serde(default)]
    pub vco1_quantization: Option<u32>,

    // VCO2
    #[serde(default = "default_triangle")]
    pub vco2_waveform: String,
    #[serde(default = "default_scale")]
    pub vco2_scale: String,
    #[serde(default)]
    pub vco2_pitch_fine: f64,
    #[serde(default)]
    pub vco2_level: f64,
    #[serde(default = "default_pulse_width")]
    pub vco2_pulse_width: f64,
    #[serde(default)]
    pub vco2_quantization: Option<u32>,

    // VCO mix
    #[serde(default)]
    pub vco_mix: f64,

    // EG1 (HPF envelope)
    #[serde(default = "default_attack")]
    pub eg1_attack: f64,
    #[serde(default = "default_decay")]
    pub eg1_decay: f64,
    #[serde(default = "default_sustain")]
    pub eg1_sustain: f64,
    #[serde(default = "default_release")]
    pub eg1_release: f64,

    // EG2 (LPF and VCA envelope)
    #[serde(default = "default_attack")]
    pub eg2_attack: f64,
    #[serde(default = "default_decay")]
    pub eg2_decay: f64,
    #[serde(default = "default_sustain")]
    pub eg2_sustain: f64,
    #[serde(default = "default_release")]
    pub eg2_release: f64,

    // Filters
    #[serde(default = "default_hpf_cutoff")]
    pub hpf_cutoff: f64,
    #[serde(default = "default_hpf_resonance")]
    pub hpf_resonance: f64,
    #[serde(default = "default_lpf_cutoff")]
    pub lpf_cutoff: f64,
    #[serde(default = "default_lpf_resonance")]
    pub lpf_resonance: f64,

    // LFO
    #[serde(default = "default_lfo_rate")]
    pub lfo_rate: f64,

    // Pitch glide
    #[serde(default)]
    pub glide_enabled: bool,
    #[serde(default = "default_glide_time")]
    pub glide_time: f64,

    // Voice mode
    /// Voice mode string: "high_1", "low_1", "high_10", "low_10".
    #[serde(default = "default_voice_mode")]
    pub voice_mode: String,

    // Velocity depth controls
    /// Velocity depth for VCA level (0.0 = no effect, 1.0 = full velocity control).
    #[serde(default)]
    pub velocity_vca_depth: f64,
    /// Velocity depth for LPF cutoff in octaves (0.0 = no effect).
    #[serde(default)]
    pub velocity_filter_depth: f64,
    /// Velocity depth for envelope scaling (0.0 = no effect, 1.0 = full velocity control).
    #[serde(default)]
    pub velocity_envelope_depth: f64,
    /// Velocity depth for VCO pitch in semitones (0.0 = no effect).
    #[serde(default)]
    pub velocity_pitch_depth: f64,

    // Modulation routings (optional overrides)
    #[serde(default)]
    pub mod_routings: Vec<ModRoutingConfig>,
}

/// A single modulation routing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModRoutingConfig {
    pub source: String,
    pub dest: String,
    pub depth: f64,
}

impl Default for PatchConfig {
    fn default() -> Self {
        Self {
            name: String::from("Default"),
            vco1_waveform: String::from("sawtooth"),
            vco1_scale: default_scale(),
            vco1_pitch_fine: 0.0,
            vco1_level: 1.0,
            vco1_pulse_width: 0.5,
            vco1_quantization: None,
            vco2_waveform: default_triangle(),
            vco2_scale: default_scale(),
            vco2_pitch_fine: 0.0,
            vco2_level: 0.0,
            vco2_pulse_width: 0.5,
            vco2_quantization: None,
            vco_mix: 0.0,
            eg1_attack: default_attack(),
            eg1_decay: default_decay(),
            eg1_sustain: default_sustain(),
            eg1_release: default_release(),
            eg2_attack: default_attack(),
            eg2_decay: default_decay(),
            eg2_sustain: default_sustain(),
            eg2_release: default_release(),
            hpf_cutoff: default_hpf_cutoff(),
            hpf_resonance: default_hpf_resonance(),
            lpf_cutoff: default_lpf_cutoff(),
            lpf_resonance: default_lpf_resonance(),
            lfo_rate: default_lfo_rate(),
            glide_enabled: false,
            glide_time: default_glide_time(),
            voice_mode: default_voice_mode(),
            velocity_vca_depth: 0.0,
            velocity_filter_depth: 0.0,
            velocity_envelope_depth: 0.0,
            velocity_pitch_depth: 0.0,
            mod_routings: default_mod_routings(),
        }
    }
}

/// Effect chain configuration for one channel.
///
/// Serialized as TOML in the effects/ folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectChainConfig {
    pub name: String,
    #[serde(default)]
    pub pre_pedals: Vec<PedalConfig>,
    #[serde(default)]
    pub post_pedals_left: Vec<PedalConfig>,
    #[serde(default)]
    pub post_pedals_right: Vec<PedalConfig>,
}

impl EffectChainConfig {
    /// Get a pedal parameter value by chain ID, pedal index, and parameter index.
    ///
    /// chain_id: 0 = pre, 1 = post-left, 2 = post-right.
    pub fn get_pedal_param(&self, chain_id: u8, pedal_idx: usize, param_idx: usize) -> Option<f64> {
        let pedals = match chain_id {
            0 => &self.pre_pedals,
            1 => &self.post_pedals_left,
            2 => &self.post_pedals_right,
            _ => return None,
        };
        pedals.get(pedal_idx)?.params.get(param_idx).map(|p| p.value)
    }

    /// Get the pedal list for a given chain ID.
    ///
    /// chain_id: 0 = pre, 1 = post-left, 2 = post-right.
    pub fn get_chain_pedals(&self, chain_id: u8) -> Option<&[PedalConfig]> {
        match chain_id {
            0 => Some(&self.pre_pedals),
            1 => Some(&self.post_pedals_left),
            2 => Some(&self.post_pedals_right),
            _ => None,
        }
    }
}

impl Default for EffectChainConfig {
    fn default() -> Self {
        Self {
            name: String::from("None"),
            pre_pedals: Vec::new(),
            post_pedals_left: Vec::new(),
            post_pedals_right: Vec::new(),
        }
    }
}

/// Configuration for a single pedal effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PedalConfig {
    pub pedal_type: String,
    #[serde(default)]
    pub params: Vec<PedalParam>,
    /// Whether this pedal is bypassed (signal passes through unprocessed).
    #[serde(default)]
    pub bypassed: bool,
}

/// A named parameter on a pedal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PedalParam {
    pub name: String,
    pub value: f64,
}

/// All known pedal type names, organized by category.
pub const PEDAL_TYPES: &[(&str, &[&str])] = &[
    ("Distortion", &["Overdrive", "Distortion", "Fuzz", "TapeSaturation"]),
    ("Wah", &["AutoWah", "WahManual"]),
    ("Modulation", &["Chorus", "Flanger", "Phaser", "Tremolo"]),
    ("Delay", &["DigitalDelay", "AnalogDelay", "PingPongDelay"]),
    ("Reverb", &["PlateReverb", "HallReverb", "RoomReverb"]),
    ("Special", &["Bitcrusher", "RingModulator", "PitchShifter", "AutoPan"]),
    ("Dynamics", &["NoiseGate", "Compressor"]),
    ("Utility", &["Eq", "Volume", "Buffer"]),
];

/// Create a `PedalConfig` with default parameter values for the given pedal type.
///
/// Returns `None` if the type name is not recognized.
pub fn default_pedal_config(pedal_type: &str) -> Option<PedalConfig> {
    let params = match pedal_type {
        "Overdrive" => alloc::vec![
            PedalParam { name: String::from("gain"), value: 2.0 },
            PedalParam { name: String::from("tone"), value: 0.5 },
            PedalParam { name: String::from("mix"), value: 1.0 },
        ],
        "Distortion" => alloc::vec![
            PedalParam { name: String::from("gain"), value: 5.0 },
            PedalParam { name: String::from("threshold"), value: 0.5 },
            PedalParam { name: String::from("tone"), value: 0.5 },
            PedalParam { name: String::from("mix"), value: 1.0 },
        ],
        "Fuzz" => alloc::vec![
            PedalParam { name: String::from("gain"), value: 8.0 },
            PedalParam { name: String::from("tone"), value: 0.5 },
            PedalParam { name: String::from("mix"), value: 1.0 },
        ],
        "TapeSaturation" => alloc::vec![
            PedalParam { name: String::from("drive"), value: 1.5 },
            PedalParam { name: String::from("warmth"), value: 0.3 },
            PedalParam { name: String::from("mix"), value: 1.0 },
        ],
        "AutoWah" => alloc::vec![
            PedalParam { name: String::from("sensitivity"), value: 0.5 },
            PedalParam { name: String::from("min_freq"), value: 200.0 },
            PedalParam { name: String::from("max_freq"), value: 2000.0 },
            PedalParam { name: String::from("q"), value: 2.0 },
            PedalParam { name: String::from("mix"), value: 1.0 },
        ],
        "WahManual" => alloc::vec![
            PedalParam { name: String::from("position"), value: 0.5 },
            PedalParam { name: String::from("min_freq"), value: 200.0 },
            PedalParam { name: String::from("max_freq"), value: 2000.0 },
            PedalParam { name: String::from("q"), value: 2.0 },
            PedalParam { name: String::from("mix"), value: 1.0 },
        ],
        "Chorus" => alloc::vec![
            PedalParam { name: String::from("rate_hz"), value: 1.0 },
            PedalParam { name: String::from("depth_ms"), value: 3.0 },
            PedalParam { name: String::from("mix"), value: 0.5 },
        ],
        "Flanger" => alloc::vec![
            PedalParam { name: String::from("rate_hz"), value: 0.5 },
            PedalParam { name: String::from("depth_ms"), value: 2.0 },
            PedalParam { name: String::from("feedback"), value: 0.3 },
            PedalParam { name: String::from("mix"), value: 0.5 },
        ],
        "Phaser" => alloc::vec![
            PedalParam { name: String::from("rate_hz"), value: 0.5 },
            PedalParam { name: String::from("depth"), value: 0.5 },
            PedalParam { name: String::from("feedback"), value: 0.3 },
            PedalParam { name: String::from("mix"), value: 0.5 },
            PedalParam { name: String::from("stages"), value: 4.0 },
        ],
        "Tremolo" => alloc::vec![
            PedalParam { name: String::from("rate_hz"), value: 5.0 },
            PedalParam { name: String::from("depth"), value: 0.5 },
        ],
        "DigitalDelay" => alloc::vec![
            PedalParam { name: String::from("time_ms"), value: 250.0 },
            PedalParam { name: String::from("feedback"), value: 0.3 },
            PedalParam { name: String::from("mix"), value: 0.4 },
        ],
        "AnalogDelay" => alloc::vec![
            PedalParam { name: String::from("time_ms"), value: 300.0 },
            PedalParam { name: String::from("feedback"), value: 0.3 },
            PedalParam { name: String::from("tone"), value: 0.5 },
            PedalParam { name: String::from("mix"), value: 0.4 },
        ],
        "PingPongDelay" => alloc::vec![
            PedalParam { name: String::from("time_ms"), value: 250.0 },
            PedalParam { name: String::from("feedback"), value: 0.3 },
            PedalParam { name: String::from("mix"), value: 0.4 },
        ],
        "PlateReverb" | "HallReverb" | "RoomReverb" => alloc::vec![
            PedalParam { name: String::from("decay"), value: 0.5 },
            PedalParam { name: String::from("damping"), value: 0.5 },
            PedalParam { name: String::from("mix"), value: 0.3 },
        ],
        "Bitcrusher" => alloc::vec![
            PedalParam { name: String::from("bit_depth"), value: 8.0 },
            PedalParam { name: String::from("rate_factor"), value: 0.5 },
            PedalParam { name: String::from("mix"), value: 1.0 },
        ],
        "RingModulator" => alloc::vec![
            PedalParam { name: String::from("frequency_hz"), value: 440.0 },
            PedalParam { name: String::from("mix"), value: 0.5 },
        ],
        "PitchShifter" => alloc::vec![
            PedalParam { name: String::from("semitones"), value: 0.0 },
            PedalParam { name: String::from("mix"), value: 1.0 },
        ],
        "AutoPan" => alloc::vec![
            PedalParam { name: String::from("rate_hz"), value: 1.0 },
            PedalParam { name: String::from("depth"), value: 0.5 },
        ],
        "NoiseGate" => alloc::vec![
            PedalParam { name: String::from("threshold"), value: 0.01 },
            PedalParam { name: String::from("attack_ms"), value: 1.0 },
            PedalParam { name: String::from("release_ms"), value: 50.0 },
        ],
        "Compressor" => alloc::vec![
            PedalParam { name: String::from("threshold"), value: 0.5 },
            PedalParam { name: String::from("ratio"), value: 4.0 },
            PedalParam { name: String::from("attack_ms"), value: 10.0 },
            PedalParam { name: String::from("release_ms"), value: 100.0 },
            PedalParam { name: String::from("makeup_gain"), value: 1.0 },
        ],
        "Eq" => alloc::vec![
            PedalParam { name: String::from("low_gain_db"), value: 0.0 },
            PedalParam { name: String::from("mid_gain_db"), value: 0.0 },
            PedalParam { name: String::from("high_gain_db"), value: 0.0 },
            PedalParam { name: String::from("low_freq"), value: 200.0 },
            PedalParam { name: String::from("mid_freq"), value: 1000.0 },
            PedalParam { name: String::from("high_freq"), value: 4000.0 },
        ],
        "Volume" => alloc::vec![
            PedalParam { name: String::from("gain"), value: 1.0 },
        ],
        "Buffer" => alloc::vec![
            PedalParam { name: String::from("gain"), value: 1.0 },
        ],
        _ => return None,
    };
    Some(PedalConfig {
        pedal_type: String::from(pedal_type),
        params,
        bypassed: false,
    })
}

// Serde default functions.
fn default_scale() -> String {
    String::from("8'")
}
fn default_triangle() -> String {
    String::from("triangle")
}
fn default_level() -> f64 {
    1.0
}
fn default_pulse_width() -> f64 {
    0.5
}
fn default_attack() -> f64 {
    0.01
}
fn default_decay() -> f64 {
    0.1
}
fn default_sustain() -> f64 {
    0.8
}
fn default_release() -> f64 {
    0.2
}
fn default_hpf_cutoff() -> f64 {
    20.0
}
fn default_hpf_resonance() -> f64 {
    0.707
}
fn default_lpf_cutoff() -> f64 {
    8000.0
}
fn default_lpf_resonance() -> f64 {
    0.707
}
fn default_lfo_rate() -> f64 {
    5.0
}
fn default_glide_time() -> f64 {
    0.1
}
fn default_voice_mode() -> String {
    String::from("high_1")
}

/// Return the three standard MS-20 modulation routings.
///
/// These match `ModMatrix::ms20_default()` in the audio crate:
/// - EG1 -> HPF cutoff (depth 2.0 octaves)
/// - EG2 -> LPF cutoff (depth 4.0 octaves)
/// - EG2 -> VCA level (depth 1.0, full envelope control)
///
/// The EG2 -> VCA routing is critical for producing audible output.
/// Without it, the VCA multiplier is zero and the channel is silent.
fn default_mod_routings() -> Vec<ModRoutingConfig> {
    alloc::vec![
        ModRoutingConfig {
            source: String::from("eg1"),
            dest: String::from("hpf_cutoff"),
            depth: 2.0,
        },
        ModRoutingConfig {
            source: String::from("eg2"),
            dest: String::from("lpf_cutoff"),
            depth: 4.0,
        },
        ModRoutingConfig {
            source: String::from("eg2"),
            dest: String::from("vca_level"),
            depth: 1.0,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_patch_has_sawtooth() {
        let patch = PatchConfig::default();
        assert_eq!(patch.vco1_waveform, "sawtooth");
        assert_eq!(patch.name, "Default");
    }

    #[test]
    fn default_effect_chain_empty() {
        let chain = EffectChainConfig::default();
        assert!(chain.pre_pedals.is_empty());
        assert!(chain.post_pedals_left.is_empty());
        assert!(chain.post_pedals_right.is_empty());
    }

    #[test]
    fn default_pedal_config_all_types() {
        for (_category, types) in PEDAL_TYPES {
            for pedal_type in *types {
                let config = default_pedal_config(pedal_type);
                assert!(config.is_some(), "default_pedal_config should recognize {pedal_type}");
                let config = config.unwrap();
                assert_eq!(config.pedal_type, *pedal_type);
                assert!(!config.params.is_empty(), "{pedal_type} should have params");
            }
        }
    }

    #[test]
    fn default_pedal_config_unknown() {
        assert!(default_pedal_config("Unknown").is_none());
    }

    #[test]
    fn pedal_types_covers_25() {
        let count: usize = PEDAL_TYPES.iter().map(|(_, types)| types.len()).sum();
        assert_eq!(count, 25);
    }
}
