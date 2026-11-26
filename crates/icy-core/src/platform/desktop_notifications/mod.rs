use std::io;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(windows)]
use windows as backend;

/// Local notification text. Platform adapters treat both fields as plain text.
pub struct DesktopNotification {
    pub title: String,
    pub body: String,
    pub needs_input: bool,
}

#[derive(Default)]
pub struct DesktopNotifier {
    #[cfg(any(target_os = "linux", windows))]
    backend: backend::Notifier,
}

impl DesktopNotifier {
    /// Call from a notification worker because the desktop service may block.
    #[cfg(any(target_os = "linux", windows))]
    pub fn show(&mut self, notice: &DesktopNotification) -> io::Result<()> {
        self.backend.show(notice).map_err(io::Error::other)
    }

    #[cfg(not(any(target_os = "linux", windows)))]
    pub fn show(&mut self, _: &DesktopNotification) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native desktop notifications are unavailable on this platform",
        ))
    }
}
