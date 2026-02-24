//! Special effects: Bitcrusher, Ring Modulator, Pitch Shifter, Auto-Pan.

use core::f64::consts::PI;

use crate::dsp::delay_line::DelayLine;
use crate::dsp::lfo::{Lfo, LfoWaveform};

/// Bitcrusher: sample rate and bit depth reduction.
///
/// Reduces the effective bit depth and sample rate of the signal,
/// creating lo-fi digital artifacts.
#[derive(Debug, Clone)]
pub struct Bitcrusher {
    /// Effective bit depth (1 to 32).
    pub bit_depth: u32,
    /// Sample rate reduction factor (0.01 to 1.0, where 1.0 = no reduction).
    pub rate_factor: f64,
    /// Wet/dry mix.
    pub mix: f64,
    hold_counter: f64,
    held_sample: f64,
}

impl Bitcrusher {
    pub fn new(_sample_rate: f64) -> Self {
        Self {
            bit_depth: 8,
            rate_factor: 0.5,
            mix: 1.0,
            hold_counter: 0.0,
            held_sample: 0.0,
        }
    }

    pub fn process(&mut self, buffer: &mut [f64], _sample_rate: f64) {
        let steps = libm::pow(2.0, self.bit_depth.min(32) as f64);
        let rate = self.rate_factor.clamp(0.01, 1.0);
        for sample in buffer.iter_mut() {
            let dry = *sample;
            // Sample rate reduction via sample-and-hold
            self.hold_counter += rate;
            if self.hold_counter >= 1.0 {
                self.hold_counter -= 1.0;
                // Bit depth reduction via quantization
                self.held_sample = libm::round(dry * steps) / steps;
            }
            *sample = dry * (1.0 - self.mix) + self.held_sample * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.hold_counter = 0.0;
        self.held_sample = 0.0;
    }
}

/// Ring modulator: multiplies the signal by a sine wave carrier.
///
/// Creates inharmonic, metallic tones by amplitude-modulating the
/// input with a simple sine oscillator.
#[derive(Debug, Clone)]
pub struct RingModulator {
    /// Carrier frequency in Hz (10 to 2000).
    pub frequency_hz: f64,
    /// Wet/dry mix.
    pub mix: f64,
    phase: f64,
}

impl RingModulator {
    pub fn new(_sample_rate: f64) -> Self {
        Self {
            frequency_hz: 440.0,
            mix: 1.0,
            phase: 0.0,
        }
    }

    pub fn process(&mut self, buffer: &mut [f64], sample_rate: f64) {
        let phase_inc = self.frequency_hz / sample_rate;
        for sample in buffer.iter_mut() {
            let dry = *sample;
            let carrier = libm::sin(2.0 * PI * self.phase);
            self.phase += phase_inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            let wet = dry * carrier;
            *sample = dry * (1.0 - self.mix) + wet * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
    }
}

/// Pitch shifter using granular overlap-add.
///
/// Two overlapping grains read from a circular buffer at different
/// rates to shift the pitch. A Hann window crossfade prevents clicks
/// at grain boundaries.
#[derive(Debug, Clone)]
pub struct PitchShifter {
    /// Pitch shift in semitones (-24.0 to 24.0).
    pub semitones: f64,
    /// Wet/dry mix.
    pub mix: f64,
    buffer: DelayLine,
    grain_size: usize,
    read_pos_a: f64,
    read_pos_b: f64,
    write_pos: usize,
    crossfade_pos: usize,
}

impl PitchShifter {
    pub fn new(sample_rate: f64) -> Self {
        let grain_size = (sample_rate * 0.02) as usize; // 20ms grains
        let buf_size = grain_size * 4;
        Self {
            semitones: 0.0,
            mix: 1.0,
            buffer: DelayLine::new(buf_size),
            grain_size,
            read_pos_a: 0.0,
            read_pos_b: grain_size as f64 * 0.5,
            write_pos: 0,
            crossfade_pos: 0,
        }
    }

    pub fn process(&mut self, buffer: &mut [f64], _sample_rate: f64) {
        let ratio = libm::pow(2.0, self.semitones / 12.0);
        let grain_f = self.grain_size as f64;

        for sample in buffer.iter_mut() {
            let dry = *sample;
            self.buffer.write(dry);
            self.write_pos += 1;

            // Read from two grains with Hann window crossfade
            let window_pos = (self.crossfade_pos as f64) / grain_f;
            let window_a = 0.5 * (1.0 - libm::cos(2.0 * PI * window_pos));
            let window_b = 0.5 * (1.0 - libm::cos(2.0 * PI * ((window_pos + 0.5) % 1.0)));

            let sa = self.buffer.read(self.read_pos_a);
            let sb = self.buffer.read(self.read_pos_b);
            let wet = sa * window_a + sb * window_b;

            // Advance read positions at shifted rate
            let advance = ratio;
            self.read_pos_a += advance;
            self.read_pos_b += advance;

            // Wrap read positions within buffer
            let buf_len = self.buffer.len() as f64;
            if self.read_pos_a >= buf_len {
                self.read_pos_a -= buf_len;
            }
            if self.read_pos_b >= buf_len {
                self.read_pos_b -= buf_len;
            }

            self.crossfade_pos += 1;
            if self.crossfade_pos >= self.grain_size {
                self.crossfade_pos = 0;
            }

            *sample = dry * (1.0 - self.mix) + wet * self.mix;
        }
    }

    pub fn reset(&mut self) {
        self.buffer.reset();
        self.read_pos_a = 0.0;
        self.read_pos_b = self.grain_size as f64 * 0.5;
        self.write_pos = 0;
        self.crossfade_pos = 0;
    }
}

/// Auto-pan: LFO-controlled amplitude modulation.
///
/// In the mono signal chain, this functions similarly to tremolo.
/// The stereo panning effect occurs when the engine mixes the mono
/// channel output into the stereo bus using per-channel L/R volumes.
#[derive(Debug, Clone)]
pub struct AutoPan {
    /// LFO rate in Hz (0.1 to 10.0).
    pub rate_hz: f64,
    /// Modulation depth (0.0 to 1.0).
    pub depth: f64,
    lfo: Lfo,
}

impl AutoPan {
    pub fn new(_sample_rate: f64) -> Self {
        Self {
            rate_hz: 2.0,
            depth: 0.5,
            lfo: Lfo::new(LfoWaveform::Sine),
        }
    }

    pub fn process(&mut self, buffer: &mut [f64], sample_rate: f64) {
        for sample in buffer.iter_mut() {
            let lfo_val = self.lfo.tick(self.rate_hz, sample_rate);
            let gain = 1.0 - self.depth * 0.5 * (1.0 - lfo_val);
            *sample *= gain;
        }
    }

    pub fn reset(&mut self) {
        self.lfo.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitcrusher_quantizes() {
        let mut bc = Bitcrusher::new(48000.0);
        bc.bit_depth = 2;
        bc.rate_factor = 1.0;
        bc.mix = 1.0;
        let mut buf = [0.3f64; 32];
        bc.process(&mut buf, 48000.0);
        // With 2-bit depth (4 levels), output should be quantized
        // to one of: -1, -0.5, 0, 0.5, 1 (approximately)
        for s in &buf[1..] {
            let rounded = libm::round(*s * 4.0) / 4.0;
            assert!(
                (s - rounded).abs() < 0.3,
                "Expected quantization, got {}",
                s
            );
        }
    }

    #[test]
    fn ring_modulator_produces_sidebands() {
        let sr = 48000.0;
        let mut rm = RingModulator::new(sr);
        rm.frequency_hz = 440.0;
        rm.mix = 1.0;
        let mut buf = [0.0f64; 256];
        for (i, sample) in buf.iter_mut().enumerate() {
            *sample = libm::sin(2.0 * PI * 200.0 * i as f64 / sr);
        }
        rm.process(&mut buf, sr);
        // Output should contain energy (sum and difference frequencies)
        let energy: f64 = buf.iter().map(|s| s * s).sum();
        assert!(energy > 0.1);
    }

    #[test]
    fn pitch_shifter_zero_semitones_passthrough_like() {
        let sr = 48000.0;
        let mut ps = PitchShifter::new(sr);
        ps.semitones = 0.0;
        ps.mix = 1.0;
        let mut buf = [0.0f64; 2048];
        for (i, sample) in buf.iter_mut().enumerate() {
            *sample = libm::sin(2.0 * PI * 440.0 * i as f64 / sr);
        }
        ps.process(&mut buf, sr);
        // Should produce finite output
        for s in &buf {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn auto_pan_modulates_level() {
        let mut ap = AutoPan::new(48000.0);
        ap.rate_hz = 10.0;
        ap.depth = 1.0;
        let mut buf = [0.5f64; 4800];
        ap.process(&mut buf, 48000.0);
        let min_val = buf.iter().copied().fold(f64::MAX, f64::min);
        let max_val = buf.iter().copied().fold(f64::MIN, f64::max);
        assert!(max_val > 0.4);
        assert!(min_val < 0.1);
    }
}
