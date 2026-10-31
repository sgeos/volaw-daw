//! The shutdown order between `SdlAudioOutput` and `SDL_Quit`, checked
//! against a real SDL audio stream.
//!
//! The tests select SDL's dummy audio driver, which needs no device and runs
//! the stream callback on its own thread, so they run in an agent sandbox and
//! in continuous integration alike. SDL's state is global to the process, and
//! these tests call `SDL_Quit`, so each one holds `SDL_LOCK` throughout. This
//! file is its own test binary, which keeps the unit tests of `audio-sdl`,
//! which never start SDL, out of the way.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use audio::{AudioOutput, AudioSource};
use audio_sdl::SdlAudioOutput;
use sdl3_sys::hints::{SDL_HINT_AUDIO_DRIVER, SDL_SetHint};
use sdl3_sys::init::{SDL_INIT_AUDIO, SDL_Quit, SDL_WasInit};

static SDL_LOCK: Mutex<()> = Mutex::new(());

const RATE: u32 = 48_000;

/// Counts the samples SDL asks for and the times it is dropped, standing in
/// for the engine.
struct Probe {
    rendered: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}

impl AudioSource for Probe {
    fn render_f32(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        self.rendered.fetch_add(out.len(), Ordering::Relaxed);
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

struct Counts {
    rendered: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}

fn probe() -> (Probe, Counts) {
    let rendered = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let counts = Counts {
        rendered: Arc::clone(&rendered),
        drops: Arc::clone(&drops),
    };
    (Probe { rendered, drops }, counts)
}

/// Take the lock, recovering it from a test that panicked, and select the
/// dummy driver. `SDL_Quit` clears hints, so every test sets it again.
fn dummy_sdl() -> MutexGuard<'static, ()> {
    let guard = SDL_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    dummy_driver();
    guard
}

/// Select the dummy driver, with the lock already held.
fn dummy_driver() {
    // Safety: both arguments are static C strings.
    assert!(unsafe { SDL_SetHint(SDL_HINT_AUDIO_DRIVER, c"dummy".as_ptr()) });
}

fn audio_initialized() -> bool {
    // Safety: plain SDL call with no pointer arguments.
    unsafe { SDL_WasInit(SDL_INIT_AUDIO) }.0 & SDL_INIT_AUDIO.0 != 0
}

fn sdl_quit() {
    // Safety: no other thread uses SDL while `SDL_LOCK` is held.
    unsafe { SDL_Quit() };
}

fn start(out: &mut SdlAudioOutput<Probe>, source: Probe) {
    if let Err((error, _)) = out.start(RATE, source) {
        panic!("the dummy driver should open: {error}");
    }
}

/// Wait, at most two seconds, for the callback to render.
fn wait_for_render(counts: &Counts) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if counts.rendered.load(Ordering::Relaxed) > 0 {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn stopping_before_sdl_quit_destroys_the_stream_and_releases_audio() {
    let _sdl = dummy_sdl();
    let (source, counts) = probe();
    let mut out = SdlAudioOutput::new();
    start(&mut out, source);
    assert!(audio_initialized());
    assert!(!out.stream_lost());
    assert!(wait_for_render(&counts), "the callback never ran");

    let source = out.stop().expect("the output was running");
    assert!(!out.is_running());
    assert!(!out.stream_lost());
    assert!(!audio_initialized(), "stop releases its audio reference");
    drop(source);
    assert_eq!(counts.drops.load(Ordering::Relaxed), 1);
    sdl_quit();
}

#[test]
fn sdl_quit_before_stop_is_detected_and_the_source_comes_back() {
    let _sdl = dummy_sdl();
    let (source, counts) = probe();
    let mut out = SdlAudioOutput::new();
    start(&mut out, source);
    assert!(wait_for_render(&counts), "the callback never ran");

    // The wrong order. `SDL_Quit` destroys the stream under the output.
    sdl_quit();
    assert!(out.stream_lost());
    assert!(out.is_running(), "the output still holds the source");
    let rendered = counts.rendered.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        counts.rendered.load(Ordering::Relaxed),
        rendered,
        "no callback runs on a destroyed stream"
    );

    // `stop` must neither destroy the stream again nor release a reference
    // that it no longer holds, and the source is intact.
    let source = out.stop().expect("the source comes back");
    assert!(!out.stream_lost());
    assert!(!audio_initialized());
    assert_eq!(counts.drops.load(Ordering::Relaxed), 0);
    drop(source);
    assert_eq!(counts.drops.load(Ordering::Relaxed), 1);
}

#[test]
fn dropping_after_sdl_quit_frees_the_source_once() {
    let _sdl = dummy_sdl();
    let (source, counts) = probe();
    let mut out = SdlAudioOutput::new();
    start(&mut out, source);
    sdl_quit();
    assert!(out.stream_lost());
    drop(out);
    assert_eq!(counts.drops.load(Ordering::Relaxed), 1);
}

#[test]
fn a_host_that_restarts_sdl_keeps_its_audio_reference() {
    let _sdl = dummy_sdl();
    let (source, _counts) = probe();
    let mut out = SdlAudioOutput::new();
    start(&mut out, source);
    sdl_quit();

    // The host starts audio again on its own before the output stops. The
    // late `stop` must not release the host's reference.
    dummy_driver();
    // Safety: no other thread uses SDL while `SDL_LOCK` is held.
    assert!(unsafe { sdl3_sys::init::SDL_InitSubSystem(SDL_INIT_AUDIO) });
    drop(out.stop().expect("the source comes back"));
    assert!(audio_initialized(), "the host's reference survives");
    sdl_quit();
}

#[test]
fn an_output_can_start_again_after_a_lost_stream() {
    let _sdl = dummy_sdl();
    let (first, _) = probe();
    let mut out = SdlAudioOutput::new();
    start(&mut out, first);
    sdl_quit();
    drop(out.stop());

    dummy_driver();
    let (second, counts) = probe();
    start(&mut out, second);
    assert!(!out.stream_lost(), "the new stream is watched afresh");
    assert!(wait_for_render(&counts), "the callback never ran");
    drop(out.stop().expect("running"));
    assert!(!audio_initialized());
    sdl_quit();
}
