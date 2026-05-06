use crate::pedals::PedalChain;
use crate::position::PositionalStage;
use crate::sample::SampleInput;
use crate::synth::Ms20Voice;

/// Clamp a volume value to the range 0.0 to 1.0.
pub(crate) fn clamp_volume(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Configuration and state for one audio channel.
///
/// Each channel contains a complete signal chain: sample audio input,
/// virtual Korg MS-20 synthesizer, pre-positional pedal chain (mono),
/// positional stage (mono-to-stereo split), and post-positional pedal
/// chains (one per speaker). The channel has independent left and right
/// volume controls used as final trim levels.
///
/// ## Signal Chain
///
/// ```text
/// Sample Input --> MS-20 Voice --> Pre-Positional Pedals (mono)
///     --> Positional Stage (mono --> left, right)
///         --> Post-Positional Left Pedals --> * volume_left --> Left Mix
///         --> Post-Positional Right Pedals --> * volume_right --> Right Mix
/// ```
///
/// The sample input feeds audio into the MS-20 voice via its external
/// signal processor input. The MS-20 output is processed through the
/// pre-positional mono pedal chain, then split into stereo by the
/// positional stage (pan or 3D position). Each stereo side is then
/// processed through its own post-positional pedal chain before being
/// scaled by the per-channel volume trim levels.
///
/// ## Stereo Positioning
///
/// The `PositionalStage` handles spatial positioning (pan, 3D). When
/// inactive, mono is copied to both channels unchanged. The `volume_left`
/// and `volume_right` fields serve as final trim levels applied after
/// spatial processing.
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
    /// Left speaker volume trim (0.0 to 1.0, clamped on set).
    pub volume_left: f64,
    /// Right speaker volume trim (0.0 to 1.0, clamped on set).
    pub volume_right: f64,
    /// Whether this channel is producing audio.
    pub enabled: bool,
    /// Virtual Korg MS-20 synthesizer voice.
    pub synth: Ms20Voice,
    /// Audio sample input (feeds into the MS-20 signal chain).
    pub sample_in: SampleInput,
    /// Pre-positional mono pedal chain (processes MS-20 output before
    /// the stereo split).
    pub pre_pedals: PedalChain,
    /// Positional stage for mono-to-stereo conversion (pan, 3D position).
    pub position: PositionalStage,
    /// Post-positional left speaker pedal chain.
    pub post_pedals_left: PedalChain,
    /// Post-positional right speaker pedal chain.
    pub post_pedals_right: PedalChain,
}

impl Channel {
    /// Create a new channel with the given stereo trim volumes.
    ///
    /// Both volumes are clamped to the range 0.0 to 1.0.
    pub fn new(volume_left: f64, volume_right: f64) -> Self {
        Self {
            volume_left: clamp_volume(volume_left),
            volume_right: clamp_volume(volume_right),
            enabled: true,
            synth: Ms20Voice::new(),
            sample_in: SampleInput::new(),
            pre_pedals: PedalChain::new(),
            position: PositionalStage::default(),
            post_pedals_left: PedalChain::new(),
            post_pedals_right: PedalChain::new(),
        }
    }

    /// Create a channel routed to the left speaker only.
    pub fn left(volume: f64) -> Self {
        Self::new(volume, 0.0)
    }

    /// Create a channel routed to the right speaker only.
    pub fn right(volume: f64) -> Self {
        Self::new(0.0, volume)
    }

    /// Create a channel routed to both speakers equally.
    pub fn center(volume: f64) -> Self {
        Self::new(volume, volume)
    }

    /// Process the full signal chain into stereo output buffers.
    ///
    /// Runs: sample input, MS-20 synth, pre-positional pedals (mono),
    /// positional stage (mono to stereo), post-positional pedals (per speaker),
    /// and finally applies volume trim levels.
    ///
    /// When the channel is disabled, both output buffers are zeroed.
    ///
    /// The `mono_buf` is used as scratch space for the mono stage. It must
    /// be at least as long as `left_out` and `right_out`.
    pub fn process_stereo(
        &mut self,
        mono_buf: &mut [f64],
        left_out: &mut [f64],
        right_out: &mut [f64],
        sample_rate: u32,
    ) {
        let len = mono_buf.len().min(left_out.len()).min(right_out.len());

        if !self.enabled {
            for s in left_out[..len].iter_mut() {
                *s = 0.0;
            }
            for s in right_out[..len].iter_mut() {
                *s = 0.0;
            }
            return;
        }

        // Mono signal chain: sample input, MS-20 synth, pre-positional pedals.
        self.sample_in.process(&mut mono_buf[..len]);
        self.synth.process(&mut mono_buf[..len], sample_rate);
        debug_assert!(
            len == 0 || mono_buf[0].is_finite(),
            "Channel post-synth non-finite: {}",
            if len > 0 { mono_buf[0] } else { 0.0 }
        );
        self.pre_pedals.process(&mut mono_buf[..len], sample_rate);

        // Mono-to-stereo split via positional stage.
        self.position.process(
            &mono_buf[..len],
            &mut left_out[..len],
            &mut right_out[..len],
            sample_rate as f64,
        );

        // Post-positional per-speaker pedal chains.
        self.post_pedals_left
            .process(&mut left_out[..len], sample_rate);
        self.post_pedals_right
            .process(&mut right_out[..len], sample_rate);

        // Apply per-channel volume trim.
        let vol_l = self.volume_left;
        let vol_r = self.volume_right;
        for s in left_out[..len].iter_mut() {
            *s *= vol_l;
        }
        for s in right_out[..len].iter_mut() {
            *s *= vol_r;
        }
        debug_assert!(
            len == 0 || (left_out[0].is_finite() && right_out[0].is_finite()),
            "Channel post-trim non-finite: L={} R={}",
            if len > 0 { left_out[0] } else { 0.0 },
            if len > 0 { right_out[0] } else { 0.0 }
        );
    }

    /// Set the left volume trim. Clamped to 0.0 to 1.0.
    pub fn set_volume_left(&mut self, volume: f64) {
        self.volume_left = clamp_volume(volume);
    }

    /// Set the right volume trim. Clamped to 0.0 to 1.0.
    pub fn set_volume_right(&mut self, volume: f64) {
        self.volume_right = clamp_volume(volume);
    }

    /// Set both volume trims simultaneously. Both clamped to 0.0 to 1.0.
    pub fn set_volume(&mut self, left: f64, right: f64) {
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
        assert!((ch.volume_left - 0.8).abs() < f64::EPSILON);
        assert!(ch.volume_right.abs() < f64::EPSILON);
        assert!(ch.enabled);
    }

    #[test]
    fn right_channel() {
        let ch = Channel::right(0.7);
        assert!(ch.volume_left.abs() < f64::EPSILON);
        assert!((ch.volume_right - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn center_channel() {
        let ch = Channel::center(0.5);
        assert!((ch.volume_left - 0.5).abs() < f64::EPSILON);
        assert!((ch.volume_right - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn volume_clamping() {
        let ch = Channel::new(1.5, -0.5);
        assert!((ch.volume_left - 1.0).abs() < f64::EPSILON);
        assert!(ch.volume_right.abs() < f64::EPSILON);
    }

    #[test]
    fn set_volume_left_clamps() {
        let mut ch = Channel::center(0.5);
        ch.set_volume_left(2.0);
        assert!((ch.volume_left - 1.0).abs() < f64::EPSILON);
        ch.set_volume_left(-1.0);
        assert!(ch.volume_left.abs() < f64::EPSILON);
    }

    #[test]
    fn set_volume_right_clamps() {
        let mut ch = Channel::center(0.5);
        ch.set_volume_right(2.0);
        assert!((ch.volume_right - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn set_volume_both() {
        let mut ch = Channel::center(0.5);
        ch.set_volume(0.3, 0.7);
        assert!((ch.volume_left - 0.3).abs() < f64::EPSILON);
        assert!((ch.volume_right - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn default_is_center() {
        let ch = Channel::default();
        assert!((ch.volume_left - 0.8).abs() < f64::EPSILON);
        assert!((ch.volume_right - 0.8).abs() < f64::EPSILON);
        assert!(ch.enabled);
    }

    #[test]
    fn has_synth_sample_pedals() {
        let ch = Channel::center(0.5);
        assert!(!ch.synth.external_input_enabled);
        assert!(!ch.sample_in.active);
        assert!(!ch.pre_pedals.active);
        assert!(!ch.post_pedals_left.active);
        assert!(!ch.post_pedals_right.active);
        assert!(!ch.position.active);
    }

    #[test]
    fn disabled_channel_zeros_stereo() {
        let mut ch = Channel::center(0.5);
        ch.enabled = false;
        let mut mono = [1.0f64; 16];
        let mut left = [1.0f64; 16];
        let mut right = [1.0f64; 16];
        ch.process_stereo(&mut mono, &mut left, &mut right, 48000);
        for s in &left {
            assert!(s.abs() < f64::EPSILON);
        }
        for s in &right {
            assert!(s.abs() < f64::EPSILON);
        }
    }

    #[test]
    fn enabled_channel_stereo_silence() {
        let mut ch = Channel::center(0.5);
        ch.enabled = true;
        // With stubs (synth writes silence, pedals and position passthrough),
        // the output should be silence on both channels.
        let mut mono = [0.5f64; 16];
        let mut left = [0.0f64; 16];
        let mut right = [0.0f64; 16];
        ch.process_stereo(&mut mono, &mut left, &mut right, 48000);
        for s in &left {
            assert!(s.abs() < f64::EPSILON, "Synth stub should write silence");
        }
        for s in &right {
            assert!(s.abs() < f64::EPSILON, "Synth stub should write silence");
        }
    }
}
