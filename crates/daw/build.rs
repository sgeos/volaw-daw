//! Sets a run path on the composer so that it runs directly, not only through
//! `cargo run`.
//!
//! `sdl3-sys` and `sdl3-ttf-sys` build SDL3 and SDL3_ttf from source as shared
//! libraries and copy them into the profile directory, such as `target/debug`,
//! next to the binary. Their install names begin with `@rpath` on macOS, and on
//! Linux the loader searches only the system paths, so a binary without a run
//! path finds neither library. `cargo run` and `cargo test` hide the defect,
//! because they set the loader's library path to the build directories.
//!
//! The run path is relative to the executable, so a build directory can move.
//! Examples live one directory below the binaries, in `examples/`. Windows finds
//! a library next to the executable by default and needs nothing, and any other
//! target is left alone rather than guessed at.
//!
//! Assumption. The libraries stay in the profile directory. A change of the copy
//! step in `sdl3-sys` would break this, and `tests/run_path.rs` would show it.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let origin = match os.as_str() {
        "macos" | "ios" => "@executable_path",
        "linux" | "android" | "freebsd" | "netbsd" | "openbsd" | "dragonfly" => "$ORIGIN",
        _ => return,
    };
    println!("cargo::rustc-link-arg-bins=-Wl,-rpath,{origin}");
    println!("cargo::rustc-link-arg-examples=-Wl,-rpath,{origin}/..");
}
