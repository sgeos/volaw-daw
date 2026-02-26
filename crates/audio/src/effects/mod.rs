//! Audio effect implementations for the pedal chain.
//!
//! Each effect module contains one or more effect types that can be
//! inserted into a [`crate::PedalChain`] as [`Pedal`] enum variants.
//!
//! ## Categories
//!
//! | Category | Effects |
//! |----------|---------|
//! | Distortion / Saturation | Overdrive, Distortion, Fuzz, Tape Saturation |
//! | Dynamic / Modulation | Auto-Wah, Manual Wah, Chorus, Flanger, Phaser, Tremolo |
//! | Time-Based | Digital Delay, Analog Delay, Ping-Pong Delay, Plate/Hall/Room Reverb |
//! | Special FX | Bitcrusher, Ring Modulator, Pitch Shifter, Auto-Pan |
//! | Utility | Noise Gate, Compressor, EQ, Volume, Buffer |

pub mod delay;
pub mod distortion;
pub mod dynamics;
pub mod modulation;
pub mod reverb;
pub mod special;
pub mod utility;
pub mod wah;

pub use delay::{AnalogDelay, DigitalDelay, PingPongDelay};
pub use distortion::{Distortion, Fuzz, Overdrive, TapeSaturation};
pub use dynamics::{Compressor, NoiseGate};
pub use modulation::{Chorus, Flanger, Phaser, Tremolo};
pub use reverb::{HallReverb, PlateReverb, RoomReverb};
pub use special::{AutoPan, Bitcrusher, PitchShifter, RingModulator};
pub use utility::{Buffer, Eq, Volume};
pub use wah::{AutoWah, WahManual};

/// A single effect pedal in the signal chain.
///
/// This enum avoids dynamic dispatch by encoding all 25 effect types
/// as variants. Each variant holds its own parameters and DSP state.
#[derive(Debug, Clone)]
pub enum Pedal {
    Overdrive(Overdrive),
    Distortion(Distortion),
    Fuzz(Fuzz),
    TapeSaturation(TapeSaturation),
    AutoWah(AutoWah),
    WahManual(WahManual),
    Chorus(Chorus),
    Flanger(Flanger),
    Phaser(Phaser),
    Tremolo(Tremolo),
    DigitalDelay(DigitalDelay),
    AnalogDelay(AnalogDelay),
    PingPongDelay(PingPongDelay),
    PlateReverb(PlateReverb),
    HallReverb(HallReverb),
    RoomReverb(RoomReverb),
    Bitcrusher(Bitcrusher),
    RingModulator(RingModulator),
    PitchShifter(PitchShifter),
    AutoPan(AutoPan),
    NoiseGate(NoiseGate),
    Compressor(Compressor),
    Eq(Eq),
    Volume(Volume),
    Buffer(Buffer),
}

impl Pedal {
    /// Process a mono audio buffer through this effect.
    pub fn process(&mut self, buffer: &mut [f64], sample_rate: u32) {
        let sr = sample_rate as f64;
        match self {
            Pedal::Overdrive(p) => p.process(buffer, sr),
            Pedal::Distortion(p) => p.process(buffer, sr),
            Pedal::Fuzz(p) => p.process(buffer, sr),
            Pedal::TapeSaturation(p) => p.process(buffer, sr),
            Pedal::AutoWah(p) => p.process(buffer, sr),
            Pedal::WahManual(p) => p.process(buffer, sr),
            Pedal::Chorus(p) => p.process(buffer, sr),
            Pedal::Flanger(p) => p.process(buffer, sr),
            Pedal::Phaser(p) => p.process(buffer, sr),
            Pedal::Tremolo(p) => p.process(buffer, sr),
            Pedal::DigitalDelay(p) => p.process(buffer, sr),
            Pedal::AnalogDelay(p) => p.process(buffer, sr),
            Pedal::PingPongDelay(p) => p.process(buffer, sr),
            Pedal::PlateReverb(p) => p.process(buffer, sr),
            Pedal::HallReverb(p) => p.process(buffer, sr),
            Pedal::RoomReverb(p) => p.process(buffer, sr),
            Pedal::Bitcrusher(p) => p.process(buffer, sr),
            Pedal::RingModulator(p) => p.process(buffer, sr),
            Pedal::PitchShifter(p) => p.process(buffer, sr),
            Pedal::AutoPan(p) => p.process(buffer, sr),
            Pedal::NoiseGate(p) => p.process(buffer, sr),
            Pedal::Compressor(p) => p.process(buffer, sr),
            Pedal::Eq(p) => p.process(buffer, sr),
            Pedal::Volume(p) => p.process(buffer, sr),
            Pedal::Buffer(p) => p.process(buffer, sr),
        }
    }

    /// Set a named parameter on this pedal by name and value.
    ///
    /// Returns `true` if the parameter was found and set, `false` otherwise.
    /// All parameters are f64 except Bitcrusher::bit_depth (cast from f64)
    /// and Phaser::stages (cast from f64).
    #[allow(clippy::too_many_lines)]
    pub fn set_param(&mut self, name: &str, value: f64) -> bool {
        match self {
            Pedal::Overdrive(p) => match name {
                "gain" => { p.gain = value; true }
                "tone" => { p.tone = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::Distortion(p) => match name {
                "gain" => { p.gain = value; true }
                "threshold" => { p.threshold = value; true }
                "tone" => { p.tone = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::Fuzz(p) => match name {
                "gain" => { p.gain = value; true }
                "tone" => { p.tone = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::TapeSaturation(p) => match name {
                "drive" => { p.drive = value; true }
                "warmth" => { p.warmth = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::AutoWah(p) => match name {
                "sensitivity" => { p.sensitivity = value; true }
                "min_freq" => { p.min_freq = value; true }
                "max_freq" => { p.max_freq = value; true }
                "q" => { p.q = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::WahManual(p) => match name {
                "position" => { p.position = value; true }
                "min_freq" => { p.min_freq = value; true }
                "max_freq" => { p.max_freq = value; true }
                "q" => { p.q = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::Chorus(p) => match name {
                "rate_hz" => { p.rate_hz = value; true }
                "depth_ms" => { p.depth_ms = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::Flanger(p) => match name {
                "rate_hz" => { p.rate_hz = value; true }
                "depth_ms" => { p.depth_ms = value; true }
                "feedback" => { p.feedback = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::Phaser(p) => match name {
                "rate_hz" => { p.rate_hz = value; true }
                "depth" => { p.depth = value; true }
                "feedback" => { p.feedback = value; true }
                "mix" => { p.mix = value; true }
                "stages" => { p.stages = value as usize; true }
                _ => false,
            },
            Pedal::Tremolo(p) => match name {
                "rate_hz" => { p.rate_hz = value; true }
                "depth" => { p.depth = value; true }
                _ => false,
            },
            Pedal::DigitalDelay(p) => match name {
                "time_ms" => { p.time_ms = value; true }
                "feedback" => { p.feedback = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::AnalogDelay(p) => match name {
                "time_ms" => { p.time_ms = value; true }
                "feedback" => { p.feedback = value; true }
                "tone" => { p.tone = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::PingPongDelay(p) => match name {
                "time_ms" => { p.time_ms = value; true }
                "feedback" => { p.feedback = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::PlateReverb(p) => match name {
                "decay" => { p.decay = value; true }
                "damping" => { p.damping = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::HallReverb(p) => match name {
                "decay" => { p.decay = value; true }
                "damping" => { p.damping = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::RoomReverb(p) => match name {
                "decay" => { p.decay = value; true }
                "damping" => { p.damping = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::Bitcrusher(p) => match name {
                "bit_depth" => { p.bit_depth = value as u32; true }
                "rate_factor" => { p.rate_factor = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::RingModulator(p) => match name {
                "frequency_hz" => { p.frequency_hz = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::PitchShifter(p) => match name {
                "semitones" => { p.semitones = value; true }
                "mix" => { p.mix = value; true }
                _ => false,
            },
            Pedal::AutoPan(p) => match name {
                "rate_hz" => { p.rate_hz = value; true }
                "depth" => { p.depth = value; true }
                _ => false,
            },
            Pedal::NoiseGate(p) => match name {
                "threshold" => { p.threshold = value; true }
                "attack_ms" => { p.attack_ms = value; true }
                "release_ms" => { p.release_ms = value; true }
                _ => false,
            },
            Pedal::Compressor(p) => match name {
                "threshold" => { p.threshold = value; true }
                "ratio" => { p.ratio = value; true }
                "attack_ms" => { p.attack_ms = value; true }
                "release_ms" => { p.release_ms = value; true }
                "makeup_gain" => { p.makeup_gain = value; true }
                _ => false,
            },
            Pedal::Eq(p) => match name {
                "low_gain_db" => { p.low_gain_db = value; true }
                "mid_gain_db" => { p.mid_gain_db = value; true }
                "high_gain_db" => { p.high_gain_db = value; true }
                "low_freq" => { p.low_freq = value; true }
                "mid_freq" => { p.mid_freq = value; true }
                "high_freq" => { p.high_freq = value; true }
                _ => false,
            },
            Pedal::Volume(p) => match name {
                "gain" => { p.gain = value; true }
                _ => false,
            },
            Pedal::Buffer(p) => match name {
                "gain" => { p.gain = value; true }
                _ => false,
            },
        }
    }

    /// Create a pedal from its type name string.
    ///
    /// Returns `None` if the name does not match any known pedal type.
    /// Names are case-insensitive and match the enum variant names.
    pub fn from_name(name: &str, sample_rate: f64) -> Option<Self> {
        let lower = name.to_lowercase();
        match lower.as_str() {
            "overdrive" => Some(Pedal::Overdrive(Overdrive::new(sample_rate))),
            "distortion" => Some(Pedal::Distortion(Distortion::new(sample_rate))),
            "fuzz" => Some(Pedal::Fuzz(Fuzz::new(sample_rate))),
            "tapesaturation" | "tape_saturation" => {
                Some(Pedal::TapeSaturation(TapeSaturation::new(sample_rate)))
            }
            "autowah" | "auto_wah" => Some(Pedal::AutoWah(AutoWah::new(sample_rate))),
            "wahmanual" | "wah_manual" => Some(Pedal::WahManual(WahManual::new(sample_rate))),
            "chorus" => Some(Pedal::Chorus(Chorus::new(sample_rate))),
            "flanger" => Some(Pedal::Flanger(Flanger::new(sample_rate))),
            "phaser" => Some(Pedal::Phaser(Phaser::new(sample_rate))),
            "tremolo" => Some(Pedal::Tremolo(Tremolo::new(sample_rate))),
            "digitaldelay" | "digital_delay" => {
                Some(Pedal::DigitalDelay(DigitalDelay::new(sample_rate)))
            }
            "analogdelay" | "analog_delay" => {
                Some(Pedal::AnalogDelay(AnalogDelay::new(sample_rate)))
            }
            "pingpongdelay" | "ping_pong_delay" => {
                Some(Pedal::PingPongDelay(PingPongDelay::new(sample_rate)))
            }
            "platereverb" | "plate_reverb" => {
                Some(Pedal::PlateReverb(PlateReverb::new(sample_rate)))
            }
            "hallreverb" | "hall_reverb" => {
                Some(Pedal::HallReverb(HallReverb::new(sample_rate)))
            }
            "roomreverb" | "room_reverb" => {
                Some(Pedal::RoomReverb(RoomReverb::new(sample_rate)))
            }
            "bitcrusher" => Some(Pedal::Bitcrusher(Bitcrusher::new(sample_rate))),
            "ringmodulator" | "ring_modulator" => {
                Some(Pedal::RingModulator(RingModulator::new(sample_rate)))
            }
            "pitchshifter" | "pitch_shifter" => {
                Some(Pedal::PitchShifter(PitchShifter::new(sample_rate)))
            }
            "autopan" | "auto_pan" => Some(Pedal::AutoPan(AutoPan::new(sample_rate))),
            "noisegate" | "noise_gate" => Some(Pedal::NoiseGate(NoiseGate::new(sample_rate))),
            "compressor" => Some(Pedal::Compressor(Compressor::new(sample_rate))),
            "eq" => Some(Pedal::Eq(Eq::new(sample_rate))),
            "volume" => Some(Pedal::Volume(Volume::new(sample_rate))),
            "buffer" => Some(Pedal::Buffer(Buffer::new(sample_rate))),
            _ => None,
        }
    }

    /// Return the type name string for this pedal variant.
    pub fn type_name(&self) -> &'static str {
        match self {
            Pedal::Overdrive(_) => "Overdrive",
            Pedal::Distortion(_) => "Distortion",
            Pedal::Fuzz(_) => "Fuzz",
            Pedal::TapeSaturation(_) => "TapeSaturation",
            Pedal::AutoWah(_) => "AutoWah",
            Pedal::WahManual(_) => "WahManual",
            Pedal::Chorus(_) => "Chorus",
            Pedal::Flanger(_) => "Flanger",
            Pedal::Phaser(_) => "Phaser",
            Pedal::Tremolo(_) => "Tremolo",
            Pedal::DigitalDelay(_) => "DigitalDelay",
            Pedal::AnalogDelay(_) => "AnalogDelay",
            Pedal::PingPongDelay(_) => "PingPongDelay",
            Pedal::PlateReverb(_) => "PlateReverb",
            Pedal::HallReverb(_) => "HallReverb",
            Pedal::RoomReverb(_) => "RoomReverb",
            Pedal::Bitcrusher(_) => "Bitcrusher",
            Pedal::RingModulator(_) => "RingModulator",
            Pedal::PitchShifter(_) => "PitchShifter",
            Pedal::AutoPan(_) => "AutoPan",
            Pedal::NoiseGate(_) => "NoiseGate",
            Pedal::Compressor(_) => "Compressor",
            Pedal::Eq(_) => "Eq",
            Pedal::Volume(_) => "Volume",
            Pedal::Buffer(_) => "Buffer",
        }
    }

    /// Reset the effect state (clear delay lines, filters, etc.).
    pub fn reset(&mut self) {
        match self {
            Pedal::Overdrive(p) => p.reset(),
            Pedal::Distortion(p) => p.reset(),
            Pedal::Fuzz(p) => p.reset(),
            Pedal::TapeSaturation(p) => p.reset(),
            Pedal::AutoWah(p) => p.reset(),
            Pedal::WahManual(p) => p.reset(),
            Pedal::Chorus(p) => p.reset(),
            Pedal::Flanger(p) => p.reset(),
            Pedal::Phaser(p) => p.reset(),
            Pedal::Tremolo(p) => p.reset(),
            Pedal::DigitalDelay(p) => p.reset(),
            Pedal::AnalogDelay(p) => p.reset(),
            Pedal::PingPongDelay(p) => p.reset(),
            Pedal::PlateReverb(p) => p.reset(),
            Pedal::HallReverb(p) => p.reset(),
            Pedal::RoomReverb(p) => p.reset(),
            Pedal::Bitcrusher(p) => p.reset(),
            Pedal::RingModulator(p) => p.reset(),
            Pedal::PitchShifter(p) => p.reset(),
            Pedal::AutoPan(p) => p.reset(),
            Pedal::NoiseGate(p) => p.reset(),
            Pedal::Compressor(p) => p.reset(),
            Pedal::Eq(p) => p.reset(),
            Pedal::Volume(p) => p.reset(),
            Pedal::Buffer(p) => p.reset(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_all_25_types() {
        let names = [
            "Overdrive", "Distortion", "Fuzz", "TapeSaturation",
            "AutoWah", "WahManual", "Chorus", "Flanger", "Phaser", "Tremolo",
            "DigitalDelay", "AnalogDelay", "PingPongDelay",
            "PlateReverb", "HallReverb", "RoomReverb",
            "Bitcrusher", "RingModulator", "PitchShifter", "AutoPan",
            "NoiseGate", "Compressor", "Eq", "Volume", "Buffer",
        ];
        for name in &names {
            let pedal = Pedal::from_name(name, 48000.0);
            assert!(pedal.is_some(), "from_name should recognize {name}");
            assert_eq!(pedal.unwrap().type_name(), *name);
        }
    }

    #[test]
    fn from_name_case_insensitive() {
        assert!(Pedal::from_name("overdrive", 48000.0).is_some());
        assert!(Pedal::from_name("CHORUS", 48000.0).is_some());
    }

    #[test]
    fn from_name_snake_case() {
        assert!(Pedal::from_name("tape_saturation", 48000.0).is_some());
        assert!(Pedal::from_name("digital_delay", 48000.0).is_some());
        assert!(Pedal::from_name("noise_gate", 48000.0).is_some());
    }

    #[test]
    fn from_name_unknown_returns_none() {
        assert!(Pedal::from_name("unknown", 48000.0).is_none());
    }
}
