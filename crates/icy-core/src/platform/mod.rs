//! OS-specific helpers isolated behind a small platform boundary.

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
