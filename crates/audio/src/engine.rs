use crate::channel::{clamp_volume, Channel};

/// Number of synthesizer channels.
///
/// The 8-channel design is inspired by game audio hardware such as the
/// SNES SPC700, Neo Geo YM2610, and Megadrive YM2612.
///
/// ## Example Configurations
///
/// **Orchestra** (8 instruments):
///
/// | Channel | Role | Left | Right |
/// |---------|------|------|-------|
/// | 0 | Bass (Cello / Double Bass) | 0.8 | 0.8 |
/// | 1 | Lead (Violin / Flute) | 0.8 | 0.8 |
/// | 2 | Harmony 1 (Viola / Clarinet) | 0.7 | 0.3 |
/// | 3 | Harmony 2 (Oboe / Alto Sax) | 0.3 | 0.7 |
/// | 4 | Pad / Atmosphere | 0.6 | 0.6 |
/// | 5 | Counter-melody / Arpeggio | 0.5 | 0.5 |
/// | 6 | Percussive / Rhythmic | 0.8 | 0.8 |
/// | 7 | Extra lead / Bass doubling | 0.8 | 0.8 |
///
/// **Three-man band per speaker** (6 music + 2 SFX):
///
/// | Channel | Role | Left | Right |
/// |---------|------|------|-------|
/// | 0 | Left Bass | 0.8 | 0.0 |
/// | 1 | Left Lead | 0.8 | 0.0 |
/// | 2 | Left Harmony | 0.8 | 0.0 |
/// | 3 | Right Bass | 0.0 | 0.8 |
/// | 4 | Right Lead | 0.0 | 0.8 |
/// | 5 | Right Harmony | 0.0 | 0.8 |
/// | 6 | Primary SFX | 0.8 | 0.8 |
/// | 7 | Secondary SFX | 0.8 | 0.8 |
pub const NUM_CHANNELS: usize = 8;

/// 8-channel audio engine with per-channel Korg MS-20 synthesis.
///
/// The audio engine runs as pure `no_std` DSP math. The platform wrapper
/// initializes an audio device and calls `render_samples()` from the audio
/// callback thread. The engine itself contains no synchronization primitives.
/// Thread safety is the responsibility of the `std` wrapper (typically via
/// `Arc<Mutex<AudioEngine>>`).
///
/// ## Per-Channel Signal Chain
///
/// Each channel contains a complete signal path:
///
/// ```text
/// Sample Input --> MS-20 Voice --> Pedal Chain --> (volume_left, volume_right) --> Mix
/// ```
///
/// - **Sample Input**: Recorded SFX and voice samples can be piped through
///   the entire synth and pedal chain.
/// - **MS-20 Voice**: Virtual Korg MS-20 synthesizer with VCOs, VCFs,
///   envelope generators, and VCA.
/// - **Pedal Chain**: Virtual guitar pedal effects chain that processes
///   the MS-20 output.
///
/// ## Default Configuration
///
/// The default layout is a "three-man band" per speaker with two SFX channels:
/// channels 0-2 routed left, channels 3-5 routed right, channels 6-7 center.
pub struct AudioEngine {
    /// The eight synthesizer channels.
    pub channels: [Channel; NUM_CHANNELS],
    /// Global master volume applied after channel mixing (0.0 to 1.0).
    pub master_volume: f32,
    /// Audio sample rate in Hz.
    pub sample_rate: u32,
}

impl AudioEngine {
    /// Create a new audio engine with the default three-man-band layout.
    ///
    /// Channels 0-2 are routed to the left speaker, channels 3-5 to the
    /// right speaker, and channels 6-7 to both speakers (center, for SFX).
    pub fn new(sample_rate: u32) -> Self {
        Self {
            channels: [
                Channel::left(0.8),   // 0: Left Bass
                Channel::left(0.8),   // 1: Left Lead
                Channel::left(0.8),   // 2: Left Harmony
                Channel::right(0.8),  // 3: Right Bass
                Channel::right(0.8),  // 4: Right Lead
                Channel::right(0.8),  // 5: Right Harmony
                Channel::center(0.8), // 6: Primary SFX
                Channel::center(0.8), // 7: Secondary SFX
            ],
            master_volume: 0.8,
            sample_rate,
        }
    }

    /// Fill a stereo interleaved f32 sample buffer (L, R, L, R, ...).
    ///
    /// Called by the platform audio callback at the configured sample rate.
    /// The buffer length must be even (pairs of left and right samples).
    ///
    /// Current implementation writes silence. Future milestones will
    /// synthesize audio from the eight channels, running each channel
    /// through its signal chain (sample input, MS-20 voice, pedal chain)
    /// and mixing into the stereo output according to per-channel
    /// left and right volume levels.
    pub fn render_samples(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = 0.0;
        }
    }

    /// Set the left volume for a specific channel.
    ///
    /// Volume is clamped to the range 0.0 to 1.0.
    /// Out-of-bounds channel indices are silently ignored.
    pub fn set_channel_volume_left(&mut self, channel: usize, volume: f32) {
        if channel < NUM_CHANNELS {
            self.channels[channel].set_volume_left(volume);
        }
    }

    /// Set the right volume for a specific channel.
    ///
    /// Volume is clamped to the range 0.0 to 1.0.
    /// Out-of-bounds channel indices are silently ignored.
    pub fn set_channel_volume_right(&mut self, channel: usize, volume: f32) {
        if channel < NUM_CHANNELS {
            self.channels[channel].set_volume_right(volume);
        }
    }

    /// Set both left and right volumes for a specific channel.
    ///
    /// Both volumes are clamped to the range 0.0 to 1.0.
    /// Out-of-bounds channel indices are silently ignored.
    pub fn set_channel_volume(&mut self, channel: usize, left: f32, right: f32) {
        if channel < NUM_CHANNELS {
            self.channels[channel].set_volume(left, right);
        }
    }

    /// Enable or disable a specific channel.
    ///
    /// Out-of-bounds channel indices are silently ignored.
    pub fn enable_channel(&mut self, channel: usize, enabled: bool) {
        if channel < NUM_CHANNELS {
            self.channels[channel].enabled = enabled;
        }
    }

    /// Set the global master volume. Clamped to 0.0 to 1.0.
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = clamp_volume(volume);
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new(48000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_routing_three_man_band() {
        let engine = AudioEngine::new(48000);
        // Left channels (0-2)
        for i in 0..3 {
            assert!((engine.channels[i].volume_left - 0.8).abs() < f32::EPSILON);
            assert!(engine.channels[i].volume_right.abs() < f32::EPSILON);
        }
        // Right channels (3-5)
        for i in 3..6 {
            assert!(engine.channels[i].volume_left.abs() < f32::EPSILON);
            assert!((engine.channels[i].volume_right - 0.8).abs() < f32::EPSILON);
        }
        // Center SFX channels (6-7)
        for i in 6..8 {
            assert!((engine.channels[i].volume_left - 0.8).abs() < f32::EPSILON);
            assert!((engine.channels[i].volume_right - 0.8).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn default_volume() {
        let engine = AudioEngine::new(48000);
        assert!((engine.master_volume - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn all_channels_enabled_by_default() {
        let engine = AudioEngine::new(48000);
        for ch in &engine.channels {
            assert!(ch.enabled);
        }
    }

    #[test]
    fn set_channel_volume_left_clamps() {
        let mut engine = AudioEngine::new(48000);
        engine.set_channel_volume_left(0, 1.5);
        assert!((engine.channels[0].volume_left - 1.0).abs() < f32::EPSILON);
        engine.set_channel_volume_left(0, -0.5);
        assert!(engine.channels[0].volume_left.abs() < f32::EPSILON);
    }

    #[test]
    fn set_channel_volume_right_clamps() {
        let mut engine = AudioEngine::new(48000);
        engine.set_channel_volume_right(3, 2.0);
        assert!((engine.channels[3].volume_right - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn set_channel_volume_both() {
        let mut engine = AudioEngine::new(48000);
        engine.set_channel_volume(6, 0.3, 0.7);
        assert!((engine.channels[6].volume_left - 0.3).abs() < f32::EPSILON);
        assert!((engine.channels[6].volume_right - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn enable_disable() {
        let mut engine = AudioEngine::new(48000);
        assert!(engine.channels[0].enabled);
        engine.enable_channel(0, false);
        assert!(!engine.channels[0].enabled);
        engine.enable_channel(0, true);
        assert!(engine.channels[0].enabled);
    }

    #[test]
    fn render_samples_writes_silence() {
        let mut engine = AudioEngine::new(48000);
        let mut buffer = [1.0f32; 64];
        engine.render_samples(&mut buffer);
        for sample in &buffer {
            assert!(sample.abs() < f32::EPSILON);
        }
    }

    #[test]
    fn out_of_bounds_channel_ignored() {
        let mut engine = AudioEngine::new(48000);
        engine.set_channel_volume_left(8, 0.5);
        engine.set_channel_volume_right(99, 0.5);
        engine.set_channel_volume(100, 0.3, 0.7);
        engine.enable_channel(8, false);
        // No panic, no change to valid channels.
        for ch in &engine.channels {
            assert!(ch.enabled);
        }
    }

    #[test]
    fn master_volume_clamping() {
        let mut engine = AudioEngine::new(48000);
        engine.set_master_volume(2.0);
        assert!((engine.master_volume - 1.0).abs() < f32::EPSILON);
        engine.set_master_volume(-1.0);
        assert!(engine.master_volume.abs() < f32::EPSILON);
    }

    #[test]
    fn eight_channels() {
        let engine = AudioEngine::new(48000);
        assert_eq!(engine.channels.len(), 8);
        assert_eq!(NUM_CHANNELS, 8);
    }

    #[test]
    fn sample_rate_preserved() {
        let engine = AudioEngine::new(44100);
        assert_eq!(engine.sample_rate, 44100);
    }

    #[test]
    fn default_impl() {
        let engine = AudioEngine::default();
        assert_eq!(engine.sample_rate, 48000);
        assert_eq!(engine.channels.len(), 8);
    }
}
