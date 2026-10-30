//! The platform abstraction through which the `no_std` core reaches files,
//! logging and time.
//!
//! The std platform layer implements `Platform`, and the entry point injects
//! it. `MemoryPlatform` is an in-memory implementation for tests and for
//! headless hosts. Paths are `/`-separated strings, which every supported
//! platform accepts.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Platform services for the DAW core.
pub trait Platform {
    /// Read a whole file.
    fn read_file(&self, path: &str) -> Result<Vec<u8>, String>;

    /// Write a whole file, creating parent directories as needed.
    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), String>;

    /// List the files in `directory` whose extension is `extension`, without
    /// the dot. Returns full paths, sorted.
    fn list_files(&self, directory: &str, extension: &str) -> Result<Vec<String>, String>;

    /// Whether a file or directory exists at `path`.
    fn path_exists(&self, path: &str) -> bool;

    /// An opaque modification stamp for a file, or `None` if the file is
    /// missing or the platform cannot tell. Two reads of an unchanged file
    /// return the same stamp, which is all that hot reloading relies on.
    fn modified_stamp(&self, path: &str) -> Option<u64>;

    /// Log a message at the given severity level.
    fn log(&self, level: LogLevel, message: &str);

    /// Milliseconds elapsed since the platform was created. Monotonic.
    fn elapsed_ms(&self) -> u64;

    /// Whether audio output is available.
    fn audio_available(&self) -> bool;
}

/// An in-memory `Platform` with a manual clock.
///
/// A directory exists when some file lies under it. Each write bumps the
/// file's modification stamp. Log lines are kept for inspection.
#[derive(Debug, Default)]
pub struct MemoryPlatform {
    files: RefCell<BTreeMap<String, (Vec<u8>, u64)>>,
    writes: Cell<u64>,
    now_ms: Cell<u64>,
    logs: RefCell<Vec<(LogLevel, String)>>,
}

impl MemoryPlatform {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the clock that `elapsed_ms` reports.
    pub fn advance_ms(&self, ms: u64) {
        self.now_ms.set(self.now_ms.get().saturating_add(ms));
    }

    pub fn file_count(&self) -> usize {
        self.files.borrow().len()
    }

    pub fn logs(&self) -> Vec<(LogLevel, String)> {
        self.logs.borrow().clone()
    }
}

impl Platform for MemoryPlatform {
    fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
        self.files
            .borrow()
            .get(path)
            .map(|(data, _)| data.clone())
            .ok_or_else(|| format!("read_file({path}): not found"))
    }

    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), String> {
        let stamp = self.writes.get() + 1;
        self.writes.set(stamp);
        self.files
            .borrow_mut()
            .insert(String::from(path), (data.to_vec(), stamp));
        Ok(())
    }

    fn list_files(&self, directory: &str, extension: &str) -> Result<Vec<String>, String> {
        let prefix = format!("{}/", directory.trim_end_matches('/'));
        let suffix = format!(".{extension}");
        Ok(self
            .files
            .borrow()
            .keys()
            .filter(|k| {
                k.strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.contains('/') && rest.ends_with(&suffix))
            })
            .cloned()
            .collect())
    }

    fn path_exists(&self, path: &str) -> bool {
        let files = self.files.borrow();
        if files.contains_key(path) {
            return true;
        }
        let prefix = format!("{}/", path.trim_end_matches('/'));
        files.keys().any(|k| k.starts_with(&prefix))
    }

    fn modified_stamp(&self, path: &str) -> Option<u64> {
        self.files.borrow().get(path).map(|(_, stamp)| *stamp)
    }

    fn log(&self, level: LogLevel, message: &str) {
        self.logs.borrow_mut().push((level, String::from(message)));
    }

    fn elapsed_ms(&self) -> u64 {
        self.now_ms.get()
    }

    fn audio_available(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_platform_files_and_directories() {
        let p = MemoryPlatform::new();
        assert!(p.read_file("a/b.txt").is_err());
        assert!(!p.path_exists("a"));
        p.write_file("a/b.txt", b"x").unwrap();
        p.write_file("a/c.toml", b"y").unwrap();
        p.write_file("a/d/e.toml", b"z").unwrap();
        assert!(p.path_exists("a"));
        assert!(p.path_exists("a/d"));
        assert!(!p.path_exists("a/b"));
        assert_eq!(p.read_file("a/b.txt").unwrap(), b"x");
        assert_eq!(p.list_files("a", "toml").unwrap(), ["a/c.toml"]);
    }

    #[test]
    fn memory_platform_stamps_change_on_write() {
        let p = MemoryPlatform::new();
        assert_eq!(p.modified_stamp("f"), None);
        p.write_file("f", b"1").unwrap();
        let first = p.modified_stamp("f");
        assert_eq!(p.modified_stamp("f"), first);
        p.write_file("f", b"2").unwrap();
        assert_ne!(p.modified_stamp("f"), first);
    }

    #[test]
    fn memory_platform_clock_and_logs() {
        let p = MemoryPlatform::new();
        assert_eq!(p.elapsed_ms(), 0);
        p.advance_ms(250);
        assert_eq!(p.elapsed_ms(), 250);
        p.log(LogLevel::Warn, "careful");
        assert_eq!(p.logs(), [(LogLevel::Warn, String::from("careful"))]);
    }
}
