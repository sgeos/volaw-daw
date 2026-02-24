use std::sync::{Arc, Mutex};

use audio::AudioEngine;
use sdl3_sys::audio::*;

/// Bridge between the SDL3 audio subsystem and the no_std AudioEngine.
///
/// The AudioEngine is wrapped in Arc<Mutex> so the SDL audio callback thread
/// can safely access it. The main thread also holds a reference for updating
/// channel configuration.
pub struct AudioBridge {
    engine: Arc<Mutex<AudioEngine>>,
    stream: *mut SDL_AudioStream,
}

// SDL_AudioStream is thread-safe in SDL3 and AudioEngine is Mutex-protected.
unsafe impl Send for AudioBridge {}

impl AudioBridge {
    /// Create a new audio bridge wrapping the given engine (shared via Arc).
    pub fn new(engine: Arc<Mutex<AudioEngine>>) -> Self {
        Self {
            engine,
            stream: std::ptr::null_mut(),
        }
    }

    /// Open the default playback device with a callback that renders from the engine.
    pub fn open_device(&mut self, sample_rate: u32) -> Result<(), String> {
        let spec = SDL_AudioSpec {
            format: SDL_AUDIO_F32,
            channels: 2,
            freq: sample_rate as i32,
        };

        let userdata = Arc::into_raw(Arc::clone(&self.engine)) as *mut std::ffi::c_void;

        let stream = unsafe {
            SDL_OpenAudioDeviceStream(
                SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK,
                &spec,
                Some(audio_callback),
                userdata,
            )
        };

        if stream.is_null() {
            unsafe {
                let _ = Arc::from_raw(userdata as *const Mutex<AudioEngine>);
            }
            return Err("Failed to open audio device stream".to_string());
        }

        self.stream = stream;
        Ok(())
    }

    /// Resume audio playback (unpause the stream device).
    pub fn resume(&self) -> bool {
        if self.stream.is_null() {
            return false;
        }
        unsafe { SDL_ResumeAudioStreamDevice(self.stream) }
    }
}

impl Drop for AudioBridge {
    fn drop(&mut self) {
        if !self.stream.is_null() {
            unsafe {
                SDL_DestroyAudioStream(self.stream);
            }
            self.stream = std::ptr::null_mut();
        }
    }
}

/// SDL3 audio stream callback.
unsafe extern "C" fn audio_callback(
    userdata: *mut std::ffi::c_void,
    stream: *mut SDL_AudioStream,
    additional_amount: std::ffi::c_int,
    _total_amount: std::ffi::c_int,
) {
    if userdata.is_null() || additional_amount <= 0 {
        return;
    }

    let engine_mutex = unsafe { &*(userdata as *const Mutex<AudioEngine>) };
    let num_samples = additional_amount as usize / std::mem::size_of::<f32>();
    if num_samples == 0 {
        return;
    }

    let mut stack_buf = [0.0f32; 4096];
    let mut heap_buf;
    let buffer: &mut [f32] = if num_samples <= stack_buf.len() {
        &mut stack_buf[..num_samples]
    } else {
        heap_buf = vec![0.0f32; num_samples];
        &mut heap_buf
    };

    if let Ok(mut engine) = engine_mutex.try_lock() {
        engine.render_samples_f32(buffer);
    }

    unsafe {
        SDL_PutAudioStreamData(
            stream,
            buffer.as_ptr() as *const std::ffi::c_void,
            additional_amount,
        );
    }
}
