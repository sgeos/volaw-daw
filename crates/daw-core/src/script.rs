extern crate alloc;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use audio::AudioEngine;
use keleos::bytecode::Value;
use keleos::compiler::compile;
use keleos::lexer::tokenize;
use keleos::parser::parse;
use keleos::vm::{Vm, VmError, VmState};

/// An audio command produced by a Keleos script during execution.
///
/// Scripts call native functions like `audio::note_on(freq, vel)` which
/// push commands to a shared buffer. After script evaluation, the host
/// applies these commands to the audio engine.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioCommand {
    /// Trigger a note at the given frequency and velocity (0.0 to 1.0).
    NoteOn { freq: f64, velocity: f64 },
    /// Release all active notes.
    NoteOff,
    /// Release a specific note by frequency.
    NoteOffFreq { freq: f64 },
    /// Set the low-pass filter cutoff frequency in Hz.
    SetLpfCutoff(f64),
    /// Set the low-pass filter resonance (0.0 to 2.0).
    SetLpfResonance(f64),
    /// Set the high-pass filter cutoff frequency in Hz.
    SetHpfCutoff(f64),
    /// Set the high-pass filter resonance (0.5 to 25.0).
    SetHpfResonance(f64),
    /// Set the VCO mix balance (0.0 = all VCO1, 1.0 = all VCO2).
    SetVcoMix(f64),
    /// Set VCO1 fine pitch in semitones (-12 to +12).
    SetVco1PitchFine(f64),
    /// Set VCO2 fine pitch in semitones (-12 to +12).
    SetVco2PitchFine(f64),
    /// Set envelope generator 1 parameters (attack, decay, sustain, release).
    SetEg1Adsr { attack: f64, decay: f64, sustain: f64, release: f64 },
    /// Set envelope generator 2 parameters (attack, decay, sustain, release).
    SetEg2Adsr { attack: f64, decay: f64, sustain: f64, release: f64 },
    /// Set LFO rate in Hz.
    SetLfoRate(f64),
    /// Set channel volume (left, right).
    SetVolume { left: f64, right: f64 },
    /// Set channel pan (-1.0 left, 0.0 center, 1.0 right).
    SetPan(f64),
    /// Set VCO1 pulse width (0.01 to 0.99).
    SetVco1PulseWidth(f64),
    /// Set VCO2 pulse width (0.01 to 0.99).
    SetVco2PulseWidth(f64),
}

/// Extract an f64 from a Keleos Value, accepting both Float and Int.
fn extract_f64(val: &Value) -> Result<f64, VmError> {
    match val {
        Value::Float(f) => Ok(*f),
        Value::Int(i) => Ok(*i as f64),
        other => Err(VmError::TypeError(format!(
            "expected f64, got {}",
            other.type_name()
        ))),
    }
}

/// Register audio command native functions on a VM.
///
/// These closures push AudioCommands to the shared buffer when called.
/// The host reads the buffer after each tick and applies commands to the engine.
fn register_audio_command_natives(
    vm: &mut Vm,
    commands: &Rc<RefCell<Vec<AudioCommand>>>,
) {
    // audio::note_on(freq: f64, velocity: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::note_on", move |args| {
        if args.len() != 2 {
            return Err(VmError::NativeError(format!(
                "audio::note_on: expected 2 arguments, got {}",
                args.len()
            )));
        }
        let freq = extract_f64(&args[0])?;
        let vel = extract_f64(&args[1])?;
        cmds.borrow_mut().push(AudioCommand::NoteOn {
            freq,
            velocity: vel,
        });
        Ok(Value::Unit)
    });

    // audio::note_off() -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::note_off", move |args| {
        if !args.is_empty() {
            return Err(VmError::NativeError(format!(
                "audio::note_off: expected 0 arguments, got {}",
                args.len()
            )));
        }
        cmds.borrow_mut().push(AudioCommand::NoteOff);
        Ok(Value::Unit)
    });

    // audio::note_off_freq(freq: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::note_off_freq", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::note_off_freq: expected 1 argument, got {}",
                args.len()
            )));
        }
        let freq = extract_f64(&args[0])?;
        cmds.borrow_mut().push(AudioCommand::NoteOffFreq { freq });
        Ok(Value::Unit)
    });

    // audio::set_lpf_cutoff(cutoff: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_lpf_cutoff", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_lpf_cutoff: expected 1 argument, got {}",
                args.len()
            )));
        }
        let cutoff = extract_f64(&args[0])?;
        cmds.borrow_mut().push(AudioCommand::SetLpfCutoff(cutoff));
        Ok(Value::Unit)
    });

    // audio::set_lpf_resonance(resonance: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_lpf_resonance", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_lpf_resonance: expected 1 argument, got {}",
                args.len()
            )));
        }
        let res = extract_f64(&args[0])?;
        cmds.borrow_mut().push(AudioCommand::SetLpfResonance(res));
        Ok(Value::Unit)
    });

    // audio::set_hpf_cutoff(cutoff: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_hpf_cutoff", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_hpf_cutoff: expected 1 argument, got {}",
                args.len()
            )));
        }
        let cutoff = extract_f64(&args[0])?;
        cmds.borrow_mut().push(AudioCommand::SetHpfCutoff(cutoff));
        Ok(Value::Unit)
    });

    // audio::set_hpf_resonance(resonance: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_hpf_resonance", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_hpf_resonance: expected 1 argument, got {}",
                args.len()
            )));
        }
        let res = extract_f64(&args[0])?;
        cmds.borrow_mut()
            .push(AudioCommand::SetHpfResonance(res));
        Ok(Value::Unit)
    });

    // audio::set_vco_mix(mix: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_vco_mix", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_vco_mix: expected 1 argument, got {}",
                args.len()
            )));
        }
        let mix = extract_f64(&args[0])?;
        cmds.borrow_mut().push(AudioCommand::SetVcoMix(mix));
        Ok(Value::Unit)
    });

    // audio::set_vco1_pitch_fine(semitones: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_vco1_pitch_fine", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_vco1_pitch_fine: expected 1 argument, got {}",
                args.len()
            )));
        }
        let st = extract_f64(&args[0])?;
        cmds.borrow_mut()
            .push(AudioCommand::SetVco1PitchFine(st));
        Ok(Value::Unit)
    });

    // audio::set_vco2_pitch_fine(semitones: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_vco2_pitch_fine", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_vco2_pitch_fine: expected 1 argument, got {}",
                args.len()
            )));
        }
        let st = extract_f64(&args[0])?;
        cmds.borrow_mut()
            .push(AudioCommand::SetVco2PitchFine(st));
        Ok(Value::Unit)
    });

    // audio::set_eg1(attack: f64, decay: f64, sustain: f64, release: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_eg1", move |args| {
        if args.len() != 4 {
            return Err(VmError::NativeError(format!(
                "audio::set_eg1: expected 4 arguments, got {}",
                args.len()
            )));
        }
        let a = extract_f64(&args[0])?;
        let d = extract_f64(&args[1])?;
        let s = extract_f64(&args[2])?;
        let r = extract_f64(&args[3])?;
        cmds.borrow_mut().push(AudioCommand::SetEg1Adsr {
            attack: a,
            decay: d,
            sustain: s,
            release: r,
        });
        Ok(Value::Unit)
    });

    // audio::set_eg2(attack: f64, decay: f64, sustain: f64, release: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_eg2", move |args| {
        if args.len() != 4 {
            return Err(VmError::NativeError(format!(
                "audio::set_eg2: expected 4 arguments, got {}",
                args.len()
            )));
        }
        let a = extract_f64(&args[0])?;
        let d = extract_f64(&args[1])?;
        let s = extract_f64(&args[2])?;
        let r = extract_f64(&args[3])?;
        cmds.borrow_mut().push(AudioCommand::SetEg2Adsr {
            attack: a,
            decay: d,
            sustain: s,
            release: r,
        });
        Ok(Value::Unit)
    });

    // audio::set_lfo_rate(rate: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_lfo_rate", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_lfo_rate: expected 1 argument, got {}",
                args.len()
            )));
        }
        let rate = extract_f64(&args[0])?;
        cmds.borrow_mut().push(AudioCommand::SetLfoRate(rate));
        Ok(Value::Unit)
    });

    // audio::set_volume(left: f64, right: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_volume", move |args| {
        if args.len() != 2 {
            return Err(VmError::NativeError(format!(
                "audio::set_volume: expected 2 arguments, got {}",
                args.len()
            )));
        }
        let left = extract_f64(&args[0])?;
        let right = extract_f64(&args[1])?;
        cmds.borrow_mut()
            .push(AudioCommand::SetVolume { left, right });
        Ok(Value::Unit)
    });

    // audio::set_pan(pan: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_pan", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_pan: expected 1 argument, got {}",
                args.len()
            )));
        }
        let pan = extract_f64(&args[0])?;
        cmds.borrow_mut().push(AudioCommand::SetPan(pan));
        Ok(Value::Unit)
    });

    // audio::set_vco1_pulse_width(width: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_vco1_pulse_width", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_vco1_pulse_width: expected 1 argument, got {}",
                args.len()
            )));
        }
        let w = extract_f64(&args[0])?;
        cmds.borrow_mut()
            .push(AudioCommand::SetVco1PulseWidth(w));
        Ok(Value::Unit)
    });

    // audio::set_vco2_pulse_width(width: f64) -> ()
    let cmds = commands.clone();
    vm.register_native_closure("audio::set_vco2_pulse_width", move |args| {
        if args.len() != 1 {
            return Err(VmError::NativeError(format!(
                "audio::set_vco2_pulse_width: expected 1 argument, got {}",
                args.len()
            )));
        }
        let w = extract_f64(&args[0])?;
        cmds.borrow_mut()
            .push(AudioCommand::SetVco2PulseWidth(w));
        Ok(Value::Unit)
    });
}

/// Error from script compilation or loading.
#[derive(Debug, Clone)]
pub struct ScriptError(pub String);

/// A compiled Keleos script runner bound to a single audio channel.
///
/// The runner manages a Keleos VM instance with registered audio native
/// functions. Each frame tick, the host resumes the VM coroutine with
/// transport state. Native function calls during execution accumulate
/// audio commands in a shared buffer. After execution, the host reads
/// the commands and applies them to the audio engine.
pub struct ScriptRunner {
    vm: Vm,
    commands: Rc<RefCell<Vec<AudioCommand>>>,
    started: bool,
    /// Last error from script execution, if any.
    pub last_error: Option<String>,
}

impl ScriptRunner {
    /// Compile a Keleos script source and create a runner.
    ///
    /// Registers both pure audio utility functions (midi_to_freq, etc.)
    /// and audio command functions (note_on, set_lpf_cutoff, etc.).
    pub fn from_source(source: &str) -> Result<Self, ScriptError> {
        let tokens = tokenize(source)
            .map_err(|e| ScriptError(format!("lex error: {:?}", e)))?;
        let program = parse(&tokens)
            .map_err(|e| ScriptError(format!("parse error: {:?}", e)))?;
        let module = compile(&program)
            .map_err(|e| ScriptError(format!("compile error: {:?}", e)))?;

        let mut vm = Vm::new(module);
        let commands: Rc<RefCell<Vec<AudioCommand>>> =
            Rc::new(RefCell::new(Vec::new()));

        // Register pure utility natives.
        keleos::audio_natives::register_audio_natives(&mut vm);
        // Register command-producing natives.
        register_audio_command_natives(&mut vm, &commands);

        Ok(Self {
            vm,
            commands,
            started: false,
            last_error: None,
        })
    }

    /// Tick the script coroutine with transport state.
    ///
    /// The script receives a `TickState` struct with transport position
    /// and timing information. Returns any audio commands produced during
    /// execution. Errors are stored in `last_error` but do not propagate.
    pub fn tick(
        &mut self,
        position_ticks: u64,
        tempo: f64,
        beat: f64,
    ) -> Vec<AudioCommand> {
        let state = Value::Struct {
            type_name: String::from("TickState"),
            fields: alloc::vec![
                (String::from("ticks"), Value::Int(position_ticks as i64)),
                (String::from("tempo"), Value::Float(tempo)),
                (String::from("beat"), Value::Float(beat)),
            ],
        };

        let result = if !self.started {
            self.started = true;
            self.vm.call(&[state])
        } else {
            self.vm.resume(state)
        };

        match result {
            Ok(VmState::Yielded(_)) | Ok(VmState::Finished(_)) => {
                self.last_error = None;
            }
            Err(e) => {
                self.last_error = Some(format!("{:?}", e));
            }
        }

        self.take_commands()
    }

    /// Drain and return all pending audio commands.
    pub fn take_commands(&self) -> Vec<AudioCommand> {
        self.commands.borrow_mut().drain(..).collect()
    }
}

/// Apply a list of audio commands to a specific channel on the engine.
///
/// Each command modifies the synthesizer or channel parameters.
/// Commands are applied in order.
pub fn apply_commands(
    engine: &mut AudioEngine,
    channel: usize,
    commands: &[AudioCommand],
) {
    for cmd in commands {
        match cmd {
            AudioCommand::NoteOn { freq, velocity } => {
                engine.channels[channel]
                    .synth
                    .note_on_with_velocity(*freq, *velocity);
                engine.channels[channel].enabled = true;
            }
            AudioCommand::NoteOff => {
                engine.channels[channel].synth.note_off();
            }
            AudioCommand::NoteOffFreq { freq } => {
                engine.channels[channel].synth.note_off_freq(*freq);
            }
            AudioCommand::SetLpfCutoff(cutoff) => {
                for voice in &mut engine.channels[channel].synth.voices {
                    voice.lpf.cutoff = *cutoff;
                }
            }
            AudioCommand::SetLpfResonance(res) => {
                for voice in &mut engine.channels[channel].synth.voices {
                    voice.lpf.resonance = *res;
                }
            }
            AudioCommand::SetHpfCutoff(cutoff) => {
                for voice in &mut engine.channels[channel].synth.voices {
                    voice.hpf.cutoff = *cutoff;
                }
            }
            AudioCommand::SetHpfResonance(res) => {
                for voice in &mut engine.channels[channel].synth.voices {
                    voice.hpf.resonance = *res;
                }
            }
            AudioCommand::SetVcoMix(mix) => {
                engine.channels[channel].synth.vco_mix = *mix;
            }
            AudioCommand::SetVco1PitchFine(st) => {
                for voice in &mut engine.channels[channel].synth.voices {
                    voice.vco1.pitch_fine = *st;
                }
            }
            AudioCommand::SetVco2PitchFine(st) => {
                for voice in &mut engine.channels[channel].synth.voices {
                    voice.vco2.pitch_fine = *st;
                }
            }
            AudioCommand::SetEg1Adsr {
                attack,
                decay,
                sustain,
                release,
            } => {
                for voice in &mut engine.channels[channel].synth.voices {
                    voice.eg1.attack = *attack;
                    voice.eg1.decay = *decay;
                    voice.eg1.sustain = *sustain;
                    voice.eg1.release = *release;
                }
            }
            AudioCommand::SetEg2Adsr {
                attack,
                decay,
                sustain,
                release,
            } => {
                for voice in &mut engine.channels[channel].synth.voices {
                    voice.eg2.attack = *attack;
                    voice.eg2.decay = *decay;
                    voice.eg2.sustain = *sustain;
                    voice.eg2.release = *release;
                }
            }
            AudioCommand::SetLfoRate(rate) => {
                engine.channels[channel].synth.lfo_rate = *rate;
            }
            AudioCommand::SetVolume { left, right } => {
                engine.channels[channel].set_volume(*left, *right);
            }
            AudioCommand::SetPan(pan) => {
                // Constant-power pan: left = cos(angle), right = sin(angle)
                // where angle goes from 0 (full left) to pi/2 (full right).
                let angle = (*pan + 1.0) * 0.25 * core::f64::consts::PI;
                let left = libm::cos(angle);
                let right = libm::sin(angle);
                engine.channels[channel].set_volume(left, right);
            }
            AudioCommand::SetVco1PulseWidth(w) => {
                for voice in &mut engine.channels[channel].synth.voices {
                    voice.vco1.pulse_width = *w;
                }
            }
            AudioCommand::SetVco2PulseWidth(w) => {
                for voice in &mut engine.channels[channel].synth.voices {
                    voice.vco2.pulse_width = *w;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_runner_simple_yield() {
        let src = concat!(
            "use audio::note_on\n",
            "use audio::midi_to_freq\n",
            "loop main(state: TickState) -> () {\n",
            "  let freq = audio::midi_to_freq(60);\n",
            "  audio::note_on(freq, 0.8);\n",
            "  let state = yield ();\n",
            "  state\n",
            "}\n",
        );
        let mut runner = ScriptRunner::from_source(src).expect("compile failed");
        let cmds = runner.tick(0, 120.0, 0.0);
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            AudioCommand::NoteOn { freq, velocity } => {
                assert!((*freq - 261.63).abs() < 0.01);
                assert!((*velocity - 0.8).abs() < 0.001);
            }
            other => panic!("expected NoteOn, got {:?}", other),
        }
    }

    #[test]
    fn script_runner_multiple_commands() {
        let src = concat!(
            "use audio::note_on\n",
            "use audio::set_lpf_cutoff\n",
            "use audio::midi_to_freq\n",
            "loop main(state: TickState) -> () {\n",
            "  audio::set_lpf_cutoff(2000.0);\n",
            "  let freq = audio::midi_to_freq(69);\n",
            "  audio::note_on(freq, 1.0);\n",
            "  let state = yield ();\n",
            "  state\n",
            "}\n",
        );
        let mut runner = ScriptRunner::from_source(src).expect("compile failed");
        let cmds = runner.tick(0, 120.0, 0.0);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0], AudioCommand::SetLpfCutoff(2000.0));
        match &cmds[1] {
            AudioCommand::NoteOn { freq, velocity } => {
                assert!((*freq - 440.0).abs() < 0.01);
                assert!((*velocity - 1.0).abs() < 0.001);
            }
            other => panic!("expected NoteOn, got {:?}", other),
        }
    }

    #[test]
    fn script_runner_resume_produces_commands() {
        let src = concat!(
            "use audio::set_lpf_cutoff\n",
            "loop main(state: TickState) -> () {\n",
            "  audio::set_lpf_cutoff(state.ticks as f64);\n",
            "  let state = yield ();\n",
            "  state\n",
            "}\n",
        );
        let mut runner = ScriptRunner::from_source(src).expect("compile failed");

        let cmds1 = runner.tick(100, 120.0, 0.0);
        assert_eq!(cmds1, alloc::vec![AudioCommand::SetLpfCutoff(100.0)]);

        let cmds2 = runner.tick(200, 120.0, 0.0);
        assert_eq!(cmds2, alloc::vec![AudioCommand::SetLpfCutoff(200.0)]);

        let cmds3 = runner.tick(300, 120.0, 0.0);
        assert_eq!(cmds3, alloc::vec![AudioCommand::SetLpfCutoff(300.0)]);
    }

    #[test]
    fn script_runner_error_stored() {
        // Script that finishes after first call (non-loop).
        // tick() passes a TickState argument, so the script must accept one parameter.
        let src = "fn main(state: TickState) -> () { () }";
        let mut runner = ScriptRunner::from_source(src).expect("compile failed");

        // First tick: should finish successfully.
        let _ = runner.tick(0, 120.0, 0.0);

        // Second tick: resume on a finished VM should produce an error.
        let _ = runner.tick(1, 120.0, 0.0);
        assert!(runner.last_error.is_some());
    }

    #[test]
    fn apply_commands_note_on() {
        let mut engine = AudioEngine::new(48000);
        let cmds = alloc::vec![AudioCommand::NoteOn {
            freq: 440.0,
            velocity: 0.8,
        }];
        apply_commands(&mut engine, 0, &cmds);
        assert!(engine.channels[0].enabled);
    }

    #[test]
    fn apply_commands_lpf_cutoff() {
        let mut engine = AudioEngine::new(48000);
        let cmds = alloc::vec![AudioCommand::SetLpfCutoff(1500.0)];
        apply_commands(&mut engine, 0, &cmds);
        for voice in &engine.channels[0].synth.voices {
            assert!((voice.lpf.cutoff - 1500.0).abs() < 0.001);
        }
    }

    #[test]
    fn apply_commands_vco_mix() {
        let mut engine = AudioEngine::new(48000);
        let cmds = alloc::vec![AudioCommand::SetVcoMix(0.7)];
        apply_commands(&mut engine, 0, &cmds);
        assert!((engine.channels[0].synth.vco_mix - 0.7).abs() < 0.001);
    }

    #[test]
    fn apply_commands_eg1() {
        let mut engine = AudioEngine::new(48000);
        let cmds = alloc::vec![AudioCommand::SetEg1Adsr {
            attack: 0.01,
            decay: 0.2,
            sustain: 0.5,
            release: 0.3,
        }];
        apply_commands(&mut engine, 0, &cmds);
        for voice in &engine.channels[0].synth.voices {
            assert!((voice.eg1.attack - 0.01).abs() < 0.001);
            assert!((voice.eg1.sustain - 0.5).abs() < 0.001);
        }
    }

    #[test]
    fn compile_error_reported() {
        let result = ScriptRunner::from_source("this is not valid keleos");
        assert!(result.is_err());
    }
}
