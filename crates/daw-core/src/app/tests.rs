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

/// One thing drawn, as a box. Text is recorded at its origin with no size,
/// because its true size depends on the platform's font.
#[derive(Debug, Clone, PartialEq)]
enum Drawn {
    Shape(Rect),
    Text(String, f32, f32),
}

/// A renderer that records what is drawn instead of drawing it.
#[derive(Default)]
struct Recorder {
    drawn: Vec<Drawn>,
}

impl Recorder {
    fn text_x(&self, wanted: &str) -> Option<f32> {
        self.drawn.iter().find_map(|d| match d {
            Drawn::Text(t, x, _) if t == wanted => Some(*x),
            _ => None,
        })
    }
}

impl Renderer for Recorder {
    fn clear(&mut self, _: Color) {}
    fn present(&mut self) {}
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, _: Color) {
        self.drawn.push(Drawn::Shape(Rect::new(x, y, w, h)));
    }
    fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, _: Color) {
        self.drawn.push(Drawn::Shape(Rect::new(x, y, w, h)));
    }
    fn draw_hline(&mut self, x1: f32, x2: f32, y: f32, _: Color) {
        self.drawn
            .push(Drawn::Shape(Rect::new(x1.min(x2), y, (x2 - x1).abs(), 0.0)));
    }
    fn draw_vline(&mut self, x: f32, y1: f32, y2: f32, _: Color) {
        self.drawn
            .push(Drawn::Shape(Rect::new(x, y1.min(y2), 0.0, (y2 - y1).abs())));
    }
    fn draw_text(&mut self, text: &str, x: f32, y: f32, _: FontSize, _: Color) {
        self.drawn.push(Drawn::Text(String::from(text), x, y));
    }
    /// A rough average glyph width, so that layout code that centres text
    /// gets plausible figures. No assertion depends on it.
    fn text_size(&mut self, text: &str, _: FontSize) -> (f32, f32) {
        (text.chars().count() as f32 * 7.0, 16.0)
    }
    /// Deliberately wrong, so that a layout that read the renderer's size
    /// instead of the app's would fail the tests below.
    fn screen_width(&self) -> f32 {
        1.0
    }
    fn screen_height(&self) -> f32 {
        1.0
    }
}

/// Sizes at both bounds, the default, and one that matches none of them.
fn test_sizes() -> [WindowSize; 4] {
    [
        WindowSize::MIN,
        WindowSize::DEFAULT,
        WindowSize::new(1601, 1003).unwrap(),
        WindowSize::MAX,
    ]
}

fn sized_app(size: WindowSize) -> DawApp<MemoryPlatform, NullSink> {
    let mut state = app();
    state.set_window_size(size);
    state
}

fn click_at(state: &mut DawApp<MemoryPlatform, NullSink>, x: f32, y: f32) {
    state.handle_input(DawInput::MouseMove(x, y));
    state.handle_input(DawInput::Click(x, y));
    state.handle_input(DawInput::MouseUp(x, y));
}

#[test]
fn every_page_draws_inside_the_window() {
    for size in test_sizes() {
        let mut state = sized_app(size);
        for name in ["Overdrive", "Chorus", "Delay"] {
            for chain in 0..3 {
                state.add_pedal_to_chain(chain, name);
            }
        }
        state.status_message = None;
        let (w, h) = (size.width_f32(), size.height_f32());
        let pages = DawPage::ALL.iter().map(|p| (*p, false));
        for (page, picker) in pages.chain([(DawPage::Studio, true)]) {
            state.current_page = page;
            state.pedal_picker_open = picker;
            let mut rec = Recorder::default();
            state.render(&mut rec);
            assert!(rec.drawn.len() > 50, "{page:?} drew almost nothing");
            for d in &rec.drawn {
                let inside = match d {
                    Drawn::Shape(r) => r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= w && r.y + r.h <= h,
                    Drawn::Text(_, x, y) => (0.0..w).contains(x) && (0.0..h).contains(y),
                };
                assert!(inside, "{size:?} {page:?} picker={picker}: {d:?}");
            }
        }
    }
}

#[test]
fn the_frame_spans_the_window() {
    for size in test_sizes() {
        let state = sized_app(size);
        let mut rec = Recorder::default();
        state.render(&mut rec);
        // The toolbar background is drawn first, across the whole width.
        assert_eq!(
            rec.drawn[0],
            Drawn::Shape(Rect::new(0.0, 0.0, size.width_f32(), TOOLBAR_HEIGHT)),
            "{size:?}"
        );
    }
}

#[test]
fn channel_tabs_divide_the_width() {
    for size in test_sizes() {
        let mut state = sized_app(size);
        let tab_w = size.width_f32() / NUM_CHANNELS as f32;
        let y = CHANNEL_BAR_Y + CHANNEL_BAR_HEIGHT / 2.0;
        for ch in (0..NUM_CHANNELS).rev() {
            click_at(&mut state, (ch as f32 + 0.5) * tab_w, y);
            assert_eq!(state.selected_channel, ch, "{size:?}");
        }
    }
}

#[test]
fn overview_lanes_divide_the_height() {
    for size in test_sizes() {
        let mut state = sized_app(size);
        state.current_page = DawPage::Overview;
        let lane_h = (size.height_f32() - CONTENT_Y) / NUM_CHANNELS as f32;
        for ch in (0..NUM_CHANNELS).rev() {
            click_at(&mut state, 400.0, CONTENT_Y + (ch as f32 + 0.5) * lane_h);
            assert_eq!(state.selected_channel, ch, "{size:?}");
        }
    }
}

#[test]
fn setup_navigation_follows_the_right_edge() {
    for size in test_sizes() {
        let mut state = sized_app(size);
        state.current_page = DawPage::SynthSetup;
        let y = CONTENT_Y + NAV_BAR_HEIGHT / 2.0;
        // Just inside the right-hand target, which a fixed 1440 layout
        // would miss in a wider window and misplace in a narrower one.
        click_at(&mut state, size.width_f32() - 100.0, y);
        assert_eq!(state.current_page, DawPage::MonoChain, "{size:?}");
        click_at(&mut state, size.width_f32() / 2.0, y);
        assert_eq!(state.current_page, DawPage::MonoChain, "{size:?}");
    }
}

#[test]
fn pedal_picker_is_centred() {
    for size in test_sizes() {
        let p = picker_rect(size);
        assert_eq!(p.x + p.w / 2.0, size.width_f32() / 2.0, "{size:?}");
        assert_eq!(p.y + p.h / 2.0, size.height_f32() / 2.0, "{size:?}");
    }
}

#[test]
fn piano_roll_fills_the_height() {
    for size in test_sizes() {
        let state = sized_app(size);
        assert_eq!(
            DawApp::<MemoryPlatform, NullSink>::roll_y() + state.roll_h() + VELOCITY_LANE_HEIGHT,
            size.height_f32(),
            "{size:?}"
        );
    }
}

#[test]
fn a_new_app_uses_the_default_size() {
    assert_eq!(app().window_size(), WindowSize::DEFAULT);
    let size = WindowSize::new(1600, 1000).unwrap();
    assert_eq!(sized_app(size).window_size(), size);
}

#[test]
fn toolbar_status_never_starts_over_the_indicators() {
    for size in test_sizes() {
        let mut state = sized_app(size);
        state.metronome_enabled = true;
        state.transport.loop_enabled = true;
        state.status_message = Some(String::from("status probe"));
        let mut rec = Recorder::default();
        state.render(&mut rec);
        let met_x = rec.text_x("MET").expect("no MET indicator");
        let met_w = rec.text_size("MET", FontSize::Medium).0;
        let status_x = rec.text_x("status probe").expect("no status text");
        assert!(status_x >= met_x + met_w, "{size:?}: {status_x}");
        let anchored = size.width_f32() - TOOLBAR_HINT_W;
        if anchored >= TOOLBAR_HINT_MIN_X {
            assert_eq!(status_x, anchored, "{size:?}");
        }
    }
}

/// Note audition timing, kept in its own module so that other test
/// modules can be added at the end of this file without conflict.
mod audition {
    use super::*;

    /// A note event as the engine receives it. `EngineCommand` has no
    /// `PartialEq`, so the sink keeps only the note events, in this form.
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum NoteEvent {
        On { channel: u8, freq: f64 },
        OffFreq { channel: u8, freq: f64 },
        Off { channel: u8 },
    }

    /// A sink that records every note event and discards other commands.
    #[derive(Default)]
    struct NoteLog {
        events: Vec<NoteEvent>,
    }

    impl CommandSink for NoteLog {
        fn send(&mut self, cmd: EngineCommand) -> Result<(), EngineCommand> {
            let event = match cmd {
                EngineCommand::NoteOn { channel, freq, .. } => NoteEvent::On { channel, freq },
                EngineCommand::NoteOffFreq { channel, freq } => {
                    NoteEvent::OffFreq { channel, freq }
                }
                EngineCommand::NoteOff { channel } => NoteEvent::Off { channel },
                _ => return Ok(()),
            };
            self.events.push(event);
            Ok(())
        }
    }

    /// A Piano Roll app that records note events, and the y of the piano key
    /// `keys_down` rows below the top of the visible roll with its frequency.
    fn audition_app(keys_down: f32) -> (DawApp<MemoryPlatform, NoteLog>, f32, f64) {
        let mut state = DawApp::new(MemoryPlatform::new(), NoteLog::default(), 48_000);
        state.current_page = DawPage::PianoRoll;
        let roll_y = DawApp::<MemoryPlatform, NoteLog>::roll_y();
        let y = roll_y + (keys_down + 0.5) * NOTE_HEIGHT;
        let freq = MidiTrack::note_to_freq(state.y_to_pitch(y, roll_y));
        (state, y, freq)
    }

    fn click_key(state: &mut DawApp<MemoryPlatform, NoteLog>, y: f32) {
        let x = PIANO_KEY_WIDTH / 2.0;
        state.handle_input(DawInput::MouseMove(x, y));
        state.handle_input(DawInput::Click(x, y));
        state.handle_input(DawInput::MouseUp(x, y));
    }

    #[test]
    fn piano_key_audition_lasts_preview_note_ms() {
        let (mut state, y, freq) = audition_app(10.0);
        let on = NoteEvent::On { channel: 0, freq };
        let off = NoteEvent::OffFreq { channel: 0, freq };

        click_key(&mut state, y);
        assert_eq!(state.sink_mut().events, [on]);

        // Still sounding one millisecond before the end, through the frame
        // loop's own entry point.
        state.platform().advance_ms(PREVIEW_NOTE_MS - 1);
        state.update();
        assert_eq!(state.sink_mut().events, [on]);

        state.platform().advance_ms(1);
        state.update();
        assert_eq!(state.sink_mut().events, [on, off]);

        // The note is ended once, not on every later frame.
        state.platform().advance_ms(PREVIEW_NOTE_MS * 3);
        state.update();
        assert_eq!(state.sink_mut().events, [on, off]);
    }

    #[test]
    fn a_second_audition_ends_the_first_and_restarts_the_duration() {
        let (mut state, y1, f1) = audition_app(10.0);
        let (_, y2, f2) = audition_app(20.0);
        assert_ne!(f1, f2);

        click_key(&mut state, y1);
        let half = PREVIEW_NOTE_MS / 2;
        state.platform().advance_ms(half);
        state.update();
        click_key(&mut state, y2);
        let first = [
            NoteEvent::On {
                channel: 0,
                freq: f1,
            },
            NoteEvent::OffFreq {
                channel: 0,
                freq: f1,
            },
            NoteEvent::On {
                channel: 0,
                freq: f2,
            },
        ];
        assert_eq!(state.sink_mut().events, first);

        // The first note's deadline passes without ending the second.
        state.platform().advance_ms(PREVIEW_NOTE_MS - half);
        state.update();
        assert_eq!(state.sink_mut().events, first);

        // The second note lasts its own full duration.
        state.platform().advance_ms(half);
        state.update();
        let last = state.sink_mut().events.last().copied();
        assert_eq!(
            last,
            Some(NoteEvent::OffFreq {
                channel: 0,
                freq: f2
            })
        );
        assert_eq!(state.sink_mut().events.len(), first.len() + 1);
    }
}

/// Layout checks for the Synth Setup and Stereo Chain pages. Each test reads
/// positions from what was drawn, so it checks the drawing and the hit
/// testing against each other rather than against pinned coordinates.
mod layout {
    use super::*;

    /// The section panels drawn, each found as the border that
    /// `draw_section_panel` draws just before its title.
    fn section_panels(rec: &Recorder) -> Vec<(String, Rect)> {
        rec.drawn
            .windows(2)
            .filter_map(|pair| match pair {
                [Drawn::Shape(r), Drawn::Text(title, x, y)]
                    if *x == r.x + 8.0 && *y == r.y + 5.0 =>
                {
                    Some((title.clone(), *r))
                }
                _ => None,
            })
            .collect()
    }

    fn overlaps(a: &Rect, b: &Rect) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }

    fn assert_disjoint(boxes: &[(String, Rect)], context: &str) {
        for (i, (name_a, a)) in boxes.iter().enumerate() {
            for (name_b, b) in &boxes[i + 1..] {
                assert!(
                    !overlaps(a, b),
                    "{context}: {name_a} {a:?} overlaps {name_b} {b:?}"
                );
            }
        }
    }

    /// Every origin at which `wanted` was drawn, in drawing order.
    fn text_positions(rec: &Recorder, wanted: &str) -> Vec<(f32, f32)> {
        rec.drawn
            .iter()
            .filter_map(|d| match d {
                Drawn::Text(t, x, y) if t == wanted => Some((*x, *y)),
                _ => None,
            })
            .collect()
    }

    fn rendered(state: &DawApp<MemoryPlatform, NullSink>) -> Recorder {
        let mut rec = Recorder::default();
        state.render(&mut rec);
        rec
    }

    #[test]
    fn no_two_synth_setup_panels_overlap() {
        for size in test_sizes() {
            let mut state = sized_app(size);
            state.current_page = DawPage::SynthSetup;
            let panels = section_panels(&rendered(&state));
            let titles: Vec<&str> = panels.iter().map(|(t, _)| t.as_str()).collect();
            for wanted in ["LPF", "GLIDE", "VELOCITY", "MOD MATRIX"] {
                assert!(titles.contains(&wanted), "{size:?}: {titles:?}");
            }
            assert_disjoint(&panels, &format!("{size:?}"));
        }
    }

    #[test]
    fn clicking_glide_enabled_toggles_glide_and_nothing_else() {
        for size in test_sizes() {
            let mut state = sized_app(size);
            state.current_page = DawPage::SynthSetup;
            let rec = rendered(&state);
            let glide = section_panels(&rec)
                .into_iter()
                .find(|(t, _)| t == "GLIDE")
                .map(|(_, r)| r)
                .expect("no GLIDE panel");
            let (x, y) = text_positions(&rec, "Enabled")
                .into_iter()
                .find(|&(x, y)| glide.contains(x, y))
                .expect("no Enabled row in the GLIDE panel");
            let before = state.patches[0].clone();

            click_at(&mut state, x + 2.0, y + PARAM_ROW_H / 2.0);

            let after = &state.patches[0];
            assert_eq!(after.glide_enabled, !before.glide_enabled, "{size:?}");
            assert_eq!(after.lpf_saturation, before.lpf_saturation, "{size:?}");
            assert_eq!(after.lpf_resonance, before.lpf_resonance, "{size:?}");
        }
    }

    /// A Stereo Chain app with `n` Overdrive pedals on the left chain and `n`
    /// Chorus pedals on the right, so that each pedal's name says its chain.
    fn stereo_app(size: WindowSize, n: usize) -> DawApp<MemoryPlatform, NullSink> {
        let mut state = sized_app(size);
        for _ in 0..n {
            state.add_pedal_to_chain(1, "Overdrive");
            state.add_pedal_to_chain(2, "Chorus");
        }
        state.status_message = None;
        state.current_page = DawPage::StereoChain;
        state
    }

    /// The box of each pedal drawn with display name `name`, found from the
    /// name that `draw_pedal_box` draws 6 pixels in and 4 down.
    fn pedal_boxes(rec: &Recorder, name: &str, pedals: &[PedalConfig]) -> Vec<(String, Rect)> {
        text_positions(rec, name)
            .into_iter()
            .zip(pedals)
            .map(|((x, y), p)| {
                (
                    String::from(name),
                    Rect::new(x - 6.0, y - 4.0, PEDAL_W, pedal_h(p)),
                )
            })
            .collect()
    }

    #[test]
    fn each_stereo_chain_stays_in_its_own_column() {
        for size in test_sizes() {
            let n = 5;
            let state = stereo_app(size, n);
            let rec = rendered(&state);
            let cols = state.stereo_chain_columns(CONTENT_Y);
            let fx = &state.effects[0];
            let left_name = pedal_display_name(&fx.post_pedals_left[0].pedal_type);
            let right_name = pedal_display_name(&fx.post_pedals_right[0].pedal_type);
            let left = pedal_boxes(&rec, &left_name, &fx.post_pedals_left);
            let right = pedal_boxes(&rec, &right_name, &fx.post_pedals_right);
            assert_eq!((left.len(), right.len()), (n, n), "{size:?}");

            let cards: Vec<(String, Rect)> = text_positions(&rec, "Add effect")
                .into_iter()
                .map(|(x, y)| {
                    let card = Rect::new(x - 22.0, y - 40.0, ADD_CARD_W, ADD_CARD_H);
                    (String::from("add card"), card)
                })
                .collect();
            assert_eq!(cards.len(), 2, "{size:?}");

            let columns = [
                (cols.left_x, &left, &cards[0]),
                (cols.right_x, &right, &cards[1]),
            ];
            for (x0, boxes, card) in columns {
                let x1 = x0 + cols.width;
                for (name, r) in boxes.iter().chain([card]) {
                    assert!(
                        r.x >= x0 && r.x + r.w <= x1 && r.y >= cols.pedals_y,
                        "{size:?}: {name} {r:?} is outside {x0}..{x1}"
                    );
                }
            }
            let all: Vec<(String, Rect)> = left.into_iter().chain(right).chain(cards).collect();
            assert_disjoint(&all, &format!("{size:?}"));
        }
    }

    #[test]
    fn a_wrapped_pedal_is_hit_where_it_is_drawn() {
        let size = WindowSize::MIN;
        let mut state = stereo_app(size, 5);
        // Orbital mode makes the positional stage tallest, which parameter
        // lookup once ignored.
        state.effects[0].orbital_enabled = true;
        let rec = rendered(&state);
        let fx = &state.effects[0];
        let name = pedal_display_name(&fx.post_pedals_left[0].pedal_type);
        let boxes = pedal_boxes(&rec, &name, &fx.post_pedals_left);
        let last = boxes.len() - 1;
        let (_, first_box) = boxes[0];
        let (_, wrapped) = boxes[last];
        assert!(wrapped.y > first_box.y, "the chain did not wrap");

        // A parameter bar of the wrapped pedal, then a click on its box.
        let bar_y = wrapped.y + PEDAL_HEADER_H + PARAM_ROW_H / 2.0;
        let mid_x = wrapped.x + PEDAL_W / 2.0;
        let found = state.find_pedal_param_at(mid_x, bar_y, CONTENT_Y);
        assert_eq!(found.map(|f| (f.0, f.1, f.2)), Some((1, last, 0)));

        click_at(&mut state, wrapped.x + 2.0, wrapped.y + 2.0);
        assert_eq!(state.selected_pedal, Some((1, last)));
    }
}
