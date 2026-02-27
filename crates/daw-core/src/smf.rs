//! Standard MIDI File (SMF) parser.
//!
//! Parses Type 0 (single track) and Type 1 (multi-track) MIDI files into
//! [`MidiTrack`] sequences. Only note on/off events and tempo changes are
//! extracted. All other event types are silently skipped.
//!
//! The parser operates on a byte slice and requires no heap allocation beyond
//! the output vectors. Compatible with `no_std + alloc`.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::midi::{MidiNote, MidiTrack, DEFAULT_TICKS_PER_BEAT};

/// Errors that can occur during SMF parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmfError {
    /// Input is too short to contain the required data.
    UnexpectedEof,
    /// The header chunk magic bytes are not "MThd".
    InvalidHeader,
    /// The track chunk magic bytes are not "MTrk".
    InvalidTrackHeader,
    /// An unsupported MIDI file format (not 0 or 1).
    UnsupportedFormat(u16),
    /// A variable-length quantity encoding is malformed.
    InvalidVarLen,
    /// A MIDI event has an invalid status byte.
    InvalidEvent(u8),
}

impl core::fmt::Display for SmfError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SmfError::UnexpectedEof => write!(f, "unexpected end of file"),
            SmfError::InvalidHeader => write!(f, "invalid MIDI header (expected MThd)"),
            SmfError::InvalidTrackHeader => write!(f, "invalid track header (expected MTrk)"),
            SmfError::UnsupportedFormat(fmt) => write!(f, "unsupported MIDI format: {fmt}"),
            SmfError::InvalidVarLen => write!(f, "invalid variable-length quantity"),
            SmfError::InvalidEvent(b) => write!(f, "invalid MIDI event status: 0x{b:02x}"),
        }
    }
}

/// Result of parsing a Standard MIDI File.
#[derive(Debug, Clone)]
pub struct SmfResult {
    /// Parsed tracks. Type 0 files produce one track. Type 1 files produce
    /// one track per MTrk chunk (the first is typically a tempo/conductor track).
    pub tracks: Vec<MidiTrack>,
    /// Ticks per beat (from the MIDI header division field).
    pub ticks_per_beat: u16,
    /// MIDI file format (0 or 1).
    pub format: u16,
}

/// Parse a Standard MIDI File from a byte slice.
///
/// Returns tracks with note events converted to [`MidiNote`] entries. Each
/// track in the SMF becomes one [`MidiTrack`]. Notes are paired (note-on
/// matched with note-off) and sorted by start tick.
pub fn parse_smf(data: &[u8]) -> Result<SmfResult, SmfError> {
    let mut cursor = Cursor::new(data);

    // Parse header chunk.
    let magic = cursor.read_bytes(4)?;
    if magic != b"MThd" {
        return Err(SmfError::InvalidHeader);
    }
    let header_len = cursor.read_u32()? as usize;
    let format = cursor.read_u16()?;
    let ntrks = cursor.read_u16()?;
    let division = cursor.read_u16()?;

    if format > 1 {
        return Err(SmfError::UnsupportedFormat(format));
    }

    // Skip any extra header bytes beyond the standard 6.
    if header_len > 6 {
        cursor.skip(header_len - 6)?;
    }

    // SMPTE-based timing is not supported. Treat the division as ticks per
    // beat when the high bit is clear.
    let ticks_per_beat = if division & 0x8000 == 0 {
        division
    } else {
        DEFAULT_TICKS_PER_BEAT
    };

    let mut tracks = Vec::with_capacity(ntrks as usize);

    for _ in 0..ntrks {
        let track = parse_track(&mut cursor, ticks_per_beat)?;
        tracks.push(track);
    }

    Ok(SmfResult {
        tracks,
        ticks_per_beat,
        format,
    })
}

/// Parse a single MTrk chunk.
fn parse_track(cursor: &mut Cursor<'_>, ticks_per_beat: u16) -> Result<MidiTrack, SmfError> {
    let magic = cursor.read_bytes(4)?;
    if magic != b"MTrk" {
        return Err(SmfError::InvalidTrackHeader);
    }
    let chunk_len = cursor.read_u32()? as usize;
    let chunk_end = cursor.pos + chunk_len;

    let mut abs_tick: u64 = 0;
    let mut running_status: u8 = 0;

    // Pending note-on events awaiting a matching note-off. Indexed by
    // (channel, pitch) for efficient lookup. With only 16 channels and
    // 128 pitches, a flat array is practical.
    let mut pending: [Option<(u64, u8)>; 2048] = [None; 2048];
    let mut notes = Vec::new();

    while cursor.pos < chunk_end {
        let delta = cursor.read_var_len()?;
        abs_tick += delta as u64;

        let peek = cursor.peek()?;

        if peek == 0xFF {
            // Meta event.
            cursor.skip(1)?; // consume 0xFF
            let _meta_type = cursor.read_u8()?;
            let meta_len = cursor.read_var_len()? as usize;
            cursor.skip(meta_len)?;
        } else if peek == 0xF0 || peek == 0xF7 {
            // SysEx event.
            cursor.skip(1)?;
            let sysex_len = cursor.read_var_len()? as usize;
            cursor.skip(sysex_len)?;
        } else if peek >= 0x80 {
            // Channel event with new status byte.
            let status = cursor.read_u8()?;
            running_status = status;
            parse_channel_event(
                cursor,
                status,
                abs_tick,
                &mut pending,
                &mut notes,
            )?;
        } else {
            // Running status (no new status byte).
            if running_status == 0 {
                return Err(SmfError::InvalidEvent(peek));
            }
            parse_channel_event(
                cursor,
                running_status,
                abs_tick,
                &mut pending,
                &mut notes,
            )?;
        }
    }

    // Close any pending notes at the final tick.
    for (idx, slot) in pending.iter_mut().enumerate() {
        if let Some((start_tick, velocity)) = slot.take() {
            let duration = abs_tick.saturating_sub(start_tick).max(1);
            let pitch = (idx % 128) as u8;
            notes.push(MidiNote {
                tick: start_tick,
                duration,
                pitch,
                velocity,
            });
        }
    }

    // Sort notes and build the track.
    notes.sort_by(|a, b| a.tick.cmp(&b.tick).then(a.pitch.cmp(&b.pitch)));

    // Advance cursor to the chunk boundary in case we stopped early.
    if cursor.pos < chunk_end {
        cursor.pos = chunk_end;
    }

    Ok(MidiTrack {
        notes,
        ticks_per_beat,
    })
}

/// Parse a MIDI channel event and extract note on/off pairs.
fn parse_channel_event(
    cursor: &mut Cursor<'_>,
    status: u8,
    abs_tick: u64,
    pending: &mut [Option<(u64, u8)>; 2048],
    notes: &mut Vec<MidiNote>,
) -> Result<(), SmfError> {
    let msg_type = status & 0xF0;
    let channel = (status & 0x0F) as usize;

    match msg_type {
        0x80 => {
            // Note Off.
            let pitch = cursor.read_u8()?;
            let _velocity = cursor.read_u8()?;
            close_note(pending, channel, pitch, abs_tick, notes);
        }
        0x90 => {
            // Note On (velocity 0 = note off).
            let pitch = cursor.read_u8()?;
            let velocity = cursor.read_u8()?;
            if velocity == 0 {
                close_note(pending, channel, pitch, abs_tick, notes);
            } else {
                let idx = channel * 128 + pitch as usize;
                // Close any existing note on this channel+pitch first.
                if pending[idx].is_some() {
                    close_note(pending, channel, pitch, abs_tick, notes);
                }
                pending[idx] = Some((abs_tick, velocity));
            }
        }
        0xA0 => {
            // Polyphonic aftertouch: 2 data bytes. Skip.
            cursor.skip(2)?;
        }
        0xB0 => {
            // Control change: 2 data bytes. Skip.
            cursor.skip(2)?;
        }
        0xC0 => {
            // Program change: 1 data byte. Skip.
            cursor.skip(1)?;
        }
        0xD0 => {
            // Channel aftertouch: 1 data byte. Skip.
            cursor.skip(1)?;
        }
        0xE0 => {
            // Pitch bend: 2 data bytes. Skip.
            cursor.skip(2)?;
        }
        _ => {
            return Err(SmfError::InvalidEvent(status));
        }
    }

    Ok(())
}

/// Close a pending note-on by creating a MidiNote with computed duration.
fn close_note(
    pending: &mut [Option<(u64, u8)>; 2048],
    channel: usize,
    pitch: u8,
    off_tick: u64,
    notes: &mut Vec<MidiNote>,
) {
    let idx = channel * 128 + pitch as usize;
    if let Some((start_tick, velocity)) = pending[idx].take() {
        let duration = off_tick.saturating_sub(start_tick).max(1);
        notes.push(MidiNote {
            tick: start_tick,
            duration,
            pitch,
            velocity,
        });
    }
}

/// Byte-level cursor for reading binary data from a slice.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn peek(&self) -> Result<u8, SmfError> {
        self.data.get(self.pos).copied().ok_or(SmfError::UnexpectedEof)
    }

    fn read_u8(&mut self) -> Result<u8, SmfError> {
        let b = self.data.get(self.pos).copied().ok_or(SmfError::UnexpectedEof)?;
        self.pos += 1;
        Ok(b)
    }

    fn read_u16(&mut self) -> Result<u16, SmfError> {
        if self.pos + 2 > self.data.len() {
            return Err(SmfError::UnexpectedEof);
        }
        let val = u16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(val)
    }

    fn read_u32(&mut self) -> Result<u32, SmfError> {
        if self.pos + 4 > self.data.len() {
            return Err(SmfError::UnexpectedEof);
        }
        let val = u32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(val)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], SmfError> {
        if self.pos + n > self.data.len() {
            return Err(SmfError::UnexpectedEof);
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn skip(&mut self, n: usize) -> Result<(), SmfError> {
        if self.pos + n > self.data.len() {
            return Err(SmfError::UnexpectedEof);
        }
        self.pos += n;
        Ok(())
    }

    /// Read a MIDI variable-length quantity (up to 4 bytes).
    fn read_var_len(&mut self) -> Result<u32, SmfError> {
        let mut result: u32 = 0;
        for _ in 0..4 {
            let b = self.read_u8()?;
            result = (result << 7) | (b & 0x7F) as u32;
            if b & 0x80 == 0 {
                return Ok(result);
            }
        }
        Err(SmfError::InvalidVarLen)
    }
}

/// Build a minimal Standard MIDI File (Type 0) from a set of MidiTracks.
///
/// All tracks are merged into a single SMF track. This is useful for
/// exporting DAW compositions to standard .mid files. The output uses the
/// given ticks per beat for timing resolution. The tempo and time signature
/// are written as meta events at tick 0.
pub fn build_smf(tracks: &[MidiTrack], ticks_per_beat: u16, tempo_bpm: f64, beats_per_bar: u8) -> Vec<u8> {
    // Collect all note events from all tracks with channel assignment.
    let mut events: Vec<(u64, u8, u8, u8, bool)> = Vec::new(); // (tick, channel, pitch, velocity, is_on)

    for (ch, track) in tracks.iter().enumerate() {
        let channel = (ch & 0x0F) as u8;
        for note in &track.notes {
            events.push((note.tick, channel, note.pitch, note.velocity, true));
            let off_tick = note.tick + note.duration;
            events.push((off_tick, channel, note.pitch, 0, false));
        }
    }

    // Sort events by tick, with note-off before note-on at the same tick.
    events.sort_by(|a, b| a.0.cmp(&b.0).then(a.4.cmp(&b.4)));

    // Build track data with tempo and time signature meta events at tick 0.
    let mut track_data: Vec<u8> = Vec::new();

    // Tempo meta event: FF 51 03 tt tt tt (microseconds per quarter note).
    let usec_per_beat = (60_000_000.0 / tempo_bpm) as u32;
    write_var_len(&mut track_data, 0); // delta time = 0
    track_data.push(0xFF);
    track_data.push(0x51);
    track_data.push(0x03);
    track_data.push((usec_per_beat >> 16) as u8);
    track_data.push((usec_per_beat >> 8) as u8);
    track_data.push(usec_per_beat as u8);

    // Time signature meta event: FF 58 04 nn dd cc bb.
    // nn = numerator (beats_per_bar), dd = denominator power (2 = quarter note),
    // cc = MIDI clocks per metronome click (24), bb = 32nd notes per quarter (8).
    write_var_len(&mut track_data, 0); // delta time = 0
    track_data.push(0xFF);
    track_data.push(0x58);
    track_data.push(0x04);
    track_data.push(beats_per_bar);
    track_data.push(0x02); // 2^2 = 4, meaning quarter note denominator
    track_data.push(24);   // MIDI clocks per metronome click
    track_data.push(8);    // 32nd notes per quarter note

    let mut prev_tick: u64 = 0;

    for (tick, channel, pitch, velocity, is_on) in &events {
        let delta = tick.saturating_sub(prev_tick) as u32;
        write_var_len(&mut track_data, delta);
        prev_tick = *tick;

        if *is_on {
            track_data.push(0x90 | channel);
            track_data.push(*pitch);
            track_data.push(*velocity);
        } else {
            track_data.push(0x80 | channel);
            track_data.push(*pitch);
            track_data.push(0);
        }
    }

    // End of track meta event.
    write_var_len(&mut track_data, 0);
    track_data.extend_from_slice(&[0xFF, 0x2F, 0x00]);

    // Build complete file.
    let mut out: Vec<u8> = Vec::new();

    // Header chunk: MThd.
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // Format 0
    out.extend_from_slice(&1u16.to_be_bytes()); // 1 track
    out.extend_from_slice(&ticks_per_beat.to_be_bytes());

    // Track chunk: MTrk.
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(track_data.len() as u32).to_be_bytes());
    out.extend_from_slice(&track_data);

    out
}

/// Write a variable-length quantity to a byte vector.
fn write_var_len(out: &mut Vec<u8>, mut value: u32) {
    if value == 0 {
        out.push(0);
        return;
    }

    let mut buf = [0u8; 4];
    let mut i = 3;
    buf[i] = (value & 0x7F) as u8;
    value >>= 7;

    while value > 0 {
        i -= 1;
        buf[i] = (value & 0x7F) as u8 | 0x80;
        value >>= 7;
    }

    out.extend_from_slice(&buf[i..]);
}

/// Convenience: describe what an imported MIDI file contains.
pub fn describe_import(result: &SmfResult) -> String {
    let total_notes: usize = result.tracks.iter().map(|t| t.notes.len()).sum();
    let non_empty: usize = result.tracks.iter().filter(|t| !t.notes.is_empty()).count();
    alloc::format!(
        "Format {}, {} tracks ({} with notes), {} total notes, {} tpb",
        result.format,
        result.tracks.len(),
        non_empty,
        total_notes,
        result.ticks_per_beat,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Build a minimal valid SMF Type 0 file with a single note.
    fn make_minimal_smf() -> Vec<u8> {
        let mut data = Vec::new();

        // Header: MThd, length=6, format=0, ntrks=1, division=480.
        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&480u16.to_be_bytes());

        // Track: MTrk with one note (C4, velocity 100, duration 480 ticks).
        let mut track_data = vec![
            0x00, // delta=0
            0x90, // note on ch0
            60,   // pitch C4
            100,  // velocity
        ];

        // Delta=480 (var-len encoded), Note Off channel 0, pitch 60.
        write_var_len(&mut track_data, 480);
        track_data.extend_from_slice(&[0x80, 60, 0]); // note off

        // End of track.
        track_data.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);

        data.extend_from_slice(b"MTrk");
        data.extend_from_slice(&(track_data.len() as u32).to_be_bytes());
        data.extend_from_slice(&track_data);

        data
    }

    #[test]
    fn parse_minimal_smf() {
        let data = make_minimal_smf();
        let result = parse_smf(&data).expect("parse should succeed");
        assert_eq!(result.format, 0);
        assert_eq!(result.ticks_per_beat, 480);
        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.tracks[0].notes.len(), 1);

        let note = &result.tracks[0].notes[0];
        assert_eq!(note.tick, 0);
        assert_eq!(note.duration, 480);
        assert_eq!(note.pitch, 60);
        assert_eq!(note.velocity, 100);
    }

    #[test]
    fn parse_multiple_notes() {
        let mut data = Vec::new();

        // Header.
        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&480u16.to_be_bytes());

        // Track with three notes: C4, E4, G4 each 240 ticks.
        let mut track_data = Vec::new();

        // Note 1: C4 at tick 0.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0x90, 60, 80]);
        write_var_len(&mut track_data, 240);
        track_data.extend_from_slice(&[0x80, 60, 0]);

        // Note 2: E4 at tick 240.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0x90, 64, 90]);
        write_var_len(&mut track_data, 240);
        track_data.extend_from_slice(&[0x80, 64, 0]);

        // Note 3: G4 at tick 480.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0x90, 67, 100]);
        write_var_len(&mut track_data, 240);
        track_data.extend_from_slice(&[0x80, 67, 0]);

        // End of track.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0xFF, 0x2F, 0x00]);

        data.extend_from_slice(b"MTrk");
        data.extend_from_slice(&(track_data.len() as u32).to_be_bytes());
        data.extend_from_slice(&track_data);

        let result = parse_smf(&data).expect("parse should succeed");
        assert_eq!(result.tracks[0].notes.len(), 3);
        assert_eq!(result.tracks[0].notes[0].pitch, 60);
        assert_eq!(result.tracks[0].notes[1].pitch, 64);
        assert_eq!(result.tracks[0].notes[2].pitch, 67);
    }

    #[test]
    fn parse_note_on_velocity_zero_as_note_off() {
        let mut data = Vec::new();

        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&480u16.to_be_bytes());

        let mut track_data = Vec::new();

        // Note On with velocity > 0.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0x90, 60, 100]);

        // Note On with velocity 0 (acts as note off).
        write_var_len(&mut track_data, 480);
        track_data.extend_from_slice(&[0x90, 60, 0]);

        // End of track.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0xFF, 0x2F, 0x00]);

        data.extend_from_slice(b"MTrk");
        data.extend_from_slice(&(track_data.len() as u32).to_be_bytes());
        data.extend_from_slice(&track_data);

        let result = parse_smf(&data).expect("parse should succeed");
        assert_eq!(result.tracks[0].notes.len(), 1);
        assert_eq!(result.tracks[0].notes[0].duration, 480);
    }

    #[test]
    fn parse_running_status() {
        let mut data = Vec::new();

        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&480u16.to_be_bytes());

        let mut track_data = Vec::new();

        // Note On with status byte.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0x90, 60, 100]);

        // Running status: another Note On (no status byte).
        write_var_len(&mut track_data, 480);
        track_data.extend_from_slice(&[64, 90]); // pitch=64, vel=90

        // Note Off for first note using explicit status.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0x80, 60, 0]);

        // Note Off for second note.
        write_var_len(&mut track_data, 240);
        track_data.extend_from_slice(&[0x80, 64, 0]);

        // End of track.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0xFF, 0x2F, 0x00]);

        data.extend_from_slice(b"MTrk");
        data.extend_from_slice(&(track_data.len() as u32).to_be_bytes());
        data.extend_from_slice(&track_data);

        let result = parse_smf(&data).expect("parse should succeed");
        assert_eq!(result.tracks[0].notes.len(), 2);
        assert_eq!(result.tracks[0].notes[0].pitch, 60);
        assert_eq!(result.tracks[0].notes[0].tick, 0);
        assert_eq!(result.tracks[0].notes[0].duration, 480);
        assert_eq!(result.tracks[0].notes[1].pitch, 64);
        assert_eq!(result.tracks[0].notes[1].tick, 480);
        assert_eq!(result.tracks[0].notes[1].duration, 240);
    }

    #[test]
    fn parse_type1_multi_track() {
        let mut data = Vec::new();

        // Header: format 1, 2 tracks.
        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes()); // format 1
        data.extend_from_slice(&2u16.to_be_bytes()); // 2 tracks
        data.extend_from_slice(&480u16.to_be_bytes());

        // Track 1: tempo track (empty of notes).
        let mut t1 = Vec::new();
        write_var_len(&mut t1, 0);
        t1.extend_from_slice(&[0xFF, 0x2F, 0x00]);
        data.extend_from_slice(b"MTrk");
        data.extend_from_slice(&(t1.len() as u32).to_be_bytes());
        data.extend_from_slice(&t1);

        // Track 2: one note.
        let mut t2 = Vec::new();
        write_var_len(&mut t2, 0);
        t2.extend_from_slice(&[0x90, 72, 100]);
        write_var_len(&mut t2, 960);
        t2.extend_from_slice(&[0x80, 72, 0]);
        write_var_len(&mut t2, 0);
        t2.extend_from_slice(&[0xFF, 0x2F, 0x00]);
        data.extend_from_slice(b"MTrk");
        data.extend_from_slice(&(t2.len() as u32).to_be_bytes());
        data.extend_from_slice(&t2);

        let result = parse_smf(&data).expect("parse should succeed");
        assert_eq!(result.format, 1);
        assert_eq!(result.tracks.len(), 2);
        assert!(result.tracks[0].notes.is_empty());
        assert_eq!(result.tracks[1].notes.len(), 1);
        assert_eq!(result.tracks[1].notes[0].pitch, 72);
        assert_eq!(result.tracks[1].notes[0].duration, 960);
    }

    #[test]
    fn roundtrip_build_parse() {
        let track = MidiTrack {
            notes: vec![
                MidiNote { tick: 0, duration: 480, pitch: 60, velocity: 100 },
                MidiNote { tick: 480, duration: 240, pitch: 64, velocity: 80 },
                MidiNote { tick: 720, duration: 480, pitch: 67, velocity: 90 },
            ],
            ticks_per_beat: 480,
        };

        let smf_data = build_smf(core::slice::from_ref(&track), 480, 120.0, 4);
        let result = parse_smf(&smf_data).expect("roundtrip parse");

        assert_eq!(result.ticks_per_beat, 480);
        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.tracks[0].notes.len(), 3);

        for (i, note) in result.tracks[0].notes.iter().enumerate() {
            assert_eq!(note.tick, track.notes[i].tick);
            assert_eq!(note.duration, track.notes[i].duration);
            assert_eq!(note.pitch, track.notes[i].pitch);
            assert_eq!(note.velocity, track.notes[i].velocity);
        }
    }

    #[test]
    fn empty_track_roundtrip() {
        let smf_data = build_smf(&[], 480, 120.0, 4);
        let result = parse_smf(&smf_data).expect("parse empty");
        assert_eq!(result.tracks.len(), 1);
        assert!(result.tracks[0].notes.is_empty());
    }

    #[test]
    fn var_len_encoding() {
        let mut buf = Vec::new();

        write_var_len(&mut buf, 0);
        assert_eq!(buf, vec![0x00]);

        buf.clear();
        write_var_len(&mut buf, 127);
        assert_eq!(buf, vec![0x7F]);

        buf.clear();
        write_var_len(&mut buf, 128);
        assert_eq!(buf, vec![0x81, 0x00]);

        buf.clear();
        write_var_len(&mut buf, 16383);
        assert_eq!(buf, vec![0xFF, 0x7F]);

        buf.clear();
        write_var_len(&mut buf, 16384);
        assert_eq!(buf, vec![0x81, 0x80, 0x00]);
    }

    #[test]
    fn invalid_header_rejected() {
        let err = parse_smf(b"RIFF1234567890").unwrap_err();
        assert_eq!(err, SmfError::InvalidHeader);
    }

    #[test]
    fn truncated_file_rejected() {
        let err = parse_smf(b"MTh").unwrap_err();
        assert_eq!(err, SmfError::UnexpectedEof);
    }

    #[test]
    fn unsupported_format_rejected() {
        let mut data = Vec::new();
        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(&2u16.to_be_bytes()); // format 2
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&480u16.to_be_bytes());

        let err = parse_smf(&data).unwrap_err();
        assert_eq!(err, SmfError::UnsupportedFormat(2));
    }

    #[test]
    fn describe_import_format() {
        let result = SmfResult {
            tracks: vec![
                MidiTrack { notes: vec![], ticks_per_beat: 480 },
                MidiTrack {
                    notes: vec![
                        MidiNote { tick: 0, duration: 480, pitch: 60, velocity: 100 },
                    ],
                    ticks_per_beat: 480,
                },
            ],
            ticks_per_beat: 480,
            format: 1,
        };
        let desc = describe_import(&result);
        assert!(desc.contains("Format 1"));
        assert!(desc.contains("2 tracks"));
        assert!(desc.contains("1 with notes"));
        assert!(desc.contains("1 total notes"));
    }

    #[test]
    fn meta_events_skipped() {
        let mut data = Vec::new();

        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&480u16.to_be_bytes());

        let mut track_data = Vec::new();

        // Tempo meta event: FF 51 03 <3 bytes>.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20]);

        // Note on.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0x90, 60, 100]);

        // Note off.
        write_var_len(&mut track_data, 480);
        track_data.extend_from_slice(&[0x80, 60, 0]);

        // End of track.
        write_var_len(&mut track_data, 0);
        track_data.extend_from_slice(&[0xFF, 0x2F, 0x00]);

        data.extend_from_slice(b"MTrk");
        data.extend_from_slice(&(track_data.len() as u32).to_be_bytes());
        data.extend_from_slice(&track_data);

        let result = parse_smf(&data).expect("parse with meta events");
        assert_eq!(result.tracks[0].notes.len(), 1);
        assert_eq!(result.tracks[0].notes[0].pitch, 60);
    }

    #[test]
    fn build_smf_includes_tempo_meta_event() {
        let smf_data = build_smf(&[], 480, 140.0, 4);
        // At 140 BPM, microseconds per beat = 60_000_000 / 140 = 428571.
        let expected_usec: u32 = (60_000_000.0 / 140.0) as u32;
        let tempo_bytes = [
            (expected_usec >> 16) as u8,
            (expected_usec >> 8) as u8,
            expected_usec as u8,
        ];
        // Search for the tempo meta event (FF 51 03 + 3 bytes) in the file.
        let pattern = [0xFF, 0x51, 0x03, tempo_bytes[0], tempo_bytes[1], tempo_bytes[2]];
        let found = smf_data.windows(pattern.len()).any(|w| w == pattern);
        assert!(found, "tempo meta event not found in SMF data");
    }

    #[test]
    fn build_smf_includes_time_signature_meta_event() {
        let smf_data = build_smf(&[], 480, 120.0, 3);
        // Time signature: FF 58 04 03 02 18 08 (3/4 time).
        let pattern = [0xFF, 0x58, 0x04, 3, 0x02, 24, 8];
        let found = smf_data.windows(pattern.len()).any(|w| w == pattern);
        assert!(found, "time signature meta event not found in SMF data");
    }

    #[test]
    fn build_smf_roundtrip_with_tempo() {
        let track = MidiTrack {
            notes: vec![
                MidiNote { tick: 0, duration: 480, pitch: 60, velocity: 100 },
            ],
            ticks_per_beat: 480,
        };
        let smf_data = build_smf(core::slice::from_ref(&track), 480, 90.0, 3);
        let result = parse_smf(&smf_data).expect("roundtrip with tempo");
        assert_eq!(result.tracks[0].notes.len(), 1);
        assert_eq!(result.tracks[0].notes[0].pitch, 60);
        assert_eq!(result.tracks[0].notes[0].velocity, 100);
    }
}
