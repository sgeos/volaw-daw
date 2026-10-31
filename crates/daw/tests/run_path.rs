//! The composer must start when run directly, without the loader paths that
//! `cargo run` and `cargo test` set.
//!
//! The binary links SDL3 and SDL3_ttf as shared libraries that sit next to it,
//! so it starts only if its run path, set in `build.rs`, points there. The test
//! runs it with an empty environment, which drops `DYLD_FALLBACK_LIBRARY_PATH`
//! and `LD_LIBRARY_PATH`, and with an argument it does not know. The warning
//! for that argument is printed at the top of `main`, before SDL is
//! initialised, so seeing it proves that the loader resolved every library. A
//! loader failure ends the process before `main` and the warning never appears.
//!
//! The process is killed once the warning appears, so it opens no window and
//! touches no file. The dummy SDL drivers are set as well, so that a slow kill
//! still opens nothing.
//!
//! Failure modes. A library that the loader also finds in a system location,
//! such as one installed in `/usr/local/lib`, would let the test pass without
//! the run path. The test waits at most `TIMEOUT` in all for the warning,
//! measured from one deadline, so neither a binary that hangs before `main` nor
//! one that keeps printing other lines can stall the suite. The timeout is the
//! only nondeterminism, and it is far longer than a start takes.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// An argument the composer rejects with a warning.
const PROBE_ARG: &str = "--run-path-probe";

/// The longest wait for the warning, in all.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The most stderr lines kept for the failure message, so that a chatty
/// child cannot grow the test's memory without bound.
const MAX_KEPT_LINES: usize = 200;

#[test]
fn binary_starts_without_loader_paths() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_daw"))
        .arg(PROBE_ARG)
        .env_clear()
        .env("SDL_VIDEO_DRIVER", "dummy")
        .env("SDL_AUDIO_DRIVER", "dummy")
        .current_dir(env!("CARGO_TARGET_TMPDIR"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the daw binary");

    let stderr = child.stderr.take().expect("stderr is piped");
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let expected = format!("Unknown or malformed argument: {PROBE_ARG}");
    let mut seen = Vec::new();
    let deadline = Instant::now() + TIMEOUT;
    // Ends when the warning arrives, when stderr closes because the process
    // exited, or at the deadline. Each wait is only for the time left, so
    // lines that keep arriving do not extend it. The explicit check stops
    // the loop at the deadline even while lines are still queued, because a
    // zero wait still returns a queued line.
    let reached_main = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break false;
        }
        match rx.recv_timeout(left) {
            Ok(line) => {
                let hit = line.contains(&expected);
                if seen.len() < MAX_KEPT_LINES {
                    seen.push(line);
                }
                if hit {
                    break true;
                }
            }
            Err(_) => break false,
        }
    };

    // The process may already have exited, in which case kill reports an
    // error that does not matter here.
    let _ = child.kill();
    let status = child.wait().expect("wait for the daw binary");
    drop(rx);
    reader.join().expect("join the stderr reader");

    assert!(
        reached_main,
        "the daw binary did not reach main without loader paths, status {status}, stderr:\n{}",
        seen.join("\n")
    );
}
