/// Input events for the DAW.
#[derive(Debug, Clone, PartialEq)]
pub enum DawInput {
    /// Quit the application.
    Quit,
    /// Mouse button pressed at pixel coordinates (x, y).
    Click(f32, f32),
    /// Mouse button released at pixel coordinates (x, y).
    MouseUp(f32, f32),
    /// Mouse moved to pixel coordinates (x, y).
    MouseMove(f32, f32),
    /// Mouse scroll wheel delta (vertical).
    Scroll(i32),
    /// Arrow key or equivalent navigation.
    Up,
    Down,
    Left,
    Right,
    /// Confirm selection (Enter/Return).
    Confirm,
    /// Cancel or go back (Escape).
    Cancel,
    /// Toggle playback (Space).
    TogglePlay,
    /// Stop playback and reset to beginning.
    Stop,
    /// Select channel 1-8 (0-indexed).
    SelectChannel(u8),
    /// Delete selected item(s).
    Delete,
    /// Save project (Ctrl+S).
    Save,
    /// Undo last action (Ctrl+Z).
    Undo,
    /// Redo last undone action (Ctrl+Shift+Z or Ctrl+Y).
    Redo,
    /// Zoom in.
    ZoomIn,
    /// Zoom out.
    ZoomOut,
    /// Cycle to the next preset on the selected channel.
    NextPreset,
    /// Cycle to the previous preset on the selected channel.
    PrevPreset,
    /// Toggle mute on the selected channel.
    ToggleMute,
    /// Cycle to the next page (Tab).
    NextPage,
    /// Cycle to the previous page (Shift+Tab).
    PrevPage,
    /// Insert/add item (A key on chain pages).
    Insert,
    /// Toggle bypass on the selected pedal (B key on chain pages).
    ToggleBypass,
    /// Move selected item left within its container (Shift+Left).
    ShiftLeft,
    /// Move selected item right within its container (Shift+Right).
    ShiftRight,
    /// Copy selected note(s) (Ctrl+C).
    Copy,
    /// Paste copied note(s) (Ctrl+V).
    Paste,
    /// Shift-click at pixel coordinates for multi-select.
    ShiftClick(f32, f32),
    /// Cycle grid subdivision (1/1 -> 1/2 -> 1/4 -> 1/8 -> 1/16 -> 1/32 -> 1/1).
    GridCycle,
    /// Increase tempo by 1 BPM.
    TempoUp,
    /// Decrease tempo by 1 BPM.
    TempoDown,
    /// Quantize selected notes to current grid.
    Quantize,
    /// Toggle loop mode on/off.
    ToggleLoop,
    /// Transpose selected notes up one octave (Shift+Up).
    ShiftUp,
    /// Transpose selected notes down one octave (Shift+Down).
    ShiftDown,
    /// Duplicate selected notes (Ctrl+D).
    Duplicate,
    /// Select all notes in the current track (Ctrl+A).
    SelectAll,
    /// Toggle solo on the selected channel.
    ToggleSolo,
    /// Reset transport to the beginning.
    Home,
    /// Cycle time signature (Shift+T): 4/4 -> 3/4 -> 6/4 -> 5/4 -> 7/4 -> 4/4.
    CycleTimeSig,
    /// Export project as Standard MIDI File (Ctrl+E).
    ExportMidi,
    /// Toggle metronome on/off (N key).
    ToggleMetronome,
    /// Character typed during text editing.
    TextChar(char),
    /// Backspace key (remove last character during text editing).
    Backspace,
}
