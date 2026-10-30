//! Saving and loading projects as TOML through the `Platform` trait.
//!
//! A project root holds four folders. `saves/{name}.toml` is the project
//! wrapper, and `patches/`, `effects/` and `midi/` hold the per-channel files
//! the wrapper names. A referenced file that does not exist loads as the
//! default, so a project can name files it has not saved yet.
//!
//! ## Trust boundary
//!
//! Project files are user-editable and may come from elsewhere, so every
//! file name they reference is checked by `validate_file_name` before it is
//! joined to the root. An absolute name, a drive prefix or a `..` component
//! is rejected, which keeps reads and writes inside the project root.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::engine_setup::join_path;
use crate::midi::MidiTrack;
use crate::patch::{EffectChainConfig, PatchConfig};
use crate::platform::Platform;
use crate::project::{DawProject, NUM_CHANNELS};

/// A project wrapper and the per-channel data it references.
pub type LoadedProject = (
    DawProject,
    [MidiTrack; NUM_CHANNELS],
    [PatchConfig; NUM_CHANNELS],
    [EffectChainConfig; NUM_CHANNELS],
);

/// Fixed subfolder names under the project root.
pub const SAVES_DIR: &str = "saves";
pub const PATCHES_DIR: &str = "patches";
pub const EFFECTS_DIR: &str = "effects";
pub const MIDI_DIR: &str = "midi";

/// Reject a referenced file name that could escape the project root.
pub fn validate_file_name(name: &str) -> Result<(), String> {
    let escapes = name.is_empty()
        || name.starts_with('/')
        || name.starts_with('\\')
        || name.as_bytes().get(1) == Some(&b':')
        || name.split(['/', '\\']).any(|part| part == "..");
    if escapes {
        return Err(format!(
            "file name {name:?} must be a relative path inside the project"
        ));
    }
    Ok(())
}

fn encode<T: Serialize>(value: &T, what: &str) -> Result<String, String> {
    toml::to_string_pretty(value).map_err(|e| format!("serialize {what}: {e}"))
}

fn read_toml<P: Platform + ?Sized, T: DeserializeOwned>(
    platform: &P,
    path: &str,
    what: &str,
) -> Result<T, String> {
    let bytes = platform.read_file(path)?;
    let text = String::from_utf8(bytes).map_err(|e| format!("{path} is not UTF-8: {e}"))?;
    toml::from_str(&text).map_err(|e| format!("parse {what}: {e}"))
}

/// Save a project and every file it references. Returns the path of the
/// project wrapper.
pub fn save_project<P: Platform + ?Sized>(
    platform: &P,
    root: &str,
    filename: &str,
    project: &DawProject,
    tracks: &[MidiTrack; NUM_CHANNELS],
    patches: &[PatchConfig; NUM_CHANNELS],
    effects: &[EffectChainConfig; NUM_CHANNELS],
) -> Result<String, String> {
    validate_file_name(filename)?;
    let patches_dir = join_path(root, PATCHES_DIR);
    let effects_dir = join_path(root, EFFECTS_DIR);
    let midi_dir = join_path(root, MIDI_DIR);

    for (i, ch) in project.channels.iter().enumerate() {
        if !ch.patch_file.is_empty() {
            validate_file_name(&ch.patch_file)?;
            let text = encode(&patches[i], "patch")?;
            platform.write_file(&join_path(&patches_dir, &ch.patch_file), text.as_bytes())?;
        }
        if !ch.effect_chain_file.is_empty() {
            validate_file_name(&ch.effect_chain_file)?;
            let text = encode(&effects[i], "effects")?;
            platform.write_file(
                &join_path(&effects_dir, &ch.effect_chain_file),
                text.as_bytes(),
            )?;
        }
        if !ch.midi_file.is_empty() {
            validate_file_name(&ch.midi_file)?;
            let text = encode(&tracks[i], "midi")?;
            platform.write_file(&join_path(&midi_dir, &ch.midi_file), text.as_bytes())?;
        }
    }

    let project_path = join_path(&join_path(root, SAVES_DIR), &format!("{filename}.toml"));
    let text = encode(project, "project")?;
    platform.write_file(&project_path, text.as_bytes())?;
    Ok(project_path)
}

/// Load a project wrapper and every file it references.
pub fn load_project<P: Platform + ?Sized>(
    platform: &P,
    root: &str,
    filename: &str,
) -> Result<LoadedProject, String> {
    validate_file_name(filename)?;
    let project_path = join_path(&join_path(root, SAVES_DIR), &format!("{filename}.toml"));
    let project: DawProject = read_toml(platform, &project_path, "project")?;

    let patches_dir = join_path(root, PATCHES_DIR);
    let effects_dir = join_path(root, EFFECTS_DIR);
    let midi_dir = join_path(root, MIDI_DIR);

    let mut tracks: [MidiTrack; NUM_CHANNELS] = core::array::from_fn(|_| MidiTrack::new());
    let mut patches: [PatchConfig; NUM_CHANNELS] = core::array::from_fn(|_| PatchConfig::default());
    let mut effects: [EffectChainConfig; NUM_CHANNELS] =
        core::array::from_fn(|_| EffectChainConfig::default());

    for (i, ch) in project.channels.iter().enumerate() {
        if !ch.patch_file.is_empty() {
            validate_file_name(&ch.patch_file)?;
            let path = join_path(&patches_dir, &ch.patch_file);
            if platform.path_exists(&path) {
                patches[i] = read_toml(platform, &path, &format!("patch {}", ch.patch_file))?;
            }
        }
        if !ch.effect_chain_file.is_empty() {
            validate_file_name(&ch.effect_chain_file)?;
            let path = join_path(&effects_dir, &ch.effect_chain_file);
            if platform.path_exists(&path) {
                effects[i] = read_toml(
                    platform,
                    &path,
                    &format!("effects {}", ch.effect_chain_file),
                )?;
            }
        }
        if !ch.midi_file.is_empty() {
            validate_file_name(&ch.midi_file)?;
            let path = join_path(&midi_dir, &ch.midi_file);
            if platform.path_exists(&path) {
                tracks[i] = read_toml(platform, &path, &format!("midi {}", ch.midi_file))?;
            }
        }
    }

    Ok((project, tracks, patches, effects))
}

/// List saved project names, sorted. A missing saves folder lists nothing.
pub fn list_projects<P: Platform + ?Sized>(
    platform: &P,
    root: &str,
) -> Result<Vec<String>, String> {
    let saves_dir = join_path(root, SAVES_DIR);
    if !platform.path_exists(&saves_dir) {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = platform
        .list_files(&saves_dir, "toml")?
        .iter()
        .filter_map(|path| {
            let file = path.rsplit(['/', '\\']).next()?;
            file.strip_suffix(".toml").map(ToString::to_string)
        })
        .collect();
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::MidiNote;
    use crate::platform::MemoryPlatform;
    use alloc::vec;

    fn empty_data() -> (
        [MidiTrack; NUM_CHANNELS],
        [PatchConfig; NUM_CHANNELS],
        [EffectChainConfig; NUM_CHANNELS],
    ) {
        (
            core::array::from_fn(|_| MidiTrack::new()),
            core::array::from_fn(|_| PatchConfig::default()),
            core::array::from_fn(|_| EffectChainConfig::default()),
        )
    }

    #[test]
    fn save_and_load_roundtrip() {
        let platform = MemoryPlatform::new();
        let mut project = DawProject {
            name: "Test Project".to_string(),
            ..DawProject::default()
        };
        project.channels[0].patch_file = "ch1_patch.toml".to_string();
        project.channels[0].midi_file = "ch1.toml".to_string();
        let (mut tracks, mut patches, effects) = empty_data();
        tracks[0].notes.push(MidiNote {
            tick: 0,
            duration: 480,
            pitch: 60,
            velocity: 100,
        });
        patches[0].name = "Saved".to_string();

        let path = save_project(
            &platform, "root", "test", &project, &tracks, &patches, &effects,
        )
        .expect("save");
        assert_eq!(path, "root/saves/test.toml");
        assert!(platform.path_exists("root/patches/ch1_patch.toml"));
        assert!(platform.path_exists("root/midi/ch1.toml"));

        let (loaded, loaded_tracks, loaded_patches, _) =
            load_project(&platform, "root", "test").expect("load");
        assert_eq!(loaded.name, "Test Project");
        assert_eq!(loaded_tracks[0].notes, tracks[0].notes);
        assert_eq!(loaded_patches[0].name, "Saved");
        assert_eq!(loaded_patches[1].name, PatchConfig::default().name);
    }

    #[test]
    fn missing_referenced_file_loads_default() {
        let platform = MemoryPlatform::new();
        let mut project = DawProject::default();
        project.channels[2].patch_file = "absent.toml".to_string();
        let text = toml::to_string_pretty(&project).unwrap();
        platform
            .write_file("r/saves/p.toml", text.as_bytes())
            .unwrap();
        let (_, _, patches, _) = load_project(&platform, "r", "p").expect("load");
        assert_eq!(patches[2].name, PatchConfig::default().name);
    }

    #[test]
    fn missing_project_is_an_error() {
        let platform = MemoryPlatform::new();
        assert!(load_project(&platform, "r", "nothing").is_err());
    }

    #[test]
    fn malformed_toml_is_an_error() {
        let platform = MemoryPlatform::new();
        platform
            .write_file("r/saves/bad.toml", b"name = [")
            .unwrap();
        let err = load_project(&platform, "r", "bad").unwrap_err();
        assert!(err.contains("parse project"), "{err}");
    }

    #[test]
    fn non_utf8_project_is_an_error() {
        let platform = MemoryPlatform::new();
        platform
            .write_file("r/saves/bin.toml", &[0xff, 0xfe])
            .unwrap();
        let err = load_project(&platform, "r", "bin").unwrap_err();
        assert!(err.contains("UTF-8"), "{err}");
    }

    #[test]
    fn escaping_file_names_are_rejected() {
        for bad in [
            "",
            "/etc/passwd",
            "\\\\server\\x",
            "C:\\x",
            "../x",
            "a/../../x",
            "a\\..\\x",
        ] {
            assert!(
                validate_file_name(bad).is_err(),
                "{bad:?} should be rejected"
            );
        }
        for good in ["ch1.toml", "sub/ch1.toml", "a..b.toml"] {
            assert!(
                validate_file_name(good).is_ok(),
                "{good:?} should be accepted"
            );
        }
    }

    #[test]
    fn load_rejects_a_traversing_reference() {
        let platform = MemoryPlatform::new();
        let mut project = DawProject::default();
        project.channels[0].midi_file = "../../outside.toml".to_string();
        let text = toml::to_string_pretty(&project).unwrap();
        platform
            .write_file("r/saves/evil.toml", text.as_bytes())
            .unwrap();
        platform.write_file("outside.toml", b"").unwrap();
        assert!(load_project(&platform, "r", "evil").is_err());
    }

    #[test]
    fn save_rejects_a_traversing_project_name() {
        let platform = MemoryPlatform::new();
        let (tracks, patches, effects) = empty_data();
        let project = DawProject::default();
        let result = save_project(
            &platform, "r", "../x", &project, &tracks, &patches, &effects,
        );
        assert!(result.is_err());
        assert!(platform.file_count() == 0, "nothing may be written");
    }

    #[test]
    fn list_projects_empty_and_sorted() {
        let platform = MemoryPlatform::new();
        assert!(list_projects(&platform, "r").unwrap().is_empty());
        let (tracks, patches, effects) = empty_data();
        let project = DawProject::default();
        for name in ["beta", "alpha"] {
            save_project(&platform, "r", name, &project, &tracks, &patches, &effects).unwrap();
        }
        assert_eq!(
            list_projects(&platform, "r").unwrap(),
            vec!["alpha", "beta"]
        );
    }
}
