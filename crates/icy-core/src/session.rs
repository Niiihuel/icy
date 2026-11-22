use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;

use crate::platform;
use crate::pty::{PtyHandle, PtyWriterSlot};
use crate::vt::{AlacrittyVt, Screen, VtCallbacks, VtEngine};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SessionInfo {
    pub id: u64,
    pub title: String,
    pub cli: String,
    pub cwd: PathBuf,
    pub alive: bool,
}

pub struct Session {
    pub id: u64,
    default_title: String,
    dynamic_title: Arc<Mutex<Option<String>>>,
    pub cwd: PathBuf,
    vt: Arc<Mutex<Box<dyn VtEngine>>>,
    pty: PtyHandle,
    alive: Arc<AtomicBool>,
}

impl Session {
    pub fn spawn_shell(
        id: u64,
        title: impl Into<String>,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        on_update: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        let (program, args) = platform::default_shell();
        Self::spawn_command(id, title, cwd, &program, &args, cols, rows, on_update)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_command(
        id: u64,
        title: impl Into<String>,
        cwd: PathBuf,
        program: &str,
        args: &[String],
        cols: u16,
        rows: u16,
        on_update: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        let writer_slot: PtyWriterSlot = Arc::new(Mutex::new(None));
        let dynamic_title: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let reply_slot = Arc::clone(&writer_slot);
        let title_slot = Arc::clone(&dynamic_title);
        let title_notify = Arc::clone(&on_update);

        let callbacks = VtCallbacks {
            on_pty_write: Some(Arc::new(move |bytes: &[u8]| {
                if let Ok(mut guard) = reply_slot.lock()
                    && let Some(writer) = guard.as_mut()
                {
                    let _ = writer.write_all(bytes);
                    let _ = writer.flush();
                }
            })),
            on_title: Some(Arc::new(move |next: Option<String>| {
                if let Ok(mut guard) = title_slot.lock() {
                    let cleaned = next.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
                    if *guard != cleaned {
                        *guard = cleaned;
                        title_notify();
                    }
                }
            })),
        };

        let vt: Arc<Mutex<Box<dyn VtEngine>>> = Arc::new(Mutex::new(Box::new(
            AlacrittyVt::with_callbacks(cols, rows, callbacks),
        )));
        let alive = Arc::new(AtomicBool::new(true));

        let vt_feed = Arc::clone(&vt);
        let notify_out = Arc::clone(&on_update);
        let alive_exit = Arc::clone(&alive);
        let notify_exit = Arc::clone(&on_update);

        let pty = PtyHandle::spawn(
            program,
            args,
            cols,
            rows,
            Some(&cwd),
            writer_slot,
            move |bytes| {
                if let Ok(mut guard) = vt_feed.lock() {
                    guard.feed(bytes);
                }
                notify_out();
            },
            move || {
                alive_exit.store(false, Ordering::Release);
                notify_exit();
            },
        )?;

        Ok(Self {
            id,
            default_title: title.into(),
            dynamic_title,
            cwd,
            vt,
            pty,
            alive,
        })
    }

    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        self.pty.write(bytes)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        if let Ok(mut guard) = self.vt.lock() {
            guard.resize(cols, rows);
        }
        self.pty.resize(cols, rows)
    }

    pub fn snapshot(&self) -> Screen {
        self.vt.lock().unwrap().snapshot()
    }

    /// Feeds bytes directly into the VT state without writing to the PTY (used for offline previews).
    pub fn feed_local(&self, bytes: &[u8]) {
        if let Ok(mut guard) = self.vt.lock() {
            guard.feed(bytes);
        }
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    pub fn title(&self) -> String {
        self.dynamic_title
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_else(|| self.default_title.clone())
    }

    pub fn info(&self) -> SessionInfo {
        let title = self.title();
        let cli = infer_cli_kind(&title);
        SessionInfo {
            id: self.id,
            title,
            cli,
            cwd: self.cwd.clone(),
            alive: self.is_alive(),
        }
    }

    pub fn kill(&self) {
        self.pty.kill();
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }
}

fn infer_cli_kind(title: &str) -> String {
    let lower = title.to_ascii_lowercase();
    if lower.contains("claude") {
        "claude".into()
    } else if lower.contains("codex") {
        "codex".into()
    } else if lower.contains("grok") {
        "grok".into()
    } else if lower.contains("junie") {
        "junie".into()
    } else if lower.contains("opencode") {
        "opencode".into()
    } else if lower.contains("antigravity") {
        "antigravity".into()
    } else {
        "terminal".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn spawns_echo_command_and_captures_output_in_vt() {
        let (tx, rx) = mpsc::channel();
        let notify = Arc::new(move || {
            let _ = tx.send(());
        });
        let (program, args) = platform::echo_command("hello-icy");
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let session = Session::spawn_command(1, "echo", cwd, &program, &args, 80, 24, notify)
            .expect("spawn echo command");

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let screen = session.snapshot();
            let text: String = screen
                .rows
                .iter()
                .map(|r| r.plain_text())
                .collect::<Vec<_>>()
                .join("\n");
            if text.contains("hello-icy") {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "timed out waiting for hello-icy in screen:\n{text}"
            );
            let _ = rx.recv_timeout(remaining.min(Duration::from_millis(50)));
        }
    }

    #[test]
    fn vt_replies_to_primary_device_attributes_query() {
        let replies = Arc::new(Mutex::new(Vec::<u8>::new()));
        let sink = Arc::clone(&replies);
        let mut vt = AlacrittyVt::with_callbacks(
            80,
            24,
            VtCallbacks {
                on_pty_write: Some(Arc::new(move |bytes| {
                    sink.lock().unwrap().extend_from_slice(bytes);
                })),
                on_title: None,
            },
        );
        // Primary Device Attributes (DA1) query sent by fish and other shells at startup.
        vt.feed(b"\x1b[c");
        let reply = String::from_utf8_lossy(&replies.lock().unwrap()).to_string();
        assert!(
            reply.starts_with("\x1b[?"),
            "expected DA1 reply, got {reply:?}"
        );
    }
}
