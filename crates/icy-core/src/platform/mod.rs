//! OS-specific helpers isolated behind a small platform boundary.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as os;
#[cfg(windows)]
use windows as os;

/// Returns the user's preferred interactive shell program and default flags.
pub fn default_shell() -> (String, Vec<String>) {
    os::default_shell()
}

/// Returns a cross-platform shell invocation that prints `message` and exits.
pub fn echo_command(message: &str) -> (String, Vec<String>) {
    os::echo_command(message)
}

/// Returns the user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    os::home_dir()
}

/// Returns icy's persistent configuration directory (`~/.config/icy` or `%APPDATA%\icy`).
pub fn config_dir() -> PathBuf {
    os::config_dir()
}

/// Opens a URL or file in the host operating system's default handler.
pub fn open_external(target: &str) -> io::Result<()> {
    os::open_external(target)
}

/// Expected GitHub release artifact stem for the current platform.
pub fn release_asset_name() -> &'static str {
    os::release_asset_name()
}

/// Stages an OS-native helper that replaces `current_exe` with `staged_bin` and restarts `icy`.
pub fn spawn_self_update_installer(staged_bin: &Path, current_exe: &Path) -> io::Result<()> {
    os::spawn_self_update_installer(staged_bin, current_exe)
}
