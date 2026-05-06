extern crate alloc;
use alloc::vec::Vec;
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

impl MidiTrack {
    /// Create an empty track.
    pub fn new() -> Self {
        Self::default()
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
}
