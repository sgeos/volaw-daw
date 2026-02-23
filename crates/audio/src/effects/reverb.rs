//! Reverb effects: Plate, Hall, Room.
//!
//! All three reverb types use the Schroeder/Moorer algorithm:
//! four parallel feedback comb filters feeding into two series
//! allpass diffusers. Different delay lengths and feedback values
//! create the characteristic sound of each space type.

use crate::dsp::delay_line::DelayLine;

/// Feedback comb filter with one-pole lowpass damping.
#[derive(Debug, Clone)]
struct CombFilter {
    delay: DelayLine,
    feedback: f32,
    damping: f32,
    prev_filtered: f32,
}

impl CombFilter {
    fn new(delay_samples: usize, feedback: f32, damping: f32) -> Self {
        Self {
            delay: DelayLine::new(delay_samples + 1),
            feedback,
            damping,
            prev_filtered: 0.0,
        }
    }

    #[inline]
    fn tick(&mut self, input: f32) -> f32 {
        let delayed = self.delay.read_integer(self.delay.len() - 1);
        // One-pole lowpass in feedback path for damping
        let filtered = delayed * (1.0 - self.damping) + self.prev_filtered * self.damping;
        self.prev_filtered = filtered;
        self.delay.write(input + self.feedback * filtered);
        delayed
    }

    fn reset(&mut self) {
        self.delay.reset();
        self.prev_filtered = 0.0;
    }
}

/// Schroeder allpass section for diffusion.
#[derive(Debug, Clone)]
struct SchroederAllpass {
    delay: DelayLine,
    gain: f32,
}

impl SchroederAllpass {
    fn new(delay_samples: usize, gain: f32) -> Self {
        Self {
            delay: DelayLine::new(delay_samples + 1),
            gain,
        }
    }

    #[inline]
    fn tick(&mut self, input: f32) -> f32 {
        let delayed = self.delay.read_integer(self.delay.len() - 1);
        let v = input + self.gain * delayed;
        let output = delayed - self.gain * v;
        self.delay.write(v);
        output
    }

    fn reset(&mut self) {
        self.delay.reset();
    }
}

/// Common reverb implementation used by Plate, Hall, and Room variants.
#[derive(Debug, Clone)]
struct ReverbCore {
    combs: [CombFilter; 4],
    allpasses: [SchroederAllpass; 2],
}

impl ReverbCore {
    fn new(
        comb_delays: [usize; 4],
        comb_feedback: f32,
        damping: f32,
        ap_delays: [usize; 2],
        ap_gain: f32,
    ) -> Self {
        Self {
            combs: [
                CombFilter::new(comb_delays[0], comb_feedback, damping),
                CombFilter::new(comb_delays[1], comb_feedback, damping),
                CombFilter::new(comb_delays[2], comb_feedback, damping),
                CombFilter::new(comb_delays[3], comb_feedback, damping),
            ],
            allpasses: [
                SchroederAllpass::new(ap_delays[0], ap_gain),
                SchroederAllpass::new(ap_delays[1], ap_gain),
            ],
        }
    }

    fn tick(&mut self, input: f32) -> f32 {
        // Parallel comb filters
        let mut sum = 0.0f32;
        for comb in &mut self.combs {
            sum += comb.tick(input);
        }
        sum *= 0.25; // Average the four combs
        // Series allpass diffusers
        let mut out = sum;
        for ap in &mut self.allpasses {
            out = ap.tick(out);
        }
        out
    }

    fn reset(&mut self) {
        for c in &mut self.combs {
            c.reset();
        }
        for a in &mut self.allpasses {
            a.reset();
        }
    }
}

/// Milliseconds to samples at a given sample rate.
fn ms_to_samples(ms: f32, sample_rate: f32) -> usize {
    (ms * 0.001 * sample_rate) as usize
}

/// Plate reverb: bright, dense, relatively short.
///
/// Based on the classic EMT 140 plate reverb character.
#[derive(Debug, Clone)]
pub struct PlateReverb {
    /// Decay time (0.0 = short, 1.0 = long).
    pub decay: f32,
    /// High-frequency damping (0.0 = bright, 1.0 = dark).
    pub damping: f32,
    /// Wet/dry mix.
    pub mix: f32,
    core: ReverbCore,
}

impl PlateReverb {
    pub fn new(sample_rate: f32) -> Self {
        let feedback = 0.7;
        let damp = 0.3;
        Self {
            decay: 0.5,
            damping: 0.3,
            mix: 0.3,
            core: ReverbCore::new(
                [
                    ms_to_samples(29.7, sample_rate),
                    ms_to_samples(37.1, sample_rate),
                    ms_to_samples(41.1, sample_rate),
                    ms_to_samples(43.7, sample_rate),
                ],
                feedback,
                damp,
                [
                    ms_to_samples(5.0, sample_rate),
                    ms_to_samples(1.7, sample_rate),
                ],
                0.5,
            ),
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], _sample_rate: f32) {
        // Scale feedback by decay parameter
        let fb = 0.5 + self.decay * 0.45;
        for comb in &mut self.core.combs {
            comb.feedback = fb;
            comb.damping = self.damping;
        }
        for sample in buffer.iter_mut() {
            let dry = *sample;
            let wet = self.core.tick(dry);
            *sample = dry * (1.0 - self.mix) + wet * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.core.reset();
    }
}

/// Hall reverb: spacious, long decay.
#[derive(Debug, Clone)]
pub struct HallReverb {
    /// Decay time (0.0 = short, 1.0 = long).
    pub decay: f32,
    /// High-frequency damping.
    pub damping: f32,
    /// Wet/dry mix.
    pub mix: f32,
    core: ReverbCore,
}

impl HallReverb {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            decay: 0.6,
            damping: 0.4,
            mix: 0.3,
            core: ReverbCore::new(
                [
                    ms_to_samples(47.3, sample_rate),
                    ms_to_samples(57.1, sample_rate),
                    ms_to_samples(62.3, sample_rate),
                    ms_to_samples(67.7, sample_rate),
                ],
                0.75,
                0.4,
                [
                    ms_to_samples(10.0, sample_rate),
                    ms_to_samples(3.4, sample_rate),
                ],
                0.5,
            ),
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], _sample_rate: f32) {
        let fb = 0.5 + self.decay * 0.47;
        for comb in &mut self.core.combs {
            comb.feedback = fb;
            comb.damping = self.damping;
        }
        for sample in buffer.iter_mut() {
            let dry = *sample;
            let wet = self.core.tick(dry);
            *sample = dry * (1.0 - self.mix) + wet * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.core.reset();
    }
}

/// Room reverb: short, intimate, natural.
#[derive(Debug, Clone)]
pub struct RoomReverb {
    /// Decay time (0.0 = tight, 1.0 = loose).
    pub decay: f32,
    /// High-frequency damping.
    pub damping: f32,
    /// Wet/dry mix.
    pub mix: f32,
    core: ReverbCore,
}

impl RoomReverb {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            decay: 0.3,
            damping: 0.5,
            mix: 0.2,
            core: ReverbCore::new(
                [
                    ms_to_samples(17.3, sample_rate),
                    ms_to_samples(21.1, sample_rate),
                    ms_to_samples(23.7, sample_rate),
                    ms_to_samples(27.3, sample_rate),
                ],
                0.6,
                0.5,
                [
                    ms_to_samples(3.4, sample_rate),
                    ms_to_samples(1.1, sample_rate),
                ],
                0.5,
            ),
        }
    }

    pub fn process(&mut self, buffer: &mut [f32], _sample_rate: f32) {
        let fb = 0.3 + self.decay * 0.5;
        for comb in &mut self.core.combs {
            comb.feedback = fb;
            comb.damping = self.damping;
        }
        for sample in buffer.iter_mut() {
            let dry = *sample;
            let wet = self.core.tick(dry);
            *sample = dry * (1.0 - self.mix) + wet * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.core.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plate_reverb_produces_output() {
        let mut rev = PlateReverb::new(48000.0);
        rev.mix = 1.0;
        let mut buf = alloc::vec![0.0f32; 9600];
        buf[0] = 1.0;
        rev.process(&mut buf, 48000.0);
        // Reverb tail should have energy
        let tail_energy: f32 = buf[4800..].iter().map(|s| s * s).sum();
        assert!(tail_energy > 1e-6, "Reverb should produce a tail");
    }

    #[test]
    fn hall_reverb_longer_than_room() {
        let mut hall = HallReverb::new(48000.0);
        hall.decay = 0.8;
        hall.mix = 1.0;
        let mut room = RoomReverb::new(48000.0);
        room.decay = 0.3;
        room.mix = 1.0;

        let mut hall_buf = alloc::vec![0.0f32; 19200]; // 400ms
        let mut room_buf = alloc::vec![0.0f32; 19200];
        hall_buf[0] = 1.0;
        room_buf[0] = 1.0;

        hall.process(&mut hall_buf, 48000.0);
        room.process(&mut room_buf, 48000.0);

        let hall_tail: f32 = hall_buf[14400..].iter().map(|s| s * s).sum();
        let room_tail: f32 = room_buf[14400..].iter().map(|s| s * s).sum();
        assert!(
            hall_tail > room_tail,
            "Hall should have longer tail than room"
        );
    }

    #[test]
    fn reverb_zero_mix_passthrough() {
        let mut rev = PlateReverb::new(48000.0);
        rev.mix = 0.0;
        let mut buf = [0.5f32; 64];
        rev.process(&mut buf, 48000.0);
        for s in &buf {
            assert!((s - 0.5).abs() < 1e-5);
        }
    }

    #[test]
    fn reverb_output_is_finite() {
        let mut rev = HallReverb::new(48000.0);
        rev.decay = 1.0;
        rev.mix = 0.5;
        let mut buf = alloc::vec![0.0f32; 48000];
        buf[0] = 1.0;
        rev.process(&mut buf, 48000.0);
        for s in &buf {
            assert!(s.is_finite(), "Reverb output must be finite");
        }
    }
}
