use std::time::Instant;

use daw_core::platform::{LogLevel, Platform};

/// SDL3 platform implementation for the DAW.
pub struct SdlPlatform {
    start_time: Instant,
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
        self.start_time.elapsed().as_millis() as u64
    }

    fn audio_available(&self) -> bool {
        true
    }
}
