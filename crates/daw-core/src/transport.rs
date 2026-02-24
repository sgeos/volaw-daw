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
        }
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

    /// Return the current position as (bar, beat, tick) assuming 4/4 time.
    pub fn position_bbt(&self) -> (u64, u64, u64) {
        let total_beats = self.position_ticks / self.ticks_per_beat as u64;
        let bar = total_beats / 4;
        let beat = total_beats % 4;
        let tick = self.position_ticks % self.ticks_per_beat as u64;
        (bar + 1, beat + 1, tick)
    }

    /// Advance the transport by the given number of audio samples.
    /// Returns the number of ticks advanced.
    pub fn advance_samples(&mut self, num_samples: u32) -> u64 {
        if !self.playing {
            return 0;
        }

        let seconds = num_samples as f64 / self.sample_rate as f64;
        let ticks = self.seconds_to_ticks(seconds);
        self.position_ticks += ticks;

        // Handle looping.
        if self.loop_enabled && self.loop_end > self.loop_start && self.position_ticks >= self.loop_end {
            let loop_len = self.loop_end - self.loop_start;
            self.position_ticks = self.loop_start + (self.position_ticks - self.loop_end) % loop_len;
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
        assert!(transport.position_ticks < 960, "position {} should be < 960", transport.position_ticks);
    }

    #[test]
    fn samples_per_tick_at_120() {
        let transport = Transport::new(48000, 480);
        // At 120 BPM, 480 tpb: seconds_per_tick = 60/(120*480) = 1/960
        // samples_per_tick = 48000/960 = 50
        let spt = transport.samples_per_tick();
        assert!((spt - 50.0).abs() < 0.01);
    }
}
