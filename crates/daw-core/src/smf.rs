//! Standard MIDI File (SMF) parser.
//!
//! Parses Type 0 (single track) and Type 1 (multi-track) MIDI files into
//! [`MidiTrack`] sequences, one for each MIDI channel used in each track
//! chunk. Only note on/off events are extracted. All other event types are
//! silently skipped.
//!
//! A track chunk is split by channel because a Type 0 file, which is what
//! [`build_smf`] writes, tells its parts apart only by channel. Without the
//! split, every part of such a file would arrive as one track.
//!
//! The parser operates on a byte slice and requires no heap allocation beyond
//! the output vectors. Compatible with `no_std + alloc`.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::midi::{DEFAULT_TICKS_PER_BEAT, MidiNote, MidiTrack};

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
    /// A note event has a data byte above 127.
    InvalidData(u8),
    /// The header chunk states a length below the six bytes it must hold.
    InvalidHeaderLength(u32),
    /// The division field gives zero ticks per beat.
    InvalidDivision,
    /// A chunk states a length that runs past the end of the file.
    ChunkPastEnd(u32),
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
            SmfError::InvalidData(b) => write!(f, "invalid MIDI note data byte: 0x{b:02x}"),
            SmfError::InvalidHeaderLength(n) => {
                write!(f, "MIDI header length {n} is below 6")
            }
            SmfError::InvalidDivision => write!(f, "MIDI header gives 0 ticks per beat"),
            SmfError::ChunkPastEnd(n) => {
                write!(f, "chunk length {n} runs past the end of the file")
            }
        }
    }
}

/// The notes of one MIDI channel within one track chunk.
#[derive(Debug, Clone)]
pub struct SmfTrack {
    /// The MIDI channel of every note in `track`, 0 to 15.
    pub channel: u8,
    /// The notes, sorted by start tick and then by pitch. Never empty.
    pub track: MidiTrack,
}

/// Result of parsing a Standard MIDI File.
#[derive(Debug, Clone)]
pub struct SmfResult {
    /// One track for each channel used in each MTrk chunk, in chunk order
    /// and then in channel order. A chunk without notes, such as the tempo
    /// track that usually leads a Type 1 file, contributes none.
    pub tracks: Vec<SmfTrack>,
    /// Number of MTrk chunks in the file.
    pub chunks: u16,
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
///
/// Every chunk is read only within the length it states, and a stated
/// length that runs past the end of the input is an error. An event that
/// would run past the end of its chunk is an error too, rather than being
/// read from the bytes of the next chunk.
///
/// Limitation. A note on for a pitch that is already sounding on the same
/// channel ends the open note there, because the format pairs a note off with
/// the open note of its pitch. Two overlapping notes of one pitch therefore
/// come back with the first one shortened. [`build_smf`] states the same
/// limit from the writing side.
pub fn parse_smf(data: &[u8]) -> Result<SmfResult, SmfError> {
    let mut cursor = Cursor::new(data);

    // Parse header chunk.
    let magic = cursor.read_bytes(4)?;
    if magic != b"MThd" {
        return Err(SmfError::InvalidHeader);
    }
    let header_len = cursor.read_u32()?;
    if header_len < 6 {
        return Err(SmfError::InvalidHeaderLength(header_len));
    }
    // Bytes past the standard six are left unread inside the header chunk.
    let mut header = cursor.chunk(header_len)?;
    let format = header.read_u16()?;
    let ntrks = header.read_u16()?;
    let division = header.read_u16()?;

    if format > 1 {
        return Err(SmfError::UnsupportedFormat(format));
    }
    if division == 0 {
        return Err(SmfError::InvalidDivision);
    }

    // SMPTE-based timing is not supported. Treat the division as ticks per
    // beat when the high bit is clear.
    let ticks_per_beat = if division & 0x8000 == 0 {
        division
    } else {
        DEFAULT_TICKS_PER_BEAT
    };

    let mut tracks = Vec::new();

    for _ in 0..ntrks {
        parse_track(&mut cursor, ticks_per_beat, &mut tracks)?;
    }

    Ok(SmfResult {
        tracks,
        chunks: ntrks,
        ticks_per_beat,
        format,
    })
}

/// Parse a single MTrk chunk, and append one track to `out` for each
/// channel that has notes in it.
fn parse_track(
    cursor: &mut Cursor<'_>,
    ticks_per_beat: u16,
    out: &mut Vec<SmfTrack>,
) -> Result<(), SmfError> {
    let magic = cursor.read_bytes(4)?;
    if magic != b"MTrk" {
        return Err(SmfError::InvalidTrackHeader);
    }
    let chunk_len = cursor.read_u32()?;
    // Every read below goes through this cursor, which ends where the chunk
    // ends, so no event can take bytes from the next chunk.
    let mut chunk = cursor.chunk(chunk_len)?;
    let cursor = &mut chunk;

    let mut abs_tick: u64 = 0;
    let mut running_status: u8 = 0;

    // Pending note-on events awaiting a matching note-off. Indexed by
    // (channel, pitch) for efficient lookup. With only 16 channels and
    // 128 pitches, a flat array is practical.
    let mut pending: [Option<(u64, u8)>; 2048] = [None; 2048];
    // Finished notes with their channel.
    let mut notes: Vec<(u8, MidiNote)> = Vec::new();

    while !cursor.is_empty() {
        let delta = cursor.read_var_len()?;
        // A chunk of at most 2^32 bytes holds at most 2^32 deltas of below
        // 2^28 each, so the sum stays below 2^60.
        abs_tick += u64::from(delta);

        let peek = cursor.peek()?;

        if peek == 0xFF {
            // Meta event.
            cursor.skip(1)?; // consume 0xFF
            let _meta_type = cursor.read_u8()?;
            let meta_len = cursor.read_var_len()?;
            cursor.skip_var_len(meta_len)?;
        } else if peek == 0xF0 || peek == 0xF7 {
            // SysEx event.
            cursor.skip(1)?;
            let sysex_len = cursor.read_var_len()?;
            cursor.skip_var_len(sysex_len)?;
        } else if peek >= 0x80 {
            // Channel event with new status byte.
            let status = cursor.read_u8()?;
            running_status = status;
            parse_channel_event(cursor, status, abs_tick, &mut pending, &mut notes)?;
        } else {
            // Running status (no new status byte).
            if running_status == 0 {
                return Err(SmfError::InvalidEvent(peek));
            }
            parse_channel_event(cursor, running_status, abs_tick, &mut pending, &mut notes)?;
        }
    }

    // Close any pending notes at the final tick.
    for (idx, slot) in pending.iter_mut().enumerate() {
        if let Some((start_tick, velocity)) = slot.take() {
            let duration = abs_tick.saturating_sub(start_tick).max(1);
            let channel = (idx / 128) as u8;
            let pitch = (idx % 128) as u8;
            notes.push((
                channel,
                MidiNote {
                    tick: start_tick,
                    duration,
                    pitch,
                    velocity,
                },
            ));
        }
    }

    // Group by channel, with each group sorted like a MidiTrack.
    notes.sort_by(|(ca, a), (cb, b)| {
        ca.cmp(cb)
            .then(a.tick.cmp(&b.tick))
            .then(a.pitch.cmp(&b.pitch))
    });
    // Tracks before `first_own` came from earlier chunks and are never
    // extended, even when their channel matches.
    let first_own = out.len();
    for (channel, note) in notes {
        let extend = out.len() > first_own && out.last().is_some_and(|t| t.channel == channel);
        match out.last_mut() {
            Some(last) if extend => last.track.notes.push(note),
            _ => out.push(SmfTrack {
                channel,
                track: MidiTrack {
                    notes: alloc::vec![note],
                    ticks_per_beat,
                },
            }),
        }
    }

    Ok(())
}

/// Parse a MIDI channel event and extract note on/off pairs.
fn parse_channel_event(
    cursor: &mut Cursor<'_>,
    status: u8,
    abs_tick: u64,
    pending: &mut [Option<(u64, u8)>; 2048],
    notes: &mut Vec<(u8, MidiNote)>,
) -> Result<(), SmfError> {
    let msg_type = status & 0xF0;
    let channel = (status & 0x0F) as usize;

    match msg_type {
        0x80 => {
            // Note Off.
            let pitch = cursor.read_data_byte()?;
            let _velocity = cursor.read_data_byte()?;
            close_note(pending, channel, pitch, abs_tick, notes);
        }
        0x90 => {
            // Note On (velocity 0 = note off).
            let pitch = cursor.read_data_byte()?;
            let velocity = cursor.read_data_byte()?;
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
    notes: &mut Vec<(u8, MidiNote)>,
) {
    let idx = channel * 128 + pitch as usize;
    if let Some((start_tick, velocity)) = pending[idx].take() {
        let duration = off_tick.saturating_sub(start_tick).max(1);
        notes.push((
            channel as u8,
            MidiNote {
                tick: start_tick,
                duration,
                pitch,
                velocity,
            },
        ));
    }
}

/// Byte-level cursor for reading binary data from a slice.
///
/// Invariant. `pos <= data.len()`. Every read goes through [`Cursor::take`],
/// which checks the end with checked arithmetic before it moves `pos`.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Take the next `n` bytes, or fail without moving if fewer remain.
    fn take(&mut self, n: usize) -> Result<&'a [u8], SmfError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&end| end <= self.data.len())
            .ok_or(SmfError::UnexpectedEof)?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// Take a chunk body of the stated length as a cursor of its own, which
    /// cannot read past the chunk. A length that runs past the end of this
    /// cursor, or that does not fit in `usize`, is [`SmfError::ChunkPastEnd`].
    fn chunk(&mut self, len: u32) -> Result<Cursor<'a>, SmfError> {
        let body = usize::try_from(len)
            .ok()
            .and_then(|n| self.take(n).ok())
            .ok_or(SmfError::ChunkPastEnd(len))?;
        Ok(Cursor::new(body))
    }

    fn peek(&self) -> Result<u8, SmfError> {
        self.data
            .get(self.pos)
            .copied()
            .ok_or(SmfError::UnexpectedEof)
    }

    fn read_u8(&mut self) -> Result<u8, SmfError> {
        Ok(self.take(1)?[0])
    }

    /// Read a data byte of a channel event, which is 0 to 127. A larger
    /// pitch would index past the pending-note table.
    fn read_data_byte(&mut self) -> Result<u8, SmfError> {
        let b = self.read_u8()?;
        if b > 0x7F {
            return Err(SmfError::InvalidData(b));
        }
        Ok(b)
    }

    fn read_u16(&mut self) -> Result<u16, SmfError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, SmfError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], SmfError> {
        self.take(n)
    }

    fn skip(&mut self, n: usize) -> Result<(), SmfError> {
        self.take(n).map(|_| ())
    }

    /// Skip a length read as a variable-length quantity. A length that does
    /// not fit in `usize` cannot fit in the input either.
    fn skip_var_len(&mut self, n: u32) -> Result<(), SmfError> {
        let n = usize::try_from(n).map_err(|_| SmfError::UnexpectedEof)?;
        self.skip(n)
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

/// The largest value a variable-length quantity of four bytes holds, and so
/// the longest gap between two events that [`build_smf`] can write.
pub const MAX_VAR_LEN: u32 = 0x0FFF_FFFF;

/// The largest tempo value a set-tempo meta event holds, in microseconds per
/// quarter note.
const MAX_USEC_PER_BEAT: f64 = 0xFF_FFFF as f64;

/// What is wrong with a note that [`build_smf`] refuses to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteFault {
    /// The pitch is above 127.
    Pitch(u8),
    /// The velocity is 0, which the format reads as a note off, or above 127.
    Velocity(u8),
    /// The duration is 0, so the note off would precede its own note on.
    ZeroDuration,
    /// The end tick does not fit in a `u64`.
    EndOverflows,
}

/// Errors that stop [`build_smf`] from writing a file that would read back
/// differently or not at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmfBuildError {
    /// More tracks than the 16 MIDI channels.
    TooManyTracks(usize),
    /// Ticks per beat is 0, or has the high bit set, which marks SMPTE timing.
    InvalidTicksPerBeat(u16),
    /// The tempo is not finite, or gives a quarter note outside 1 to
    /// 16,777,215 microseconds.
    InvalidTempo,
    /// A note that cannot be written. `track` is its index in the input.
    InvalidNote {
        track: usize,
        tick: u64,
        fault: NoteFault,
    },
    /// The event at `tick` follows the one before it by more than
    /// [`MAX_VAR_LEN`] ticks.
    GapTooLong { tick: u64 },
    /// The track chunk would exceed the 4 GiB its length field can state.
    TrackTooLong,
    /// A track with notes counts its ticks at a resolution other than the
    /// one the file is written at. `track` is its index in the input.
    MixedTicksPerBeat { track: usize, ticks_per_beat: u16 },
}

impl core::fmt::Display for SmfBuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SmfBuildError::TooManyTracks(n) => {
                write!(f, "{n} tracks, but a MIDI file has 16 channels")
            }
            SmfBuildError::InvalidTicksPerBeat(t) => write!(f, "invalid ticks per beat: {t}"),
            SmfBuildError::InvalidTempo => write!(f, "tempo out of range for a MIDI file"),
            SmfBuildError::InvalidNote { track, tick, fault } => {
                write!(f, "channel {} note at tick {tick}: ", track + 1)?;
                match fault {
                    NoteFault::Pitch(p) => write!(f, "pitch {p} is above 127"),
                    NoteFault::Velocity(v) => write!(f, "velocity {v} is outside 1 to 127"),
                    NoteFault::ZeroDuration => write!(f, "duration is 0"),
                    NoteFault::EndOverflows => write!(f, "end tick overflows"),
                }
            }
            SmfBuildError::GapTooLong { tick } => write!(
                f,
                "the event at tick {tick} is more than {MAX_VAR_LEN} ticks after the one before"
            ),
            SmfBuildError::TrackTooLong => write!(f, "track too long for a MIDI file"),
            SmfBuildError::MixedTicksPerBeat {
                track,
                ticks_per_beat,
            } => write!(
                f,
                "channel {} counts {ticks_per_beat} ticks per beat, unlike the project",
                track + 1
            ),
        }
    }
}

/// Build a minimal Standard MIDI File (Type 0) from a set of MidiTracks.
///
/// All tracks are merged into a single SMF track, and track `i` is written
/// on MIDI channel `i`, which is how [`parse_smf`] tells them apart again.
/// The output uses the given ticks per beat for timing resolution. The tempo
/// and time signature are written as meta events at tick 0. A denominator
/// that is not a power of two up to 32 is written as a quarter note.
///
/// Input that the format cannot carry is refused with an error rather than
/// written wrongly. See [`SmfBuildError`].
///
/// A file has one resolution, so every track with notes must count its ticks
/// at `ticks_per_beat`. A track that does not is refused rather than
/// rescaled, because rescaling rounds note times to a coarser grid without
/// saying so, and because it is not certain which of the two figures the
/// notes were written in. An empty track carries no times, so its own figure
/// is not checked.
///
/// Limitation. Two notes of the same pitch that overlap on one track are
/// written, but cannot be read back as they were, because the format pairs a
/// note off with the open note of its pitch. The reader ends the first note
/// where the second starts.
pub fn build_smf(
    tracks: &[MidiTrack],
    ticks_per_beat: u16,
    tempo_bpm: f64,
    beats_per_bar: u8,
    beat_denominator: u8,
) -> Result<Vec<u8>, SmfBuildError> {
    if tracks.len() > 16 {
        return Err(SmfBuildError::TooManyTracks(tracks.len()));
    }
    if ticks_per_beat == 0 || ticks_per_beat & 0x8000 != 0 {
        return Err(SmfBuildError::InvalidTicksPerBeat(ticks_per_beat));
    }
    let usec = 60_000_000.0 / tempo_bpm;
    if !usec.is_finite() || !(1.0..=MAX_USEC_PER_BEAT).contains(&usec) {
        return Err(SmfBuildError::InvalidTempo);
    }
    // In range by the check above, so the cast neither saturates nor wraps.
    let usec_per_beat = usec as u32;

    // Collect all note events from all tracks with channel assignment.
    let mut events: Vec<(u64, u8, u8, u8, bool)> = Vec::new(); // (tick, channel, pitch, velocity, is_on)

    for (ch, track) in tracks.iter().enumerate() {
        // At most 16 tracks, checked above.
        let channel = ch as u8;
        if !track.notes.is_empty() && track.ticks_per_beat != ticks_per_beat {
            return Err(SmfBuildError::MixedTicksPerBeat {
                track: ch,
                ticks_per_beat: track.ticks_per_beat,
            });
        }
        for note in &track.notes {
            let fault = if note.pitch > 0x7F {
                Some(NoteFault::Pitch(note.pitch))
            } else if note.velocity == 0 || note.velocity > 0x7F {
                Some(NoteFault::Velocity(note.velocity))
            } else if note.duration == 0 {
                Some(NoteFault::ZeroDuration)
            } else if note.tick.checked_add(note.duration).is_none() {
                Some(NoteFault::EndOverflows)
            } else {
                None
            };
            if let Some(fault) = fault {
                return Err(SmfBuildError::InvalidNote {
                    track: ch,
                    tick: note.tick,
                    fault,
                });
            }
            // Cannot overflow, checked above.
            let off_tick = note.tick + note.duration;
            events.push((note.tick, channel, note.pitch, note.velocity, true));
            events.push((off_tick, channel, note.pitch, 0, false));
        }
    }

    // Sort events by tick, with note-off before note-on at the same tick.
    events.sort_by(|a, b| a.0.cmp(&b.0).then(a.4.cmp(&b.4)));

    // Build track data with tempo and time signature meta events at tick 0.
    let mut track_data: Vec<u8> = Vec::new();

    // Tempo meta event: FF 51 03 tt tt tt (microseconds per quarter note).
    write_var_len(&mut track_data, 0); // delta time = 0
    track_data.push(0xFF);
    track_data.push(0x51);
    track_data.push(0x03);
    track_data.push((usec_per_beat >> 16) as u8);
    track_data.push((usec_per_beat >> 8) as u8);
    track_data.push(usec_per_beat as u8);

    // Time signature meta event: FF 58 04 nn dd cc bb.
    // nn = numerator (beats_per_bar), dd = denominator power (2^dd),
    // cc = MIDI clocks per metronome click (24), bb = 32nd notes per quarter (8).
    let denom_power = match beat_denominator {
        1 => 0,  // 2^0 = 1
        2 => 1,  // 2^1 = 2
        4 => 2,  // 2^2 = 4
        8 => 3,  // 2^3 = 8
        16 => 4, // 2^4 = 16
        32 => 5, // 2^5 = 32
        _ => 2,  // Default to quarter note.
    };
    write_var_len(&mut track_data, 0); // delta time = 0
    track_data.push(0xFF);
    track_data.push(0x58);
    track_data.push(0x04);
    track_data.push(beats_per_bar);
    track_data.push(denom_power);
    track_data.push(24); // MIDI clocks per metronome click
    track_data.push(8); // 32nd notes per quarter note

    let mut prev_tick: u64 = 0;

    for (tick, channel, pitch, velocity, is_on) in &events {
        // Events are sorted, so the subtraction cannot underflow.
        let delta = match u32::try_from(tick - prev_tick) {
            Ok(d) if d <= MAX_VAR_LEN => d,
            _ => return Err(SmfBuildError::GapTooLong { tick: *tick }),
        };
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
    let track_len = u32::try_from(track_data.len()).map_err(|_| SmfBuildError::TrackTooLong)?;

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
    out.extend_from_slice(&track_len.to_be_bytes());
    out.extend_from_slice(&track_data);

    Ok(out)
}

/// Write a variable-length quantity to a byte vector.
///
/// # Panics
///
/// If `value` exceeds [`MAX_VAR_LEN`], which needs a fifth byte the format
/// does not allow. Callers check the bound first.
fn write_var_len(out: &mut Vec<u8>, mut value: u32) {
    assert!(
        value <= MAX_VAR_LEN,
        "variable-length quantity {value} exceeds {MAX_VAR_LEN}"
    );
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
    let total_notes: usize = result.tracks.iter().map(|t| t.track.notes.len()).sum();
    alloc::format!(
        "Format {}, {} track chunks, {} channel tracks with notes, {} total notes, {} tpb",
        result.format,
        result.chunks,
        result.tracks.len(),
        total_notes,
        result.ticks_per_beat,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
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
        assert_eq!(result.tracks[0].track.notes.len(), 1);

        let note = &result.tracks[0].track.notes[0];
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
        assert_eq!(result.tracks[0].track.notes.len(), 3);
        assert_eq!(result.tracks[0].track.notes[0].pitch, 60);
        assert_eq!(result.tracks[0].track.notes[1].pitch, 64);
        assert_eq!(result.tracks[0].track.notes[2].pitch, 67);
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
        assert_eq!(result.tracks[0].track.notes.len(), 1);
        assert_eq!(result.tracks[0].track.notes[0].duration, 480);
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
        assert_eq!(result.tracks[0].track.notes.len(), 2);
        assert_eq!(result.tracks[0].track.notes[0].pitch, 60);
        assert_eq!(result.tracks[0].track.notes[0].tick, 0);
        assert_eq!(result.tracks[0].track.notes[0].duration, 480);
        assert_eq!(result.tracks[0].track.notes[1].pitch, 64);
        assert_eq!(result.tracks[0].track.notes[1].tick, 480);
        assert_eq!(result.tracks[0].track.notes[1].duration, 240);
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
        assert_eq!(result.chunks, 2);
        // The tempo chunk has no notes, so only the second chunk yields a track.
        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.tracks[0].channel, 0);
        assert_eq!(result.tracks[0].track.notes.len(), 1);
        assert_eq!(result.tracks[0].track.notes[0].pitch, 72);
        assert_eq!(result.tracks[0].track.notes[0].duration, 960);
    }

    #[test]
    fn roundtrip_build_parse() {
        let track = MidiTrack {
            notes: vec![
                MidiNote {
                    tick: 0,
                    duration: 480,
                    pitch: 60,
                    velocity: 100,
                },
                MidiNote {
                    tick: 480,
                    duration: 240,
                    pitch: 64,
                    velocity: 80,
                },
                MidiNote {
                    tick: 720,
                    duration: 480,
                    pitch: 67,
                    velocity: 90,
                },
            ],
            ticks_per_beat: 480,
        };

        let smf_data = build_smf(core::slice::from_ref(&track), 480, 120.0, 4, 4).expect("build");
        let result = parse_smf(&smf_data).expect("roundtrip parse");

        assert_eq!(result.ticks_per_beat, 480);
        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.tracks[0].track.notes.len(), 3);

        for (i, note) in result.tracks[0].track.notes.iter().enumerate() {
            assert_eq!(note.tick, track.notes[i].tick);
            assert_eq!(note.duration, track.notes[i].duration);
            assert_eq!(note.pitch, track.notes[i].pitch);
            assert_eq!(note.velocity, track.notes[i].velocity);
        }
    }

    #[test]
    fn empty_track_roundtrip() {
        let smf_data = build_smf(&[], 480, 120.0, 4, 4).expect("build");
        let result = parse_smf(&smf_data).expect("parse empty");
        assert_eq!(result.chunks, 1);
        assert!(result.tracks.is_empty());
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
            tracks: vec![SmfTrack {
                channel: 3,
                track: MidiTrack {
                    notes: vec![MidiNote {
                        tick: 0,
                        duration: 480,
                        pitch: 60,
                        velocity: 100,
                    }],
                    ticks_per_beat: 480,
                },
            }],
            chunks: 2,
            ticks_per_beat: 480,
            format: 1,
        };
        let desc = describe_import(&result);
        assert!(desc.contains("Format 1"));
        assert!(desc.contains("2 track chunks"));
        assert!(desc.contains("1 channel tracks with notes"));
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
        assert_eq!(result.tracks[0].track.notes.len(), 1);
        assert_eq!(result.tracks[0].track.notes[0].pitch, 60);
    }

    #[test]
    fn build_smf_includes_tempo_meta_event() {
        let smf_data = build_smf(&[], 480, 140.0, 4, 4).expect("build");
        // At 140 BPM, microseconds per beat = 60_000_000 / 140 = 428571.
        let expected_usec: u32 = (60_000_000.0 / 140.0) as u32;
        let tempo_bytes = [
            (expected_usec >> 16) as u8,
            (expected_usec >> 8) as u8,
            expected_usec as u8,
        ];
        // Search for the tempo meta event (FF 51 03 + 3 bytes) in the file.
        let pattern = [
            0xFF,
            0x51,
            0x03,
            tempo_bytes[0],
            tempo_bytes[1],
            tempo_bytes[2],
        ];
        let found = smf_data.windows(pattern.len()).any(|w| w == pattern);
        assert!(found, "tempo meta event not found in SMF data");
    }

    #[test]
    fn build_smf_includes_time_signature_meta_event() {
        let smf_data = build_smf(&[], 480, 120.0, 3, 4).expect("build");
        // Time signature: FF 58 04 03 02 18 08 (3/4 time).
        let pattern = [0xFF, 0x58, 0x04, 3, 0x02, 24, 8];
        let found = smf_data.windows(pattern.len()).any(|w| w == pattern);
        assert!(found, "time signature meta event not found in SMF data");
    }

    #[test]
    fn build_smf_roundtrip_with_tempo() {
        let track = MidiTrack {
            notes: vec![MidiNote {
                tick: 0,
                duration: 480,
                pitch: 60,
                velocity: 100,
            }],
            ticks_per_beat: 480,
        };
        let smf_data = build_smf(core::slice::from_ref(&track), 480, 90.0, 3, 4).expect("build");
        let result = parse_smf(&smf_data).expect("roundtrip with tempo");
        assert_eq!(result.tracks[0].track.notes.len(), 1);
        assert_eq!(result.tracks[0].track.notes[0].pitch, 60);
        assert_eq!(result.tracks[0].track.notes[0].velocity, 100);
    }

    /// Wrap track chunk bodies, each ending in its own end-of-track event,
    /// in a file of the given format at 480 ticks per beat.
    fn smf_file(format: u16, chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(&format.to_be_bytes());
        data.extend_from_slice(&(chunks.len() as u16).to_be_bytes());
        data.extend_from_slice(&480u16.to_be_bytes());
        for chunk in chunks {
            data.extend_from_slice(b"MTrk");
            data.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
            data.extend_from_slice(chunk);
        }
        data
    }

    fn note(tick: u64, duration: u64, pitch: u8, velocity: u8) -> MidiNote {
        MidiNote {
            tick,
            duration,
            pitch,
            velocity,
        }
    }

    fn one_note_track(n: MidiNote) -> MidiTrack {
        MidiTrack {
            notes: vec![n],
            ticks_per_beat: 480,
        }
    }

    #[test]
    fn parse_splits_a_chunk_by_channel() {
        // Channel 9 first in the file, channel 0 later, so the output order
        // shows the sort by channel.
        let chunk = vec![
            0x00, 0x99, 36, 100, // ch 10 on
            0x00, 0x90, 60, 80, // ch 1 on
            0x60, 0x89, 36, 0, // ch 10 off at 96
            0x60, 0x80, 60, 0, // ch 1 off at 192
            0x00, 0xFF, 0x2F, 0x00,
        ];
        let result = parse_smf(&smf_file(0, &[chunk])).expect("parse");
        assert_eq!(result.tracks.len(), 2);
        assert_eq!(result.tracks[0].channel, 0);
        assert_eq!(result.tracks[0].track.notes, vec![note(0, 192, 60, 80)]);
        assert_eq!(result.tracks[1].channel, 9);
        assert_eq!(result.tracks[1].track.notes, vec![note(0, 96, 36, 100)]);
    }

    #[test]
    fn parse_keeps_chunks_apart_on_one_channel() {
        let chunk = |pitch: u8| {
            vec![
                0x00, 0x90, pitch, 90, 0x60, 0x80, pitch, 0, 0x00, 0xFF, 0x2F, 0x00,
            ]
        };
        let result = parse_smf(&smf_file(1, &[chunk(60), chunk(62)])).expect("parse");
        assert_eq!(result.tracks.len(), 2);
        assert_eq!(result.tracks[0].track.notes, vec![note(0, 96, 60, 90)]);
        assert_eq!(result.tracks[1].track.notes, vec![note(0, 96, 62, 90)]);
        assert!(result.tracks.iter().all(|t| t.channel == 0));
    }

    /// A pitch byte above 127 on channel 16 used to index past the
    /// pending-note table.
    #[test]
    fn parse_rejects_a_data_byte_above_127() {
        let chunk = vec![0x00, 0x9F, 0xFF, 100, 0x00, 0xFF, 0x2F, 0x00];
        let err = parse_smf(&smf_file(0, &[chunk])).unwrap_err();
        assert_eq!(err, SmfError::InvalidData(0xFF));
        let chunk = vec![0x00, 0x90, 60, 0x80, 0x00, 0xFF, 0x2F, 0x00];
        let err = parse_smf(&smf_file(0, &[chunk])).unwrap_err();
        assert_eq!(err, SmfError::InvalidData(0x80));
    }

    #[test]
    fn build_smf_writes_track_index_as_channel() {
        let tracks = [
            one_note_track(note(0, 10, 60, 100)),
            MidiTrack::new(),
            one_note_track(note(5, 10, 61, 100)),
        ];
        let result =
            parse_smf(&build_smf(&tracks, 480, 120.0, 4, 4).expect("build")).expect("parse");
        let channels: Vec<u8> = result.tracks.iter().map(|t| t.channel).collect();
        assert_eq!(channels, vec![0, 2]);
        assert_eq!(result.tracks[0].track.notes, tracks[0].notes);
        assert_eq!(result.tracks[1].track.notes, tracks[2].notes);
    }

    #[test]
    fn build_smf_rejects_more_tracks_than_channels() {
        let tracks = vec![MidiTrack::new(); 17];
        assert_eq!(
            build_smf(&tracks, 480, 120.0, 4, 4),
            Err(SmfBuildError::TooManyTracks(17))
        );
        assert!(build_smf(&tracks[..16], 480, 120.0, 4, 4).is_ok());
    }

    #[test]
    fn build_smf_rejects_invalid_ticks_per_beat() {
        for tpb in [0, 0x8000, u16::MAX] {
            assert_eq!(
                build_smf(&[], tpb, 120.0, 4, 4),
                Err(SmfBuildError::InvalidTicksPerBeat(tpb))
            );
        }
        assert!(build_smf(&[], 0x7FFF, 120.0, 4, 4).is_ok());
    }

    #[test]
    fn build_smf_rejects_a_tempo_the_format_cannot_hold() {
        // 3 BPM needs 20,000,000 microseconds per beat, above 0xFFFFFF.
        for bpm in [0.0, -1.0, f64::NAN, f64::INFINITY, 3.0, 1e9] {
            assert_eq!(
                build_smf(&[], 480, bpm, 4, 4),
                Err(SmfBuildError::InvalidTempo),
                "{bpm}"
            );
        }
        // The slowest tempo that fits, from the format's bound.
        let slowest = 60_000_000.0 / MAX_USEC_PER_BEAT;
        assert!(build_smf(&[], 480, slowest, 4, 4).is_ok());
    }

    #[test]
    fn build_smf_rejects_notes_it_cannot_write() {
        let cases = [
            (note(7, 1, 128, 100), NoteFault::Pitch(128)),
            (note(7, 1, 60, 0), NoteFault::Velocity(0)),
            (note(7, 1, 60, 128), NoteFault::Velocity(128)),
            (note(7, 0, 60, 100), NoteFault::ZeroDuration),
            (note(u64::MAX, 1, 60, 100), NoteFault::EndOverflows),
        ];
        for (n, fault) in cases {
            let tracks = [MidiTrack::new(), one_note_track(n)];
            assert_eq!(
                build_smf(&tracks, 480, 120.0, 4, 4),
                Err(SmfBuildError::InvalidNote {
                    track: 1,
                    tick: n.tick,
                    fault
                }),
                "{n:?}"
            );
        }
    }

    #[test]
    fn build_smf_bounds_the_gap_between_events() {
        let longest = u64::from(MAX_VAR_LEN);
        let fits = [one_note_track(note(longest, 1, 60, 100))];
        let data = build_smf(&fits, 480, 120.0, 4, 4).expect("the longest gap fits");
        let result = parse_smf(&data).expect("parse");
        assert_eq!(result.tracks[0].track.notes, fits[0].notes);

        let too_far = [one_note_track(note(longest + 1, 1, 60, 100))];
        assert_eq!(
            build_smf(&too_far, 480, 120.0, 4, 4),
            Err(SmfBuildError::GapTooLong { tick: longest + 1 })
        );
        // A long note is a gap too, between its own on and off.
        let long_note = [one_note_track(note(0, longest + 1, 60, 100))];
        assert_eq!(
            build_smf(&long_note, 480, 120.0, 4, 4),
            Err(SmfBuildError::GapTooLong { tick: longest + 1 })
        );
    }

    #[test]
    #[should_panic(expected = "exceeds")]
    fn write_var_len_refuses_a_fifth_byte() {
        write_var_len(&mut Vec::new(), MAX_VAR_LEN + 1);
    }

    #[test]
    fn build_errors_name_the_problem() {
        let err = SmfBuildError::InvalidNote {
            track: 2,
            tick: 40,
            fault: NoteFault::Velocity(0),
        };
        assert_eq!(
            err.to_string(),
            "channel 3 note at tick 40: velocity 0 is outside 1 to 127"
        );
    }

    /// A header with the given stated length and body, and no track chunks
    /// unless the body says so.
    fn header(len: u32, body: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&len.to_be_bytes());
        data.extend_from_slice(body);
        data
    }

    /// A one-note chunk on channel 1, which parses on its own.
    fn note_chunk() -> Vec<u8> {
        vec![
            0x00, 0x90, 60, 90, 0x60, 0x80, 60, 0, 0x00, 0xFF, 0x2F, 0x00,
        ]
    }

    #[test]
    fn parse_keeps_every_event_inside_its_chunk() {
        // Each first chunk ends partway through an event, and the bytes of the
        // second chunk would complete it if the parser read past the end.
        let cut_short = [
            // A text meta event that states 5 bytes and holds 2.
            vec![0x00, 0xFF, 0x01, 0x05, b'a', b'b'],
            // A SysEx event that states 4 bytes and holds 1.
            vec![0x00, 0xF0, 0x04, 0x01],
            // A note on without its velocity.
            vec![0x00, 0x90, 60],
            // A delta time whose continuation bit runs off the end.
            vec![0x81],
        ];
        for first in cut_short {
            let data = smf_file(1, &[first.clone(), note_chunk()]);
            assert_eq!(
                parse_smf(&data).unwrap_err(),
                SmfError::UnexpectedEof,
                "{first:02x?}"
            );
        }
        // The same second chunk parses after a first chunk that is whole.
        let whole = vec![0x00, 0xFF, 0x01, 0x02, b'a', b'b', 0x00, 0xFF, 0x2F, 0x00];
        let result = parse_smf(&smf_file(1, &[whole, note_chunk()])).expect("parse");
        assert_eq!(result.tracks.len(), 1);
    }

    #[test]
    fn parse_rejects_a_chunk_longer_than_the_file() {
        let mut data = smf_file(0, &[note_chunk()]);
        let len = note_chunk().len() as u32;
        // The length field of the only track chunk follows the 14-byte
        // header and the 4-byte chunk tag.
        for stated in [len + 1, u32::MAX] {
            data[18..22].copy_from_slice(&stated.to_be_bytes());
            assert_eq!(
                parse_smf(&data).unwrap_err(),
                SmfError::ChunkPastEnd(stated)
            );
        }
        data[18..22].copy_from_slice(&len.to_be_bytes());
        assert!(parse_smf(&data).is_ok());
    }

    #[test]
    fn parse_checks_the_header_length() {
        let fields = [0x00, 0x00, 0x00, 0x00, 0x01, 0xE0];
        for len in [0, 5] {
            assert_eq!(
                parse_smf(&header(len, &fields)).unwrap_err(),
                SmfError::InvalidHeaderLength(len)
            );
        }
        // Two extra header bytes are skipped, and a length past the end of
        // the file is refused rather than read into what follows.
        let mut long = fields.to_vec();
        long.extend_from_slice(&[0xAA, 0xBB]);
        let result = parse_smf(&header(8, &long)).expect("an 8-byte header");
        assert_eq!(result.ticks_per_beat, 480);
        assert_eq!(
            parse_smf(&header(9, &long)).unwrap_err(),
            SmfError::ChunkPastEnd(9)
        );
    }

    #[test]
    fn parse_rejects_zero_ticks_per_beat() {
        assert_eq!(
            parse_smf(&header(6, &[0, 0, 0, 0, 0, 0])).unwrap_err(),
            SmfError::InvalidDivision
        );
        assert_eq!(
            parse_smf(&header(6, &[0, 0, 0, 0, 0, 1]))
                .expect("one tick per beat")
                .ticks_per_beat,
            1
        );
    }

    /// The bound checks use checked arithmetic, so a length near `usize::MAX`
    /// fails instead of overflowing.
    #[test]
    fn cursor_refuses_a_length_that_overflows() {
        let mut cursor = Cursor::new(&[1, 2, 3]);
        cursor.read_u8().expect("one byte");
        assert_eq!(cursor.skip(usize::MAX), Err(SmfError::UnexpectedEof));
        assert_eq!(cursor.read_bytes(usize::MAX), Err(SmfError::UnexpectedEof));
        assert_eq!(cursor.pos, 1, "a failed read does not move the cursor");
        assert_eq!(cursor.skip(3), Err(SmfError::UnexpectedEof));
        assert_eq!(cursor.skip(2), Ok(()));
        assert!(cursor.is_empty());
    }

    #[test]
    fn build_smf_refuses_a_track_at_another_resolution() {
        let mut coarse = one_note_track(note(0, 10, 60, 100));
        coarse.ticks_per_beat = 96;
        let tracks = [one_note_track(note(0, 10, 62, 100)), coarse];
        assert_eq!(
            build_smf(&tracks, 480, 120.0, 4, 4),
            Err(SmfBuildError::MixedTicksPerBeat {
                track: 1,
                ticks_per_beat: 96
            })
        );
        assert_eq!(
            SmfBuildError::MixedTicksPerBeat {
                track: 1,
                ticks_per_beat: 96
            }
            .to_string(),
            "channel 2 counts 96 ticks per beat, unlike the project"
        );
        // An empty track has no times to misread, so its figure is ignored.
        let mut empty = MidiTrack::new();
        empty.ticks_per_beat = 96;
        let tracks = [one_note_track(note(0, 10, 62, 100)), empty];
        assert!(build_smf(&tracks, 480, 120.0, 4, 4).is_ok());
    }
}
