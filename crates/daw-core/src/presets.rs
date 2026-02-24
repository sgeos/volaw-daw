extern crate alloc;
use alloc::string::String;
use alloc::vec;

use crate::patch::{ModRoutingConfig, PatchConfig};

/// Create a named preset patch configuration.
///
/// Each preset approximates a real instrument using the MS-20 synthesizer
/// architecture (dual VCO, dual EG, HPF, LPF, modulation matrix).
pub fn preset(name: &str) -> Option<PatchConfig> {
    match name {
        "bass" => Some(bass()),
        "lead" => Some(lead()),
        "pad" => Some(pad()),
        "strings" => Some(strings()),
        "organ" => Some(organ()),
        "brass" => Some(brass()),
        "pluck" => Some(pluck()),
        "bell" => Some(bell()),
        "flute" => Some(flute()),
        "piano" => Some(piano()),
        "kick" => Some(kick()),
        "snare" => Some(snare()),
        "hihat" => Some(hihat()),
        "synth_lead" => Some(synth_lead()),
        "synth_bass" => Some(synth_bass()),
        "choir" => Some(choir()),
        _ => None,
    }
}

/// List all available preset names.
pub fn preset_names() -> &'static [&'static str] {
    &[
        "bass",
        "bell",
        "brass",
        "choir",
        "flute",
        "hihat",
        "kick",
        "lead",
        "organ",
        "pad",
        "piano",
        "pluck",
        "snare",
        "strings",
        "synth_bass",
        "synth_lead",
    ]
}

/// Fingered bass: sawtooth low, short decay, moderate LPF.
fn bass() -> PatchConfig {
    PatchConfig {
        name: String::from("Bass"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("16'"),
        vco1_level: 1.0,
        vco2_waveform: String::from("square"),
        vco2_scale: String::from("16'"),
        vco2_level: 0.3,
        vco2_pitch_fine: -0.05,
        vco_mix: 0.7,
        eg2_attack: 0.005,
        eg2_decay: 0.3,
        eg2_sustain: 0.4,
        eg2_release: 0.15,
        lpf_cutoff: 1200.0,
        lpf_resonance: 1.2,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("eg2"),
            dest: String::from("lpf_cutoff"),
            depth: 0.5,
        }],
        ..PatchConfig::default()
    }
}

/// Expressive lead: sawtooth with vibrato.
fn lead() -> PatchConfig {
    PatchConfig {
        name: String::from("Lead"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("8'"),
        vco1_level: 1.0,
        vco2_waveform: String::from("sawtooth"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.5,
        vco2_pitch_fine: 0.08,
        vco_mix: 0.5,
        eg2_attack: 0.02,
        eg2_decay: 0.2,
        eg2_sustain: 0.7,
        eg2_release: 0.3,
        lpf_cutoff: 3000.0,
        lpf_resonance: 1.0,
        lfo_rate: 5.5,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("lfo"),
            dest: String::from("vco1_pitch"),
            depth: 0.02,
        }],
        ..PatchConfig::default()
    }
}

/// Warm pad: detuned sawtooths with slow attack.
fn pad() -> PatchConfig {
    PatchConfig {
        name: String::from("Pad"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.7,
        vco2_waveform: String::from("sawtooth"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.7,
        vco2_pitch_fine: 0.1,
        vco_mix: 0.5,
        eg2_attack: 0.8,
        eg2_decay: 0.5,
        eg2_sustain: 0.8,
        eg2_release: 1.5,
        lpf_cutoff: 2000.0,
        lpf_resonance: 0.5,
        ..PatchConfig::default()
    }
}

/// Strings: slow attack, sustained, filtered.
fn strings() -> PatchConfig {
    PatchConfig {
        name: String::from("Strings"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.6,
        vco2_waveform: String::from("sawtooth"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.6,
        vco2_pitch_fine: 0.06,
        vco_mix: 0.5,
        eg2_attack: 0.6,
        eg2_decay: 0.3,
        eg2_sustain: 0.9,
        eg2_release: 0.8,
        lpf_cutoff: 4000.0,
        lpf_resonance: 0.3,
        lfo_rate: 4.0,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("lfo"),
            dest: String::from("vco1_pitch"),
            depth: 0.01,
        }],
        ..PatchConfig::default()
    }
}

/// Hammond-style organ: square wave, no attack/release.
fn organ() -> PatchConfig {
    PatchConfig {
        name: String::from("Organ"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.7,
        vco1_pulse_width: 0.5,
        vco2_waveform: String::from("square"),
        vco2_scale: String::from("4'"),
        vco2_level: 0.4,
        vco2_pulse_width: 0.5,
        vco_mix: 0.5,
        eg2_attack: 0.002,
        eg2_decay: 0.01,
        eg2_sustain: 1.0,
        eg2_release: 0.05,
        lpf_cutoff: 5000.0,
        lpf_resonance: 0.3,
        ..PatchConfig::default()
    }
}

/// Brass: sawtooth with medium attack and filter sweep.
fn brass() -> PatchConfig {
    PatchConfig {
        name: String::from("Brass"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("8'"),
        vco1_level: 1.0,
        vco2_waveform: String::from("sawtooth"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.4,
        vco2_pitch_fine: 0.03,
        vco_mix: 0.6,
        eg2_attack: 0.08,
        eg2_decay: 0.15,
        eg2_sustain: 0.8,
        eg2_release: 0.2,
        lpf_cutoff: 1500.0,
        lpf_resonance: 0.8,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("eg2"),
            dest: String::from("lpf_cutoff"),
            depth: 0.7,
        }],
        ..PatchConfig::default()
    }
}

/// Pluck: short envelope, resonant filter.
fn pluck() -> PatchConfig {
    PatchConfig {
        name: String::from("Pluck"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("8'"),
        vco1_level: 1.0,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.4,
        eg2_sustain: 0.0,
        eg2_release: 0.3,
        lpf_cutoff: 5000.0,
        lpf_resonance: 1.5,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("eg2"),
            dest: String::from("lpf_cutoff"),
            depth: 0.8,
        }],
        ..PatchConfig::default()
    }
}

/// Bell: FM-like with ring modulator, quick decay.
fn bell() -> PatchConfig {
    PatchConfig {
        name: String::from("Bell"),
        vco1_waveform: String::from("sine"),
        vco1_scale: String::from("4'"),
        vco1_level: 0.8,
        vco2_waveform: String::from("ring"),
        vco2_scale: String::from("2'"),
        vco2_level: 0.5,
        vco_mix: 0.4,
        eg2_attack: 0.001,
        eg2_decay: 1.5,
        eg2_sustain: 0.0,
        eg2_release: 1.0,
        lpf_cutoff: 6000.0,
        lpf_resonance: 0.3,
        ..PatchConfig::default()
    }
}

/// Flute: sine wave with breathy noise.
fn flute() -> PatchConfig {
    PatchConfig {
        name: String::from("Flute"),
        vco1_waveform: String::from("sine"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.9,
        vco2_waveform: String::from("noise"),
        vco2_level: 0.05,
        vco_mix: 0.9,
        eg2_attack: 0.1,
        eg2_decay: 0.1,
        eg2_sustain: 0.85,
        eg2_release: 0.2,
        lpf_cutoff: 3000.0,
        lpf_resonance: 0.2,
        lfo_rate: 5.0,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("lfo"),
            dest: String::from("vco1_pitch"),
            depth: 0.015,
        }],
        ..PatchConfig::default()
    }
}

/// Electric piano: quick attack, medium decay, mild detuning.
fn piano() -> PatchConfig {
    PatchConfig {
        name: String::from("Piano"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.8,
        vco2_waveform: String::from("square"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.3,
        vco2_pitch_fine: 0.02,
        vco_mix: 0.7,
        eg2_attack: 0.002,
        eg2_decay: 0.8,
        eg2_sustain: 0.2,
        eg2_release: 0.5,
        lpf_cutoff: 4000.0,
        lpf_resonance: 0.5,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("eg2"),
            dest: String::from("lpf_cutoff"),
            depth: 0.4,
        }],
        ..PatchConfig::default()
    }
}

/// Kick drum: sine pitch sweep downward.
fn kick() -> PatchConfig {
    PatchConfig {
        name: String::from("Kick"),
        vco1_waveform: String::from("sine"),
        vco1_scale: String::from("32'"),
        vco1_level: 1.0,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.25,
        eg2_sustain: 0.0,
        eg2_release: 0.1,
        lpf_cutoff: 500.0,
        lpf_resonance: 0.5,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("eg2"),
            dest: String::from("vco1_pitch"),
            depth: 0.9,
        }],
        ..PatchConfig::default()
    }
}

/// Snare drum: noise with short decay.
fn snare() -> PatchConfig {
    PatchConfig {
        name: String::from("Snare"),
        vco1_waveform: String::from("noise"),
        vco1_level: 0.8,
        vco2_waveform: String::from("sine"),
        vco2_scale: String::from("16'"),
        vco2_level: 0.5,
        vco_mix: 0.5,
        eg2_attack: 0.001,
        eg2_decay: 0.15,
        eg2_sustain: 0.0,
        eg2_release: 0.1,
        lpf_cutoff: 8000.0,
        hpf_cutoff: 200.0,
        ..PatchConfig::default()
    }
}

/// Hi-hat: filtered noise with very short decay.
fn hihat() -> PatchConfig {
    PatchConfig {
        name: String::from("Hi-hat"),
        vco1_waveform: String::from("noise"),
        vco1_level: 1.0,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.05,
        eg2_sustain: 0.0,
        eg2_release: 0.03,
        hpf_cutoff: 6000.0,
        lpf_cutoff: 12000.0,
        ..PatchConfig::default()
    }
}

/// Analog synth lead: dual sawtooth, resonant filter.
fn synth_lead() -> PatchConfig {
    PatchConfig {
        name: String::from("Synth Lead"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.8,
        vco2_waveform: String::from("square"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.6,
        vco2_pulse_width: 0.3,
        vco_mix: 0.5,
        eg2_attack: 0.01,
        eg2_decay: 0.3,
        eg2_sustain: 0.6,
        eg2_release: 0.2,
        lpf_cutoff: 2500.0,
        lpf_resonance: 2.0,
        glide_enabled: true,
        glide_time: 0.08,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("eg2"),
            dest: String::from("lpf_cutoff"),
            depth: 0.6,
        }],
        ..PatchConfig::default()
    }
}

/// Synth bass: square wave sub with filter modulation.
fn synth_bass() -> PatchConfig {
    PatchConfig {
        name: String::from("Synth Bass"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("16'"),
        vco1_level: 1.0,
        vco1_pulse_width: 0.3,
        vco2_waveform: String::from("sawtooth"),
        vco2_scale: String::from("32'"),
        vco2_level: 0.5,
        vco_mix: 0.6,
        eg2_attack: 0.003,
        eg2_decay: 0.2,
        eg2_sustain: 0.3,
        eg2_release: 0.1,
        lpf_cutoff: 800.0,
        lpf_resonance: 2.5,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("eg2"),
            dest: String::from("lpf_cutoff"),
            depth: 0.7,
        }],
        ..PatchConfig::default()
    }
}

/// Choir: detuned sawtooths with slow attack, filtered.
fn choir() -> PatchConfig {
    PatchConfig {
        name: String::from("Choir"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.6,
        vco2_waveform: String::from("sawtooth"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.6,
        vco2_pitch_fine: 0.12,
        vco_mix: 0.5,
        eg2_attack: 1.0,
        eg2_decay: 0.3,
        eg2_sustain: 0.85,
        eg2_release: 1.2,
        lpf_cutoff: 2500.0,
        lpf_resonance: 0.4,
        lfo_rate: 3.5,
        mod_routings: vec![ModRoutingConfig {
            source: String::from("lfo"),
            dest: String::from("vco2_pitch"),
            depth: 0.015,
        }],
        ..PatchConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_load() {
        for name in preset_names() {
            let p = preset(name);
            assert!(p.is_some(), "preset {name} should exist");
        }
    }

    #[test]
    fn unknown_preset_returns_none() {
        assert!(preset("nonexistent").is_none());
    }

    #[test]
    fn preset_names_sorted() {
        let names = preset_names();
        for i in 1..names.len() {
            assert!(names[i - 1] <= names[i], "{} should be <= {}", names[i - 1], names[i]);
        }
    }
}
