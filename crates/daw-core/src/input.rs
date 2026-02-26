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
}
