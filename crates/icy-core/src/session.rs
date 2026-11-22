use std::io;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;

use crate::platform;
use crate::pty::PtyHandle;
use crate::vt::{AlacrittyVt, Screen, VtEngine};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SessionInfo {
    pub id: u64,
    pub title: String,
    pub cwd: PathBuf,
    pub alive: bool,
}

pub struct Session {
    pub id: u64,
    pub title: String,
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
        let vt: Arc<Mutex<Box<dyn VtEngine>>> =
            Arc::new(Mutex::new(Box::new(AlacrittyVt::new(cols, rows))));
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
            title: title.into(),
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

    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            id: self.id,
            title: self.title.clone(),
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
}
