use std::path::{Path, PathBuf};

use daw_core::midi::MidiTrack;
use daw_core::patch::{EffectChainConfig, PatchConfig};
use daw_core::project::DawProject;

/// Type alias for the loaded project data.
type LoadedProject = (
    DawProject,
    [MidiTrack; 8],
    [PatchConfig; 8],
    [EffectChainConfig; 8],
);

/// Fixed subfolder names under the project root.
const SAVES_DIR: &str = "saves";
const PATCHES_DIR: &str = "patches";
const EFFECTS_DIR: &str = "effects";
const MIDI_DIR: &str = "midi";

/// Ensure a directory exists, creating it if necessary.
fn ensure_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        std::fs::create_dir_all(path).map_err(|e| format!("create dir {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Save a DAW project and its referenced files to the project root.
///
/// The project TOML is written to `{root}/saves/{filename}.toml`.
/// Any non-empty patch, effect, and MIDI file references are also saved
/// to their respective folders.
pub fn save_project(
    root: &Path,
    filename: &str,
    project: &DawProject,
    tracks: &[MidiTrack; 8],
    patches: &[PatchConfig; 8],
    effects: &[EffectChainConfig; 8],
) -> Result<PathBuf, String> {
    let saves_dir = root.join(SAVES_DIR);
    let patches_dir = root.join(PATCHES_DIR);
    let effects_dir = root.join(EFFECTS_DIR);
    let midi_dir = root.join(MIDI_DIR);

    ensure_dir(&saves_dir)?;
    ensure_dir(&patches_dir)?;
    ensure_dir(&effects_dir)?;
    ensure_dir(&midi_dir)?;

    // Save per-channel data.
    for i in 0..8 {
        let ch = &project.channels[i];

        // Save patch if a file is referenced.
        if !ch.patch_file.is_empty() {
            let patch_path = patches_dir.join(&ch.patch_file);
            let toml = toml::to_string_pretty(&patches[i])
                .map_err(|e| format!("serialize patch {i}: {e}"))?;
            std::fs::write(&patch_path, toml)
                .map_err(|e| format!("write {}: {e}", patch_path.display()))?;
        }

        // Save effect chain if a file is referenced.
        if !ch.effect_chain_file.is_empty() {
            let fx_path = effects_dir.join(&ch.effect_chain_file);
            let toml = toml::to_string_pretty(&effects[i])
                .map_err(|e| format!("serialize effects {i}: {e}"))?;
            std::fs::write(&fx_path, toml)
                .map_err(|e| format!("write {}: {e}", fx_path.display()))?;
        }

        // Save MIDI data if a file is referenced.
        if !ch.midi_file.is_empty() {
            let midi_path = midi_dir.join(&ch.midi_file);
            let toml = toml::to_string_pretty(&tracks[i])
                .map_err(|e| format!("serialize midi {i}: {e}"))?;
            std::fs::write(&midi_path, toml)
                .map_err(|e| format!("write {}: {e}", midi_path.display()))?;
        }
    }

    // Save the project wrapper TOML.
    let project_path = saves_dir.join(format!("{filename}.toml"));
    let toml = toml::to_string_pretty(project).map_err(|e| format!("serialize project: {e}"))?;
    std::fs::write(&project_path, toml)
        .map_err(|e| format!("write {}: {e}", project_path.display()))?;

    Ok(project_path)
}

/// Load a DAW project and its referenced files from the project root.
///
/// Reads the project TOML from `{root}/saves/{filename}.toml`, then loads
/// all referenced patch, effect, and MIDI files from their folders.
pub fn load_project(root: &Path, filename: &str) -> Result<LoadedProject, String> {
    let project_path = root.join(SAVES_DIR).join(format!("{filename}.toml"));
    let project_toml = std::fs::read_to_string(&project_path)
        .map_err(|e| format!("read {}: {e}", project_path.display()))?;
    let project: DawProject =
        toml::from_str(&project_toml).map_err(|e| format!("parse project: {e}"))?;

    let patches_dir = root.join(PATCHES_DIR);
    let effects_dir = root.join(EFFECTS_DIR);
    let midi_dir = root.join(MIDI_DIR);

    let mut tracks: [MidiTrack; 8] = core::array::from_fn(|_| MidiTrack::new());
    let mut patches: [PatchConfig; 8] = core::array::from_fn(|_| PatchConfig::default());
    let mut effects: [EffectChainConfig; 8] =
        core::array::from_fn(|_| EffectChainConfig::default());

    for i in 0..8 {
        let ch = &project.channels[i];

        // Load patch.
        if !ch.patch_file.is_empty() {
            let patch_path = patches_dir.join(&ch.patch_file);
            if patch_path.exists() {
                let toml_str = std::fs::read_to_string(&patch_path)
                    .map_err(|e| format!("read {}: {e}", patch_path.display()))?;
                patches[i] = toml::from_str(&toml_str)
                    .map_err(|e| format!("parse patch {}: {e}", ch.patch_file))?;
            }
        }

        // Load effect chain.
        if !ch.effect_chain_file.is_empty() {
            let fx_path = effects_dir.join(&ch.effect_chain_file);
            if fx_path.exists() {
                let toml_str = std::fs::read_to_string(&fx_path)
                    .map_err(|e| format!("read {}: {e}", fx_path.display()))?;
                effects[i] = toml::from_str(&toml_str)
                    .map_err(|e| format!("parse effects {}: {e}", ch.effect_chain_file))?;
            }
        }

        // Load MIDI data.
        if !ch.midi_file.is_empty() {
            let midi_path = midi_dir.join(&ch.midi_file);
            if midi_path.exists() {
                let toml_str = std::fs::read_to_string(&midi_path)
                    .map_err(|e| format!("read {}: {e}", midi_path.display()))?;
                tracks[i] = toml::from_str(&toml_str)
                    .map_err(|e| format!("parse midi {}: {e}", ch.midi_file))?;
            }
        }
    }

    Ok((project, tracks, patches, effects))
}

/// List available project files in the saves directory.
#[allow(dead_code)]
pub fn list_projects(root: &Path) -> Result<Vec<String>, String> {
    let saves_dir = root.join(SAVES_DIR);
    if !saves_dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    let entries =
        std::fs::read_dir(&saves_dir).map_err(|e| format!("read {}: {e}", saves_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let path = entry.path();
        if path.is_file()
            && let Some(ext) = path.extension()
            && ext == "toml"
            && let Some(stem) = path.file_stem()
            && let Some(name) = stem.to_str()
        {
            names.push(name.to_string());
        }
    }
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("daw_test_save_load");
        let _ = std::fs::remove_dir_all(&dir);

        let mut project = DawProject {
            name: "Test Project".to_string(),
            ..DawProject::default()
        };
        project.channels[0].patch_file = "ch1_patch.toml".to_string();
        project.channels[0].midi_file = "ch1.toml".to_string();

        let mut tracks: [MidiTrack; 8] = core::array::from_fn(|_| MidiTrack::new());
        tracks[0].notes.push(daw_core::midi::MidiNote {
            tick: 0,
            duration: 480,
            pitch: 60,
            velocity: 100,
        });

        let patches: [PatchConfig; 8] = core::array::from_fn(|_| PatchConfig::default());
        let effects: [EffectChainConfig; 8] =
            core::array::from_fn(|_| EffectChainConfig::default());

        let result = save_project(&dir, "test", &project, &tracks, &patches, &effects);
        assert!(result.is_ok(), "save failed: {:?}", result.err());

        let (loaded_project, loaded_tracks, loaded_patches, _loaded_effects) =
            load_project(&dir, "test").expect("load failed");

        assert_eq!(loaded_project.name, "Test Project");
        assert_eq!(loaded_tracks[0].notes.len(), 1);
        assert_eq!(loaded_tracks[0].notes[0].pitch, 60);
        assert_eq!(loaded_patches[0].name, "Default");

        // Cleanup.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_projects_empty() {
        let dir = std::env::temp_dir().join("daw_test_list_empty");
        let _ = std::fs::remove_dir_all(&dir);

        let projects = list_projects(&dir).expect("list failed");
        assert!(projects.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_projects_finds_saves() {
        let dir = std::env::temp_dir().join("daw_test_list_finds");
        let _ = std::fs::remove_dir_all(&dir);

        let project = DawProject::default();
        let tracks: [MidiTrack; 8] = core::array::from_fn(|_| MidiTrack::new());
        let patches: [PatchConfig; 8] = core::array::from_fn(|_| PatchConfig::default());
        let effects: [EffectChainConfig; 8] =
            core::array::from_fn(|_| EffectChainConfig::default());

        save_project(&dir, "alpha", &project, &tracks, &patches, &effects).unwrap();
        save_project(&dir, "beta", &project, &tracks, &patches, &effects).unwrap();

        let projects = list_projects(&dir).expect("list failed");
        assert_eq!(projects, vec!["alpha", "beta"]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
