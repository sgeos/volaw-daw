extern crate alloc;
use alloc::vec::Vec;
use core::num::NonZeroU16;
use serde::{Deserialize, Serialize};

/// Default ticks per beat (MIDI resolution).
pub const DEFAULT_TICKS_PER_BEAT: u16 = 480;

/// A single MIDI note event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiNote {
    /// Start time in ticks from the beginning of the track.
    pub tick: u64,
    /// Duration in ticks.
    pub duration: u64,
    /// MIDI note number (0-127). Middle C is 60.
    pub pitch: u8,
    /// Velocity (1-127). 0 means note off.
    pub velocity: u8,
}

/// A single-channel MIDI track.
///
/// Supports both monophonic and polyphonic note arrangements. Notes are
/// sorted by start tick. Overlapping notes are valid for polyphonic playback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiTrack {
    /// Notes sorted by start tick.
    pub notes: Vec<MidiNote>,
    /// Ticks per beat (resolution). Default is 480.
    #[serde(default = "default_ticks_per_beat")]
    pub ticks_per_beat: u16,
}

fn default_ticks_per_beat() -> u16 {
    DEFAULT_TICKS_PER_BEAT
}

impl Default for MidiTrack {
    fn default() -> Self {
        Self {
            notes: Vec::new(),
            ticks_per_beat: DEFAULT_TICKS_PER_BEAT,
        }
    }
}

/// Strategy for resolving overlapping notes in monophonic tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutofixStrategy {
    /// Keep the most recent note, truncating the previous note at the overlap point.
    LatestNote,
    /// Keep the highest pitch note, truncating lower notes at the overlap point.
    HighestNote,
    /// Keep the lowest pitch note, truncating higher notes at the overlap point.
    LowestNote,
}

/// Convert a tick count from `from` to `to` ticks per beat, rounded half up,
/// saturating at `u64::MAX`. The notes, the transport and the grid all
/// convert through it, so that a note and a loop point at the same time
/// before a conversion stay at the same time after it.
pub(crate) fn rescale_tick(tick: u64, from: NonZeroU16, to: NonZeroU16) -> u64 {
    let (from, to) = (u128::from(from.get()), u128::from(to.get()));
    // Below 2^64 * 2^16 + 2^16, so the product fits in a u128.
    u64::try_from((u128::from(tick) * to + from / 2) / from).unwrap_or(u64::MAX)
}

impl MidiTrack {
    /// Create an empty track.
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert every note from the track's own ticks per beat to
    /// `ticks_per_beat`, and adopt it.
    ///
    /// Rounding. A tick `t` becomes `t * to / from` rounded half up. The
    /// start and the end of each note are converted, and the duration is
    /// their difference, so that notes that met end to end still meet and
    /// the rounding errors do not add up along a phrase. A note that would
    /// shrink to nothing keeps a duration of one tick. A conversion to a finer
    /// resolution whose ratio divides evenly is exact. A conversion to a
    /// coarser one moves each time by at most half a target tick, and can
    /// give two notes the same start. A time that would pass `u64::MAX`
    /// saturates there.
    ///
    /// # Errors
    ///
    /// If either resolution is 0, which cannot be converted from or to. The
    /// track is then unchanged.
    pub fn rescale(&mut self, ticks_per_beat: u16) -> Result<(), &'static str> {
        let (Some(from), Some(to)) = (
            NonZeroU16::new(self.ticks_per_beat),
            NonZeroU16::new(ticks_per_beat),
        ) else {
            return Err("cannot rescale a track to or from 0 ticks per beat");
        };
        if from == to {
            return Ok(());
        }
        let convert = |t: u64| rescale_tick(t, from, to);
        for note in &mut self.notes {
            let start = convert(note.tick);
            let end = convert(note.tick.saturating_add(note.duration));
            note.tick = start;
            note.duration = end.saturating_sub(start).max(1);
        }
        self.ticks_per_beat = ticks_per_beat;
        self.sort();
        Ok(())
    }

    /// Sort notes by start tick, then by pitch (ascending).
    pub fn sort(&mut self) {
        self.notes
            .sort_by(|a, b| a.tick.cmp(&b.tick).then(a.pitch.cmp(&b.pitch)));
    }

    /// Check whether the track is monophonic (no overlapping notes).
    pub fn is_monophonic(&self) -> bool {
        if self.notes.len() <= 1 {
            return true;
        }
        let mut sorted = self.notes.clone();
        sorted.sort_by(|a, b| a.tick.cmp(&b.tick).then(a.pitch.cmp(&b.pitch)));
        for i in 1..sorted.len() {
            let prev_end = sorted[i - 1].tick + sorted[i - 1].duration;
            if sorted[i].tick < prev_end {
                return false;
            }
        }
        true
    }

    /// Fix overlapping notes to make the track monophonic.
    ///
    /// Notes are first sorted by tick. Then, depending on the strategy:
    /// - `LatestNote`: The later note takes priority. The earlier note is
    ///   truncated to end at the start of the later note.
    /// - `HighestNote`: The higher-pitched note takes priority at any overlap.
    ///   The lower-pitched note is truncated.
    /// - `LowestNote`: The lower-pitched note takes priority at any overlap.
    ///   The higher-pitched note is truncated.
    ///
    /// Notes with zero or negative duration after truncation are removed.
    pub fn autofix(&mut self, strategy: AutofixStrategy) {
        self.sort();

        match strategy {
            AutofixStrategy::LatestNote => self.autofix_latest(),
            AutofixStrategy::HighestNote => self.autofix_by_pitch(true),
            AutofixStrategy::LowestNote => self.autofix_by_pitch(false),
        }

        // Remove zero-duration notes.
        self.notes.retain(|n| n.duration > 0);
    }

    /// Latest-note-wins strategy: truncate earlier notes at the overlap point.
    fn autofix_latest(&mut self) {
        if self.notes.len() <= 1 {
            return;
        }
        for i in 0..self.notes.len() - 1 {
            let this_end = self.notes[i].tick + self.notes[i].duration;
            let next_start = self.notes[i + 1].tick;
            if this_end > next_start {
                let new_dur = next_start.saturating_sub(self.notes[i].tick);
                self.notes[i].duration = new_dur;
            }
        }
    }

    /// Pitch-priority strategy: higher or lower pitch wins at overlap.
    fn autofix_by_pitch(&mut self, prefer_high: bool) {
        if self.notes.len() <= 1 {
            return;
        }
        // Process pairwise, resolving overlaps.
        let mut i = 0;
        while i < self.notes.len().saturating_sub(1) {
            let a_end = self.notes[i].tick + self.notes[i].duration;
            let b_start = self.notes[i + 1].tick;

            if a_end <= b_start {
                i += 1;
                continue;
            }

            // Overlap detected. Determine winner.
            let a_wins = if prefer_high {
                self.notes[i].pitch >= self.notes[i + 1].pitch
            } else {
                self.notes[i].pitch <= self.notes[i + 1].pitch
            };

            if a_wins {
                // Truncate note B: move its start to after A ends.
                let overlap = a_end - b_start;
                if overlap >= self.notes[i + 1].duration {
                    // B is completely covered. Remove it.
                    self.notes[i + 1].duration = 0;
                } else {
                    self.notes[i + 1].tick = a_end;
                    self.notes[i + 1].duration -= overlap;
                }
                i += 1;
            } else {
                // Truncate note A at the start of B.
                let new_dur = b_start.saturating_sub(self.notes[i].tick);
                self.notes[i].duration = new_dur;
                i += 1;
            }
        }
    }

    /// Convert a MIDI note number to a frequency in Hz (A4 = 440 Hz).
    pub fn note_to_freq(note: u8) -> f64 {
        440.0 * libm::pow(2.0, (note as f64 - 69.0) / 12.0)
    }

    /// Return the note name and octave for a MIDI note number.
    /// Uses Cubase-compatible naming: note 0 = C-2, note 60 = C3, note 127 = G8.
    pub fn note_name(note: u8) -> (&'static str, i32) {
        const NAMES: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let name = NAMES[(note % 12) as usize];
        let octave = (note as i32 / 12) - 2;
        (name, octave)
    }

    /// Return whether a MIDI note number corresponds to a black key.
    pub fn is_black_key(note: u8) -> bool {
        matches!(note % 12, 1 | 3 | 6 | 8 | 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_track_is_monophonic() {
        let track = MidiTrack::new();
        assert!(track.is_monophonic());
    }

    #[test]
    fn single_note_is_monophonic() {
        let track = MidiTrack {
            notes: alloc::vec![MidiNote {
                tick: 0,
                duration: 480,
                pitch: 60,
                velocity: 100,
            }],
            ticks_per_beat: 480,
        };
        assert!(track.is_monophonic());
    }

    #[test]
    fn non_overlapping_is_monophonic() {
        let track = MidiTrack {
            notes: alloc::vec![
                MidiNote {
                    tick: 0,
                    duration: 480,
                    pitch: 60,
                    velocity: 100
                },
                MidiNote {
                    tick: 480,
                    duration: 480,
                    pitch: 64,
                    velocity: 100
                },
                MidiNote {
                    tick: 960,
                    duration: 480,
                    pitch: 67,
                    velocity: 100
                },
            ],
            ticks_per_beat: 480,
        };
        assert!(track.is_monophonic());
    }

    #[test]
    fn overlapping_is_not_monophonic() {
        let track = MidiTrack {
            notes: alloc::vec![
                MidiNote {
                    tick: 0,
                    duration: 600,
                    pitch: 60,
                    velocity: 100
                },
                MidiNote {
                    tick: 480,
                    duration: 480,
                    pitch: 64,
                    velocity: 100
                },
            ],
            ticks_per_beat: 480,
        };
        assert!(!track.is_monophonic());
    }

    #[test]
    fn autofix_latest_truncates_earlier() {
        let mut track = MidiTrack {
            notes: alloc::vec![
                MidiNote {
                    tick: 0,
                    duration: 600,
                    pitch: 60,
                    velocity: 100
                },
                MidiNote {
                    tick: 480,
                    duration: 480,
                    pitch: 64,
                    velocity: 100
                },
            ],
            ticks_per_beat: 480,
        };
        track.autofix(AutofixStrategy::LatestNote);
        assert!(track.is_monophonic());
        assert_eq!(track.notes[0].duration, 480);
        assert_eq!(track.notes[1].duration, 480);
    }

    #[test]
    fn autofix_highest_keeps_higher_pitch() {
        let mut track = MidiTrack {
            notes: alloc::vec![
                MidiNote {
                    tick: 0,
                    duration: 600,
                    pitch: 72,
                    velocity: 100
                },
                MidiNote {
                    tick: 480,
                    duration: 480,
                    pitch: 60,
                    velocity: 100
                },
            ],
            ticks_per_beat: 480,
        };
        track.autofix(AutofixStrategy::HighestNote);
        assert!(track.is_monophonic());
        // Higher pitch (72) wins. Lower pitch (60) is pushed to after 72 ends.
        assert_eq!(track.notes[0].duration, 600);
        assert_eq!(track.notes[1].tick, 600);
        assert_eq!(track.notes[1].duration, 360);
    }

    #[test]
    fn autofix_lowest_keeps_lower_pitch() {
        let mut track = MidiTrack {
            notes: alloc::vec![
                MidiNote {
                    tick: 0,
                    duration: 600,
                    pitch: 72,
                    velocity: 100
                },
                MidiNote {
                    tick: 480,
                    duration: 480,
                    pitch: 60,
                    velocity: 100
                },
            ],
            ticks_per_beat: 480,
        };
        track.autofix(AutofixStrategy::LowestNote);
        assert!(track.is_monophonic());
        // Lower pitch (60) wins. Higher pitch (72) is truncated.
        assert_eq!(track.notes[0].duration, 480);
        assert_eq!(track.notes[1].tick, 480);
    }

    #[test]
    fn autofix_removes_zero_duration() {
        let mut track = MidiTrack {
            notes: alloc::vec![
                MidiNote {
                    tick: 0,
                    duration: 960,
                    pitch: 72,
                    velocity: 100
                },
                MidiNote {
                    tick: 480,
                    duration: 100,
                    pitch: 60,
                    velocity: 100
                },
            ],
            ticks_per_beat: 480,
        };
        track.autofix(AutofixStrategy::HighestNote);
        // The lower note (60) is completely covered by the higher note (72).
        assert_eq!(track.notes.len(), 1);
        assert_eq!(track.notes[0].pitch, 72);
    }

    #[test]
    fn note_to_freq_a4() {
        let freq = MidiTrack::note_to_freq(69);
        assert!((freq - 440.0).abs() < 0.01);
    }

    #[test]
    fn note_to_freq_middle_c() {
        let freq = MidiTrack::note_to_freq(60);
        assert!((freq - 261.63).abs() < 0.1);
    }

    #[test]
    fn note_name_middle_c() {
        // Cubase convention: MIDI note 60 = C3.
        let (name, octave) = MidiTrack::note_name(60);
        assert_eq!(name, "C");
        assert_eq!(octave, 3);
    }

    #[test]
    fn note_name_a3() {
        // Cubase convention: MIDI note 69 = A3.
        let (name, octave) = MidiTrack::note_name(69);
        assert_eq!(name, "A");
        assert_eq!(octave, 3);
    }

    #[test]
    fn black_keys() {
        assert!(MidiTrack::is_black_key(61)); // C#
        assert!(MidiTrack::is_black_key(63)); // D#
        assert!(MidiTrack::is_black_key(66)); // F#
        assert!(MidiTrack::is_black_key(68)); // G#
        assert!(MidiTrack::is_black_key(70)); // A#
        assert!(!MidiTrack::is_black_key(60)); // C
        assert!(!MidiTrack::is_black_key(64)); // E
        assert!(!MidiTrack::is_black_key(65)); // F
    }

    #[test]
    fn sort_by_tick() {
        let mut track = MidiTrack {
            notes: alloc::vec![
                MidiNote {
                    tick: 960,
                    duration: 480,
                    pitch: 67,
                    velocity: 100
                },
                MidiNote {
                    tick: 0,
                    duration: 480,
                    pitch: 60,
                    velocity: 100
                },
                MidiNote {
                    tick: 480,
                    duration: 480,
                    pitch: 64,
                    velocity: 100
                },
            ],
            ticks_per_beat: 480,
        };
        track.sort();
        assert_eq!(track.notes[0].tick, 0);
        assert_eq!(track.notes[1].tick, 480);
        assert_eq!(track.notes[2].tick, 960);
    }

    fn at(tick: u64, duration: u64, pitch: u8) -> MidiNote {
        MidiNote {
            tick,
            duration,
            pitch,
            velocity: 100,
        }
    }

    fn track_at(ticks_per_beat: u16, notes: &[MidiNote]) -> MidiTrack {
        MidiTrack {
            notes: notes.to_vec(),
            ticks_per_beat,
        }
    }

    #[test]
    fn rescale_to_a_finer_resolution_is_exact() {
        let notes = [at(0, 96, 60), at(96, 48, 62), at(1_000_000, 1, 64)];
        let mut track = track_at(96, &notes);
        track.rescale(480).unwrap();
        assert_eq!(track.ticks_per_beat, 480);
        let scaled: Vec<MidiNote> = notes
            .iter()
            .map(|n| at(n.tick * 5, n.duration * 5, n.pitch))
            .collect();
        assert_eq!(track.notes, scaled);
        // And back again, since every time is a multiple of the ratio.
        track.rescale(96).unwrap();
        assert_eq!(track.notes, notes);
    }

    /// 480 to 96 divides by 5. The first note ends at 3, which rounds to 1,
    /// where the second note now starts, so the two still meet. The second and
    /// third notes would shrink to nothing, and keep one tick each.
    #[test]
    fn rescale_to_a_coarser_resolution_rounds_ends() {
        let mut track = track_at(480, &[at(0, 3, 60), at(3, 4, 62), at(10, 2, 64)]);
        track.rescale(96).unwrap();
        assert_eq!(
            track.notes,
            alloc::vec![at(0, 1, 60), at(1, 1, 62), at(2, 1, 64)]
        );
        let first_end = track.notes[0].tick + track.notes[0].duration;
        assert_eq!(first_end, track.notes[1].tick);
    }

    #[test]
    fn rescale_saturates_instead_of_overflowing() {
        let mut track = track_at(1, &[at(u64::MAX / 2, u64::MAX, 60)]);
        track.rescale(u16::MAX).unwrap();
        assert_eq!(track.notes, alloc::vec![at(u64::MAX, 1, 60)]);
    }

    #[test]
    fn rescale_refuses_zero_ticks_per_beat() {
        let notes = [at(10, 10, 60)];
        let mut track = track_at(480, &notes);
        assert!(track.rescale(0).is_err());
        let mut zero = track_at(0, &notes);
        assert!(zero.rescale(480).is_err());
        assert_eq!(
            (track.ticks_per_beat, track.notes.as_slice()),
            (480, &notes[..])
        );
        assert_eq!(
            (zero.ticks_per_beat, zero.notes.as_slice()),
            (0, &notes[..])
        );
    }
}
