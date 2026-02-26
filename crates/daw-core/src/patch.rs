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
            mod_routings: Vec::new(),
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
}

/// A named parameter on a pedal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PedalParam {
    pub name: String,
    pub value: f64,
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
}
