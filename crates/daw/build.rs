//! Sets a run path on the composer, and puts the SDL libraries beside it, so
//! that it runs directly, not only through `cargo run`.
//!
//! `sdl3-sys` and `sdl3-ttf-sys` build SDL3 and SDL3_ttf from source as shared
//! libraries. Their install names begin with `@rpath` on macOS, and on Linux
//! the loader searches only the system paths, so a binary without a run path
//! finds neither library. `cargo run` and `cargo test` hide the defect,
//! because they set the loader's library path to the build directories.
//!
//! The run path is relative to the executable, so a build directory can move.
//! Examples live one directory below the binaries, in `examples/`. Windows finds
//! a library next to the executable by default and needs nothing, and any other
//! target is left alone rather than guessed at.
//!
//! The sys crates copy each library into the profile directory, such as
//! `target/debug`, but only when their build scripts run. A cache that restores
//! their build output without rerunning them, as continuous integration does,
//! leaves the profile directory without the libraries. This script therefore
//! copies them too, from the `CMAKE_DIR` metadata each sys crate publishes
//! through its `links` key. It runs whenever `daw` is rebuilt, which a cache of
//! dependencies does not prevent.
//!
//! Assumptions. The libraries sit in the parent of the parent of `CMAKE_DIR`,
//! as CMake installs them, and the profile directory is the third ancestor of
//! `OUT_DIR`. A change to either would make `tests/run_path.rs` fail.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The `links` metadata variable for each library, and the library's name.
const LIBRARIES: [(&str, &str); 2] = [
    ("DEP_SDL3_CMAKE_DIR", "SDL3"),
    ("DEP_SDL3_TTF_CMAKE_DIR", "SDL3_ttf"),
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let origin = match os.as_str() {
        "macos" | "ios" => "@executable_path",
        "linux" | "android" | "freebsd" | "netbsd" | "openbsd" | "dragonfly" => "$ORIGIN",
        _ => return,
    };
    println!("cargo::rustc-link-arg-bins=-Wl,-rpath,{origin}");
    println!("cargo::rustc-link-arg-examples=-Wl,-rpath,{origin}/..");

    let Some(profile_dir) = profile_dir() else {
        println!("cargo::warning=no profile directory above OUT_DIR, SDL libraries not copied");
        return;
    };
    let macos = matches!(os.as_str(), "macos" | "ios");
    for (var, name) in LIBRARIES {
        println!("cargo::rerun-if-env-changed={var}");
        if let Err(e) = copy_library(var, name, macos, &profile_dir) {
            println!("cargo::warning=lib{name} not copied beside the binary: {e}");
        }
    }
}

/// The profile directory, such as `target/debug`, from
/// `<profile>/build/<package>-<hash>/out`.
fn profile_dir() -> Option<PathBuf> {
    let out = PathBuf::from(env::var_os("OUT_DIR")?);
    out.ancestors().nth(3).map(Path::to_path_buf)
}

/// Copy the versioned shared library named in `var`'s metadata into
/// `profile_dir`, the name the loader asks for, such as `libSDL3.so.0` or
/// `libSDL3.0.dylib`. A copy follows a symbolic link to its file.
fn copy_library(var: &str, name: &str, macos: bool, profile_dir: &Path) -> Result<(), String> {
    let cmake_dir = env::var_os(var).ok_or_else(|| format!("{var} is not set"))?;
    let lib_dir = Path::new(&cmake_dir)
        .ancestors()
        .nth(2)
        .ok_or_else(|| format!("{var} has no grandparent"))?;
    let entries = fs::read_dir(lib_dir).map_err(|e| format!("{}: {e}", lib_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if is_loader_name(file_name, name, macos) {
            println!("cargo::rerun-if-changed={}", entry.path().display());
            fs::copy(entry.path(), profile_dir.join(file_name))
                .map_err(|e| format!("{file_name}: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("no versioned library in {}", lib_dir.display()))
}

/// Whether `file_name` is the name the loader asks for, with a single
/// numeric major version, as the sys crates themselves select it.
fn is_loader_name(file_name: &str, name: &str, macos: bool) -> bool {
    let version = if macos {
        file_name
            .strip_prefix(&format!("lib{name}."))
            .and_then(|rest| rest.strip_suffix(".dylib"))
    } else {
        file_name.strip_prefix(&format!("lib{name}.so."))
    };
    version.is_some_and(|v| v.parse::<u32>().is_ok())
}
