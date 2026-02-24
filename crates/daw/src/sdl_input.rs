use daw_core::input::DawInput;
use sdl3_sys::events::*;
use sdl3_sys::scancode::*;

/// Translate an SDL3 event into a DAW input event.
///
/// Returns `None` for events the DAW does not handle.
pub unsafe fn translate_event(event: &SDL_Event) -> Option<DawInput> {
    let event_type = unsafe { event.r#type };

    // Quit events.
    if event_type == SDL_EventType::QUIT.0
        || event_type == SDL_EventType::WINDOW_CLOSE_REQUESTED.0
    {
        return Some(DawInput::Quit);
    }

    // Key down events.
    if event_type == SDL_EventType::KEY_DOWN.0 {
        let key = unsafe { &event.key };
        if key.repeat {
            return None;
        }
        return translate_scancode(key.scancode);
    }

    // Mouse click events.
    if event_type == SDL_EventType::MOUSE_BUTTON_DOWN.0 {
        let button = unsafe { &event.button };
        return Some(DawInput::Click(button.x, button.y));
    }

    // Mouse motion events.
    if event_type == SDL_EventType::MOUSE_MOTION.0 {
        let motion = unsafe { &event.motion };
        return Some(DawInput::MouseMove(motion.x, motion.y));
    }

    // Mouse wheel events.
    if event_type == SDL_EventType::MOUSE_WHEEL.0 {
        let wheel = unsafe { &event.wheel };
        return Some(DawInput::Scroll(wheel.y as i32));
    }

    None
}

fn translate_scancode(scancode: SDL_Scancode) -> Option<DawInput> {
    match scancode {
        SDL_SCANCODE_Q => Some(DawInput::Quit),
        SDL_SCANCODE_SPACE => Some(DawInput::TogglePlay),
        SDL_SCANCODE_S => Some(DawInput::Stop),
        SDL_SCANCODE_UP => Some(DawInput::Up),
        SDL_SCANCODE_DOWN => Some(DawInput::Down),
        SDL_SCANCODE_LEFT => Some(DawInput::Left),
        SDL_SCANCODE_RIGHT => Some(DawInput::Right),
        SDL_SCANCODE_RETURN => Some(DawInput::Confirm),
        SDL_SCANCODE_ESCAPE => Some(DawInput::Cancel),
        SDL_SCANCODE_DELETE | SDL_SCANCODE_BACKSPACE => Some(DawInput::Delete),
        SDL_SCANCODE_EQUALS | SDL_SCANCODE_KP_PLUS => Some(DawInput::ZoomIn),
        SDL_SCANCODE_MINUS | SDL_SCANCODE_KP_MINUS => Some(DawInput::ZoomOut),
        // Channel selection: number keys 1-8.
        SDL_SCANCODE_1 => Some(DawInput::SelectChannel(0)),
        SDL_SCANCODE_2 => Some(DawInput::SelectChannel(1)),
        SDL_SCANCODE_3 => Some(DawInput::SelectChannel(2)),
        SDL_SCANCODE_4 => Some(DawInput::SelectChannel(3)),
        SDL_SCANCODE_5 => Some(DawInput::SelectChannel(4)),
        SDL_SCANCODE_6 => Some(DawInput::SelectChannel(5)),
        SDL_SCANCODE_7 => Some(DawInput::SelectChannel(6)),
        SDL_SCANCODE_8 => Some(DawInput::SelectChannel(7)),
        _ => None,
    }
}
