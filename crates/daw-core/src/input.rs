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
}
