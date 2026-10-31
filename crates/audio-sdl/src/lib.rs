//! SDL3 platform layer for the `audio` engine.
//!
//! `SdlAudioOutput` implements `audio::AudioOutput` by opening the default
//! playback device as a stereo `f32` stream whose callback calls
//! `AudioSource::render_f32`. It is the only place in the workspace that
//! touches the SDL audio API.
//!
//! ## Ownership and safety
//!
//! `start` boxes the source and hands SDL the raw pointer as callback userdata.
//! The output keeps that pointer and reclaims the box in `stop`, after
//! `SDL_DestroyAudioStream` returns. SDL3 runs the callback with the stream
//! lock held and destroying the stream takes the same lock, so no callback
//! can observe the source once `stop` has destroyed the stream. This replaces
//! the earlier design in which the binary leaked and reclaimed the box itself.
//!
//! `start` initializes the SDL audio subsystem, which SDL reference-counts,
//! and `stop` releases that reference. A host that has already initialized
//! SDL, such as a game client, is unaffected.
//!
//! ## Shutdown order
//!
//! Stop or drop the output before the host calls `SDL_Quit`. `SDL_Quit`
//! destroys every audio stream, so a later `stop` or drop that destroyed the
//! stream again would pass SDL a dangling pointer.
//!
//! The output checks the order at run time instead of trusting it. SDL offers
//! no query for whether a stream is alive, but it destroys a stream's
//! properties whenever it destroys the stream, through `SDL_DestroyAudioStream`
//! or `SDL_Quit` alike, and runs the cleanup callback of each pointer property.
//! `start` registers such a property, whose cleanup clears a flag that the
//! output owns. `stop` then destroys the stream and releases the audio
//! subsystem only while the flag is set. When the host has called `SDL_Quit`
//! first, `stop` skips both, because `SDL_Quit` has already done them, and
//! still returns the source, because SDL runs no callback on a destroyed
//! stream. `stream_lost` reports that state to a host that wants to log it.
//!
//! The order could not be enforced through ownership or types without
//! changing `volaw_audio::AudioSystem::start`, which the game calls, because
//! `SDL_Quit` is a free function that any code in the process may call.
//!
//! The check assumes that `SDL_Quit` and `stop` do not run at the same time
//! on two threads, which SDL does not support in any case.

use core::ffi::{c_int, c_void};
use core::marker::PhantomData;
use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicBool, Ordering};
use std::ffi::CStr;

use audio::{AudioOutput, AudioSource};
use sdl3_sys::audio::*;
use sdl3_sys::error::SDL_GetError;
use sdl3_sys::init::{SDL_INIT_AUDIO, SDL_InitSubSystem, SDL_QuitSubSystem};
use sdl3_sys::properties::SDL_SetPointerPropertyWithCleanup;

/// The stream property whose cleanup tells the output that SDL destroyed
/// the stream.
const ALIVE_PROPERTY: &CStr = c"volaw.audio_sdl.stream_alive";

/// The stack chunk the callback renders into, in `f32` samples. Larger
/// requests are served in several chunks, so the callback never allocates.
const CHUNK_SAMPLES: usize = 4096;

/// SDL3 playback of an `AudioSource`.
///
/// Not `Send`, because SDL expects a stream to be created and destroyed on the
/// thread that owns the subsystem.
pub struct SdlAudioOutput<S: AudioSource> {
    stream: *mut SDL_AudioStream,
    source: Option<NonNull<S>>,
    /// Set by `start` and cleared by the stream's property cleanup when SDL
    /// destroys the stream. Boxed so that its address, which SDL holds as
    /// cleanup userdata, survives a move of the output. Dropped only after
    /// `Drop` has destroyed the stream, so the cleanup never sees it freed.
    stream_alive: Box<AtomicBool>,
    _source: PhantomData<S>,
}

impl<S: AudioSource> Default for SdlAudioOutput<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: AudioSource> SdlAudioOutput<S> {
    pub fn new() -> Self {
        Self {
            stream: ptr::null_mut(),
            source: None,
            stream_alive: Box::new(AtomicBool::new(false)),
            _source: PhantomData,
        }
    }

    /// Whether SDL destroyed the stream while the output still holds the
    /// source, which means that the host called `SDL_Quit` before `stop`.
    /// No sound plays in that state. `stop` still returns the source, and
    /// the output is ready to start again afterwards.
    pub fn stream_lost(&self) -> bool {
        self.source.is_some() && !self.stream_alive.load(Ordering::Acquire)
    }
}

impl<S: AudioSource> AudioOutput<S> for SdlAudioOutput<S> {
    type Error = String;

    fn start(&mut self, sample_rate: u32, source: S) -> Result<(), (String, S)> {
        if self.is_running() {
            return Err(("audio output is already running".to_string(), source));
        }
        let Ok(freq) = c_int::try_from(sample_rate) else {
            return Err((format!("sample rate {sample_rate} is out of range"), source));
        };
        if freq == 0 {
            return Err(("sample rate must be nonzero".to_string(), source));
        }
        // Safety: plain SDL call with no pointer arguments.
        if !unsafe { SDL_InitSubSystem(SDL_INIT_AUDIO) } {
            return Err((format!("SDL audio init failed: {}", sdl_error()), source));
        }

        let spec = SDL_AudioSpec {
            format: SDL_AUDIO_F32,
            channels: 2,
            freq,
        };
        let boxed = NonNull::from(Box::leak(Box::new(source)));
        // Safety: `spec` outlives the call, and `boxed` stays valid until
        // `stop` destroys the stream and reclaims it.
        let stream = unsafe {
            SDL_OpenAudioDeviceStream(
                SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK,
                &spec,
                Some(audio_callback::<S>),
                boxed.as_ptr().cast::<c_void>(),
            )
        };
        if stream.is_null() {
            let error = format!("failed to open audio device stream: {}", sdl_error());
            // Safety: SDL did not keep the pointer, because no stream exists.
            let source = unsafe { *Box::from_raw(boxed.as_ptr()) };
            // Safety: balances the successful SDL_InitSubSystem above.
            unsafe { SDL_QuitSubSystem(SDL_INIT_AUDIO) };
            return Err((error, source));
        }
        self.stream = stream;
        self.source = Some(boxed);
        self.stream_alive.store(true, Ordering::Release);

        // Safety: `stream` is live, the name is a static C string, and the
        // flag outlives the stream, as its field documents. On failure SDL
        // runs the cleanup at once, which clears the flag, so the error path
        // below must destroy the stream without consulting it.
        let watched = unsafe {
            SDL_SetPointerPropertyWithCleanup(
                SDL_GetAudioStreamProperties(stream),
                ALIVE_PROPERTY.as_ptr(),
                ptr::from_ref::<AtomicBool>(&self.stream_alive)
                    .cast_mut()
                    .cast::<c_void>(),
                Some(stream_destroyed),
                ptr::null_mut(),
            )
        };
        if !watched {
            let error = format!("failed to watch the audio stream: {}", sdl_error());
            self.stream_alive.store(true, Ordering::Release);
            let source = self.stop().expect("the source was stored just above");
            return Err((error, source));
        }

        // Safety: `stream` is a live stream returned above.
        if !unsafe { SDL_ResumeAudioStreamDevice(stream) } {
            let error = format!("failed to resume audio device: {}", sdl_error());
            let source = self.stop().expect("the source was stored just above");
            return Err((error, source));
        }
        Ok(())
    }

    fn stop(&mut self) -> Option<S> {
        let boxed = self.source.take()?;
        // A cleared flag means that `SDL_Quit` destroyed the stream and reset
        // the subsystem's reference count already. Destroying the stream again
        // would use freed memory, and releasing the subsystem could take a
        // reference that the host acquired after `SDL_Quit`.
        if self.stream_alive.swap(false, Ordering::AcqRel) {
            // Safety: the stream is alive, as the flag shows, and was created
            // in `start`. Destroying it runs the cleanup, which only clears
            // the flag again. After this returns no callback runs, so the box
            // below is exclusively ours.
            unsafe {
                SDL_DestroyAudioStream(self.stream);
                SDL_QuitSubSystem(SDL_INIT_AUDIO);
            }
        }
        self.stream = ptr::null_mut();
        // Safety: `boxed` came from `Box::leak` in `start` and is reclaimed once.
        Some(unsafe { *Box::from_raw(boxed.as_ptr()) })
    }

    fn is_running(&self) -> bool {
        self.source.is_some()
    }
}

impl<S: AudioSource> Drop for SdlAudioOutput<S> {
    fn drop(&mut self) {
        drop(self.stop());
    }
}

fn sdl_error() -> String {
    // Safety: SDL_GetError returns a valid, possibly empty, C string.
    unsafe { CStr::from_ptr(SDL_GetError()) }
        .to_string_lossy()
        .into_owned()
}

/// Cleanup of the stream property set in `start`. SDL calls it once, when it
/// destroys the stream's properties together with the stream.
///
/// # Safety
///
/// `value` is the output's `stream_alive` flag, which outlives the stream.
unsafe extern "C" fn stream_destroyed(_userdata: *mut c_void, value: *mut c_void) {
    if value.is_null() {
        return;
    }
    // Safety: see the function contract.
    let alive = unsafe { &*value.cast::<AtomicBool>() };
    alive.store(false, Ordering::Release);
}

/// SDL3 stream callback. Renders whole stereo frames in stack chunks and
/// queues them on the stream. Bounded work, no locks and no allocation.
///
/// # Safety
///
/// `userdata` is the `S` boxed in `start`, which outlives the stream.
unsafe extern "C" fn audio_callback<S: AudioSource>(
    userdata: *mut c_void,
    stream: *mut SDL_AudioStream,
    additional_amount: c_int,
    _total_amount: c_int,
) {
    let Ok(bytes) = usize::try_from(additional_amount) else {
        return;
    };
    if userdata.is_null() {
        return;
    }
    // Safety: see the function contract. SDL serializes callbacks per stream.
    let source = unsafe { &mut *userdata.cast::<S>() };
    let samples = (bytes / size_of::<f32>()) & !1;
    let mut chunk = [0.0f32; CHUNK_SAMPLES];
    let mut remaining = samples;
    while remaining > 0 {
        let len = remaining.min(CHUNK_SAMPLES);
        source.render_f32(&mut chunk[..len]);
        // `len` is at most CHUNK_SAMPLES, so the byte count fits in c_int.
        let byte_len = (len * size_of::<f32>()) as c_int;
        // Safety: `chunk` holds `byte_len` initialized bytes.
        unsafe { SDL_PutAudioStreamData(stream, chunk.as_ptr().cast::<c_void>(), byte_len) };
        remaining -= len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Counts the samples it is asked for, standing in for the engine.
    struct Counter(usize);

    impl AudioSource for Counter {
        fn render_f32(&mut self, out: &mut [f32]) {
            self.0 += out.len();
            out.fill(0.5);
        }
    }

    #[test]
    fn new_output_is_stopped() {
        let mut out = SdlAudioOutput::<Counter>::new();
        assert!(!out.is_running());
        assert!(out.stop().is_none());
    }

    #[test]
    fn zero_sample_rate_returns_the_source() {
        let mut out = SdlAudioOutput::new();
        let (error, source) = out.start(0, Counter(7)).unwrap_err();
        assert!(error.contains("nonzero"), "{error}");
        assert_eq!(source.0, 7);
        assert!(!out.is_running());
    }

    #[test]
    fn oversized_sample_rate_returns_the_source() {
        let mut out = SdlAudioOutput::new();
        let (error, source) = out.start(u32::MAX, Counter(3)).unwrap_err();
        assert!(error.contains("out of range"), "{error}");
        assert_eq!(source.0, 3);
    }

    #[test]
    fn callback_renders_whole_frames_in_chunks() {
        // A null stream makes SDL_PutAudioStreamData fail harmlessly, so the
        // callback's chunking can be checked without an audio device.
        let mut source = Counter(0);
        let userdata = (&raw mut source).cast::<c_void>();
        let bytes = (CHUNK_SAMPLES * 2 + 3) * size_of::<f32>();
        unsafe { audio_callback::<Counter>(userdata, ptr::null_mut(), bytes as c_int, 0) };
        assert_eq!(source.0, CHUNK_SAMPLES * 2 + 2);
    }

    #[test]
    fn callback_ignores_negative_amount_and_null_userdata() {
        let mut source = Counter(0);
        let userdata = (&raw mut source).cast::<c_void>();
        unsafe { audio_callback::<Counter>(userdata, ptr::null_mut(), -8, 0) };
        unsafe { audio_callback::<Counter>(ptr::null_mut(), ptr::null_mut(), 64, 0) };
        assert_eq!(source.0, 0);
    }
}
