//! Pitch correction: snaps the pitch of the input to the nearest note of a
//! scale, the "Tune" effect familiar from vocal production.
//!
//! The pitch is detected with the YIN difference function on a decimated
//! copy of the input, and the correction is applied by the granular
//! `PitchShifter`. Detection runs once per hop, not per sample, and all
//! analysis buffers are allocated once in `new`, so `process` does not
//! allocate. They live on the heap to keep the `Pedal` enum small.
//!
//! Limitations. YIN assumes one dominant pitch, so chords and noise give no
//! detection and no correction. The granular shifter adds a grain of
//! latency and some roughness, which grows with the size of the correction.
//! Detection covers 50 Hz to 1200 Hz.

use alloc::vec;
use alloc::vec::Vec;

use super::special::PitchShifter;

/// Rate the analysis runs at after decimation, in Hz.
const ANALYSIS_RATE: f64 = 12_000.0;
/// Ring buffer length of decimated samples.
const RING: usize = 512;
/// YIN integration window, in decimated samples.
const WINDOW: usize = 256;
/// Largest lag searched, in decimated samples. `WINDOW + MAX_LAG <= RING`.
const MAX_LAG: usize = 240;
/// Decimated samples between analyses, about 11 ms.
const HOP: usize = 128;
/// YIN threshold on the cumulative mean normalised difference.
const YIN_THRESHOLD: f64 = 0.15;
/// Root-mean-square level below which the input is treated as silence.
const SILENCE_RMS: f64 = 1e-3;
const MIN_HZ: f64 = 50.0;
const MAX_HZ: f64 = 1200.0;

/// Semitone offsets from the key note that each scale allows.
const MAJOR: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
const MINOR: [u8; 7] = [0, 2, 3, 5, 7, 8, 10];

/// Pitch corrector.
#[derive(Debug, Clone)]
pub struct PitchCorrector {
    /// Retune speed (0.0 to 1.0). 0.0 snaps instantly, the robotic sound,
    /// and 1.0 corrects slowly and gently.
    pub speed: f64,
    /// 0 chromatic, 1 major, 2 natural minor. Rounded and clamped.
    pub scale: f64,
    /// Key note, 0 for C up to 11 for B. Rounded and wrapped.
    pub key: f64,
    /// Wet/dry mix.
    pub mix: f64,
    shifter: PitchShifter,
    ring: Vec<f64>,
    ring_pos: usize,
    /// Decimated samples written since the last analysis.
    since_hop: usize,
    /// Input samples summed into the current decimated sample.
    acc: f64,
    acc_count: usize,
    frame: Vec<f64>,
    diff: Vec<f64>,
    detected_hz: Option<f64>,
    correction: f64,
}

impl PitchCorrector {
    pub fn new(sample_rate: f64) -> Self {
        let mut shifter = PitchShifter::new(sample_rate);
        shifter.mix = 1.0;
        Self {
            speed: 0.1,
            scale: 0.0,
            key: 0.0,
            mix: 1.0,
            shifter,
            ring: vec![0.0; RING],
            ring_pos: 0,
            since_hop: 0,
            acc: 0.0,
            acc_count: 0,
            frame: vec![0.0; WINDOW + MAX_LAG],
            diff: vec![0.0; MAX_LAG + 1],
            detected_hz: None,
            correction: 0.0,
        }
    }

    /// The pitch found by the last analysis, if the input was pitched.
    pub fn detected_hz(&self) -> Option<f64> {
        self.detected_hz
    }

    /// The correction currently applied, in semitones.
    pub fn correction_semitones(&self) -> f64 {
        self.correction
    }

    fn decimation(sample_rate: f64) -> usize {
        (libm::round(sample_rate / ANALYSIS_RATE) as usize).max(1)
    }

    pub fn process(&mut self, buffer: &mut [f64], sample_rate: f64) {
        if !(sample_rate.is_finite() && sample_rate > 0.0) {
            return;
        }
        let decim = Self::decimation(sample_rate);
        let rate = sample_rate / decim as f64;
        // Each chunk ends at an analysis boundary, so the shift applied to a
        // chunk uses the correction from the analysis just before it.
        let mut start = 0;
        while start < buffer.len() {
            let mut end = start;
            while end < buffer.len() {
                let s = buffer[end];
                end += 1;
                self.acc += if s.is_finite() { s } else { 0.0 };
                self.acc_count += 1;
                if self.acc_count == decim {
                    self.push(self.acc / decim as f64);
                    self.acc = 0.0;
                    self.acc_count = 0;
                    if self.since_hop >= HOP {
                        self.since_hop = 0;
                        self.analyse(rate);
                        break;
                    }
                }
            }
            self.shifter.semitones = self.correction;
            self.shifter.mix = self.mix.clamp(0.0, 1.0);
            self.shifter.process(&mut buffer[start..end], sample_rate);
            start = end;
        }
    }

    fn push(&mut self, sample: f64) {
        self.ring[self.ring_pos] = sample;
        self.ring_pos = (self.ring_pos + 1) % RING;
        self.since_hop += 1;
    }

    /// Detect the pitch of the latest window and move the correction toward
    /// the nearest allowed note.
    fn analyse(&mut self, rate: f64) {
        let len = self.frame.len();
        for (i, slot) in self.frame.iter_mut().enumerate() {
            *slot = self.ring[(self.ring_pos + RING - len + i) % RING];
        }
        self.detected_hz = self.yin(rate);
        let target = match self.detected_hz {
            Some(hz) => {
                let midi = 69.0 + 12.0 * libm::log2(hz / 440.0);
                self.nearest_allowed(midi) - midi
            }
            None => 0.0,
        };
        // speed 0 gives alpha 1, an instant snap. speed 1 still converges.
        let alpha = 1.0 - 0.97 * self.speed.clamp(0.0, 1.0);
        self.correction += (target - self.correction) * alpha;
    }

    /// YIN pitch estimate over `self.frame`, or `None` when the frame is
    /// silent or unpitched.
    fn yin(&mut self, rate: f64) -> Option<f64> {
        let window = &self.frame[..WINDOW];
        let energy = window.iter().map(|s| s * s).sum::<f64>() / WINDOW as f64;
        if libm::sqrt(energy) < SILENCE_RMS {
            return None;
        }
        let min_lag = (libm::ceil(rate / MAX_HZ) as usize).max(2);
        let max_lag = (libm::floor(rate / MIN_HZ) as usize).min(MAX_LAG);
        if min_lag + 1 >= max_lag {
            return None;
        }
        self.diff[0] = 1.0;
        let mut running = 0.0;
        for lag in 1..=max_lag {
            let d: f64 = (0..WINDOW)
                .map(|j| {
                    let delta = self.frame[j] - self.frame[j + lag];
                    delta * delta
                })
                .sum();
            running += d;
            // Cumulative mean normalised difference.
            self.diff[lag] = if running > 0.0 {
                d * lag as f64 / running
            } else {
                1.0
            };
        }
        let mut lag = min_lag;
        while lag < max_lag {
            if self.diff[lag] < YIN_THRESHOLD {
                while lag + 1 < max_lag && self.diff[lag + 1] < self.diff[lag] {
                    lag += 1;
                }
                return Some(rate / self.refine(lag));
            }
            lag += 1;
        }
        None
    }

    /// Parabolic interpolation of the minimum around `lag`.
    fn refine(&self, lag: usize) -> f64 {
        let (a, b, c) = (self.diff[lag - 1], self.diff[lag], self.diff[lag + 1]);
        let denom = a - 2.0 * b + c;
        if denom.abs() < 1e-12 {
            return lag as f64;
        }
        lag as f64 + (0.5 * (a - c) / denom).clamp(-1.0, 1.0)
    }

    /// The allowed note nearest to `midi`, as a MIDI note number.
    fn nearest_allowed(&self, midi: f64) -> f64 {
        let scale = libm::round(self.scale.clamp(0.0, 2.0)) as u8;
        let key = (libm::round(self.key) as i64).rem_euclid(12);
        let allowed = |note: i64| -> bool {
            let degree = (note - key).rem_euclid(12) as u8;
            match scale {
                0 => true,
                1 => MAJOR.contains(&degree),
                _ => MINOR.contains(&degree),
            }
        };
        let centre = libm::round(midi) as i64;
        // Every scale has a note within two semitones of any pitch, so the
        // search is bounded.
        let mut best = centre as f64;
        let mut best_dist = f64::MAX;
        for note in centre - 2..=centre + 2 {
            let dist = (note as f64 - midi).abs();
            if allowed(note) && dist < best_dist {
                best = note as f64;
                best_dist = dist;
            }
        }
        best
    }

    pub fn reset(&mut self) {
        self.shifter.reset();
        self.ring.fill(0.0);
        self.ring_pos = 0;
        self.since_hop = 0;
        self.acc = 0.0;
        self.acc_count = 0;
        self.detected_hz = None;
        self.correction = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::PI;

    const SR: f64 = 48_000.0;

    fn sine(freq: f64, n: usize) -> alloc::vec::Vec<f64> {
        (0..n)
            .map(|i| 0.5 * libm::sin(2.0 * PI * freq * i as f64 / SR))
            .collect()
    }

    fn semitones_between(from_hz: f64, to_hz: f64) -> f64 {
        12.0 * libm::log2(to_hz / from_hz)
    }

    #[test]
    fn detects_a_sine_and_corrects_toward_the_nearest_note() {
        let mut pc = PitchCorrector::new(SR);
        pc.speed = 0.0;
        let mut buf = sine(450.0, 24_000);
        pc.process(&mut buf, SR);
        let hz = pc.detected_hz().expect("a sine is pitched");
        assert!((hz - 450.0).abs() < 3.0, "detected {hz}");
        // 450 Hz lies between A4 and A#4, nearer A4 at 440 Hz.
        let expected = semitones_between(450.0, 440.0);
        assert!(
            (pc.correction_semitones() - expected).abs() < 0.05,
            "correction {} against {expected}",
            pc.correction_semitones()
        );
        assert!(buf.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn scale_moves_the_target() {
        // C#4 is about 277.18 Hz. Chromatic keeps it, C major moves it.
        let csharp = 440.0 * libm::exp2((61.0 - 69.0) / 12.0);
        let mut chromatic = PitchCorrector::new(SR);
        chromatic.speed = 0.0;
        let mut buf = sine(csharp, 24_000);
        chromatic.process(&mut buf, SR);
        assert!(chromatic.correction_semitones().abs() < 0.05);

        let mut major = PitchCorrector::new(SR);
        major.speed = 0.0;
        major.scale = 1.0;
        let mut buf = sine(csharp, 24_000);
        major.process(&mut buf, SR);
        assert!(
            (major.correction_semitones().abs() - 1.0).abs() < 0.05,
            "correction {}",
            major.correction_semitones()
        );
    }

    #[test]
    fn silence_gets_no_detection_and_no_correction() {
        let mut pc = PitchCorrector::new(SR);
        let mut buf = alloc::vec![0.0; 12_000];
        pc.process(&mut buf, SR);
        assert_eq!(pc.detected_hz(), None);
        assert_eq!(pc.correction_semitones(), 0.0);
        assert!(buf.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn non_finite_input_and_bad_rate_are_contained() {
        let mut pc = PitchCorrector::new(SR);
        let mut buf = sine(300.0, 4096);
        buf[100] = f64::NAN;
        buf[200] = f64::INFINITY;
        pc.process(&mut buf, SR);
        assert!(pc.correction_semitones().is_finite());
        let mut buf = [0.1f64; 64];
        pc.process(&mut buf, 0.0);
        pc.process(&mut buf, f64::NAN);
    }

    #[test]
    fn slow_speed_moves_only_part_way() {
        let mut fast = PitchCorrector::new(SR);
        fast.speed = 0.0;
        let mut slow = PitchCorrector::new(SR);
        slow.speed = 1.0;
        // Long enough for a few analyses, too short for the slow one to settle.
        let mut a = sine(450.0, 2_400);
        let mut b = a.clone();
        fast.process(&mut a, SR);
        slow.process(&mut b, SR);
        assert!(slow.correction_semitones().abs() < fast.correction_semitones().abs());
    }
}
