//! Engine command types for the lock-free audio architecture.
//!
//! Commands are sent from the UI thread to the audio callback thread
//! via the SPSC ring buffer. Hot-path variants (note events, position
//! updates, script parameters) are fixed-size data with no heap
//! allocation. Cold-path operations (patch application, pedal chain
//! rebuild) use the `Bulk` variant, which carries a boxed `EngineEdit`
//! built on the UI thread. The audio thread applies the edit and hands the
//! box back through `runtime::CommandReceiver`, so that the box and anything
//! the edit swapped out of the engine are freed on the control thread.

use alloc::boxed::Box;

use crate::engine::{AudioEngine, NUM_CHANNELS};

/// A change to the engine that fixed-size commands cannot express, built on
/// the control thread and applied on the audio thread.
///
/// `apply` runs inside the audio callback, so it must not allocate, free,
/// block or perform I/O. Build every new value, such as a pedal chain, before
/// sending the edit, and swap it into the engine in `apply`. Whatever the
/// swap takes out stays in the edit, which `runtime::EngineRunner` returns to
/// the control thread to be dropped there. The runner applies each edit once.
///
/// Every `FnMut(&mut AudioEngine) + Send` closure is an edit. A closure that
/// moves a captured value into the engine cannot be `FnMut`, and swapping
/// through `core::mem::swap` or `core::mem::replace` is the usual remedy.
///
/// The rule above is a contract on the implementer, and nothing enforces it.
/// `apply` runs arbitrary code on the audio thread, and an edit that grows a
/// vector, drops a value it took out of the engine, or takes a lock does so
/// in the callback. The runner frees only the edit itself off the audio
/// thread. The counting allocator tests in `tests/audio_thread_alloc.rs` of
/// this crate, and in `daw-core`, check the edits this workspace builds, not
/// edits from other code.
pub trait EngineEdit: Send {
    fn apply(&mut self, engine: &mut AudioEngine);
}

impl<F: FnMut(&mut AudioEngine) + Send> EngineEdit for F {
    fn apply(&mut self, engine: &mut AudioEngine) {
        self(engine);
    }
}

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
    /// A boxed edit for changes that fixed-size commands cannot express,
    /// such as applying a patch or replacing a pedal chain. It is built on
    /// the UI thread, applied on the audio thread, and freed on the UI
    /// thread once `runtime::CommandSender` reclaims it.
    Bulk(Box<dyn EngineEdit>),
}

/// Apply a single engine command to the audio engine.
///
/// Each command performs a bounded amount of work (no I/O, no
/// unbounded loops). A `Bulk` edit is dropped here once applied, which frees
/// the box and everything the edit carries on the calling thread.
/// `EngineRunner` therefore applies the edit itself and returns it instead.
/// A host that drives an engine on its own audio thread must not pass `Bulk`
/// to this function there, and should use `EngineRunner` or apply the edit
/// and send it back to the control thread itself.
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
                let left = libm::cos(angle);
                let right = libm::sin(angle);
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
        EngineCommand::Bulk(mut edit) => {
            edit.apply(engine);
        }
    }
}
