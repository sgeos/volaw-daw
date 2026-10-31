//! Configuration of the `audio` engine from DAW data.
//!
//! These functions translate the string-keyed configuration that projects and
//! presets store into engine state. They are pure mutations of an
//! `AudioEngine`, apart from `load_pcm_samples`, which reads through the
//! `Platform` trait, so the DAW and any other host of the engine share one
//! interpretation of a patch.
//!
//! A channel index outside `0..NUM_CHANNELS` is ignored rather than panicking,
//! matching `audio::command::apply_engine_command`. Unknown names fall back to
//! documented defaults, because project files are user-editable text.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use audio::command::{EngineCommand, EngineEdit};
use audio::effects::Pedal;
use audio::modmatrix::{ModDest, ModRouting, ModSource};
use audio::vco::{Scale, Waveform};
use audio::{AudioEngine, ModMatrix, Ms20Voice, NUM_CHANNELS, PedalChain, SingleVoice, VoiceMode};

use crate::patch::{EffectChainConfig, PatchConfig, PedalConfig};
use crate::platform::{LogLevel, Platform};
use crate::project::DawProject;
use crate::script::AudioCommand;
use crate::wav::decode_wav;

/// The largest voice pool a patch may ask for. `PatchEdit` reserves storage
/// for this many voices, so it must bound `parse_voice_mode`.
pub const MAX_PATCH_VOICES: u8 = 10;

/// Parse a waveform name string into the audio engine Waveform enum.
pub fn parse_waveform(s: &str) -> Waveform {
    match s {
        "triangle" => Waveform::Triangle,
        "sawtooth" => Waveform::Sawtooth,
        "square" => Waveform::PwSquare,
        "sine" => Waveform::Sine,
        "noise" => Waveform::WhiteNoise,
        "pcm" => Waveform::Pcm,
        _ => Waveform::Sawtooth,
    }
}

/// Parse an organ pipe scale string into the audio engine Scale enum.
pub fn parse_scale(s: &str) -> Scale {
    match s {
        "32'" => Scale::Feet32,
        "16'" => Scale::Feet16,
        "8'" => Scale::Feet8,
        "4'" => Scale::Feet4,
        "2'" => Scale::Feet2,
        _ => Scale::Feet8,
    }
}

/// Parse a modulation source name into the engine ModSource enum.
pub fn parse_mod_source(s: &str) -> Option<ModSource> {
    match s {
        "eg1" => Some(ModSource::Eg1),
        "eg2" => Some(ModSource::Eg2),
        "lfo" => Some(ModSource::Lfo),
        "velocity" => Some(ModSource::Velocity),
        "key_track" => Some(ModSource::KeyTrack),
        _ => None,
    }
}

/// Parse a modulation destination name into the engine ModDest enum.
pub fn parse_mod_dest(s: &str) -> Option<ModDest> {
    match s {
        "hpf_cutoff" => Some(ModDest::HpfCutoff),
        "lpf_cutoff" => Some(ModDest::LpfCutoff),
        "lpf_resonance" => Some(ModDest::LpfResonance),
        "vco1_pitch" => Some(ModDest::Vco1Pitch),
        "vco2_pitch" => Some(ModDest::Vco2Pitch),
        "vca_level" => Some(ModDest::VcaLevel),
        "pwm_width" => Some(ModDest::PwmWidth),
        _ => None,
    }
}

/// Instantiate a `PedalChain` from a list of `PedalConfig` entries.
///
/// Creates each pedal from its type name, applies saved parameter values,
/// and activates the chain if it contains any pedals.
pub fn apply_chain_to_engine(chain: &mut PedalChain, configs: &[PedalConfig], sample_rate: f64) {
    chain.clear();
    for config in configs {
        if config.bypassed {
            continue;
        }
        if let Some(mut pedal) = Pedal::from_name(&config.pedal_type, sample_rate) {
            for param in &config.params {
                pedal.set_param(&param.name, param.value);
            }
            chain.push(pedal);
        }
    }
    chain.active = !chain.is_empty();
}

/// Shared PCM sample buffer for a single VCO.
pub type PcmSamples = Option<Arc<Vec<f64>>>;

/// Load PCM wavetable samples for a patch configuration through the platform.
///
/// Returns (vco1_samples, vco2_samples) as Arc-wrapped vectors suitable
/// for sharing across voices. This function performs file I/O through
/// `platform` and must run on the UI thread, never on the audio thread.
/// A sample file that cannot be read or decoded is logged as a warning and
/// leaves that oscillator without samples.
pub fn load_pcm_samples<P: Platform + ?Sized>(
    platform: &P,
    patch: &PatchConfig,
    samples_dir: &str,
) -> (PcmSamples, PcmSamples) {
    let vco1 = load_vco_pcm(
        platform,
        "VCO1",
        &patch.vco1_waveform,
        &patch.vco1_pcm_file,
        samples_dir,
    );
    let vco2 = load_vco_pcm(
        platform,
        "VCO2",
        &patch.vco2_waveform,
        &patch.vco2_pcm_file,
        samples_dir,
    );
    (vco1, vco2)
}

fn load_vco_pcm<P: Platform + ?Sized>(
    platform: &P,
    label: &str,
    waveform: &str,
    file: &str,
    samples_dir: &str,
) -> PcmSamples {
    if parse_waveform(waveform) != Waveform::Pcm || file.is_empty() {
        return None;
    }
    let path = join_path(samples_dir, file);
    match platform
        .read_file(&path)
        .and_then(|bytes| decode_wav(&bytes))
    {
        Ok(samples) => Some(Arc::new(samples)),
        Err(e) => {
            platform.log(
                LogLevel::Warn,
                &format!("Failed to load {label} PCM {path}: {e}"),
            );
            None
        }
    }
}

/// Join a directory and a file name with a forward slash, which every
/// supported platform accepts.
pub fn join_path(dir: &str, file: &str) -> String {
    if dir.is_empty() {
        return String::from(file);
    }
    let mut path = String::from(dir.trim_end_matches('/'));
    path.push('/');
    path.push_str(file);
    path
}

/// Apply all synthesizer parameters from a patch to the audio engine.
///
/// This is a pure mutation function with no file I/O. PCM samples must
/// be pre-loaded via `load_pcm_samples` and passed in. It may allocate and
/// free, so it runs on the UI thread before the engine moves to the audio
/// thread. A running engine receives a patch through `patch_command`.
pub fn apply_patch_to_engine(
    engine: &mut AudioEngine,
    channel: usize,
    patch: &PatchConfig,
    vco1_samples: Option<Arc<Vec<f64>>>,
    vco2_samples: Option<Arc<Vec<f64>>>,
) {
    let Some(engine_channel) = engine.channels.get_mut(channel) else {
        return;
    };
    let synth = &mut engine_channel.synth;
    synth.set_voice_mode(parse_voice_mode(&patch.voice_mode, patch.voice_count));
    apply_patch_params(synth, patch);
    if let Some(arc) = vco1_samples {
        for voice in &mut synth.voices {
            voice.vco1.set_pcm_samples(Arc::clone(&arc));
        }
    }
    if let Some(arc) = vco2_samples {
        for voice in &mut synth.voices {
            voice.vco2.set_pcm_samples(Arc::clone(&arc));
        }
    }
    synth.mod_matrix = build_mod_matrix(patch);
}

/// Write a patch's scalar parameters into a synthesizer, every voice alike.
/// It parses names but neither allocates nor frees, so `PatchEdit` calls it
/// on the audio thread. Voice mode, PCM tables and the modulation matrix are
/// left to the caller, because changing them can allocate.
fn apply_patch_params(synth: &mut Ms20Voice, patch: &PatchConfig) {
    // Velocity depth controls.
    synth.velocity_vca_depth = patch.velocity_vca_depth;
    synth.velocity_filter_depth = patch.velocity_filter_depth;
    synth.velocity_envelope_depth = patch.velocity_envelope_depth;
    synth.velocity_pitch_depth = patch.velocity_pitch_depth;

    // Per-voice parameters: apply to all voices in the pool.
    for voice in &mut synth.voices {
        // VCO1.
        voice.vco1.waveform = parse_waveform(&patch.vco1_waveform);
        voice.vco1.scale = parse_scale(&patch.vco1_scale);
        voice.vco1.pitch_fine = patch.vco1_pitch_fine;
        voice.vco1.level = patch.vco1_level;
        voice.vco1.pulse_width = patch.vco1_pulse_width;
        voice.vco1.quantization = patch.vco1_quantization;

        // VCO2.
        voice.vco2.waveform = parse_waveform(&patch.vco2_waveform);
        voice.vco2.scale = parse_scale(&patch.vco2_scale);
        voice.vco2.pitch_fine = patch.vco2_pitch_fine;
        voice.vco2.level = patch.vco2_level;
        voice.vco2.pulse_width = patch.vco2_pulse_width;
        voice.vco2.quantization = patch.vco2_quantization;

        // EG1 (HPF envelope).
        voice.eg1.attack = patch.eg1_attack;
        voice.eg1.decay = patch.eg1_decay;
        voice.eg1.sustain = patch.eg1_sustain;
        voice.eg1.release = patch.eg1_release;

        // EG2 (LPF and VCA envelope).
        voice.eg2.attack = patch.eg2_attack;
        voice.eg2.decay = patch.eg2_decay;
        voice.eg2.sustain = patch.eg2_sustain;
        voice.eg2.release = patch.eg2_release;

        // Filters.
        voice.hpf.cutoff = patch.hpf_cutoff;
        voice.hpf.resonance = patch.hpf_resonance;
        voice.lpf.cutoff = patch.lpf_cutoff;
        voice.lpf.resonance = patch.lpf_resonance;
        voice.lpf.saturation = patch.lpf_saturation;

        // Pitch glide.
        voice.glide.enabled = patch.glide_enabled;
        voice.glide.time = patch.glide_time;
    }

    // Shared state.
    synth.vco_mix = patch.vco_mix;
    synth.ring_mod_depth = patch.ring_mod_depth;
    synth.lfo_rate = patch.lfo_rate;
}

/// Build the modulation matrix a patch describes.
///
/// It is built entirely from the patch's mod_routings. Each preset is
/// responsible for including all routings it needs, including the standard
/// EG2->VCA routing for audible output. PatchConfig::default() provides the
/// three standard MS-20 routings. Unknown names are skipped.
pub fn build_mod_matrix(patch: &PatchConfig) -> ModMatrix {
    let mut matrix = ModMatrix::new();
    for routing_cfg in &patch.mod_routings {
        if let (Some(src), Some(dst)) = (
            parse_mod_source(&routing_cfg.source),
            parse_mod_dest(&routing_cfg.dest),
        ) {
            matrix.add_routing(ModRouting {
                source: src,
                dest: dst,
                depth: routing_cfg.depth,
            });
        }
    }
    matrix
}

// ---------------------------------------------------------------------------
// Edits for a running engine
// ---------------------------------------------------------------------------
//
// Each function below builds an `EngineCommand::Bulk` on the UI thread. The
// edit only writes plain values into the engine or swaps prepared state with
// it, and whatever it takes out of the engine stays inside it, so the audio
// thread neither allocates nor frees. The runner hands the spent edit back,
// and the sender frees it on the UI thread. The test
// `tests/engine_edits_alloc.rs` of this crate counts heap use while each edit
// is applied.

/// A patch for one channel, prepared for the audio thread.
///
/// Changing the voice count, the PCM tables or the modulation matrix in place
/// could allocate or free, so the edit brings storage for each. `pool` is an
/// empty vector with room for `MAX_PATCH_VOICES`. When the engine's pool must
/// shrink or cannot grow in place, the voices to keep move into `pool`, the
/// two vectors are swapped, and the engine's former vector, with the voices
/// left over, travels back inside the edit. That holds for an engine of any
/// size, including one configured with more voices than a patch allows,
/// because the edit moves at most `MAX_PATCH_VOICES` voices. `old_tables`
/// receives the PCM tables the voices held.
pub struct PatchEdit {
    channel: usize,
    patch: PatchConfig,
    mode: VoiceMode,
    vco1: PcmSamples,
    vco2: PcmSamples,
    mod_matrix: ModMatrix,
    pool: Vec<SingleVoice>,
    old_tables: Vec<Arc<Vec<f64>>>,
}

impl EngineEdit for PatchEdit {
    fn apply(&mut self, engine: &mut AudioEngine) {
        let Some(engine_channel) = engine.channels.get_mut(self.channel) else {
            return;
        };
        let synth = &mut engine_channel.synth;
        // `parse_voice_mode` bounds the count by `MAX_PATCH_VOICES`, the
        // capacity `patch_command` reserves, so the moves below never
        // reallocate whatever the engine holds.
        let count = usize::from(self.mode.voice_count());
        debug_assert!(count <= self.pool.capacity());
        if synth.voices.len() > count || synth.voices.capacity() < count {
            let keep = synth.voices.len().min(count);
            self.pool.extend(synth.voices.drain(..keep));
            core::mem::swap(&mut synth.voices, &mut self.pool);
        }
        // Releases every voice and grows the pool within its capacity by
        // cloning voice 0, which copies plain data and bumps `Arc` counts.
        synth.set_voice_mode(self.mode);
        apply_patch_params(synth, &self.patch);
        for voice in &mut synth.voices {
            for (vco, table) in [(&mut voice.vco1, &self.vco1), (&mut voice.vco2, &self.vco2)] {
                let Some(table) = table else {
                    continue;
                };
                if let Some(old) = vco.replace_pcm_samples(Some(Arc::clone(table))) {
                    // Room for two tables per voice of the largest pool, and
                    // the runner applies an edit once, so this never grows.
                    debug_assert!(self.old_tables.len() < self.old_tables.capacity());
                    self.old_tables.push(old);
                }
            }
        }
        core::mem::swap(&mut synth.mod_matrix, &mut self.mod_matrix);
    }
}

/// Prepare `patch` for `channel` of a running engine. PCM samples come from
/// `load_pcm_samples`, and `None` leaves an oscillator's table unchanged, as
/// `apply_patch_to_engine` does.
pub fn patch_command(
    channel: usize,
    patch: &PatchConfig,
    vco1_samples: PcmSamples,
    vco2_samples: PcmSamples,
) -> EngineCommand {
    let max_voices = usize::from(MAX_PATCH_VOICES);
    EngineCommand::Bulk(Box::new(PatchEdit {
        channel,
        patch: patch.clone(),
        mode: parse_voice_mode(&patch.voice_mode, patch.voice_count),
        vco1: vco1_samples,
        vco2: vco2_samples,
        mod_matrix: build_mod_matrix(patch),
        pool: Vec::with_capacity(max_voices),
        old_tables: Vec::with_capacity(2 * max_voices),
    }))
}

/// The engine chain a chain identifier names, 0 for the pre-positional chain
/// and 1 and 2 for the left and right post-positional chains.
fn engine_chain(engine: &mut AudioEngine, channel: usize, chain_id: u8) -> Option<&mut PedalChain> {
    let ch = engine.channels.get_mut(channel)?;
    match chain_id {
        0 => Some(&mut ch.pre_pedals),
        1 => Some(&mut ch.post_pedals_left),
        2 => Some(&mut ch.post_pedals_right),
        _ => None,
    }
}

/// Build a pedal chain from `configs` here, and swap it into the engine on
/// the audio thread. The engine's old chain comes back inside the edit.
/// Returns `None` for an unknown chain identifier.
pub fn chain_command(
    channel: usize,
    chain_id: u8,
    configs: &[PedalConfig],
    sample_rate: f64,
) -> Option<EngineCommand> {
    if chain_id > 2 {
        return None;
    }
    let mut chain = PedalChain::new();
    apply_chain_to_engine(&mut chain, configs, sample_rate);
    Some(EngineCommand::Bulk(Box::new(
        move |engine: &mut AudioEngine| {
            if let Some(target) = engine_chain(engine, channel, chain_id) {
                core::mem::swap(target, &mut chain);
            }
        },
    )))
}

/// Replace all three pedal chains of `channel` and apply its positional stage.
pub fn effects_command(
    channel: usize,
    config: &EffectChainConfig,
    sample_rate: f64,
) -> EngineCommand {
    let mut chains = [
        &config.pre_pedals,
        &config.post_pedals_left,
        &config.post_pedals_right,
    ]
    .map(|configs| {
        let mut chain = PedalChain::new();
        apply_chain_to_engine(&mut chain, configs, sample_rate);
        chain
    });
    let config = config.clone();
    EngineCommand::Bulk(Box::new(move |engine: &mut AudioEngine| {
        for (chain_id, chain) in (0u8..).zip(chains.iter_mut()) {
            if let Some(target) = engine_chain(engine, channel, chain_id) {
                core::mem::swap(target, chain);
            }
        }
        apply_position_to_engine(engine, channel, &config);
    }))
}

/// Apply the positional stage of `config` to `channel`. It writes plain
/// values only.
pub fn position_command(channel: usize, config: &EffectChainConfig) -> EngineCommand {
    let config = config.clone();
    EngineCommand::Bulk(Box::new(move |engine: &mut AudioEngine| {
        apply_position_to_engine(engine, channel, &config);
    }))
}

/// Set one parameter of a live pedal. `Pedal::set_param` writes a field and
/// allocates nothing, and the name is freed with the spent edit.
pub fn pedal_param_command(
    channel: usize,
    chain_id: u8,
    pedal_idx: usize,
    param_name: String,
    value: f64,
) -> EngineCommand {
    EngineCommand::Bulk(Box::new(move |engine: &mut AudioEngine| {
        if let Some(pedal) = engine_chain(engine, channel, chain_id)
            .and_then(|chain| chain.pedals.get_mut(pedal_idx))
        {
            pedal.set_param(&param_name, value);
        }
    }))
}

/// Apply positional stage parameters from EffectChainConfig to the audio engine.
///
/// When orbital mode is enabled, `position_3d_enabled` and `active` are forced
/// true. The actual azimuth, elevation, and distance values are updated
/// per-frame in `advance_playback()` from the orbital computation.
pub fn apply_position_to_engine(engine: &mut AudioEngine, ch: usize, effects: &EffectChainConfig) {
    let Some(channel) = engine.channels.get_mut(ch) else {
        return;
    };
    let pos = &mut channel.position;
    let has_position =
        effects.position_3d_enabled || effects.orbital_enabled || effects.pan.abs() > 1e-10;
    pos.active = has_position;
    pos.pan = effects.pan;
    pos.position_3d_enabled = effects.position_3d_enabled || effects.orbital_enabled;
    pos.azimuth_deg = effects.azimuth;
    pos.elevation_deg = effects.elevation;
    pos.distance = effects.position_distance;
}

/// Parse a voice mode priority string and voice count into a VoiceMode enum.
///
/// The priority string determines high or low note priority. The count
/// (1 to `MAX_PATCH_VOICES`) determines polyphony. Count is clamped to the
/// valid range.
pub fn parse_voice_mode(s: &str, count: u8) -> VoiceMode {
    let n = count.clamp(1, MAX_PATCH_VOICES);
    if s.starts_with("low") {
        VoiceMode::Low(n)
    } else {
        VoiceMode::High(n)
    }
}

/// Bring a freshly constructed engine into the state a project describes.
///
/// Applies per-channel volume, pan and mute, the synthesizer patch, the three
/// pedal chains and the positional stage. `pcm` holds samples already loaded
/// by `load_pcm_samples`, one pair per channel. This runs before the engine is
/// handed to the audio thread, so it mutates the engine directly.
pub fn configure_engine(
    engine: &mut AudioEngine,
    project: &DawProject,
    patches: &[PatchConfig; NUM_CHANNELS],
    effects: &[EffectChainConfig; NUM_CHANNELS],
    pcm: [(PcmSamples, PcmSamples); NUM_CHANNELS],
) {
    let sr = engine.sample_rate as f64;
    for (ch, (s1, s2)) in pcm.into_iter().enumerate() {
        let conf = &project.channels[ch];
        let vol = conf.volume;
        let left = vol * (1.0 - conf.pan.max(0.0));
        let right = vol * (1.0 + conf.pan.min(0.0));
        engine.channels[ch].set_volume(left, right);
        engine.channels[ch].enabled = !conf.muted;

        apply_patch_to_engine(engine, ch, &patches[ch], s1, s2);

        let config = &effects[ch];
        let channel = &mut engine.channels[ch];
        apply_chain_to_engine(&mut channel.pre_pedals, &config.pre_pedals, sr);
        apply_chain_to_engine(&mut channel.post_pedals_left, &config.post_pedals_left, sr);
        apply_chain_to_engine(
            &mut channel.post_pedals_right,
            &config.post_pedals_right,
            sr,
        );
        apply_position_to_engine(engine, ch, config);
    }
}

/// Convert a `daw_core::script::AudioCommand` into an `EngineCommand`
/// for a specific channel.
pub fn audio_command_to_engine_command(channel: u8, cmd: &AudioCommand) -> EngineCommand {
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
