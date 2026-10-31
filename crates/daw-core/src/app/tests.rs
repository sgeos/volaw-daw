use super::*;
use crate::platform::MemoryPlatform;
use audio::NullSink;

const ROOT: &str = "proj";

fn app() -> DawApp<MemoryPlatform, NullSink> {
    let mut app = DawApp::new(MemoryPlatform::new(), NullSink, 48_000);
    app.set_project_root(ROOT);
    app
}

fn write_script(app: &DawApp<MemoryPlatform, NullSink>, file: &str, source: &str) {
    let path = join_path(&join_path(ROOT, "scripts"), file);
    app.platform().write_file(&path, source.as_bytes()).unwrap();
}

#[test]
fn load_scripts_reports_failure_in_status_bar() {
    let mut state = app();
    // A reordered declaration, which the entry contract rejects.
    write_script(
        &state,
        "bad.kma",
        "struct TickState { beat: Float, tempo: Float, ticks: Word }\n\
         fn main(state: TickState) -> () { () }\n",
    );
    state.project.channels[1].script_file = String::from("bad.kma");
    state.project.channels[2].script_file = String::from("missing.kma");
    state.load_scripts();

    assert!(state.scripts[1].is_none());
    assert!(state.scripts[2].is_none());
    let message = state.status_message().expect("no status message");
    assert!(
        message.starts_with("Script Ch2: contract error"),
        "{message}"
    );
    assert!(message.ends_with("(and 1 more, see the log)"), "{message}");
}

#[test]
fn load_scripts_success_leaves_status_bar_alone() {
    let mut state = app();
    write_script(&state, "ok.kma", "fn main(state: TickState) -> () { () }\n");
    state.project.channels[0].script_file = String::from("ok.kma");
    state.load_scripts();

    assert!(state.scripts[0].is_some());
    assert_eq!(state.status_message(), None);
}

#[test]
fn load_scripts_rejects_a_traversing_name() {
    let mut state = app();
    state
        .platform()
        .write_file("outside.kma", b"fn main(state: TickState) -> () { () }\n")
        .unwrap();
    state.project.channels[0].script_file = String::from("../../outside.kma");
    state.load_scripts();
    assert!(state.scripts[0].is_none());
    assert!(state.status_message().is_some());
}

#[test]
fn script_hot_reload_waits_for_the_interval_and_a_change() {
    let mut state = app();
    write_script(&state, "s.kma", "fn main(state: TickState) -> () { () }\n");
    state.assign_script(0, "s.kma").unwrap();
    state.load_scripts();
    write_script(&state, "s.kma", "fn main(state: TickState) -> () { ( }\n");

    state.platform().advance_ms(SCRIPT_CHECK_INTERVAL_MS - 1);
    state.check_script_reload();
    assert_eq!(state.status_message(), None, "checked before the interval");

    state.platform().advance_ms(1);
    state.check_script_reload();
    let message = state.status_message().expect("reload should report");
    assert!(message.starts_with("Script error"), "{message}");
}

#[test]
fn preview_note_ends_after_its_duration() {
    let mut state = app();
    state.preview_note = Some((0, 440.0, state.platform().elapsed_ms()));
    state.platform().advance_ms(PREVIEW_NOTE_MS - 1);
    state.check_preview_timeout();
    assert!(state.preview_note.is_some());
    state.platform().advance_ms(1);
    state.check_preview_timeout();
    assert!(state.preview_note.is_none());
}

#[test]
fn save_then_load_project_roundtrip() {
    let mut state = app();
    state.project.name = String::from("Round Trip");
    state.project.tempo = 97.0;
    state.tracks[3].notes.push(MidiNote {
        tick: 0,
        duration: 240,
        pitch: 64,
        velocity: 90,
    });
    state.save();
    assert!(state.status_message().unwrap().starts_with("Saved: "));

    let mut other = DawApp::new(MemoryPlatform::new(), NullSink, 48_000);
    other.set_project_root(ROOT);
    // Share the files by copying them into the second platform.
    for path in state.platform().list_files("proj/saves", "toml").unwrap() {
        let bytes = state.platform().read_file(&path).unwrap();
        other.platform().write_file(&path, &bytes).unwrap();
    }
    for dir in ["proj/patches", "proj/effects", "proj/midi"] {
        for path in state.platform().list_files(dir, "toml").unwrap() {
            let bytes = state.platform().read_file(&path).unwrap();
            other.platform().write_file(&path, &bytes).unwrap();
        }
    }
    other.load_project("round_trip").expect("load");
    assert_eq!(other.project.name, "Round Trip");
    assert_eq!(other.transport.tempo, 97.0);
    assert_eq!(other.tracks[3].notes, state.tracks[3].notes);
}

#[test]
fn load_missing_project_leaves_state_unchanged() {
    let mut state = app();
    state.project.name = String::from("Keep");
    assert!(state.load_project("absent").is_err());
    assert_eq!(state.project.name, "Keep");
}

#[test]
fn export_midi_writes_a_standard_midi_file() {
    let mut state = app();
    state.project.name = String::from("Song");
    state.export_midi();
    let data = state.platform().read_file("proj/exports/song.mid").unwrap();
    assert_eq!(&data[..4], b"MThd");
}

#[test]
fn export_midi_rejects_a_traversing_project_name() {
    let mut state = app();
    state.project.name = String::from("../evil");
    state.export_midi();
    assert_eq!(state.platform().file_count(), 0);
    assert!(state.status_message().unwrap().starts_with("Export failed"));
}

#[test]
fn assign_script_rejects_out_of_range_channel() {
    let mut state = app();
    assert!(state.assign_script(NUM_CHANNELS, "x.kma").is_err());
    assert!(state.assign_script(NUM_CHANNELS - 1, "x.kma").is_ok());
}

#[test]
fn import_midi_rejects_garbage() {
    let mut state = app();
    assert!(state.import_midi(b"not midi", 0).is_err());
}

#[test]
fn import_midi_roundtrips_an_export() {
    let mut source = app();
    source.tracks[0].notes.push(MidiNote {
        tick: 0,
        duration: 480,
        pitch: 60,
        velocity: 100,
    });
    source.project.name = String::from("m");
    source.export_midi();
    let data = source.platform().read_file("proj/exports/m.mid").unwrap();

    let mut target = app();
    let filled = target.import_midi(&data, 2).expect("import");
    assert_eq!(filled, 1);
    assert_eq!(target.tracks[2].notes.len(), 1);
    assert_eq!(target.tracks[2].notes[0].pitch, 60);
}

#[test]
fn text_input_follows_toolbar_editing() {
    let mut state = app();
    assert!(!state.wants_text_input());
    state.start_toolbar_edit(ToolbarEdit::SongName, String::from("x"));
    assert!(state.wants_text_input());
    state.finish_toolbar_edit();
    assert!(!state.wants_text_input());
}

#[test]
fn configure_engine_applies_mute_and_volume() {
    let mut state = app();
    state.project.channels[5].muted = true;
    let mut engine = AudioEngine::new(48_000);
    state.configure_engine(&mut engine);
    assert!(!engine.channels[5].enabled);
    assert!(engine.channels[0].enabled);
}

/// Standard MIDI File export and import, kept in one module so that it
/// merges apart from the tests of other lines of work.
mod midi_round_trip {
    use super::*;

    /// A project exported to a Standard MIDI File and imported into a fresh
    /// project gives back every note on the channel it came from. The project
    /// covers what a composer can make. It has a chord, a pitch repeated with no
    /// gap, the extreme pitches and velocities, a start that needs a three-byte
    /// delta time, an empty channel between full ones, the last channel, and a
    /// resolution other than the default.
    #[test]
    fn export_then_import_gives_back_the_same_notes() {
        fn note(tick: u64, duration: u64, pitch: u8, velocity: u8) -> MidiNote {
            MidiNote {
                tick,
                duration,
                pitch,
                velocity,
            }
        }

        let mut source = app();
        source.project.name = String::from("round");
        source.project.ticks_per_beat = 960;
        source.tracks[0].notes = alloc::vec![
            note(0, 960, 60, 1),
            note(0, 960, 64, 64),
            note(0, 960, 67, 127),
            note(960, 480, 60, 100),
            note(1440, 480, 60, 90),
        ];
        source.tracks[2].notes = alloc::vec![note(0, 1, 0, 100), note(2_000_000, 1, 127, 100)];
        source.tracks[NUM_CHANNELS - 1].notes = alloc::vec![note(480, 7, 72, 50)];
        for track in &mut source.tracks {
            track.ticks_per_beat = 960;
        }
        source.export_midi();
        let data = source
            .platform()
            .read_file("proj/exports/round.mid")
            .expect("the export was written");

        let mut target = app();
        target.import_midi(&data, 0).expect("import");
        assert_eq!(target.project.ticks_per_beat, source.project.ticks_per_beat);
        for ch in 0..NUM_CHANNELS {
            assert_eq!(
                target.tracks[ch].notes, source.tracks[ch].notes,
                "channel {ch}"
            );
        }
    }

    fn one_note(pitch: u8) -> MidiNote {
        MidiNote {
            tick: 0,
            duration: 96,
            pitch,
            velocity: 90,
        }
    }

    /// A Type 0 file keeps the spacing of its MIDI channels from the first
    /// channel, and a channel pushed past the last one is dropped rather than
    /// wrapped or merged.
    #[test]
    fn import_type0_offsets_by_midi_channel_and_drops_overflow() {
        let mut source = app();
        source.project.name = String::from("shift");
        source.tracks[0].notes.push(one_note(60));
        source.tracks[2].notes.push(one_note(62));
        source.export_midi();
        let data = source
            .platform()
            .read_file("proj/exports/shift.mid")
            .unwrap();

        let first = NUM_CHANNELS - 2;
        let mut target = app();
        target.tracks[first + 1].notes.push(one_note(1));
        let filled = target.import_midi(&data, first).expect("import");
        assert_eq!(filled, 1);
        assert_eq!(target.tracks[first].notes, source.tracks[0].notes);
        // The channel between keeps what it had, because the file had nothing
        // for it, and MIDI channel 2 would land past the last channel.
        assert_eq!(target.tracks[first + 1].notes, alloc::vec![one_note(1)]);
        for ch in 0..first {
            assert!(target.tracks[ch].notes.is_empty(), "channel {ch}");
        }
    }

    /// A Type 1 file fills consecutive channels in chunk order, whatever MIDI
    /// channel each chunk uses, and its tempo chunk takes no channel.
    #[test]
    fn import_type1_packs_tracks_in_file_order() {
        fn chunk(status: u8, pitch: u8) -> Vec<u8> {
            alloc::vec![
                0x00,
                status,
                pitch,
                90,
                0x60,
                status & 0xEF,
                pitch,
                0,
                0x00,
                0xFF,
                0x2F,
                0x00
            ]
        }
        let tempo = alloc::vec![0x00, 0xFF, 0x2F, 0x00];
        let chunks = [tempo, chunk(0x95, 70), chunk(0x90, 50)];
        let mut data = Vec::new();
        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&(chunks.len() as u16).to_be_bytes());
        data.extend_from_slice(&480u16.to_be_bytes());
        for c in &chunks {
            data.extend_from_slice(b"MTrk");
            data.extend_from_slice(&(c.len() as u32).to_be_bytes());
            data.extend_from_slice(c);
        }

        let mut target = app();
        let filled = target.import_midi(&data, 1).expect("import");
        assert_eq!(filled, 2);
        assert_eq!(target.tracks[1].notes, alloc::vec![one_note(70)]);
        assert_eq!(target.tracks[2].notes, alloc::vec![one_note(50)]);
    }

    #[test]
    fn export_midi_refuses_a_note_it_cannot_write() {
        let mut state = app();
        state.project.name = String::from("bad");
        let mut silent = one_note(60);
        silent.velocity = 0;
        state.tracks[3].notes.push(silent);
        state.export_midi();
        assert_eq!(state.platform().file_count(), 0);
        let message = state.status_message().unwrap();
        assert_eq!(
            message,
            "Export failed: channel 4 note at tick 0: velocity 0 is outside 1 to 127"
        );
    }

    /// The longest gap a file can hold exports, and one tick more is refused
    /// with the reason in the status bar and no file written.
    #[test]
    fn export_midi_bounds_the_gap_between_events() {
        let longest = u64::from(crate::smf::MAX_VAR_LEN);
        let mut state = app();
        state.project.name = String::from("gap");
        let mut late = one_note(60);
        late.tick = longest;
        state.tracks[0].notes.push(late);
        state.export_midi();
        assert!(state.platform().read_file("proj/exports/gap.mid").is_ok());

        let mut state = app();
        state.project.name = String::from("gap");
        late.tick = longest + 1;
        state.tracks[0].notes.push(late);
        state.export_midi();
        assert_eq!(state.platform().file_count(), 0);
        assert_eq!(
            state.status_message().unwrap(),
            alloc::format!(
                "Export failed: the event at tick {} is more than {longest} ticks after the one before",
                longest + 1
            )
        );
    }

    /// A Type 0 file at `ticks_per_beat` with these notes on MIDI channel 1.
    fn file_at(ticks_per_beat: u16, notes: &[MidiNote]) -> Vec<u8> {
        let track = MidiTrack {
            notes: notes.to_vec(),
            ticks_per_beat,
        };
        crate::smf::build_smf(&[track], ticks_per_beat, 120.0, 4, 4).expect("build")
    }

    fn timed(tick: u64, duration: u64) -> MidiNote {
        MidiNote {
            tick,
            duration,
            ..one_note(60)
        }
    }

    /// With no note left at the old resolution, the project adopts the
    /// file's, and the notes arrive exactly. A channel the import replaces
    /// does not count as keeping notes.
    #[test]
    fn import_into_an_empty_project_adopts_the_file_resolution() {
        let notes = [timed(0, 1), timed(7, 3), timed(95, 97)];
        let data = file_at(96, &notes);
        for replaced_has_notes in [false, true] {
            let mut target = app();
            if replaced_has_notes {
                target.tracks[0].notes.push(one_note(70));
            }
            target.import_midi(&data, 0).expect("import");
            assert_eq!(target.project.ticks_per_beat, 96);
            assert_eq!(target.tracks[0].notes, notes);
            assert_eq!(target.tracks[0].ticks_per_beat, 96);
        }
    }

    /// With notes kept on another channel, the project keeps its resolution
    /// and the imported notes are converted to it. 96 to 480 is exact, and
    /// 960 to 480 rounds each start and end half up.
    #[test]
    fn import_into_a_project_with_notes_converts_to_its_resolution() {
        let kept = one_note(40);
        let cases = [
            (96, [timed(0, 1), timed(7, 3)], [timed(0, 5), timed(35, 15)]),
            (960, [timed(1, 2), timed(3, 1)], [timed(1, 1), timed(2, 1)]),
        ];
        for (file_tpb, notes, expected) in cases {
            let mut target = app();
            target.tracks[5].notes.push(kept);
            target
                .import_midi(&file_at(file_tpb, &notes), 0)
                .expect("import");
            assert_eq!(target.project.ticks_per_beat, 480, "file at {file_tpb}");
            assert_eq!(target.tracks[0].notes, expected, "file at {file_tpb}");
            assert_eq!(target.tracks[0].ticks_per_beat, 480);
            assert_eq!(target.tracks[5].notes, alloc::vec![kept]);
            // The project now exports, since every channel agrees.
            target.project.name = String::from("mixed");
            target.export_midi();
            assert!(
                target
                    .platform()
                    .read_file("proj/exports/mixed.mid")
                    .is_ok()
            );
        }
    }
}
