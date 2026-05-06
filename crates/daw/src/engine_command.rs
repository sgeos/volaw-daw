//! Engine command types for the lock-free audio architecture.
//!
//! Commands are sent from the UI thread to the audio callback thread
//! via the SPSC ring buffer. Hot-path variants (note events, position
//! updates, script parameters) are fixed-size data with no heap
//! allocation. Cold-path operations (patch application, pedal chain
//! rebuild) use the `Bulk` variant, which carries a boxed closure
//! allocated on the UI thread.

use audio::AudioEngine;
use audio::NUM_CHANNELS;

/// A command sent from the UI thread to the audio thread.
pub enum EngineCommand {
    // -- Note events (hot path) --
    NoteOn {
        channel: u8,
        freq: f64,
        velocity: f64,
    },
    NoteOff {
        channel: u8,
    },
    NoteOffFreq {
        channel: u8,
        freq: f64,
    },

    // -- Metronome (hot path) --
    ClickBeat {
        sample_rate: u32,
    },
    ClickDownbeat {
        sample_rate: u32,
    },

    // -- Positional updates (hot path) --
    SetPosition {
        channel: u8,
        azimuth_deg: f64,
        elevation_deg: f64,
        distance: f64,
    },

    // -- Channel configuration (warm path) --
    SetChannelEnabled {
        channel: u8,
        enabled: bool,
    },
    SetChannelVolume {
        channel: u8,
        left: f64,
        right: f64,
    },
    SetMetronomeEnabled {
        enabled: bool,
    },

    // -- Script-generated parameter changes (hot path) --
    SetLpfCutoff {
        channel: u8,
        value: f64,
    },
    SetLpfResonance {
        channel: u8,
        value: f64,
    },
    SetHpfCutoff {
        channel: u8,
        value: f64,
    },
    SetHpfResonance {
        channel: u8,
        value: f64,
    },
    SetVcoMix {
        channel: u8,
        value: f64,
    },
    SetVco1PitchFine {
        channel: u8,
        value: f64,
    },
    SetVco2PitchFine {
        channel: u8,
        value: f64,
    },
    SetEg1Adsr {
        channel: u8,
        attack: f64,
        decay: f64,
        sustain: f64,
        release: f64,
    },
    SetEg2Adsr {
        channel: u8,
        attack: f64,
        decay: f64,
        sustain: f64,
        release: f64,
    },
    SetLfoRate {
        channel: u8,
        value: f64,
    },
    SetVolume {
        channel: u8,
        left: f64,
        right: f64,
    },
    SetPan {
        channel: u8,
        pan: f64,
    },
    SetVco1PulseWidth {
        channel: u8,
        value: f64,
    },
    SetVco2PulseWidth {
        channel: u8,
        value: f64,
    },

    // -- Bulk mutation (cold path) --
    /// Boxed closure for complex operations that cannot be expressed
    /// as fixed-size commands (patch application, pedal chain rebuild).
    /// The closure is created on the UI thread and executed on the
    /// audio thread.
    Bulk(Box<dyn FnOnce(&mut AudioEngine) + Send>),
}

/// Apply a single engine command to the audio engine.
///
/// Called from the audio callback after draining the ring buffer.
/// Each command performs a bounded amount of work (no I/O, no
/// unbounded loops).
pub fn apply_engine_command(engine: &mut AudioEngine, cmd: EngineCommand) {
    match cmd {
        EngineCommand::NoteOn {
            channel,
            freq,
            velocity,
        } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                engine.channels[ch]
                    .synth
                    .note_on_with_velocity(freq, velocity);
                engine.channels[ch].enabled = true;
            }
        }
        EngineCommand::NoteOff { channel } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                engine.channels[ch].synth.note_off();
            }
        }
        EngineCommand::NoteOffFreq { channel, freq } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                engine.channels[ch].synth.note_off_freq(freq);
            }
        }
        EngineCommand::ClickBeat { sample_rate } => {
            engine.metronome.click_beat(sample_rate);
        }
        EngineCommand::ClickDownbeat { sample_rate } => {
            engine.metronome.click_downbeat(sample_rate);
        }
        EngineCommand::SetPosition {
            channel,
            azimuth_deg,
            elevation_deg,
            distance,
        } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                let pos = &mut engine.channels[ch].position;
                pos.active = true;
                pos.position_3d_enabled = true;
                pos.azimuth_deg = azimuth_deg;
                pos.elevation_deg = elevation_deg;
                pos.distance = distance;
            }
        }
        EngineCommand::SetChannelEnabled { channel, enabled } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                engine.channels[ch].enabled = enabled;
            }
        }
        EngineCommand::SetChannelVolume {
            channel,
            left,
            right,
        } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                engine.channels[ch].set_volume(left, right);
            }
        }
        EngineCommand::SetMetronomeEnabled { enabled } => {
            engine.metronome.enabled = enabled;
        }
        EngineCommand::SetLpfCutoff { channel, value } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                for voice in &mut engine.channels[ch].synth.voices {
                    voice.lpf.cutoff = value;
                }
            }
        }
        EngineCommand::SetLpfResonance { channel, value } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                for voice in &mut engine.channels[ch].synth.voices {
                    voice.lpf.resonance = value;
                }
            }
        }
        EngineCommand::SetHpfCutoff { channel, value } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                for voice in &mut engine.channels[ch].synth.voices {
                    voice.hpf.cutoff = value;
                }
            }
        }
        EngineCommand::SetHpfResonance { channel, value } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                for voice in &mut engine.channels[ch].synth.voices {
                    voice.hpf.resonance = value;
                }
            }
        }
        EngineCommand::SetVcoMix { channel, value } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                engine.channels[ch].synth.vco_mix = value;
            }
        }
        EngineCommand::SetVco1PitchFine { channel, value } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                for voice in &mut engine.channels[ch].synth.voices {
                    voice.vco1.pitch_fine = value;
                }
            }
        }
        EngineCommand::SetVco2PitchFine { channel, value } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                for voice in &mut engine.channels[ch].synth.voices {
                    voice.vco2.pitch_fine = value;
                }
            }
        }
        EngineCommand::SetEg1Adsr {
            channel,
            attack,
            decay,
            sustain,
            release,
        } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                for voice in &mut engine.channels[ch].synth.voices {
                    voice.eg1.attack = attack;
                    voice.eg1.decay = decay;
                    voice.eg1.sustain = sustain;
                    voice.eg1.release = release;
                }
            }
        }
        EngineCommand::SetEg2Adsr {
            channel,
            attack,
            decay,
            sustain,
            release,
        } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                for voice in &mut engine.channels[ch].synth.voices {
                    voice.eg2.attack = attack;
                    voice.eg2.decay = decay;
                    voice.eg2.sustain = sustain;
                    voice.eg2.release = release;
                }
            }
        }
        EngineCommand::SetLfoRate { channel, value } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                engine.channels[ch].synth.lfo_rate = value;
            }
        }
        EngineCommand::SetVolume {
            channel,
            left,
            right,
        } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                engine.channels[ch].set_volume(left, right);
            }
        }
        EngineCommand::SetPan { channel, pan } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                // Constant-power pan law.
                let angle = (pan + 1.0) * 0.25 * core::f64::consts::PI;
                let left = angle.cos();
                let right = angle.sin();
                engine.channels[ch].set_volume(left, right);
            }
        }
        EngineCommand::SetVco1PulseWidth { channel, value } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                for voice in &mut engine.channels[ch].synth.voices {
                    voice.vco1.pulse_width = value;
                }
            }
        }
        EngineCommand::SetVco2PulseWidth { channel, value } => {
            let ch = channel as usize;
            if ch < NUM_CHANNELS {
                for voice in &mut engine.channels[ch].synth.voices {
                    voice.vco2.pulse_width = value;
                }
            }
        }
        EngineCommand::Bulk(f) => {
            f(engine);
        }
    }
}

/// Convert a `daw_core::script::AudioCommand` into an `EngineCommand`
/// for a specific channel.
pub fn audio_command_to_engine_command(
    channel: u8,
    cmd: &daw_core::script::AudioCommand,
) -> EngineCommand {
    use daw_core::script::AudioCommand;
    match cmd {
        AudioCommand::NoteOn { freq, velocity } => EngineCommand::NoteOn {
            channel,
            freq: *freq,
            velocity: *velocity,
        },
        AudioCommand::NoteOff => EngineCommand::NoteOff { channel },
        AudioCommand::NoteOffFreq { freq } => EngineCommand::NoteOffFreq {
            channel,
            freq: *freq,
        },
        AudioCommand::SetLpfCutoff(v) => EngineCommand::SetLpfCutoff { channel, value: *v },
        AudioCommand::SetLpfResonance(v) => EngineCommand::SetLpfResonance { channel, value: *v },
        AudioCommand::SetHpfCutoff(v) => EngineCommand::SetHpfCutoff { channel, value: *v },
        AudioCommand::SetHpfResonance(v) => EngineCommand::SetHpfResonance { channel, value: *v },
        AudioCommand::SetVcoMix(v) => EngineCommand::SetVcoMix { channel, value: *v },
        AudioCommand::SetVco1PitchFine(v) => EngineCommand::SetVco1PitchFine { channel, value: *v },
        AudioCommand::SetVco2PitchFine(v) => EngineCommand::SetVco2PitchFine { channel, value: *v },
        AudioCommand::SetEg1Adsr {
            attack,
            decay,
            sustain,
            release,
        } => EngineCommand::SetEg1Adsr {
            channel,
            attack: *attack,
            decay: *decay,
            sustain: *sustain,
            release: *release,
        },
        AudioCommand::SetEg2Adsr {
            attack,
            decay,
            sustain,
            release,
        } => EngineCommand::SetEg2Adsr {
            channel,
            attack: *attack,
            decay: *decay,
            sustain: *sustain,
            release: *release,
        },
        AudioCommand::SetLfoRate(v) => EngineCommand::SetLfoRate { channel, value: *v },
        AudioCommand::SetVolume { left, right } => EngineCommand::SetVolume {
            channel,
            left: *left,
            right: *right,
        },
        AudioCommand::SetPan(v) => EngineCommand::SetPan { channel, pan: *v },
        AudioCommand::SetVco1PulseWidth(v) => {
            EngineCommand::SetVco1PulseWidth { channel, value: *v }
        }
        AudioCommand::SetVco2PulseWidth(v) => {
            EngineCommand::SetVco2PulseWidth { channel, value: *v }
        }
    }
}
