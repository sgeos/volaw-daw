use core::num::NonZeroU16;

use crate::midi::rescale_tick;

/// Below the smallest fraction of a tick that one advance can add, and far
/// above the rounding error of the floating point sum, so that a count that
/// should be whole is not held back a tick by that error.
const TICK_EPSILON: f64 = 1e-9;

/// Playback transport state.
///
/// Tracks the current playback position, tempo, and play/stop state.
/// Position is measured in ticks for precision, converted to seconds
/// for audio rendering.
#[derive(Debug, Clone)]
pub struct Transport {
    /// Tempo in beats per minute.
    pub tempo: f64,
    /// Current position in ticks.
    pub position_ticks: u64,
    /// Ticks per beat (matches MIDI resolution).
    pub ticks_per_beat: u16,
    /// Whether playback is active.
    pub playing: bool,
    /// Loop start in ticks (0 means no loop start set).
    pub loop_start: u64,
    /// Loop end in ticks (0 means no loop end set).
    pub loop_end: u64,
    /// Whether looping is enabled.
    pub loop_enabled: bool,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Beats per bar (time signature numerator). Default is 4.
    pub beats_per_bar: u8,
    /// Time signature denominator (power of 2: 1, 2, 4, 8, 16, 32). Default is 4.
    pub beat_denominator: u8,
    /// The fraction of a tick that advances covered but have not counted,
    /// in `0.0..1.0`. Carried so that an advance shorter than a whole number
    /// of ticks, such as a frame at 96 ticks per beat, does not slow
    /// playback by dropping it.
    tick_remainder: f64,
}

impl Transport {
    /// Create a new transport with default settings.
    pub fn new(sample_rate: u32, ticks_per_beat: u16) -> Self {
        Self {
            tempo: 120.0,
            position_ticks: 0,
            ticks_per_beat,
            playing: false,
            loop_start: 0,
            loop_end: 0,
            loop_enabled: false,
            sample_rate,
            beats_per_bar: 4,
            beat_denominator: 4,
            tick_remainder: 0.0,
        }
    }

    /// Count ticks at `ticks_per_beat` from now on, converting the position
    /// and the loop points so that they stay on the same beats. Each is
    /// rounded half up, as `MidiTrack::rescale` rounds a note, so a loop
    /// point and a note at the same time before stay at the same time after.
    /// A conversion to a coarser resolution can bring a short loop's ends
    /// together, and the loop then no longer wraps. The fraction of a tick
    /// carried between advances is dropped, which delays playback by less
    /// than one tick once. From a transport at 0 ticks per beat, which counts
    /// nothing, the new resolution is adopted without conversion.
    ///
    /// # Errors
    ///
    /// If `ticks_per_beat` is 0, which counts no time. The transport is then
    /// unchanged.
    pub fn set_ticks_per_beat(&mut self, ticks_per_beat: u16) -> Result<(), &'static str> {
        let Some(to) = NonZeroU16::new(ticks_per_beat) else {
            return Err("a transport cannot count 0 ticks per beat");
        };
        if let Some(from) = NonZeroU16::new(self.ticks_per_beat)
            && from != to
        {
            self.position_ticks = rescale_tick(self.position_ticks, from, to);
            self.loop_start = rescale_tick(self.loop_start, from, to);
            self.loop_end = rescale_tick(self.loop_end, from, to);
            self.tick_remainder = 0.0;
        }
        self.ticks_per_beat = ticks_per_beat;
        Ok(())
    }

    /// Convert ticks to seconds at the current tempo.
    pub fn ticks_to_seconds(&self, ticks: u64) -> f64 {
        let beats = ticks as f64 / self.ticks_per_beat as f64;
        beats * 60.0 / self.tempo
    }

    /// Convert seconds to ticks at the current tempo.
    pub fn seconds_to_ticks(&self, seconds: f64) -> u64 {
        let beats = seconds * self.tempo / 60.0;
        (beats * self.ticks_per_beat as f64) as u64
    }

    /// Return the current position in seconds.
    pub fn position_seconds(&self) -> f64 {
        self.ticks_to_seconds(self.position_ticks)
    }

    /// Return the current position as (bar, beat, tick) using the current
    /// time signature. Beat size depends on the denominator.
    pub fn position_bbt(&self) -> (u64, u64, u64) {
        let bpb = self.beats_per_bar as u64;
        let tpbu = self.ticks_per_beat_unit();
        let total_beats = self.position_ticks / tpbu;
        let bar = total_beats / bpb;
        let beat = total_beats % bpb;
        let tick = self.position_ticks % tpbu;
        (bar + 1, beat + 1, tick)
    }

    /// Return ticks per beat-unit based on the time signature denominator.
    /// ticks_per_beat is ticks per quarter note (MIDI standard). The denominator
    /// selects the note value that counts as one beat: 4 = quarter, 8 = eighth, etc.
    pub fn ticks_per_beat_unit(&self) -> u64 {
        self.ticks_per_beat as u64 * 4 / self.beat_denominator.max(1) as u64
    }

    /// Return ticks per bar based on the current time signature.
    pub fn ticks_per_bar(&self) -> u64 {
        self.ticks_per_beat_unit() * self.beats_per_bar as u64
    }

    /// Advance the transport by the given number of audio samples.
    /// Returns the number of ticks advanced.
    pub fn advance_samples(&mut self, num_samples: u32) -> u64 {
        if !self.playing {
            return 0;
        }

        let seconds = num_samples as f64 / self.sample_rate as f64;
        let exact = seconds * self.tempo / 60.0 * self.ticks_per_beat as f64 + self.tick_remainder;
        if !exact.is_finite() || exact <= 0.0 {
            // A sample rate of 0 or a tempo that is not positive counts no
            // time rather than an unbounded amount.
            self.tick_remainder = 0.0;
            return 0;
        }
        let whole = libm::floor(exact + TICK_EPSILON);
        self.tick_remainder = (exact - whole).max(0.0);
        // Saturates for a count past u64::MAX, which no real advance reaches.
        let ticks = whole as u64;
        self.position_ticks = self.position_ticks.saturating_add(ticks);

        // Handle looping.
        if self.loop_enabled
            && self.loop_end > self.loop_start
            && self.position_ticks >= self.loop_end
        {
            let loop_len = self.loop_end - self.loop_start;
            self.position_ticks =
                self.loop_start + (self.position_ticks - self.loop_end) % loop_len;
        }

        ticks
    }

    /// Start playback.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Stop playback (pause at current position).
    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// Stop playback and reset to the beginning.
    pub fn reset(&mut self) {
        self.playing = false;
        self.position_ticks = 0;
        self.tick_remainder = 0.0;
    }

    /// Number of audio samples per tick at the current tempo and sample rate.
    pub fn samples_per_tick(&self) -> f64 {
        let seconds_per_tick = 60.0 / (self.tempo * self.ticks_per_beat as f64);
        seconds_per_tick * self.sample_rate as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_to_seconds_at_120_bpm() {
        let transport = Transport::new(48000, 480);
        // At 120 BPM, one beat = 0.5 seconds. 480 ticks = 1 beat = 0.5s.
        let secs = transport.ticks_to_seconds(480);
        assert!((secs - 0.5).abs() < 0.001);
    }

    #[test]
    fn seconds_to_ticks_at_120_bpm() {
        let transport = Transport::new(48000, 480);
        let ticks = transport.seconds_to_ticks(0.5);
        assert_eq!(ticks, 480);
    }

    #[test]
    fn position_bbt_start() {
        let transport = Transport::new(48000, 480);
        let (bar, beat, tick) = transport.position_bbt();
        assert_eq!((bar, beat, tick), (1, 1, 0));
    }

    #[test]
    fn position_bbt_second_beat() {
        let mut transport = Transport::new(48000, 480);
        transport.position_ticks = 480;
        let (bar, beat, tick) = transport.position_bbt();
        assert_eq!((bar, beat, tick), (1, 2, 0));
    }

    #[test]
    fn position_bbt_second_bar() {
        let mut transport = Transport::new(48000, 480);
        transport.position_ticks = 480 * 4; // 4 beats = 1 bar in 4/4.
        let (bar, beat, tick) = transport.position_bbt();
        assert_eq!((bar, beat, tick), (2, 1, 0));
    }

    #[test]
    fn advance_while_stopped_does_nothing() {
        let mut transport = Transport::new(48000, 480);
        let ticks = transport.advance_samples(48000);
        assert_eq!(ticks, 0);
        assert_eq!(transport.position_ticks, 0);
    }

    #[test]
    fn advance_while_playing() {
        let mut transport = Transport::new(48000, 480);
        transport.playing = true;
        // At 120 BPM, 48000 samples = 1 second = 2 beats = 960 ticks.
        let ticks = transport.advance_samples(48000);
        assert_eq!(ticks, 960);
        assert_eq!(transport.position_ticks, 960);
    }

    #[test]
    fn loop_wraps_position() {
        let mut transport = Transport::new(48000, 480);
        transport.playing = true;
        transport.loop_enabled = true;
        transport.loop_start = 0;
        transport.loop_end = 960; // 2 beats.
        // Advance 3 beats worth (1440 ticks). Should wrap around.
        transport.position_ticks = 900;
        transport.advance_samples(48000); // +960 ticks = 1860, wraps to 1860 - 960 = 900.
        assert!(
            transport.position_ticks < 960,
            "position {} should be < 960",
            transport.position_ticks
        );
    }

    #[test]
    fn samples_per_tick_at_120() {
        let transport = Transport::new(48000, 480);
        // At 120 BPM, 480 tpb: seconds_per_tick = 60/(120*480) = 1/960
        // samples_per_tick = 48000/960 = 50
        let spt = transport.samples_per_tick();
        assert!((spt - 50.0).abs() < 0.01);
    }

    #[test]
    fn position_bbt_three_four_time() {
        let mut transport = Transport::new(48000, 480);
        transport.beats_per_bar = 3;
        transport.position_ticks = 480 * 3; // 3 beats = 1 bar in 3/4.
        let (bar, beat, tick) = transport.position_bbt();
        assert_eq!((bar, beat, tick), (2, 1, 0));
    }

    #[test]
    fn ticks_per_bar_four_four() {
        let transport = Transport::new(48000, 480);
        assert_eq!(transport.ticks_per_bar(), 1920);
    }

    #[test]
    fn ticks_per_bar_three_four() {
        let mut transport = Transport::new(48000, 480);
        transport.beats_per_bar = 3;
        assert_eq!(transport.ticks_per_bar(), 1440);
    }

    #[test]
    fn ticks_per_bar_six_eight() {
        let mut transport = Transport::new(48000, 480);
        transport.beats_per_bar = 6;
        transport.beat_denominator = 8;
        // 6 eighth-note beats per bar. Each eighth = 480*4/8 = 240 ticks.
        assert_eq!(transport.ticks_per_bar(), 1440);
    }

    #[test]
    fn ticks_per_bar_three_two() {
        let mut transport = Transport::new(48000, 480);
        transport.beats_per_bar = 3;
        transport.beat_denominator = 2;
        // 3 half-note beats per bar. Each half = 480*4/2 = 960 ticks.
        assert_eq!(transport.ticks_per_bar(), 2880);
    }

    #[test]
    fn position_bbt_six_eight() {
        let mut transport = Transport::new(48000, 480);
        transport.beats_per_bar = 6;
        transport.beat_denominator = 8;
        // 6 eighth-note beats = 1 bar = 1440 ticks.
        transport.position_ticks = 1440;
        let (bar, beat, tick) = transport.position_bbt();
        assert_eq!((bar, beat, tick), (2, 1, 0));
    }

    /// Frames of one sixtieth of a second, as the composer advances, cover
    /// exactly the beats their time holds, whatever the resolution and
    /// tempo. Without the carried remainder, a frame at 96 ticks per beat
    /// and 120 beats per minute counts 3 ticks instead of 3.2, and plays
    /// 6 percent slow. The expected count is the exact one, rounded down.
    #[test]
    fn frames_keep_time_at_any_resolution() {
        const RATE: u32 = 48_000;
        const FRAME: u32 = RATE / 60;
        for ticks_per_beat in [1u16, 96, 100, 480, 960] {
            for tempo in [97u32, 100, 120, 300] {
                let mut transport = Transport::new(RATE, ticks_per_beat);
                transport.tempo = f64::from(tempo);
                transport.play();
                for frames in 1..=600u64 {
                    transport.advance_samples(FRAME);
                    let exact =
                        frames * u64::from(FRAME) * u64::from(tempo) * u64::from(ticks_per_beat)
                            / (60 * u64::from(RATE));
                    assert_eq!(
                        transport.position_ticks, exact,
                        "{ticks_per_beat} ticks per beat at {tempo} after {frames} frames"
                    );
                }
            }
        }
    }

    #[test]
    fn advance_without_a_sample_rate_counts_nothing() {
        let mut transport = Transport::new(0, 480);
        transport.play();
        assert_eq!(transport.advance_samples(800), 0);
        assert_eq!(transport.position_ticks, 0);
    }

    /// The position and the loop points keep their beats, rounded half up as
    /// a note is, and 0 ticks per beat is refused.
    #[test]
    fn a_new_resolution_converts_the_position_and_the_loop() {
        let mut transport = Transport::new(48_000, 480);
        transport.position_ticks = 962;
        transport.loop_start = 1920;
        transport.loop_end = 1920 * 3;
        transport.set_ticks_per_beat(96).unwrap();
        assert_eq!(transport.ticks_per_beat, 96);
        // 962 / 5 is 192.4, which rounds down.
        assert_eq!(
            (
                transport.position_ticks,
                transport.loop_start,
                transport.loop_end
            ),
            (192, 384, 1152)
        );
        assert_eq!(transport.ticks_per_bar(), 384);

        let before = (transport.ticks_per_beat, transport.position_ticks);
        assert!(transport.set_ticks_per_beat(0).is_err());
        assert_eq!((transport.ticks_per_beat, transport.position_ticks), before);

        let mut uncounted = Transport::new(48_000, 0);
        uncounted.position_ticks = 7;
        uncounted.set_ticks_per_beat(96).unwrap();
        assert_eq!(
            (uncounted.ticks_per_beat, uncounted.position_ticks),
            (96, 7)
        );
    }
}
