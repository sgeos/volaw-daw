extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Number of channels in the DAW.
pub const NUM_CHANNELS: usize = 8;

/// Top-level DAW project file.
///
/// This is the wrapper TOML that references patch files, effect chain files,
/// MIDI files, and timed events. Lives in the saves/ folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DawProject {
    /// Project name.
    pub name: String,
    /// Tempo in beats per minute.
    #[serde(default = "default_tempo")]
    pub tempo: f64,
    /// Ticks per beat (MIDI resolution).
    #[serde(default = "default_ticks_per_beat")]
    pub ticks_per_beat: u16,
    /// Per-channel configuration.
    pub channels: [ChannelConfig; NUM_CHANNELS],
    /// Timed events (PCM sample triggers, etc.).
    #[serde(default)]
    pub events: Vec<TimedEvent>,
}

/// Configuration for one channel in the project.
///
/// References external files for patch, effect chain, and MIDI data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Display name for this channel.
    #[serde(default)]
    pub name: String,
    /// Filename of the patch TOML in the patches/ folder.
    #[serde(default)]
    pub patch_file: String,
    /// Filename of the effect chain TOML in the effects/ folder.
    #[serde(default)]
    pub effect_chain_file: String,
    /// Filename of the MIDI file in the midi/ folder.
    #[serde(default)]
    pub midi_file: String,
    /// Whether this channel is muted.
    #[serde(default)]
    pub muted: bool,
    /// Whether this channel is soloed.
    #[serde(default)]
    pub solo: bool,
    /// Channel volume (0.0 to 1.0).
    #[serde(default = "default_volume")]
    pub volume: f64,
    /// Stereo pan (-1.0 left to 1.0 right, 0.0 center).
    #[serde(default)]
    pub pan: f64,
}

/// A timed event that occurs at a specific tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimedEvent {
    /// Time in ticks when this event fires.
    pub tick: u64,
    /// Channel this event affects (0-7).
    pub channel: u8,
    /// The event type and data.
    pub event: EventType,
}

/// Types of timed events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    /// Feed a PCM sample file into the channel's audio input.
    /// The string is the filename in the samples/ folder.
    FeedSample(String),
}

fn default_tempo() -> f64 {
    120.0
}

fn default_ticks_per_beat() -> u16 {
    480
}

fn default_volume() -> f64 {
    0.8
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            patch_file: String::new(),
            effect_chain_file: String::new(),
            midi_file: String::new(),
            muted: false,
            solo: false,
            volume: 0.8,
            pan: 0.0,
        }
    }
}

impl Default for DawProject {
    fn default() -> Self {
        let channels = core::array::from_fn(|i| {
            let mut name = String::from("Channel ");
            name.push(char::from(b'1' + i as u8));
            ChannelConfig {
                name,
                ..ChannelConfig::default()
            }
        });

        Self {
            name: String::from("Untitled"),
            tempo: default_tempo(),
            ticks_per_beat: default_ticks_per_beat(),
            channels,
            events: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_project_has_8_channels() {
        let project = DawProject::default();
        assert_eq!(project.channels.len(), NUM_CHANNELS);
    }

    #[test]
    fn default_project_tempo() {
        let project = DawProject::default();
        assert!((project.tempo - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn default_channel_names() {
        let project = DawProject::default();
        assert_eq!(project.channels[0].name, "Channel 1");
        assert_eq!(project.channels[7].name, "Channel 8");
    }

    #[test]
    fn default_channel_not_muted() {
        let project = DawProject::default();
        for ch in &project.channels {
            assert!(!ch.muted);
            assert!(!ch.solo);
        }
    }
}
