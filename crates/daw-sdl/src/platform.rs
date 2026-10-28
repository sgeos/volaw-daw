//! The std implementation of `daw_core::Platform`.

use std::time::{Instant, UNIX_EPOCH};

use daw_core::platform::{LogLevel, Platform};

/// File system, standard error logging and the monotonic clock.
pub struct SdlPlatform {
    start_time: Instant,
}

impl Default for SdlPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl SdlPlatform {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }
}

impl Platform for SdlPlatform {
    fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
        std::fs::read(path).map_err(|e| format!("read_file({path}): {e}"))
    }

    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), String> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create_dir_all: {e}"))?;
        }
        std::fs::write(path, data).map_err(|e| format!("write_file({path}): {e}"))
    }

    fn list_files(&self, directory: &str, extension: &str) -> Result<Vec<String>, String> {
        let mut files = Vec::new();
        let entries =
            std::fs::read_dir(directory).map_err(|e| format!("list_files({directory}): {e}"))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("read entry: {e}"))?;
            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension()
                && ext == extension
                && let Some(s) = path.to_str()
            {
                files.push(s.to_string());
            }
        }
        files.sort();
        Ok(files)
    }

    fn path_exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    fn modified_stamp(&self, path: &str) -> Option<u64> {
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
        let since_epoch = modified.duration_since(UNIX_EPOCH).ok()?;
        u64::try_from(since_epoch.as_nanos()).ok()
    }

    fn log(&self, level: LogLevel, message: &str) {
        let tag = match level {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        };
        eprintln!("[{tag}] {message}");
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.start_time.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn audio_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory under the system temporary directory, removed on drop.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("daw-sdl-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }

        fn path(&self, rel: &str) -> String {
            self.0.join(rel).to_string_lossy().into_owned()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn write_creates_parents_and_reads_back() {
        let dir = TempDir::new("rw");
        let p = SdlPlatform::new();
        let file = dir.path("a/b/c.toml");
        assert!(!p.path_exists(&file));
        assert!(p.modified_stamp(&file).is_none());
        p.write_file(&file, b"x = 1").unwrap();
        assert!(p.path_exists(&file));
        assert!(p.path_exists(&dir.path("a/b")));
        assert_eq!(p.read_file(&file).unwrap(), b"x = 1");
        assert!(p.modified_stamp(&file).is_some());
    }

    #[test]
    fn list_files_filters_and_sorts() {
        let dir = TempDir::new("list");
        let p = SdlPlatform::new();
        for name in ["b.toml", "a.toml", "c.txt"] {
            p.write_file(&dir.path(name), b"").unwrap();
        }
        let listed = p.list_files(&dir.path(""), "toml").unwrap();
        assert_eq!(listed, [dir.path("a.toml"), dir.path("b.toml")]);
    }

    #[test]
    fn missing_paths_are_errors() {
        let dir = TempDir::new("missing");
        let p = SdlPlatform::new();
        assert!(p.read_file(&dir.path("none")).is_err());
        assert!(p.list_files(&dir.path("none"), "toml").is_err());
    }

    #[test]
    fn clock_is_monotonic() {
        let p = SdlPlatform::new();
        let a = p.elapsed_ms();
        assert!(p.elapsed_ms() >= a);
    }
}
