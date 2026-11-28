//! System tray indicator for the background session server (StatusNotifierItem on Linux,
//! `Shell_NotifyIconW` on Windows).

use std::sync::Arc;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// Straight RGBA8 icon bitmap rendered by the host application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl TrayIcon {
    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.rgba.len() == (self.width as usize) * (self.height as usize) * 4
    }

    /// Network byte order ARGB32 pixels (`[A, R, G, B]`) expected by StatusNotifierItem.
    #[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
    pub fn to_argb_be(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.rgba.len());
        for px in self.rgba.as_chunks::<4>().0 {
            out.extend_from_slice(&[px[3], px[0], px[1], px[2]]);
        }
        out
    }

    /// BGRA8 pixels (`[B, G, R, A]`) expected by 32-bit Win32 DIB bitmaps.
    #[cfg_attr(not(any(windows, test)), allow(dead_code))]
    pub fn to_bgra(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.rgba.len());
        for px in self.rgba.as_chunks::<4>().0 {
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
        out
    }
}

/// Live session-server summary displayed in the tray tooltip and context menu.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayState {
    pub active_sessions: usize,
    pub total_sessions: usize,
    pub window_open: bool,
}

impl TrayState {
    pub fn status_label(&self) -> String {
        let sessions = match self.active_sessions {
            0 => "No active sessions".to_string(),
            1 => "1 active session".to_string(),
            count => format!("{count} active sessions"),
        };
        if self.window_open {
            sessions
        } else {
            format!("Running in background · {sessions}")
        }
    }

    pub fn tooltip(&self) -> String {
        format!("icy — {}", self.status_label())
    }
}

pub struct TrayCallbacks {
    pub on_open: Arc<dyn Fn() + Send + Sync>,
    pub on_quit: Arc<dyn Fn() + Send + Sync>,
}

#[cfg(target_os = "linux")]
type Inner = linux::LinuxTray;

#[cfg(windows)]
type Inner = windows::WindowsTray;

#[cfg(not(any(target_os = "linux", windows)))]
struct Inner;

/// Best-effort system tray icon handle. Dropping it removes the icon cleanly.
pub struct TrayHandle {
    #[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
    inner: Option<Inner>,
}

impl TrayHandle {
    pub fn start(icon: Option<TrayIcon>, initial: TrayState, callbacks: TrayCallbacks) -> Self {
        if std::env::var_os("ICY_NO_TRAY").is_some_and(|value| !value.is_empty() && value != "0") {
            return Self { inner: None };
        }
        let icon = icon.filter(TrayIcon::is_valid);
        #[cfg(target_os = "linux")]
        let inner = linux::LinuxTray::start(icon, initial, callbacks);
        #[cfg(windows)]
        let inner = windows::WindowsTray::start(icon, initial, callbacks);
        #[cfg(not(any(target_os = "linux", windows)))]
        let inner = {
            let _ = (icon, initial, callbacks);
            None
        };
        Self { inner }
    }

    pub fn update(&self, state: TrayState) {
        #[cfg(any(target_os = "linux", windows))]
        if let Some(inner) = &self.inner {
            inner.update(state);
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        let _ = state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_tray_status_and_tooltip_for_window_and_background_states() {
        let idle_bg = TrayState {
            active_sessions: 0,
            total_sessions: 0,
            window_open: false,
        };
        assert_eq!(
            idle_bg.status_label(),
            "Running in background · No active sessions"
        );
        assert_eq!(
            idle_bg.tooltip(),
            "icy — Running in background · No active sessions"
        );

        let one_bg = TrayState {
            active_sessions: 1,
            total_sessions: 2,
            window_open: false,
        };
        assert_eq!(
            one_bg.status_label(),
            "Running in background · 1 active session"
        );

        let many_open = TrayState {
            active_sessions: 3,
            total_sessions: 3,
            window_open: true,
        };
        assert_eq!(many_open.status_label(), "3 active sessions");
        assert_eq!(many_open.tooltip(), "icy — 3 active sessions");
    }

    #[test]
    fn converts_rgba_tray_icon_for_linux_and_windows_pixel_layouts() {
        let icon = TrayIcon {
            width: 2,
            height: 1,
            rgba: vec![0x10, 0x20, 0x30, 0x40, 0xaa, 0xbb, 0xcc, 0xff],
        };
        assert!(icon.is_valid());
        assert_eq!(
            icon.to_argb_be(),
            vec![0x40, 0x10, 0x20, 0x30, 0xff, 0xaa, 0xbb, 0xcc]
        );
        assert_eq!(
            icon.to_bgra(),
            vec![0x30, 0x20, 0x10, 0x40, 0xcc, 0xbb, 0xaa, 0xff]
        );
        assert!(
            !TrayIcon {
                width: 2,
                height: 2,
                rgba: vec![0; 4],
            }
            .is_valid()
        );
    }
}
