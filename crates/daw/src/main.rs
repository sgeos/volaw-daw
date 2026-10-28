//! Entry point of the volaw-daw composer.
//!
//! This binary only wires layers together. The application lives in the
//! `no_std` `daw_core::DawApp`, the window, input and file system come from
//! the `daw-sdl` platform layer, and audio playback comes from the shared
//! `volaw-audio` entry library, which the game client also uses.

use audio::AudioEngine;
use daw_core::DawApp;
use daw_core::app::{PROJECT_ROOT, WINDOW_HEIGHT, WINDOW_WIDTH};
use daw_core::platform::{LogLevel, Platform};
use daw_sdl::{SdlPlatform, SdlRenderer};
use volaw_audio::{AudioConfig, AudioSystem};

const WINDOW_TITLE: &str = "Vows of Love and War - Composer";

/// Font file path (same font as the game client).
const FONT_PATH: &str = "assets/fonts/NotoSans-Regular.ttf";

/// Fallback font when the bundled one is missing.
const SYSTEM_FONT_PATH: &str = "/System/Library/Fonts/Helvetica.ttc";

/// Frame delay for roughly 60 frames per second.
const FRAME_DELAY_MS: u32 = 16;

/// Parsed command line options.
#[derive(Debug, Default, PartialEq)]
struct Options {
    project_root: Option<String>,
    load_name: Option<String>,
    import_midi: Option<String>,
    import_channel: usize,
    /// (channel, script file) pairs, channel 0-based.
    scripts: Vec<(usize, String)>,
    /// Arguments that were not understood, reported as warnings.
    rejected: Vec<String>,
}

/// Parse arguments after the program name. A malformed argument is collected
/// in `rejected` rather than replaced with a default.
fn parse_args(args: &[String]) -> Options {
    let mut opts = Options::default();
    let mut i = 0;
    while i < args.len() {
        let rest = &args[i + 1..];
        let consumed = match (args[i].as_str(), rest) {
            ("--root", [dir, ..]) => {
                opts.project_root = Some(dir.clone());
                2
            }
            ("--project", [name, ..]) => {
                opts.load_name = Some(name.clone());
                2
            }
            ("--import-midi", [path, ..]) => {
                opts.import_midi = Some(path.clone());
                2
            }
            ("--import-channel", [n, ..]) => {
                match n.parse::<usize>() {
                    Ok(ch) => opts.import_channel = ch,
                    Err(_) => opts.rejected.push(format!("--import-channel {n}")),
                }
                2
            }
            ("--script", [ch, file, ..]) => {
                match ch.parse::<usize>() {
                    Ok(ch) => opts.scripts.push((ch, file.clone())),
                    Err(_) => opts.rejected.push(format!("--script {ch} {file}")),
                }
                3
            }
            (other, _) => {
                opts.rejected.push(other.to_string());
                1
            }
        };
        i += consumed;
    }
    opts
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = parse_args(&args);
    let platform = SdlPlatform::new();
    for arg in &opts.rejected {
        platform.log(
            LogLevel::Warn,
            &format!("Unknown or malformed argument: {arg}"),
        );
    }

    let font_path = if platform.path_exists(FONT_PATH) {
        FONT_PATH
    } else {
        platform.log(
            LogLevel::Warn,
            &format!("Font not found at {FONT_PATH}, trying system font"),
        );
        SYSTEM_FONT_PATH
    };
    let mut renderer = match SdlRenderer::new(WINDOW_TITLE, WINDOW_WIDTH, WINDOW_HEIGHT, font_path)
    {
        Ok(r) => r,
        Err(e) => {
            platform.log(LogLevel::Error, &format!("Failed to create renderer: {e}"));
            std::process::exit(1);
        }
    };

    let config = AudioConfig::default();
    // The sink starts empty, so commands are discarded until audio starts.
    let mut app: DawApp<SdlPlatform, Option<AudioSystem>> =
        DawApp::new(platform, None, config.sample_rate);
    app.set_project_root(opts.project_root.as_deref().unwrap_or(PROJECT_ROOT));

    if let Some(name) = &opts.load_name
        && let Err(e) = app.load_project(name)
    {
        app.platform().log(
            LogLevel::Error,
            &format!("Failed to load project {name}: {e}"),
        );
    }
    if let Some(path) = &opts.import_midi {
        let imported = app
            .platform()
            .read_file(path)
            .and_then(|data| app.import_midi(&data, opts.import_channel));
        if let Err(e) = imported {
            app.platform()
                .log(LogLevel::Error, &format!("Could not import {path}: {e}"));
        }
    }
    for (ch, file) in &opts.scripts {
        if let Err(e) = app.assign_script(*ch, file) {
            app.platform()
                .log(LogLevel::Warn, &format!("--script {ch} {file}: {e}"));
        }
    }
    app.load_scripts();

    // Configure the engine on this thread, then hand it to the audio thread.
    let mut engine = AudioEngine::new(config.sample_rate);
    app.configure_engine(&mut engine);
    match AudioSystem::start(engine, &config) {
        Ok(system) => *app.sink_mut() = Some(system),
        Err(failure) => app.platform().log(
            LogLevel::Warn,
            &format!("Audio initialization failed: {}", failure.message),
        ),
    }

    app.platform().log(
        LogLevel::Info,
        &format!(
            "DAW started: {WINDOW_WIDTH}x{WINDOW_HEIGHT} @ {}Hz",
            config.sample_rate
        ),
    );
    app.platform().log(
        LogLevel::Info,
        "Controls: Space=Play/Pause, S=Stop, 1-8=Ch, P=Preset, Shift+P=Prev, M=Mute, Ctrl+S=Save, Q=Quit",
    );

    let mut text_input = false;
    // Safety: SDL_Event is a plain C union, for which all zero bytes is a
    // valid value, and SDL_PollEvent overwrites it before it is read.
    let mut event = unsafe { std::mem::zeroed::<sdl3_sys::events::SDL_Event>() };
    while app.is_running() {
        // Safety: `event` is a valid, writable SDL_Event.
        while unsafe { sdl3_sys::events::SDL_PollEvent(&mut event) } {
            // Safety: SDL_PollEvent returned true, so SDL filled `event`.
            if let Some(input) = unsafe { daw_sdl::input::translate_event(&event) } {
                app.handle_input(input);
            }
        }
        if app.wants_text_input() != text_input {
            text_input = app.wants_text_input();
            renderer.set_text_input(text_input);
        }
        app.update();
        app.render(&mut renderer);
        // Safety: plain SDL call with no pointer arguments.
        unsafe { sdl3_sys::timer::SDL_Delay(FRAME_DELAY_MS) };
    }

    // Stop audio before the renderer tears down SDL.
    drop(app.sink_mut().take());
    app.platform().log(LogLevel::Info, "DAW shutdown");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_every_option() {
        let opts = parse_args(&args(&[
            "--root",
            "r",
            "--project",
            "p",
            "--import-midi",
            "m.mid",
            "--import-channel",
            "3",
            "--script",
            "2",
            "s.kma",
        ]));
        assert_eq!(
            opts,
            Options {
                project_root: Some("r".into()),
                load_name: Some("p".into()),
                import_midi: Some("m.mid".into()),
                import_channel: 3,
                scripts: vec![(2, "s.kma".into())],
                rejected: vec![],
            }
        );
    }

    #[test]
    fn malformed_numbers_are_rejected_not_defaulted() {
        let opts = parse_args(&args(&["--import-channel", "x", "--script", "y", "s.kma"]));
        assert_eq!(opts.import_channel, 0);
        assert!(opts.scripts.is_empty());
        assert_eq!(opts.rejected, ["--import-channel x", "--script y s.kma"]);
    }

    #[test]
    fn unknown_and_truncated_arguments_are_rejected() {
        let opts = parse_args(&args(&["--bogus", "--root"]));
        assert_eq!(opts.rejected, ["--bogus", "--root"]);
        assert_eq!(opts.project_root, None);
    }

    #[test]
    fn readme_channel_script_example_runs() {
        // The example is read from the README, so the documentation cannot
        // drift from what the composer accepts.
        let readme = include_str!("../README.md");
        let section = readme
            .split("### Channel Scripts")
            .nth(1)
            .expect("README has no Channel Scripts section");
        let example = section
            .split("```")
            .nth(1)
            .expect("Channel Scripts section has no code block");
        let mut runner =
            daw_core::script::ScriptRunner::from_source(example).expect("README example rejected");
        let cmds = runner.tick(0, 120.0, 0.5);
        assert_eq!(runner.last_error, None);
        assert_eq!(
            cmds,
            vec![daw_core::script::AudioCommand::SetLpfCutoff(
                200.0 + 0.5 * 1000.0
            )]
        );
    }
}
