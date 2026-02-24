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
