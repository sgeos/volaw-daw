extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Platform abstraction for std functionality.
///
/// The DAW core is no_std. All file I/O, logging, and timing are provided
/// by the platform implementation, which the std binary injects.
pub trait Platform {
    /// Read a file from the filesystem. Returns the file contents or an error message.
    fn read_file(&self, path: &str) -> Result<Vec<u8>, String>;

    /// Write data to a file on the filesystem. Creates parent directories as needed.
    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), String>;

    /// List files in a directory matching a glob pattern. Returns file paths.
    fn list_files(&self, directory: &str, extension: &str) -> Result<Vec<String>, String>;

    /// Log a message at the given severity level.
    fn log(&self, level: LogLevel, message: &str);

    /// Return elapsed time in milliseconds since the platform was created.
    fn elapsed_ms(&self) -> u64;

    /// Return whether audio output is available.
    fn audio_available(&self) -> bool;
}
