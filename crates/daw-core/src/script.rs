//! Keleusma script integration for per-channel audio control.
//!
//! Targets the published `keleusma` 0.2.2 crate. Scripts must follow the
//! 0.2 surface language, which uses the `Word` and `Float` numeric type
//! names in place of the removed `i64` and `f64` names. Each tick the host
//! passes a `TickState` struct to the script's entry point.
//!
//! # Entry contract
//!
//! The entry function `main` takes exactly one parameter annotated as
//! `TickState`. A script that reads its fields declares it exactly as
//! follows, with these field names, types and order.
//!
//! ```text
//! struct TickState { ticks: Word, tempo: Float, beat: Float }
//! ```
//!
//! An undeclared `TickState` is resolved by the 0.2 type checker as an
//! opaque host type. Such a script loads, and field access on it fails to
//! compile.
//!
//! The contract is checked at load, because Keleusma 0.2.2 does not check
//! it. The host struct is packed into a flat body in field order without
//! its field names, and the compiler resolves field access by the offsets
//! of the script's own declaration. A declaration that differs from the
//! host layout, under any struct name, therefore reads the wrong bytes as
//! the declared types and reports no error. A reordered declaration was
//! observed to read the tick count as `4.598e18`. `ScriptRunner::from_source`
//! rejects any entry signature or `TickState` declaration that differs from
//! the contract, rather than letting it misread values during playback.
//!
//! # Native arguments
//!
//! Script values reach the audio engine only through the `audio::*`
//! command natives. Each argument must be a finite number. A NaN or an
//! infinity is rejected as a script error, because a non-finite value
//! survives the engine's clamps and could persist in filter state.
//!
//! # Execution budget
//!
//! `ScriptRunner::tick` runs on the interface thread once per frame, so a
//! script that runs long between two yields freezes the composer. Keleusma
//! 0.2.2 rejects a loop function whose body can reach the next iteration
//! without a yield, a loop without a bound and a recursive call. It still
//! admits a bounded loop of any length whose estimated cost fits in a
//! `u32`, such as `for i in 0..10000000`, which was measured to hold one
//! tick for 34 seconds in a debug build. Its cost estimate is also not a
//! safe limit to check against. A `fn main` or `yield main` reports no
//! estimate, and a loop moved into a helper function was reported as 19
//! cycles while it held each tick for 450 milliseconds.
//!
//! The runner therefore counts steps itself. It arms a VM breakpoint on
//! every loop back edge, on the first op of every function and on every
//! native call, and one `tick` may pass at most [`MAX_STEPS_PER_TICK`] of
//! them. Loop back edges and the return to the top of a loop function are
//! the only backward jumps in the bytecode, and the latter is capped
//! separately, so between two steps the VM runs at most one function's
//! straight-line code without a call of any kind. The count therefore
//! bounds the work of a tick, and the number of commands it queues,
//! whatever the cost estimate says. A script that exceeds the budget is stopped for good, with an
//! error in `last_error`, because the VM cannot be rewound to the start of
//! the interrupted iteration. Reloading the script starts it again.
//!
//! Known defect, not mitigated. For a loop whose estimated cost overflows,
//! such as `for i in 0..1000000000`, `Vm::new` in Keleusma 0.2.2 panics in
//! a build with overflow checks, at an unchecked addition in its WCET pass,
//! so loading such a script aborts a debug build of the composer. A
//! `no_std` crate cannot catch the panic. A release build does not check
//! the overflow, so the wrapped estimate admits the script, and the step
//! budget was observed to stop it within 3 milliseconds.

extern crate alloc;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use audio::AudioEngine;
use keleusma::Arena;
use keleusma::ast::{PrimType, Program, StructDef, TypeDef, TypeExpr};
use keleusma::bytecode::{Module, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for,
};

/// An audio command produced by a Keleusma script during execution.
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
    SetEg1Adsr {
        attack: f64,
        decay: f64,
        sustain: f64,
        release: f64,
    },
    /// Set envelope generator 2 parameters (attack, decay, sustain, release).
    SetEg2Adsr {
        attack: f64,
        decay: f64,
        sustain: f64,
        release: f64,
    },
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

/// Name of the struct the host passes to the script entry point.
const TICK_STATE: &str = "TickState";

/// Name of the script entry function, fixed by the Keleusma compiler.
const ENTRY_FUNCTION: &str = "main";

/// Fields of `TickState` in the order the host packs them.
///
/// `ScriptRunner::tick` builds the struct from these names in this order,
/// and `check_tick_state_decl` requires a script declaration to match it.
/// See the module documentation for why a mismatch must be rejected.
const TICK_STATE_FIELDS: [(&str, PrimType); 3] = [
    ("ticks", PrimType::Word),
    ("tempo", PrimType::Float),
    ("beat", PrimType::Float),
];

/// The most steps, loop iterations, function calls and native calls, that
/// one `ScriptRunner::tick` may run. See the module documentation.
///
/// A tick of an empty loop just under the budget was measured on the
/// authoring host at 12 milliseconds in a debug build and 0.5 milliseconds
/// in a release build, against a frame of about 16 milliseconds. That is
/// far more than a script that sets parameters or walks a short pattern
/// needs. A script that needs more would have to spread its work over
/// several ticks.
pub const MAX_STEPS_PER_TICK: u32 = 4096;

/// The most `Reset` states one `tick` resumes through. A loop function
/// reaches `Reset` at the end of each iteration, and its productivity check
/// guarantees a yield in each, so a well-formed script stops well short.
const MAX_RESETS_PER_TICK: u32 = 16;

/// The positions where a breakpoint counts one step. These are the first
/// op of every chunk, which each call enters, every loop back edge, and
/// every native call. A native call runs host code and, for the `audio::`
/// functions, queues a command, so a straight-line body of many native
/// calls is counted as well, though it has no loop and no script call.
fn step_positions(module: &Module) -> Vec<(usize, usize)> {
    let mut positions = Vec::new();
    for (chunk_idx, chunk) in module.chunks.iter().enumerate() {
        positions.push((chunk_idx, 0));
        for (op_idx, op) in chunk.ops.iter().enumerate() {
            if matches!(
                op,
                Op::EndLoop(_) | Op::CallVerifiedNative(..) | Op::CallExternalNative(..)
            ) {
                positions.push((chunk_idx, op_idx));
            }
        }
    }
    positions
}

/// Build a contract violation error with a common prefix.
fn contract_error(detail: &str) -> ScriptError {
    ScriptError(format!("contract error: {detail}"))
}

/// Check a script's own `TickState` declaration against the host layout.
fn check_tick_state_decl(def: &StructDef) -> Result<(), ScriptError> {
    let matches_layout = def.type_params.is_empty()
        && def.const_params.is_empty()
        && def.fields.len() == TICK_STATE_FIELDS.len()
        && def
            .fields
            .iter()
            .zip(TICK_STATE_FIELDS.iter())
            .all(|(field, (name, prim))| {
                field.name == *name && matches!(&field.type_expr, TypeExpr::Prim(p, _) if p == prim)
            });
    if matches_layout {
        Ok(())
    } else {
        Err(contract_error(
            "TickState must be declared exactly as \
             `struct TickState { ticks: Word, tempo: Float, beat: Float }`",
        ))
    }
}

/// Check the entry contract described in the module documentation.
///
/// Rejects a `TickState` declaration that differs from the host layout, a
/// `TickState` that is declared as an enum or newtype, and an entry
/// function whose parameters are anything other than one `TickState`.
/// An undeclared `TickState` is accepted, since field access on it fails
/// to compile and nothing can be misread.
fn check_entry_contract(program: &Program) -> Result<(), ScriptError> {
    for def in &program.types {
        match def {
            TypeDef::Struct(s) if s.name == TICK_STATE => check_tick_state_decl(s)?,
            TypeDef::Enum(e) if e.name == TICK_STATE => {
                return Err(contract_error("TickState must be a struct, not an enum"));
            }
            TypeDef::Newtype(n) if n.name == TICK_STATE => {
                return Err(contract_error("TickState must be a struct, not a newtype"));
            }
            _ => {}
        }
    }

    let entry = program
        .functions
        .iter()
        .find(|f| f.name == ENTRY_FUNCTION)
        .ok_or_else(|| contract_error("the script has no `main` entry function"))?;
    let takes_tick_state = match entry.params.as_slice() {
        [param] => matches!(
            &param.type_expr,
            Some(TypeExpr::Named(name, args, consts, _))
                if name == TICK_STATE && args.is_empty() && consts.is_empty()
        ),
        _ => false,
    };
    if takes_tick_state {
        Ok(())
    } else {
        Err(contract_error(
            "`main` must take exactly one parameter annotated as `TickState`",
        ))
    }
}

/// Extract a finite f64 from a Keleusma Value, accepting both Float and Int.
///
/// Every command native reads its arguments through this function, so it
/// is the single boundary where script values enter the audio engine. NaN
/// and infinities are rejected, as the module documentation explains. An
/// Int always converts to a finite f64, rounding above 2^53.
fn extract_f64(val: &Value) -> Result<f64, VmError> {
    match val {
        Value::Float(f) if f.is_finite() => Ok(*f),
        Value::Float(f) => Err(VmError::NativeError(format!(
            "expected a finite number, got {f}"
        ))),
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
fn register_audio_command_natives(vm: &mut Vm<'_, '_>, commands: &Rc<RefCell<Vec<AudioCommand>>>) {
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
        cmds.borrow_mut().push(AudioCommand::SetHpfResonance(res));
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
        cmds.borrow_mut().push(AudioCommand::SetVco1PitchFine(st));
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
        cmds.borrow_mut().push(AudioCommand::SetVco2PitchFine(st));
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
        cmds.borrow_mut().push(AudioCommand::SetVco1PulseWidth(w));
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
        cmds.borrow_mut().push(AudioCommand::SetVco2PulseWidth(w));
        Ok(Value::Unit)
    });
}

/// Why one `ScriptRunner::tick` ended without a yield or a finish.
enum TickFailure {
    /// The VM reported an error.
    Vm(String),
    /// The script exceeded [`MAX_STEPS_PER_TICK`].
    OverBudget,
}

/// Error from script compilation or loading.
#[derive(Debug, Clone)]
pub struct ScriptError(pub String);

/// A Keleusma VM bound to the lifetime of the arena it borrows.
///
/// The bytecode lifetime is `'static` because the VM owns its module
/// bytes when constructed through `Vm::new`.
type ScriptVm<'arena> = Vm<'static, 'arena>;

// Keleusma 0.2 VMs borrow a host-owned arena. Each runner owns its own
// arena so that dropping a runner, for example on hot reload, releases
// the arena memory. Sharing one long-lived arena across successive VMs
// is not viable because `Vm::new` does not rewind the arena and a safe
// rewind requires exclusive access. `self_cell` provides the
// self-referential owner and dependent pair without local unsafe code.
self_cell::self_cell!(
    struct ScriptVmCell {
        owner: Arena,

        #[not_covariant]
        dependent: ScriptVm,
    }
);

/// A compiled Keleusma script runner bound to a single audio channel.
///
/// The runner manages a Keleusma VM instance with registered audio native
/// functions. Each frame tick, the host resumes the VM coroutine with
/// transport state. Native function calls during execution accumulate
/// audio commands in a shared buffer. After execution, the host reads
/// the commands and applies them to the audio engine.
pub struct ScriptRunner {
    vm: ScriptVmCell,
    commands: Rc<RefCell<Vec<AudioCommand>>>,
    started: bool,
    /// Set once the script exceeds [`MAX_STEPS_PER_TICK`]. A stopped
    /// runner never runs the VM again.
    stopped: bool,
    /// Last error from script execution, if any.
    pub last_error: Option<String>,
}

impl ScriptRunner {
    /// Compile a Keleusma script source and create a runner.
    ///
    /// Registers both pure audio utility functions (midi_to_freq, etc.)
    /// and audio command functions (note_on, set_lpf_cutoff, etc.).
    pub fn from_source(source: &str) -> Result<Self, ScriptError> {
        let tokens = tokenize(source).map_err(|e| ScriptError(format!("lex error: {:?}", e)))?;
        let program = parse(&tokens).map_err(|e| ScriptError(format!("parse error: {:?}", e)))?;
        let module =
            compile(&program).map_err(|e| ScriptError(format!("compile error: {:?}", e)))?;
        // Checked after compilation, so that an ill-formed script reports
        // its compile error first.
        check_entry_contract(&program)?;

        // The default capacity bounds the operand stack, frames, and
        // dynamic strings. `Vm::new` verifies the module's worst-case
        // memory usage against it and rejects an oversized script here
        // rather than failing mid-playback.
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        arena
            .resize_persistent(required_persistent_capacity_for(&module))
            .map_err(|e| ScriptError(format!("arena error: {:?}", e)))?;

        let commands: Rc<RefCell<Vec<AudioCommand>>> = Rc::new(RefCell::new(Vec::new()));
        let steps = step_positions(&module);
        let vm = ScriptVmCell::try_new(arena, |arena| {
            let mut vm = Vm::new(module, arena)
                .map_err(|e| ScriptError(format!("verify error: {:?}", e)))?;
            for (chunk, op) in steps {
                vm.set_breakpoint(chunk, op);
            }
            // Register pure utility natives.
            keleusma::audio_natives::register_audio_natives(&mut vm);
            // Register command-producing natives.
            register_audio_command_natives(&mut vm, &commands);
            Ok::<_, ScriptError>(vm)
        })?;

        Ok(Self {
            vm,
            commands,
            started: false,
            stopped: false,
            last_error: None,
        })
    }

    /// Tick the script coroutine with transport state.
    ///
    /// The script receives a `TickState` struct with transport position
    /// and timing information. Returns any audio commands produced during
    /// execution. Errors are stored in `last_error` but do not propagate.
    ///
    /// A `Word` is a signed 64-bit integer, so a position above `i64::MAX`
    /// saturates rather than wrapping to a negative tick count.
    ///
    /// A tick runs at most [`MAX_STEPS_PER_TICK`] steps. A script that needs
    /// more is stopped, keeps its error in `last_error`, and returns no
    /// commands from then on, including any it produced in the tick that
    /// stopped it, because those come from an iteration that did not finish.
    pub fn tick(&mut self, position_ticks: u64, tempo: f64, beat: f64) -> Vec<AudioCommand> {
        if self.stopped {
            return Vec::new();
        }
        let ticks = i64::try_from(position_ticks).unwrap_or(i64::MAX);
        let [(ticks_name, _), (tempo_name, _), (beat_name, _)] = TICK_STATE_FIELDS;
        let state = Value::struct_value(
            String::from(TICK_STATE),
            alloc::vec![
                (String::from(ticks_name), Value::Int(ticks)),
                (String::from(tempo_name), Value::Float(tempo)),
                (String::from(beat_name), Value::Float(beat)),
            ],
        );

        // Drive the VM until it yields, finishes, or errors. A loop
        // function emits VmState::Reset between iterations to clear its
        // arena. The host resumes with the same input on Reset so that
        // the next iteration executes within the same host tick. Each
        // breakpoint is one step, after which the VM continues from it.
        let first_call = !self.started;
        self.started = true;
        // The VM result is reduced to an owned outcome inside the closure
        // because yielded values may reference arena memory.
        let outcome: Result<(), TickFailure> = self.vm.with_dependent_mut(|_, vm| {
            let mut result = if first_call {
                vm.call(core::slice::from_ref(&state))
            } else {
                vm.resume(state.clone())
            };
            let mut steps = 0u32;
            let mut resets = 0u32;
            // Each pass consumes one step or one reset, or ends the loop,
            // so it ends within MAX_STEPS_PER_TICK + MAX_RESETS_PER_TICK
            // passes.
            loop {
                match result {
                    Ok(VmState::BreakpointHit { .. }) => {
                        if steps == MAX_STEPS_PER_TICK {
                            return Err(TickFailure::OverBudget);
                        }
                        steps += 1;
                        result = vm.resume_from_breakpoint();
                    }
                    Ok(VmState::Reset) if resets < MAX_RESETS_PER_TICK => {
                        resets += 1;
                        result = vm.resume(state.clone());
                    }
                    Ok(VmState::Yielded(_)) | Ok(VmState::Finished(_)) | Ok(VmState::Reset) => {
                        return Ok(());
                    }
                    Err(e) => return Err(TickFailure::Vm(format!("{:?}", e))),
                }
            }
        });
        match outcome {
            Ok(()) => self.last_error = None,
            Err(TickFailure::Vm(e)) => self.last_error = Some(e),
            Err(TickFailure::OverBudget) => {
                self.stopped = true;
                self.last_error = Some(format!(
                    "stopped, the script ran more than {MAX_STEPS_PER_TICK} loop iterations \
                     and calls, native calls included, in one tick"
                ));
                self.commands.borrow_mut().clear();
            }
        }

        self.take_commands()
    }

    /// Whether the runner was stopped for exceeding [`MAX_STEPS_PER_TICK`].
    pub fn is_stopped(&self) -> bool {
        self.stopped
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
pub fn apply_commands(engine: &mut AudioEngine, channel: usize, commands: &[AudioCommand]) {
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
            "struct TickState { ticks: Word, tempo: Float, beat: Float }\n",
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
            "struct TickState { ticks: Word, tempo: Float, beat: Float }\n",
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
            "struct TickState { ticks: Word, tempo: Float, beat: Float }\n",
            "loop main(state: TickState) -> () {\n",
            "  audio::set_lpf_cutoff(state.ticks as Float);\n",
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
        let src = concat!(
            "struct TickState { ticks: Word, tempo: Float, beat: Float }\n",
            "fn main(state: TickState) -> () { () }\n",
        );
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
    fn script_reads_every_tick_state_field() {
        // Verifies that the host-built TickState matches the declared
        // field order and types for all three fields.
        let src = concat!(
            "use audio::set_lpf_cutoff\n",
            "use audio::set_lpf_resonance\n",
            "use audio::set_vco_mix\n",
            "struct TickState { ticks: Word, tempo: Float, beat: Float }\n",
            "loop main(state: TickState) -> () {\n",
            "  audio::set_lpf_cutoff(state.ticks as Float);\n",
            "  audio::set_lpf_resonance(state.tempo);\n",
            "  audio::set_vco_mix(state.beat);\n",
            "  let state = yield ();\n",
            "  state\n",
            "}\n",
        );
        let mut runner = ScriptRunner::from_source(src).expect("compile failed");
        let cmds = runner.tick(960, 132.5, 0.25);
        assert_eq!(runner.last_error, None);
        assert_eq!(
            cmds,
            alloc::vec![
                AudioCommand::SetLpfCutoff(960.0),
                AudioCommand::SetLpfResonance(132.5),
                AudioCommand::SetVcoMix(0.25),
            ]
        );
    }

    #[test]
    fn undeclared_tick_state_field_access_rejected() {
        // Keleusma 0.2 resolves an undeclared type name as opaque.
        let src = concat!(
            "use audio::set_lpf_cutoff\n",
            "loop main(state: TickState) -> () {\n",
            "  audio::set_lpf_cutoff(state.tempo);\n",
            "  let state = yield ();\n",
            "  state\n",
            "}\n",
        );
        // Only the stage prefix, which this module writes, and the type
        // name are asserted, so a reworded Keleusma message does not break
        // the test.
        let err = ScriptRunner::from_source(src)
            .err()
            .expect("compile succeeded");
        assert!(
            err.0.starts_with("compile error") && err.0.contains(TICK_STATE),
            "{}",
            err.0
        );
    }

    #[test]
    fn undeclared_tick_state_without_field_access_accepted() {
        // A script that never reads TickState need not declare it.
        let src = concat!(
            "use audio::set_vco_mix\n",
            "loop main(state: TickState) -> () {\n",
            "  audio::set_vco_mix(0.5);\n",
            "  let state = yield ();\n",
            "  state\n",
            "}\n",
        );
        let mut runner = ScriptRunner::from_source(src).expect("compile failed");
        assert_eq!(
            runner.tick(0, 120.0, 0.0),
            alloc::vec![AudioCommand::SetVcoMix(0.5)]
        );
        assert_eq!(runner.last_error, None);
    }

    #[test]
    fn legacy_numeric_type_names_rejected() {
        // Keleusma 0.2 removed the `i64` and `f64` surface type names.
        let src = concat!(
            "struct TickState { ticks: i64, tempo: f64, beat: f64 }\n",
            "fn main(state: TickState) -> () { () }\n",
        );
        let err = ScriptRunner::from_source(src)
            .err()
            .expect("compile succeeded");
        assert!(err.0.starts_with("parse error"), "{}", err.0);
    }

    /// Wrap a declaration and entry signature into a minimal script.
    fn script_with(decl: &str, signature: &str) -> String {
        format!("{decl}fn main{signature} -> () {{ () }}\n")
    }

    /// Assert that a script is rejected by the entry contract check.
    fn assert_contract_rejected(src: &str) {
        match ScriptRunner::from_source(src) {
            Ok(_) => panic!("script was accepted:\n{src}"),
            Err(e) => assert!(e.0.starts_with("contract error"), "{}\n{src}", e.0),
        }
    }

    #[test]
    fn tick_state_declaration_mismatch_rejected() {
        // Each of these loaded before the contract check. The first three
        // were observed to misread values silently under keleusma 0.2.2.
        let declarations = [
            "struct TickState { beat: Float, tempo: Float, ticks: Word }\n",
            "struct TickState { ticks: Word, beat: Float, tempo: Float }\n",
            "struct TickState { ticks: Float, tempo: Float, beat: Float }\n",
            "struct TickState { ticks: Word, tempo: Float }\n",
            "struct TickState { ticks: Word, tempo: Float, beat: Float, bar: Float }\n",
            "struct TickState { tick: Word, tempo: Float, beat: Float }\n",
        ];
        for decl in declarations {
            assert_contract_rejected(&script_with(decl, "(state: TickState)"));
        }
    }

    #[test]
    fn tick_state_non_struct_rejected() {
        assert_contract_rejected(&script_with(
            "newtype TickState = Float;\n",
            "(state: TickState)",
        ));
        assert_contract_rejected(&script_with(
            "enum TickState { A, B }\n",
            "(state: TickState)",
        ));
    }

    #[test]
    fn entry_signature_mismatch_rejected() {
        let decl = "struct TickState { ticks: Word, tempo: Float, beat: Float }\n";
        // A correctly laid out struct under another name is rejected too,
        // because the check cannot tell which struct is meant as the host
        // state once the name differs.
        let other = "struct Other { ticks: Word, tempo: Float, beat: Float }\n";
        let cases = [
            (decl, "()"),
            (decl, "(a: TickState, b: TickState)"),
            (decl, "(state)"),
            (decl, "(state: Word)"),
            (decl, "(state: (Word, Float, Float))"),
            (other, "(state: Other)"),
        ];
        for (d, signature) in cases {
            assert_contract_rejected(&script_with(d, signature));
        }
    }

    #[test]
    fn non_finite_native_argument_rejected() {
        for expr in ["0.0 / 0.0", "1.0 / 0.0", "-1.0 / 0.0"] {
            let src = format!(
                "use audio::set_lpf_cutoff\n\
                 loop main(state: TickState) -> () {{\n\
                 \x20 audio::set_lpf_cutoff({expr});\n\
                 \x20 let state = yield ();\n\
                 \x20 state\n\
                 }}\n"
            );
            let mut runner = ScriptRunner::from_source(&src).expect("compile failed");
            let cmds = runner.tick(0, 120.0, 0.0);
            assert!(cmds.is_empty(), "{expr} produced {cmds:?}");
            let err = runner.last_error.expect("no error recorded");
            assert!(err.contains("finite"), "{expr}: {err}");
        }
    }

    #[test]
    fn tick_count_above_word_range_saturates() {
        let src = concat!(
            "use audio::set_lpf_cutoff\n",
            "struct TickState { ticks: Word, tempo: Float, beat: Float }\n",
            "loop main(state: TickState) -> () {\n",
            "  audio::set_lpf_cutoff(state.ticks as Float);\n",
            "  let state = yield ();\n",
            "  state\n",
            "}\n",
        );
        let mut runner = ScriptRunner::from_source(src).expect("compile failed");
        let boundary = u64::try_from(i64::MAX).expect("i64::MAX fits in u64");
        for position in [boundary, boundary + 1, u64::MAX] {
            assert_eq!(
                runner.tick(position, 120.0, 0.0),
                alloc::vec![AudioCommand::SetLpfCutoff(i64::MAX as f64)],
                "position {position}"
            );
        }
    }

    #[test]
    fn compile_error_reported() {
        let result = ScriptRunner::from_source("this is not valid keleusma");
        assert!(result.is_err());
    }

    /// A loop function whose body runs `body` once per tick and then yields.
    fn loop_script(uses: &str, body: &str) -> String {
        format!(
            "{uses}loop main(state: TickState) -> () {{\n  {body}\n  let state = yield ();\n  state\n}}\n"
        )
    }

    /// `for i in 0..n` around a call that produces one command per pass.
    fn volume_loop(n: u32) -> String {
        loop_script(
            "use audio::set_volume\n",
            &format!("for i in 0..{n} {{ audio::set_volume(0.5, 0.5) }}"),
        )
    }

    /// Functions `f0` to `f{depth}` where each calls the next twice, so a
    /// call of `f0` makes 2^(depth + 1) - 1 calls without any loop.
    fn call_tree(depth: u32) -> String {
        let mut src = format!("fn f{depth}(x: Word) -> Word {{ x }}\n");
        for d in (0..depth).rev() {
            src += &format!(
                "fn f{d}(x: Word) -> Word {{ f{n}(x) + f{n}(x) }}\n",
                n = d + 1
            );
        }
        src + &loop_script("", "f0(1);")
    }

    fn assert_stopped(runner: &ScriptRunner) {
        assert!(runner.is_stopped());
        let err = runner.last_error.as_deref().expect("an error is reported");
        assert!(err.starts_with("stopped"), "{err}");
    }

    #[test]
    fn a_loop_within_the_budget_runs_every_tick() {
        // Each pass costs two steps, its back edge and its native call.
        let n = MAX_STEPS_PER_TICK / 4;
        let mut runner = ScriptRunner::from_source(&volume_loop(n)).expect("load");
        for tick in 0..3 {
            let cmds = runner.tick(tick, 120.0, 0.0);
            assert_eq!(cmds.len(), n as usize, "tick {tick}");
            assert_eq!(runner.last_error, None);
        }
        assert!(!runner.is_stopped());
    }

    /// Keleusma admits a bounded loop of any length whose estimated cost
    /// fits in a `u32`. Before the step budget, this loop held one tick for
    /// about 3.4 seconds in a debug build, and one of 10,000,000 for 34.
    #[test]
    fn a_long_bounded_loop_is_stopped_instead_of_stalling_the_tick() {
        let mut runner = ScriptRunner::from_source(&volume_loop(1_000_000)).expect("load");
        let cmds = runner.tick(0, 120.0, 0.0);
        // The commands of the unfinished iteration are discarded.
        assert!(cmds.is_empty());
        assert_stopped(&runner);
        // A stopped script does not run again, and keeps its error.
        assert!(runner.tick(1, 120.0, 0.0).is_empty());
        assert_stopped(&runner);
    }

    #[test]
    fn the_budget_is_counted_per_tick() {
        // Each tick alone fits, though all ticks together exceed the budget.
        // Each pass costs two steps, its back edge and its native call.
        let n = MAX_STEPS_PER_TICK / 4;
        let mut runner = ScriptRunner::from_source(&volume_loop(n)).expect("load");
        for tick in 0..4 {
            assert_eq!(runner.tick(tick, 120.0, 0.0).len(), n as usize);
        }
        assert!(!runner.is_stopped());
        let mut over = ScriptRunner::from_source(&volume_loop(2 * MAX_STEPS_PER_TICK)).unwrap();
        over.tick(0, 120.0, 0.0);
        assert_stopped(&over);
    }

    /// Keleusma reported this script's cost as 19 cycles, because its
    /// estimate leaves out the loop inside the helper.
    #[test]
    fn a_loop_inside_a_helper_function_is_counted() {
        let src = format!(
            "fn spin(x: Word) -> Word {{\n  for i in 0..{} {{ () }}\n  x\n}}\n{}",
            2 * MAX_STEPS_PER_TICK,
            loop_script("", "spin(1);")
        );
        let mut runner = ScriptRunner::from_source(&src).expect("load");
        runner.tick(0, 120.0, 0.0);
        assert_stopped(&runner);
    }

    /// Keleusma reports no cost at all for a `fn main`.
    #[test]
    fn a_long_loop_in_fn_main_is_stopped() {
        let src = format!(
            "fn main(state: TickState) -> () {{\n  for i in 0..{} {{ () }}\n  ()\n}}\n",
            2 * MAX_STEPS_PER_TICK
        );
        let mut runner = ScriptRunner::from_source(&src).expect("load");
        runner.tick(0, 120.0, 0.0);
        assert_stopped(&runner);
    }

    /// Calls are counted as well as loop passes, because a tree of calls
    /// grows exponentially with no loop at all.
    #[test]
    fn calls_are_counted_as_steps() {
        let max_depth = MAX_STEPS_PER_TICK.ilog2();
        let mut within = ScriptRunner::from_source(&call_tree(max_depth - 2)).expect("load");
        within.tick(0, 120.0, 0.0);
        assert_eq!(within.last_error, None);
        assert!(!within.is_stopped());

        let mut over = ScriptRunner::from_source(&call_tree(max_depth)).expect("load");
        over.tick(0, 120.0, 0.0);
        assert_stopped(&over);
    }

    /// A loop body with no yield, one that can skip its yield, an unbounded
    /// loop and recursion are all rejected at load, so none of them can
    /// reach `tick`. Each is checked for its own reason, so that a script
    /// rejected for an unrelated fault, such as a typing error, would fail
    /// the test. The unbounded loop is rejected because its cost estimate
    /// overflows, not by a check aimed at unbounded loops.
    #[test]
    fn scripts_that_never_yield_are_rejected_at_load() {
        let cases = [
            (
                "loop main(state: TickState) -> () {\n  ()\n}\n",
                "Stream block must contain at least one Yield",
            ),
            (
                "struct TickState { ticks: Word, tempo: Float, beat: Float }\n\
                 loop main(state: TickState) -> () {\n  \
                 if state.ticks > 100 { let s = yield (); () } else { () }\n}\n",
                "some path from Stream to Reset does not pass through any Yield",
            ),
            (
                "loop main(state: TickState) -> () {\n  loop { () };\n  let state = yield ();\n  state\n}\n",
                "WCET (cycles) overflowed",
            ),
            (
                "fn f(x: Word) -> Word { f(x) }\n\
                 loop main(state: TickState) -> () {\n  f(1);\n  let state = yield ();\n  state\n}\n",
                "recursive call detected",
            ),
        ];
        for (src, reason) in cases {
            let err = ScriptRunner::from_source(src)
                .err()
                .unwrap_or_else(|| panic!("accepted:\n{src}"));
            assert!(err.0.contains(reason), "{} for\n{src}", err.0);
        }
    }

    /// A body of `n` native calls in a row, with no loop and no script call.
    fn straight_line_natives(n: u32) -> String {
        let body = "audio::set_volume(0.5, 0.5);\n  ".repeat(n as usize);
        loop_script("use audio::set_volume\n", &body)
    }

    /// Native calls are counted as steps, so a long straight-line body of
    /// them cannot escape the budget the way it did when only loops and
    /// script calls were counted. The control just under the budget runs,
    /// counting the entry of `main` as one step.
    #[test]
    fn native_calls_are_counted_as_steps() {
        let fits = MAX_STEPS_PER_TICK - 1;
        let mut within = ScriptRunner::from_source(&straight_line_natives(fits)).expect("load");
        assert_eq!(within.tick(0, 120.0, 0.0).len(), fits as usize);
        assert_eq!(within.last_error, None);
        assert!(!within.is_stopped());

        let mut over = ScriptRunner::from_source(&straight_line_natives(MAX_STEPS_PER_TICK + 1))
            .expect("load");
        assert!(over.tick(0, 120.0, 0.0).is_empty());
        assert_stopped(&over);
    }
}
