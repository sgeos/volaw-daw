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
