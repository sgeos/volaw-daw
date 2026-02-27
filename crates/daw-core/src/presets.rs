extern crate alloc;
use alloc::string::String;

use crate::patch::{ModRoutingConfig, PatchConfig};

/// Build modulation routings from the standard MS-20 defaults plus extra
/// preset-specific routings. If an extra routing targets the same source and
/// destination as a standard routing, it replaces the default depth.
fn routings_with(extra: &[(&str, &str, f64)]) -> alloc::vec::Vec<ModRoutingConfig> {
    let mut routes = crate::patch::default_mod_routings();
    for &(source, dest, depth) in extra {
        if let Some(existing) = routes.iter_mut().find(|r| r.source == source && r.dest == dest) {
            existing.depth = depth;
        } else {
            routes.push(ModRoutingConfig {
                source: String::from(source),
                dest: String::from(dest),
                depth,
            });
        }
    }
    routes
}

/// Create a named preset patch configuration.
///
/// Each preset approximates a real instrument using the MS-20 synthesizer
/// architecture (dual VCO, dual EG, HPF, LPF, modulation matrix).
pub fn preset(name: &str) -> Option<PatchConfig> {
    match name {
        "accordion" => Some(accordion()),
        "bass" => Some(bass()),
        "bell" => Some(bell()),
        "brass" => Some(brass()),
        "cello" => Some(cello()),
        "choir" => Some(choir()),
        "clap" => Some(clap()),
        "clarinet" => Some(clarinet()),
        "clavinet" => Some(clavinet()),
        "cowbell" => Some(cowbell()),
        "cymbal" => Some(cymbal()),
        "flute" => Some(flute()),
        "gb_noise" => Some(gb_noise()),
        "gb_pulse" => Some(gb_pulse()),
        "gb_wave" => Some(gb_wave()),
        "harp" => Some(harp()),
        "harpsichord" => Some(harpsichord()),
        "harmonica" => Some(harmonica()),
        "hihat" => Some(hihat()),
        "kick" => Some(kick()),
        "lead" => Some(lead()),
        "marimba" => Some(marimba()),
        "nes_noise" => Some(nes_noise()),
        "nes_pulse_12" => Some(nes_pulse_12()),
        "nes_pulse_25" => Some(nes_pulse_25()),
        "nes_pulse_50" => Some(nes_pulse_50()),
        "nes_triangle" => Some(nes_triangle()),
        "oboe" => Some(oboe()),
        "organ" => Some(organ()),
        "pad" => Some(pad()),
        "piano" => Some(piano()),
        "pluck" => Some(pluck()),
        "sitar" => Some(sitar()),
        "snare" => Some(snare()),
        "strings" => Some(strings()),
        "synth_bass" => Some(synth_bass()),
        "synth_lead" => Some(synth_lead()),
        "tom" => Some(tom()),
        "trumpet" => Some(trumpet()),
        "vibraphone" => Some(vibraphone()),
        _ => None,
    }
}

/// List all available preset names.
pub fn preset_names() -> &'static [&'static str] {
    &[
        "accordion",
        "bass",
        "bell",
        "brass",
        "cello",
        "choir",
        "clap",
        "clarinet",
        "clavinet",
        "cowbell",
        "cymbal",
        "flute",
        "gb_noise",
        "gb_pulse",
        "gb_wave",
        "harmonica",
        "harp",
        "harpsichord",
        "hihat",
        "kick",
        "lead",
        "marimba",
        "nes_noise",
        "nes_pulse_12",
        "nes_pulse_25",
        "nes_pulse_50",
        "nes_triangle",
        "oboe",
        "organ",
        "pad",
        "piano",
        "pluck",
        "sitar",
        "snare",
        "strings",
        "synth_bass",
        "synth_lead",
        "tom",
        "trumpet",
        "vibraphone",
    ]
}

/// Fingered bass: sawtooth low, short decay, moderate LPF.
fn bass() -> PatchConfig {
    PatchConfig {
        name: String::from("Bass"),
        voice_mode: String::from("low_1"),
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
        mod_routings: routings_with(&[("eg2", "lpf_cutoff", 0.5)]),
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
        mod_routings: routings_with(&[("lfo", "vco1_pitch", 0.02)]),
        ..PatchConfig::default()
    }
}

/// Warm pad: detuned sawtooths with slow attack.
fn pad() -> PatchConfig {
    PatchConfig {
        name: String::from("Pad"),
        voice_mode: String::from("high_10"),
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
        voice_mode: String::from("high_10"),
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
        mod_routings: routings_with(&[("lfo", "vco1_pitch", 0.01)]),
        ..PatchConfig::default()
    }
}

/// Hammond-style organ: square wave, no attack/release.
fn organ() -> PatchConfig {
    PatchConfig {
        name: String::from("Organ"),
        voice_mode: String::from("high_10"),
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
        voice_mode: String::from("high_10"),
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
        mod_routings: routings_with(&[("eg2", "lpf_cutoff", 0.7)]),
        ..PatchConfig::default()
    }
}

/// Pluck: short envelope, resonant filter.
fn pluck() -> PatchConfig {
    PatchConfig {
        name: String::from("Pluck"),
        voice_mode: String::from("high_10"),
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
        mod_routings: routings_with(&[("eg2", "lpf_cutoff", 0.8)]),
        ..PatchConfig::default()
    }
}

/// Bell: FM-like with ring modulator, quick decay.
fn bell() -> PatchConfig {
    PatchConfig {
        name: String::from("Bell"),
        voice_mode: String::from("high_10"),
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
        mod_routings: routings_with(&[("lfo", "vco1_pitch", 0.015)]),
        ..PatchConfig::default()
    }
}

/// Electric piano: quick attack, medium decay, mild detuning.
fn piano() -> PatchConfig {
    PatchConfig {
        name: String::from("Piano"),
        voice_mode: String::from("high_10"),
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
        mod_routings: routings_with(&[("eg2", "lpf_cutoff", 0.4)]),
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
        mod_routings: routings_with(&[("eg2", "vco1_pitch", 0.9)]),
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
        mod_routings: routings_with(&[("eg2", "lpf_cutoff", 0.6)]),
        ..PatchConfig::default()
    }
}

/// Synth bass: square wave sub with filter modulation.
fn synth_bass() -> PatchConfig {
    PatchConfig {
        name: String::from("Synth Bass"),
        voice_mode: String::from("low_1"),
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
        mod_routings: routings_with(&[("eg2", "lpf_cutoff", 0.7)]),
        ..PatchConfig::default()
    }
}

/// Accordion: pulse wave with slight vibrato, full sustain.
fn accordion() -> PatchConfig {
    PatchConfig {
        name: String::from("Accordion"),
        voice_mode: String::from("high_10"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.7,
        vco1_pulse_width: 0.4,
        vco2_waveform: String::from("square"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.5,
        vco2_pulse_width: 0.6,
        vco2_pitch_fine: 0.04,
        vco_mix: 0.5,
        eg2_attack: 0.05,
        eg2_decay: 0.05,
        eg2_sustain: 0.95,
        eg2_release: 0.1,
        lpf_cutoff: 3500.0,
        lpf_resonance: 0.4,
        lfo_rate: 5.5,
        mod_routings: routings_with(&[("lfo", "vco1_pitch", 0.008)]),
        ..PatchConfig::default()
    }
}

/// Cello: low sawtooth with slow attack and vibrato.
fn cello() -> PatchConfig {
    PatchConfig {
        name: String::from("Cello"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("16'"),
        vco1_level: 0.7,
        vco2_waveform: String::from("sawtooth"),
        vco2_scale: String::from("16'"),
        vco2_level: 0.5,
        vco2_pitch_fine: 0.05,
        vco_mix: 0.5,
        eg2_attack: 0.4,
        eg2_decay: 0.2,
        eg2_sustain: 0.85,
        eg2_release: 0.6,
        lpf_cutoff: 2500.0,
        lpf_resonance: 0.5,
        lfo_rate: 5.0,
        mod_routings: routings_with(&[("lfo", "vco1_pitch", 0.012)]),
        ..PatchConfig::default()
    }
}

/// Hand clap: noise with quick double-strike envelope.
fn clap() -> PatchConfig {
    PatchConfig {
        name: String::from("Clap"),
        vco1_waveform: String::from("noise"),
        vco1_level: 1.0,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.08,
        eg2_sustain: 0.0,
        eg2_release: 0.12,
        hpf_cutoff: 800.0,
        lpf_cutoff: 6000.0,
        lpf_resonance: 0.8,
        ..PatchConfig::default()
    }
}

/// Clarinet: square wave with odd harmonics, warm filter.
fn clarinet() -> PatchConfig {
    PatchConfig {
        name: String::from("Clarinet"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.9,
        vco1_pulse_width: 0.5,
        vco2_waveform: String::from("noise"),
        vco2_level: 0.03,
        vco_mix: 0.9,
        eg2_attack: 0.06,
        eg2_decay: 0.1,
        eg2_sustain: 0.8,
        eg2_release: 0.15,
        lpf_cutoff: 2200.0,
        lpf_resonance: 0.6,
        lfo_rate: 4.5,
        mod_routings: routings_with(&[("lfo", "vco1_pitch", 0.01)]),
        ..PatchConfig::default()
    }
}

/// Clavinet: bright percussive pluck, funky character.
fn clavinet() -> PatchConfig {
    PatchConfig {
        name: String::from("Clavinet"),
        voice_mode: String::from("high_10"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.9,
        vco1_pulse_width: 0.35,
        vco2_waveform: String::from("sawtooth"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.3,
        vco_mix: 0.7,
        eg2_attack: 0.001,
        eg2_decay: 0.5,
        eg2_sustain: 0.1,
        eg2_release: 0.15,
        lpf_cutoff: 5500.0,
        lpf_resonance: 1.8,
        mod_routings: routings_with(&[("eg2", "lpf_cutoff", 0.6)]),
        ..PatchConfig::default()
    }
}

/// Cowbell: two tones via ring modulation, short decay.
fn cowbell() -> PatchConfig {
    PatchConfig {
        name: String::from("Cowbell"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("4'"),
        vco1_level: 0.8,
        vco1_pulse_width: 0.5,
        vco2_waveform: String::from("ring"),
        vco2_scale: String::from("2'"),
        vco2_level: 0.6,
        vco_mix: 0.4,
        eg2_attack: 0.001,
        eg2_decay: 0.3,
        eg2_sustain: 0.0,
        eg2_release: 0.1,
        hpf_cutoff: 500.0,
        lpf_cutoff: 5000.0,
        ..PatchConfig::default()
    }
}

/// Cymbal: filtered noise with long decay and highpass.
fn cymbal() -> PatchConfig {
    PatchConfig {
        name: String::from("Cymbal"),
        vco1_waveform: String::from("noise"),
        vco1_level: 0.9,
        vco2_waveform: String::from("ring"),
        vco2_scale: String::from("2'"),
        vco2_level: 0.3,
        vco_mix: 0.7,
        eg2_attack: 0.001,
        eg2_decay: 1.5,
        eg2_sustain: 0.0,
        eg2_release: 1.0,
        hpf_cutoff: 4000.0,
        lpf_cutoff: 14000.0,
        ..PatchConfig::default()
    }
}

/// Harp: bright pluck with longer sustain and high cutoff.
fn harp() -> PatchConfig {
    PatchConfig {
        name: String::from("Harp"),
        voice_mode: String::from("high_10"),
        vco1_waveform: String::from("triangle"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.8,
        vco2_waveform: String::from("sawtooth"),
        vco2_scale: String::from("4'"),
        vco2_level: 0.2,
        vco_mix: 0.8,
        eg2_attack: 0.001,
        eg2_decay: 1.2,
        eg2_sustain: 0.0,
        eg2_release: 0.8,
        lpf_cutoff: 6000.0,
        lpf_resonance: 0.4,
        mod_routings: routings_with(&[("eg2", "lpf_cutoff", 0.5)]),
        ..PatchConfig::default()
    }
}

/// Harpsichord: bright, thin, quick attack and decay.
fn harpsichord() -> PatchConfig {
    PatchConfig {
        name: String::from("Harpsichord"),
        voice_mode: String::from("high_10"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.7,
        vco1_pulse_width: 0.15,
        vco2_waveform: String::from("square"),
        vco2_scale: String::from("4'"),
        vco2_level: 0.4,
        vco2_pulse_width: 0.2,
        vco_mix: 0.6,
        eg2_attack: 0.001,
        eg2_decay: 0.6,
        eg2_sustain: 0.05,
        eg2_release: 0.2,
        lpf_cutoff: 7000.0,
        lpf_resonance: 0.3,
        ..PatchConfig::default()
    }
}

/// Harmonica: square wave with breath noise.
fn harmonica() -> PatchConfig {
    PatchConfig {
        name: String::from("Harmonica"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.8,
        vco1_pulse_width: 0.45,
        vco2_waveform: String::from("noise"),
        vco2_level: 0.08,
        vco_mix: 0.85,
        eg2_attack: 0.04,
        eg2_decay: 0.1,
        eg2_sustain: 0.75,
        eg2_release: 0.1,
        lpf_cutoff: 2800.0,
        lpf_resonance: 0.7,
        lfo_rate: 5.0,
        mod_routings: routings_with(&[("lfo", "vco1_pitch", 0.018)]),
        ..PatchConfig::default()
    }
}

/// Marimba: sine with quick decay, warm character.
fn marimba() -> PatchConfig {
    PatchConfig {
        name: String::from("Marimba"),
        voice_mode: String::from("high_10"),
        vco1_waveform: String::from("sine"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.9,
        vco2_waveform: String::from("triangle"),
        vco2_scale: String::from("4'"),
        vco2_level: 0.2,
        vco_mix: 0.8,
        eg2_attack: 0.001,
        eg2_decay: 0.6,
        eg2_sustain: 0.0,
        eg2_release: 0.3,
        lpf_cutoff: 4500.0,
        lpf_resonance: 0.3,
        ..PatchConfig::default()
    }
}

/// Oboe: narrow pulse wave with vibrato, nasal character.
fn oboe() -> PatchConfig {
    PatchConfig {
        name: String::from("Oboe"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.9,
        vco1_pulse_width: 0.2,
        vco2_waveform: String::from("noise"),
        vco2_level: 0.02,
        vco_mix: 0.95,
        eg2_attack: 0.08,
        eg2_decay: 0.1,
        eg2_sustain: 0.8,
        eg2_release: 0.15,
        lpf_cutoff: 2000.0,
        lpf_resonance: 1.0,
        lfo_rate: 5.5,
        mod_routings: routings_with(&[("lfo", "vco1_pitch", 0.012)]),
        ..PatchConfig::default()
    }
}

/// Sitar: resonant pluck with high filter Q, twangy character.
fn sitar() -> PatchConfig {
    PatchConfig {
        name: String::from("Sitar"),
        voice_mode: String::from("high_10"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.9,
        vco2_waveform: String::from("square"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.3,
        vco2_pulse_width: 0.15,
        vco_mix: 0.7,
        eg2_attack: 0.001,
        eg2_decay: 1.5,
        eg2_sustain: 0.05,
        eg2_release: 0.8,
        lpf_cutoff: 3000.0,
        lpf_resonance: 2.5,
        mod_routings: routings_with(&[("eg2", "lpf_cutoff", 0.9)]),
        ..PatchConfig::default()
    }
}

/// Tom drum: sine with pitch sweep, medium decay.
fn tom() -> PatchConfig {
    PatchConfig {
        name: String::from("Tom"),
        vco1_waveform: String::from("sine"),
        vco1_scale: String::from("16'"),
        vco1_level: 1.0,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.35,
        eg2_sustain: 0.0,
        eg2_release: 0.15,
        lpf_cutoff: 800.0,
        lpf_resonance: 0.6,
        mod_routings: routings_with(&[("eg2", "vco1_pitch", 0.5)]),
        ..PatchConfig::default()
    }
}

/// Trumpet: bright sawtooth with pronounced filter sweep attack.
fn trumpet() -> PatchConfig {
    PatchConfig {
        name: String::from("Trumpet"),
        vco1_waveform: String::from("sawtooth"),
        vco1_scale: String::from("8'"),
        vco1_level: 1.0,
        vco2_waveform: String::from("square"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.2,
        vco2_pulse_width: 0.4,
        vco_mix: 0.8,
        eg2_attack: 0.04,
        eg2_decay: 0.1,
        eg2_sustain: 0.85,
        eg2_release: 0.15,
        lpf_cutoff: 2000.0,
        lpf_resonance: 0.6,
        lfo_rate: 5.0,
        mod_routings: routings_with(&[
            ("eg2", "lpf_cutoff", 0.8),
            ("lfo", "vco1_pitch", 0.008),
        ]),
        ..PatchConfig::default()
    }
}

/// Vibraphone: sine with slow tremolo, long sustain.
fn vibraphone() -> PatchConfig {
    PatchConfig {
        name: String::from("Vibraphone"),
        voice_mode: String::from("high_10"),
        vco1_waveform: String::from("sine"),
        vco1_scale: String::from("8'"),
        vco1_level: 0.9,
        vco2_waveform: String::from("triangle"),
        vco2_scale: String::from("8'"),
        vco2_level: 0.15,
        vco2_pitch_fine: 0.01,
        vco_mix: 0.85,
        eg2_attack: 0.001,
        eg2_decay: 2.0,
        eg2_sustain: 0.1,
        eg2_release: 1.5,
        lpf_cutoff: 5000.0,
        lpf_resonance: 0.3,
        lfo_rate: 4.5,
        mod_routings: routings_with(&[("lfo", "vca_level", 0.15)]),
        ..PatchConfig::default()
    }
}

/// Choir: detuned sawtooths with slow attack, filtered.
fn choir() -> PatchConfig {
    PatchConfig {
        name: String::from("Choir"),
        voice_mode: String::from("high_10"),
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
        mod_routings: routings_with(&[("lfo", "vco2_pitch", 0.015)]),
        ..PatchConfig::default()
    }
}

// ---------------------------------------------------------------------------
// NES / Game Boy chiptune presets
// ---------------------------------------------------------------------------
//
// All chiptune presets are monophonic (high_1) to match real hardware.
// Envelopes use instant attack and fast release for tight gate behavior.
// LPF is wide open (no analog filtering on chip hardware).
// Single oscillator per channel (VCO2 disabled).

/// NES 2A03 triangle channel: 4-bit quantized triangle, no volume control.
fn nes_triangle() -> PatchConfig {
    PatchConfig {
        name: String::from("NES Triangle"),
        vco1_waveform: String::from("triangle"),
        vco1_scale: String::from("8'"),
        vco1_level: 1.0,
        vco1_quantization: Some(4),
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.01,
        eg2_sustain: 1.0,
        eg2_release: 0.01,
        lpf_cutoff: 16000.0,
        lpf_resonance: 0.0,
        ..PatchConfig::default()
    }
}

/// NES 2A03 pulse channel at 50% duty (square wave).
fn nes_pulse_50() -> PatchConfig {
    PatchConfig {
        name: String::from("NES Pulse 50%"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 1.0,
        vco1_pulse_width: 0.5,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.01,
        eg2_sustain: 1.0,
        eg2_release: 0.01,
        lpf_cutoff: 16000.0,
        lpf_resonance: 0.0,
        ..PatchConfig::default()
    }
}

/// NES 2A03 pulse channel at 25% duty.
fn nes_pulse_25() -> PatchConfig {
    PatchConfig {
        name: String::from("NES Pulse 25%"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 1.0,
        vco1_pulse_width: 0.25,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.01,
        eg2_sustain: 1.0,
        eg2_release: 0.01,
        lpf_cutoff: 16000.0,
        lpf_resonance: 0.0,
        ..PatchConfig::default()
    }
}

/// NES 2A03 pulse channel at 12.5% duty (narrow pulse).
fn nes_pulse_12() -> PatchConfig {
    PatchConfig {
        name: String::from("NES Pulse 12.5%"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 1.0,
        vco1_pulse_width: 0.125,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.01,
        eg2_sustain: 1.0,
        eg2_release: 0.01,
        lpf_cutoff: 16000.0,
        lpf_resonance: 0.0,
        ..PatchConfig::default()
    }
}

/// NES 2A03 noise channel (LFSR-based white noise).
fn nes_noise() -> PatchConfig {
    PatchConfig {
        name: String::from("NES Noise"),
        vco1_waveform: String::from("noise"),
        vco1_level: 1.0,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.01,
        eg2_sustain: 1.0,
        eg2_release: 0.01,
        lpf_cutoff: 16000.0,
        lpf_resonance: 0.0,
        ..PatchConfig::default()
    }
}

/// Game Boy DMG pulse channel (25% duty, classic GB lead).
fn gb_pulse() -> PatchConfig {
    PatchConfig {
        name: String::from("GB Pulse"),
        vco1_waveform: String::from("square"),
        vco1_scale: String::from("8'"),
        vco1_level: 1.0,
        vco1_pulse_width: 0.25,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.01,
        eg2_sustain: 1.0,
        eg2_release: 0.01,
        lpf_cutoff: 12000.0,
        lpf_resonance: 0.0,
        ..PatchConfig::default()
    }
}

/// Game Boy DMG wave channel (4-bit programmable wavetable approximation).
fn gb_wave() -> PatchConfig {
    PatchConfig {
        name: String::from("GB Wave"),
        vco1_waveform: String::from("triangle"),
        vco1_scale: String::from("8'"),
        vco1_level: 1.0,
        vco1_quantization: Some(4),
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.01,
        eg2_sustain: 1.0,
        eg2_release: 0.01,
        lpf_cutoff: 12000.0,
        lpf_resonance: 0.0,
        ..PatchConfig::default()
    }
}

/// Game Boy DMG noise channel (LFSR-based).
fn gb_noise() -> PatchConfig {
    PatchConfig {
        name: String::from("GB Noise"),
        vco1_waveform: String::from("noise"),
        vco1_level: 1.0,
        vco2_level: 0.0,
        eg2_attack: 0.001,
        eg2_decay: 0.01,
        eg2_sustain: 1.0,
        eg2_release: 0.01,
        lpf_cutoff: 12000.0,
        lpf_resonance: 0.0,
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
