use alloc::vec;
use alloc::vec::Vec;

use crate::channel::{Channel, clamp_volume};
use crate::limiter::LookaheadLimiter;
use crate::metronome::Metronome;

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
/// - **Pedal Chain**: Virtual guitar pedal effects chain with 29 effect
///   types across 5 categories (distortion, modulation, delay, special, utility).
///
/// ## Default Configuration
///
/// The default layout is a "three-man band" per speaker with two SFX channels:
/// channels 0-2 routed left, channels 3-5 routed right, channels 6-7 center.
pub struct AudioEngine {
    /// The eight synthesizer channels.
    pub channels: [Channel; NUM_CHANNELS],
    /// Global master volume applied after channel mixing (0.0 to 1.0).
    pub master_volume: f64,
    /// Audio sample rate in Hz.
    pub sample_rate: u32,
    /// Metronome click generator (mixed into output independently of channels).
    pub metronome: Metronome,
    /// Master bus lookahead brickwall limiter.
    pub limiter: LookaheadLimiter,
    /// Scratch buffer for per-channel mono rendering.
    mono_buffer: Vec<f64>,
    /// Scratch buffer for per-channel left stereo output.
    left_buffer: Vec<f64>,
    /// Scratch buffer for per-channel right stereo output.
    right_buffer: Vec<f64>,
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
            metronome: Metronome::new(),
            limiter: LookaheadLimiter::new(sample_rate as f64),
            mono_buffer: vec![0.0f64; 1024],
            left_buffer: vec![0.0f64; 1024],
            right_buffer: vec![0.0f64; 1024],
        }
    }

    /// Fill a stereo interleaved f64 sample buffer (L, R, L, R, ...).
    ///
    /// Called by the platform audio callback at the configured sample rate.
    /// The buffer length must be even (pairs of left and right samples).
    ///
    /// Each enabled channel is processed through its full signal chain:
    /// sample input, MS-20 voice, pre-positional pedals (mono), positional
    /// stage (mono to stereo), post-positional pedals (per speaker), and
    /// volume trim. The master volume is applied as a final scaling step.
    pub fn render_samples(&mut self, buffer: &mut [f64]) {
        let num_frames = buffer.len() / 2;
        if num_frames == 0 {
            return;
        }

        // Zero the output buffer.
        for s in buffer.iter_mut() {
            *s = 0.0;
        }

        // Ensure scratch buffers are large enough.
        if self.mono_buffer.len() < num_frames {
            self.mono_buffer.resize(num_frames, 0.0);
        }
        if self.left_buffer.len() < num_frames {
            self.left_buffer.resize(num_frames, 0.0);
        }
        if self.right_buffer.len() < num_frames {
            self.right_buffer.resize(num_frames, 0.0);
        }

        let sample_rate = self.sample_rate;

        // Process each channel through the full stereo signal chain.
        for ch in &mut self.channels {
            if !ch.enabled {
                continue;
            }

            // Zero the scratch buffers for this channel.
            for s in self.mono_buffer[..num_frames].iter_mut() {
                *s = 0.0;
            }
            for s in self.left_buffer[..num_frames].iter_mut() {
                *s = 0.0;
            }
            for s in self.right_buffer[..num_frames].iter_mut() {
                *s = 0.0;
            }

            // Run the full stereo signal chain (mono stages, positional
            // split, post-positional pedals, volume trim all applied
            // inside Channel::process_stereo).
            ch.process_stereo(
                &mut self.mono_buffer[..num_frames],
                &mut self.left_buffer[..num_frames],
                &mut self.right_buffer[..num_frames],
                sample_rate,
            );

            // Mix channel stereo output into the interleaved output buffer.
            for i in 0..num_frames {
                buffer[i * 2] += self.left_buffer[i];
                buffer[i * 2 + 1] += self.right_buffer[i];
            }
        }

        debug_assert!(
            num_frames == 0 || (buffer[0].is_finite() && buffer[1].is_finite()),
            "Engine post-mix non-finite: L={} R={}",
            if num_frames > 0 { buffer[0] } else { 0.0 },
            if num_frames > 0 { buffer[1] } else { 0.0 }
        );

        // Mix metronome click (center-panned, added to both channels).
        if self.metronome.enabled && self.metronome.is_active() {
            // Render metronome mono into the mono scratch buffer.
            for s in self.mono_buffer[..num_frames].iter_mut() {
                *s = 0.0;
            }
            self.metronome.render(&mut self.mono_buffer[..num_frames]);
            for i in 0..num_frames {
                let click = self.mono_buffer[i];
                buffer[i * 2] += click;
                buffer[i * 2 + 1] += click;
            }
        }

        // Apply master volume.
        let mv = self.master_volume;
        for s in buffer.iter_mut() {
            *s *= mv;
        }

        debug_assert!(
            num_frames == 0 || (buffer[0].is_finite() && buffer[1].is_finite()),
            "Engine post-master-volume non-finite: L={} R={}",
            if num_frames > 0 { buffer[0] } else { 0.0 },
            if num_frames > 0 { buffer[1] } else { 0.0 }
        );

        // Brickwall limiter: prevent output from exceeding 0 dBFS.
        self.limiter.process_interleaved(buffer);
    }

    /// Fill a stereo interleaved f32 buffer for SDL3 interoperability.
    ///
    /// Renders internally at f64 precision and converts to f32 on output.
    /// This is the primary entry point for platform audio callbacks that
    /// require f32 output (such as SDL3).
    pub fn render_samples_f32(&mut self, buffer: &mut [f32]) {
        let num_frames = buffer.len() / 2;
        if num_frames == 0 {
            return;
        }

        // Allocate or reuse an f64 staging buffer.
        let total_samples = buffer.len();
        let mut f64_buf = vec![0.0_f64; total_samples];

        self.render_samples(&mut f64_buf);

        // Convert f64 to f32 for the output.
        for (out, &val) in buffer.iter_mut().zip(f64_buf.iter()) {
            *out = val as f32;
        }
    }

    /// Set the left volume for a specific channel.
    ///
    /// Volume is clamped to the range 0.0 to 1.0.
    /// Out-of-bounds channel indices are silently ignored.
    pub fn set_channel_volume_left(&mut self, channel: usize, volume: f64) {
        if channel < NUM_CHANNELS {
            self.channels[channel].set_volume_left(volume);
        }
    }

    /// Set the right volume for a specific channel.
    ///
    /// Volume is clamped to the range 0.0 to 1.0.
    /// Out-of-bounds channel indices are silently ignored.
    pub fn set_channel_volume_right(&mut self, channel: usize, volume: f64) {
        if channel < NUM_CHANNELS {
            self.channels[channel].set_volume_right(volume);
        }
    }

    /// Set both left and right volumes for a specific channel.
    ///
    /// Both volumes are clamped to the range 0.0 to 1.0.
    /// Out-of-bounds channel indices are silently ignored.
    pub fn set_channel_volume(&mut self, channel: usize, left: f64, right: f64) {
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
    pub fn set_master_volume(&mut self, volume: f64) {
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
            assert!((engine.channels[i].volume_left - 0.8).abs() < f64::EPSILON);
            assert!(engine.channels[i].volume_right.abs() < f64::EPSILON);
        }
        // Right channels (3-5)
        for i in 3..6 {
            assert!(engine.channels[i].volume_left.abs() < f64::EPSILON);
            assert!((engine.channels[i].volume_right - 0.8).abs() < f64::EPSILON);
        }
        // Center SFX channels (6-7)
        for i in 6..8 {
            assert!((engine.channels[i].volume_left - 0.8).abs() < f64::EPSILON);
            assert!((engine.channels[i].volume_right - 0.8).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn default_volume() {
        let engine = AudioEngine::new(48000);
        assert!((engine.master_volume - 0.8).abs() < f64::EPSILON);
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
        assert!((engine.channels[0].volume_left - 1.0).abs() < f64::EPSILON);
        engine.set_channel_volume_left(0, -0.5);
        assert!(engine.channels[0].volume_left.abs() < f64::EPSILON);
    }

    #[test]
    fn set_channel_volume_right_clamps() {
        let mut engine = AudioEngine::new(48000);
        engine.set_channel_volume_right(3, 2.0);
        assert!((engine.channels[3].volume_right - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn set_channel_volume_both() {
        let mut engine = AudioEngine::new(48000);
        engine.set_channel_volume(6, 0.3, 0.7);
        assert!((engine.channels[6].volume_left - 0.3).abs() < f64::EPSILON);
        assert!((engine.channels[6].volume_right - 0.7).abs() < f64::EPSILON);
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
    fn render_samples_writes_silence_with_stubs() {
        let mut engine = AudioEngine::new(48000);
        let mut buffer = [1.0f64; 64];
        engine.render_samples(&mut buffer);
        // With synth stubs (all channels write silence), output should be zero
        for sample in &buffer {
            assert!(sample.abs() < f64::EPSILON);
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
        assert!((engine.master_volume - 1.0).abs() < f64::EPSILON);
        engine.set_master_volume(-1.0);
        assert!(engine.master_volume.abs() < f64::EPSILON);
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

    #[test]
    fn render_empty_buffer() {
        let mut engine = AudioEngine::new(48000);
        let mut buffer = [];
        engine.render_samples(&mut buffer);
        // No panic on empty buffer
    }

    #[test]
    fn render_grows_mono_buffer() {
        let mut engine = AudioEngine::new(48000);
        // Render a large buffer to trigger mono_buffer growth
        let mut buffer = alloc::vec![0.0f64; 4096];
        engine.render_samples(&mut buffer);
        assert!(engine.mono_buffer.len() >= 2048);
    }
}
