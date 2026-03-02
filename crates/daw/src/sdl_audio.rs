use audio::AudioEngine;
use sdl3_sys::audio::*;

use crate::engine_command::{apply_engine_command, EngineCommand};
use crate::ring_buffer::Consumer;

/// Audio thread state: owns the engine, command consumer, and pre-allocated
/// staging buffer. This struct lives on the heap and is passed to the SDL3
/// audio callback as a raw pointer.
///
/// ## Ownership Model
///
/// The `AudioEngine` is exclusively owned by the audio thread after
/// initialization. The UI thread communicates with the audio thread
/// via `EngineCommand` messages pushed through the SPSC ring buffer.
/// No mutex, no shared ownership.
pub struct AudioThread {
    pub engine: AudioEngine,
    consumer: Consumer<EngineCommand>,
    /// Pre-allocated f64 staging buffer for f32 conversion.
    /// Sized once at construction and never reallocated.
    staging: Vec<f64>,
}

impl AudioThread {
    /// Create a new audio thread state with the given engine, command
    /// consumer, and staging buffer capacity (in f64 samples).
    pub fn new(engine: AudioEngine, consumer: Consumer<EngineCommand>, staging_capacity: usize) -> Self {
        Self {
            engine,
            consumer,
            staging: vec![0.0f64; staging_capacity],
        }
    }
}

/// Bridge between the SDL3 audio subsystem and the lock-free audio thread.
///
/// Holds a raw pointer to the `AudioThread` struct on the heap. The pointer
/// is passed to SDL3 as callback userdata. The `AudioBridge` does not own
/// the `AudioThread`; the caller is responsible for managing its lifetime
/// (the `AudioThread` must outlive the SDL stream).
pub struct AudioBridge {
    audio_thread: *mut AudioThread,
    stream: *mut SDL_AudioStream,
}

// Safety: The AudioBridge holds a raw pointer to AudioThread, but the
// pointer is only dereferenced inside the SDL audio callback (single
// consumer thread) and during drop (main thread, after stream is destroyed).
unsafe impl Send for AudioBridge {}

impl AudioBridge {
    /// Create a new audio bridge from a raw pointer to an `AudioThread`.
    ///
    /// # Safety
    ///
    /// The caller must ensure the `AudioThread` pointer remains valid for
    /// the lifetime of this `AudioBridge` and the SDL audio stream.
    pub fn new(audio_thread: *mut AudioThread) -> Self {
        Self {
            audio_thread,
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

        let userdata = self.audio_thread as *mut std::ffi::c_void;

        let stream = unsafe {
            SDL_OpenAudioDeviceStream(
                SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK,
                &spec,
                Some(audio_callback),
                userdata,
            )
        };

        if stream.is_null() {
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

/// SDL3 audio stream callback (lock-free).
///
/// Called by the SDL audio thread. Drains all pending commands from the
/// ring buffer, renders audio into the pre-allocated staging buffer,
/// converts f64 to f32, and pushes the result to the SDL stream.
///
/// No mutex acquisition. No heap allocation. Bounded work per invocation.
unsafe extern "C" fn audio_callback(
    userdata: *mut std::ffi::c_void,
    stream: *mut SDL_AudioStream,
    additional_amount: std::ffi::c_int,
    _total_amount: std::ffi::c_int,
) {
    if userdata.is_null() || additional_amount <= 0 {
        return;
    }

    let audio_thread = unsafe { &mut *(userdata as *mut AudioThread) };
    let num_samples = additional_amount as usize / std::mem::size_of::<f32>();
    if num_samples == 0 {
        return;
    }

    // Drain all pending commands from the ring buffer.
    while let Some(cmd) = audio_thread.consumer.pop() {
        apply_engine_command(&mut audio_thread.engine, cmd);
    }

    // Ensure staging buffer is large enough. This should not reallocate
    // in practice because the staging buffer is sized at construction,
    // but we handle the edge case defensively.
    if audio_thread.staging.len() < num_samples {
        audio_thread.staging.resize(num_samples, 0.0);
    }

    // Render f64 audio into the staging buffer.
    audio_thread
        .engine
        .render_samples(&mut audio_thread.staging[..num_samples]);

    // Convert f64 to f32 and push to the SDL stream using a stack buffer
    // to avoid heap allocation. Process in chunks of 4096 f32 samples.
    let mut offset = 0;
    while offset < num_samples {
        let mut f32_chunk = [0.0f32; 4096];
        let chunk_len = (num_samples - offset).min(4096);
        for (out, &src) in f32_chunk[..chunk_len]
            .iter_mut()
            .zip(&audio_thread.staging[offset..offset + chunk_len])
        {
            *out = src as f32;
        }
        let byte_len = (chunk_len * std::mem::size_of::<f32>()) as std::ffi::c_int;
        unsafe {
            SDL_PutAudioStreamData(
                stream,
                f32_chunk.as_ptr() as *const std::ffi::c_void,
                byte_len,
            );
        }
        offset += chunk_len;
    }
}
