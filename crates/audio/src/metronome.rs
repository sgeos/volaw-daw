/// Simple metronome click generator.
///
/// Produces short sine wave bursts at beat boundaries. Downbeat clicks
/// use a higher frequency (1500 Hz) and regular beat clicks use a lower
/// frequency (1000 Hz). The envelope is an exponential decay applied
/// per sample, producing a crisp transient click.
///
/// The metronome is rendered as a mono signal and mixed into both
/// stereo channels equally (center panned).
pub struct Metronome {
    /// Whether the metronome is audible.
    pub enabled: bool,
    /// Output volume (0.0 to 1.0).
    pub volume: f64,
    /// Phase accumulator for the sine oscillator.
    phase: f64,
    /// Phase increment per sample (frequency / sample_rate).
    phase_inc: f64,
    /// Current envelope amplitude (decays toward zero).
    envelope: f64,
    /// Exponential decay rate per sample.
    decay_rate: f64,
}

/// Click frequency for downbeat (beat 1 of the bar).
const DOWNBEAT_FREQ: f64 = 1500.0;

/// Click frequency for regular beats.
const BEAT_FREQ: f64 = 1000.0;

/// Click duration in seconds. The exponential envelope decays to near
/// silence well before this, but the value controls the decay rate.
const CLICK_DURATION_SECS: f64 = 0.025;

/// Envelope threshold below which the click is considered silent.
const SILENCE_THRESHOLD: f64 = 0.001;

impl Metronome {
    /// Create a new metronome, initially disabled.
    pub fn new() -> Self {
        Self {
            enabled: false,
            volume: 0.6,
            phase: 0.0,
            phase_inc: 0.0,
            envelope: 0.0,
            decay_rate: 0.0,
        }
    }

    /// Trigger a downbeat click (beat 1 of the bar).
    pub fn click_downbeat(&mut self, sample_rate: u32) {
        self.trigger(DOWNBEAT_FREQ, sample_rate);
    }

    /// Trigger a regular beat click.
    pub fn click_beat(&mut self, sample_rate: u32) {
        self.trigger(BEAT_FREQ, sample_rate);
    }

    /// Trigger a click at the given frequency.
    fn trigger(&mut self, freq: f64, sample_rate: u32) {
        self.phase = 0.0;
        self.phase_inc = freq / sample_rate as f64;
        self.envelope = 1.0;
        // Compute decay rate so envelope reaches ~0.001 after CLICK_DURATION_SECS.
        // envelope(t) = e^(-rate * t), solve for rate: rate = -ln(0.001) / duration.
        let total_samples = CLICK_DURATION_SECS * sample_rate as f64;
        self.decay_rate = libm::exp(-libm::log(SILENCE_THRESHOLD) / total_samples);
    }

    /// Whether the metronome is currently producing sound.
    pub fn is_active(&self) -> bool {
        self.envelope > SILENCE_THRESHOLD
    }

    /// Render mono samples into the provided buffer (additive).
    ///
    /// Adds click audio to the existing buffer contents. The caller
    /// is responsible for mixing this into both stereo channels.
    pub fn render(&mut self, buffer: &mut [f64]) {
        if !self.enabled || !self.is_active() {
            return;
        }

        let vol = self.volume;
        for sample in buffer.iter_mut() {
            let sine = libm::sin(self.phase * core::f64::consts::TAU);
            *sample += sine * self.envelope * vol;
            self.phase += self.phase_inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            self.envelope /= self.decay_rate;
        }

        if self.envelope <= SILENCE_THRESHOLD {
            self.envelope = 0.0;
        }
    }
}

impl Default for Metronome {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_metronome_is_disabled() {
        let m = Metronome::new();
        assert!(!m.enabled);
        assert!(!m.is_active());
    }

    #[test]
    fn trigger_activates_metronome() {
        let mut m = Metronome::new();
        m.enabled = true;
        m.click_beat(48000);
        assert!(m.is_active());
    }

    #[test]
    fn render_produces_nonzero_samples() {
        let mut m = Metronome::new();
        m.enabled = true;
        m.click_beat(48000);
        let mut buf = [0.0f64; 128];
        m.render(&mut buf);
        let max = buf.iter().fold(0.0f64, |a, &b| if b.abs() > a { b.abs() } else { a });
        assert!(max > 0.01, "expected nonzero audio, got max {max}");
    }

    #[test]
    fn render_does_nothing_when_disabled() {
        let mut m = Metronome::new();
        m.click_beat(48000);
        let mut buf = [0.0f64; 128];
        m.render(&mut buf);
        for s in &buf {
            assert!(s.abs() < f64::EPSILON);
        }
    }

    #[test]
    fn click_decays_to_silence() {
        let mut m = Metronome::new();
        m.enabled = true;
        m.click_beat(48000);
        // Render enough samples to decay (25ms at 48kHz = 1200 samples).
        let mut buf = [0.0f64; 2400];
        m.render(&mut buf);
        assert!(!m.is_active(), "envelope should have decayed to silence");
    }

    #[test]
    fn downbeat_higher_pitch_than_beat() {
        const { assert!(DOWNBEAT_FREQ > BEAT_FREQ) };
    }
}
