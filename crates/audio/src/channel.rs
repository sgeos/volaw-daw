use crate::pedals::PedalChain;
use crate::sample::SampleInput;
use crate::synth::Ms20Voice;

/// Clamp a volume value to the range 0.0 to 1.0.
pub(crate) fn clamp_volume(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Configuration and state for one audio channel.
///
/// Each channel contains a complete signal chain: sample audio input,
/// virtual Korg MS-20 synthesizer, and guitar pedal chain. The channel
/// has independent left and right volume controls, allowing flexible
/// stereo positioning without a separate routing enum.
///
/// ## Signal Chain
///
/// ```text
/// Sample Input --> MS-20 Voice --> Pedal Chain --> (volume_left, volume_right) --> Mix
/// ```
///
/// The sample input can feed audio into the MS-20 voice via its external
/// signal processor input. The MS-20 output is then processed through the
/// pedal chain before being mixed into the stereo output according to the
/// per-channel left and right volume levels.
///
/// ## Stereo Positioning
///
/// Instead of a routing enum (Left/Right/Both), each channel has independent
/// `volume_left` and `volume_right` controls. This is more flexible:
///
/// - Left only: `volume_left = 0.8, volume_right = 0.0`
/// - Right only: `volume_left = 0.0, volume_right = 0.8`
/// - Center: `volume_left = 0.8, volume_right = 0.8`
/// - Panned: `volume_left = 0.6, volume_right = 0.3`
///
/// ## Hardware Inspiration
///
/// This channel design is inspired by game audio hardware:
/// - SNES SPC700 (8 channels, per-channel stereo volume)
/// - Neo Geo YM2610 (4 FM + 7 ADPCM channels)
/// - Megadrive YM2612 (6 FM channels)
pub struct Channel {
    /// Left speaker volume (0.0 to 1.0, clamped on set).
    pub volume_left: f32,
    /// Right speaker volume (0.0 to 1.0, clamped on set).
    pub volume_right: f32,
    /// Whether this channel is producing audio.
    pub enabled: bool,
    /// Virtual Korg MS-20 synthesizer voice.
    pub synth: Ms20Voice,
    /// Audio sample input (feeds into the MS-20 signal chain).
    pub sample_in: SampleInput,
    /// Guitar pedal effect chain (processes MS-20 output).
    pub pedals: PedalChain,
}

impl Channel {
    /// Create a new channel with the given stereo volumes.
    ///
    /// Both volumes are clamped to the range 0.0 to 1.0.
    pub fn new(volume_left: f32, volume_right: f32) -> Self {
        Self {
            volume_left: clamp_volume(volume_left),
            volume_right: clamp_volume(volume_right),
            enabled: true,
            synth: Ms20Voice::new(),
            sample_in: SampleInput::new(),
            pedals: PedalChain::new(),
        }
    }

    /// Create a channel routed to the left speaker only.
    pub fn left(volume: f32) -> Self {
        Self::new(volume, 0.0)
    }

    /// Create a channel routed to the right speaker only.
    pub fn right(volume: f32) -> Self {
        Self::new(0.0, volume)
    }

    /// Create a channel routed to both speakers equally.
    pub fn center(volume: f32) -> Self {
        Self::new(volume, volume)
    }

    /// Process the full signal chain into a mono buffer.
    ///
    /// Runs sample input, MS-20 synth, then pedal chain in sequence.
    /// When the channel is disabled, the buffer is zeroed.
    pub fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled {
            for s in buffer.iter_mut() {
                *s = 0.0;
            }
            return;
        }
        self.sample_in.process(buffer);
        self.synth.process(buffer);
        self.pedals.process(buffer, sample_rate);
    }

    /// Set the left volume. Clamped to 0.0 to 1.0.
    pub fn set_volume_left(&mut self, volume: f32) {
        self.volume_left = clamp_volume(volume);
    }

    /// Set the right volume. Clamped to 0.0 to 1.0.
    pub fn set_volume_right(&mut self, volume: f32) {
        self.volume_right = clamp_volume(volume);
    }

    /// Set both volumes simultaneously. Both clamped to 0.0 to 1.0.
    pub fn set_volume(&mut self, left: f32, right: f32) {
        self.volume_left = clamp_volume(left);
        self.volume_right = clamp_volume(right);
    }
}

impl Default for Channel {
    fn default() -> Self {
        Self::center(0.8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_channel() {
        let ch = Channel::left(0.8);
        assert!((ch.volume_left - 0.8).abs() < f32::EPSILON);
        assert!(ch.volume_right.abs() < f32::EPSILON);
        assert!(ch.enabled);
    }

    #[test]
    fn right_channel() {
        let ch = Channel::right(0.7);
        assert!(ch.volume_left.abs() < f32::EPSILON);
        assert!((ch.volume_right - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn center_channel() {
        let ch = Channel::center(0.5);
        assert!((ch.volume_left - 0.5).abs() < f32::EPSILON);
        assert!((ch.volume_right - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn volume_clamping() {
        let ch = Channel::new(1.5, -0.5);
        assert!((ch.volume_left - 1.0).abs() < f32::EPSILON);
        assert!(ch.volume_right.abs() < f32::EPSILON);
    }

    #[test]
    fn set_volume_left_clamps() {
        let mut ch = Channel::center(0.5);
        ch.set_volume_left(2.0);
        assert!((ch.volume_left - 1.0).abs() < f32::EPSILON);
        ch.set_volume_left(-1.0);
        assert!(ch.volume_left.abs() < f32::EPSILON);
    }

    #[test]
    fn set_volume_right_clamps() {
        let mut ch = Channel::center(0.5);
        ch.set_volume_right(2.0);
        assert!((ch.volume_right - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn set_volume_both() {
        let mut ch = Channel::center(0.5);
        ch.set_volume(0.3, 0.7);
        assert!((ch.volume_left - 0.3).abs() < f32::EPSILON);
        assert!((ch.volume_right - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn default_is_center() {
        let ch = Channel::default();
        assert!((ch.volume_left - 0.8).abs() < f32::EPSILON);
        assert!((ch.volume_right - 0.8).abs() < f32::EPSILON);
        assert!(ch.enabled);
    }

    #[test]
    fn has_synth_sample_pedals() {
        let ch = Channel::center(0.5);
        assert!(!ch.synth.external_input_enabled);
        assert!(!ch.sample_in.active);
        assert!(!ch.pedals.active);
    }

    #[test]
    fn disabled_channel_zeros_buffer() {
        let mut ch = Channel::center(0.5);
        ch.enabled = false;
        let mut buffer = [1.0f32; 16];
        ch.process(&mut buffer, 48000);
        for s in &buffer {
            assert!(s.abs() < f32::EPSILON);
        }
    }

    #[test]
    fn enabled_channel_runs_chain() {
        let mut ch = Channel::center(0.5);
        ch.enabled = true;
        // With stubs (synth writes silence, pedals passthrough),
        // the output should be silence
        let mut buffer = [0.5f32; 16];
        ch.process(&mut buffer, 48000);
        for s in &buffer {
            assert!(s.abs() < f32::EPSILON, "Synth stub should write silence");
        }
    }
}
