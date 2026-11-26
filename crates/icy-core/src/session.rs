use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::agent_status::AgentStatus;
use serde::{Deserialize, Serialize};

use crate::graphics::{
    ApcSplitter, CursorContext, GraphicsState, ImageData, Segment, VisibleImage,
};
use crate::platform;
use crate::pty::{PtyHandle, PtyWriterSlot};
use crate::vt::{AlacrittyVt, CellPoint, Screen, SelectKind, VtCallbacks, VtEngine, wheel_input};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: u64,
    pub title: String,
    pub cli: String,
    pub cwd: PathBuf,
    pub alive: bool,
    pub status: AgentStatus,
}

pub struct Session {
    pub id: u64,
    default_title: String,
    cli_hint: Option<String>,
    dynamic_title: Arc<Mutex<Option<String>>>,
    pub cwd: PathBuf,
    vt: Arc<Mutex<Box<dyn VtEngine>>>,
    graphics: Arc<Mutex<GraphicsState>>,
    splitter: Arc<Mutex<ApcSplitter>>,
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
        let mut command = portable_pty::CommandBuilder::new(program);
        command.args(args);
        Self::spawn_configured(id, title, cwd, command, (cols, rows), on_update)
    }

    pub(crate) fn spawn_configured(
        id: u64,
        title: impl Into<String>,
        cwd: PathBuf,
        command: portable_pty::CommandBuilder,
        (cols, rows): (u16, u16),
        on_update: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        let writer_slot: PtyWriterSlot = Arc::new(Mutex::new(None));
        let dynamic_title: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let reply_slot = Arc::clone(&writer_slot);
        let graphics_reply_slot = Arc::clone(&writer_slot);
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

        let graphics = Arc::new(Mutex::new(GraphicsState::default()));
        let splitter = Arc::new(Mutex::new(ApcSplitter::default()));
        let vt_feed = Arc::clone(&vt);
        let graphics_feed = Arc::clone(&graphics);
        let splitter_feed = Arc::clone(&splitter);
        let notify_out = Arc::clone(&on_update);
        let alive_exit = Arc::clone(&alive);
        let notify_exit = Arc::clone(&on_update);

        let pty = PtyHandle::spawn(
            command,
            cols,
            rows,
            Some(&cwd),
            writer_slot,
            move |bytes| {
                let reply = feed_with_graphics(&vt_feed, &graphics_feed, &splitter_feed, bytes);
                if let Some(reply) = reply
                    && let Ok(mut guard) = graphics_reply_slot.lock()
                    && let Some(writer) = guard.as_mut()
                {
                    let _ = writer.write_all(&reply);
                    let _ = writer.flush();
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
            cli_hint: None,
            dynamic_title,
            cwd,
            vt,
            graphics,
            splitter,
            pty,
            alive,
        })
    }

    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        self.pty.write(bytes)
    }

    /// Keyboard input: like any terminal, typing returns a scrolled-back view to the live output.
    /// Returns whether the view moved (the screen needs a new push).
    pub fn write_input(&self, bytes: &[u8]) -> io::Result<bool> {
        let moved = self
            .vt
            .lock()
            .map(|mut vt| {
                // Typing ends the selection, as in other terminals.
                let selected = vt.selection_text().is_some();
                vt.clear_selection();
                vt.scroll_to_bottom() || selected
            })
            .unwrap_or(false);
        self.pty.write(bytes)?;
        Ok(moved)
    }

    /// Stage text in the selected CLI, never pressing Enter. If the terminal isn't in
    /// bracketed-paste mode, the caller must offer a manual paste instead of sending raw lines.
    pub fn paste_context(&self, text: &str) -> io::Result<()> {
        let bytes = context_paste(text)?;
        if !self.is_alive() || !self.vt.lock().unwrap().bracketed_paste() {
            return Err(io::Error::other(
                "This session is not accepting bracketed paste. Copy the context and paste it manually.",
            ));
        }
        self.write_input(&bytes)?;
        Ok(())
    }

    /// Clipboard paste is terminal input, not a series of key presses. In a TUI,
    /// a multiline clipboard must arrive as one paste rather than submitting each line.
    pub fn paste_clipboard(&self, text: &str) -> io::Result<bool> {
        let bracketed = self.vt.lock().unwrap().bracketed_paste();
        let bytes = encode_paste(text, bracketed);
        self.write_input(&bytes)
    }

    /// The program turned on the kitty keyboard protocol.
    pub fn kitty_keys(&self) -> bool {
        self.vt.lock().unwrap().kitty_keys()
    }

    pub fn start_selection(&self, kind: SelectKind, at: CellPoint) {
        self.vt.lock().unwrap().start_selection(kind, at);
    }

    pub fn extend_selection(&self, to: CellPoint) {
        self.vt.lock().unwrap().extend_selection(to);
    }

    pub fn clear_selection(&self) {
        self.vt.lock().unwrap().clear_selection();
    }

    pub fn selection_text(&self) -> Option<String> {
        self.vt.lock().unwrap().selection_text()
    }

    /// Mouse wheel over this terminal (`lines` > 0 up) at cell `col`, `row`: to the program when it
    /// takes the wheel (mouse reports, or arrows on the alternate screen), otherwise through the
    /// history. Returns whether the view moved.
    pub fn wheel(&self, lines: i32, col: u16, row: u16) -> io::Result<bool> {
        let input = {
            let mut vt = self.vt.lock().unwrap();
            match wheel_input(vt.wheel_modes(), lines, col, row) {
                Some(bytes) => bytes,
                None => return Ok(vt.scroll_display(lines)),
            }
        };
        self.pty.write(&input)?;
        Ok(false)
    }

    pub fn scroll_to_bottom(&self) -> bool {
        self.vt
            .lock()
            .map(|mut vt| vt.scroll_to_bottom())
            .unwrap_or(false)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<bool> {
        let mut vt = self.vt.lock().unwrap();
        let size = (cols.max(2), rows.max(1));
        if vt.size() == size {
            return Ok(false);
        }
        self.pty.resize(size.0, size.1)?;
        vt.resize(size.0, size.1);
        Ok(true)
    }

    pub fn snapshot(&self) -> Screen {
        self.vt.lock().unwrap().snapshot()
    }

    pub fn update_sync_timeout(
        &self,
        now: std::time::Instant,
    ) -> (bool, Option<std::time::Instant>) {
        let mut vt = self.vt.lock().unwrap();
        (vt.flush_sync_due(now), vt.sync_deadline())
    }

    pub fn with_cli_hint(mut self, cli: String) -> Self {
        self.cli_hint = Some(cli);
        self
    }

    /// Feeds bytes directly into the VT state without writing to the PTY (used for offline previews).
    pub fn feed_local(&self, bytes: &[u8]) {
        let _ = feed_with_graphics(&self.vt, &self.graphics, &self.splitter, bytes);
    }

    /// Inline images on screen (kitty graphics protocol), in screen cells.
    pub fn visible_images(&self, screen_rows: u16) -> Vec<VisibleImage> {
        // The first visible line: the live screen top, minus the lines scrolled back.
        let top = {
            let vt = self.vt.lock().unwrap();
            vt.screen_top_line() - vt.display_offset() as i64
        };
        self.graphics.lock().unwrap().visible(top, screen_rows)
    }

    /// Image textures transmitted since the last call, keyed by their unique sequence number.
    pub fn take_image_uploads(&self) -> Vec<(u64, ImageData)> {
        self.graphics.lock().unwrap().take_uploads()
    }

    pub(crate) fn serve_images_on_demand(&self) {
        self.graphics.lock().unwrap().serve_images_on_demand();
    }

    pub(crate) fn image(&self, seq: u64) -> Option<ImageData> {
        self.graphics.lock().unwrap().image(seq)
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
        let alive = self.is_alive();
        let foreground = self.pty.foreground_cmdline();
        let mut vt = self.vt.lock().unwrap();
        // A known foreground process is authoritative (a shell is "terminal" even if its title
        // mentions an agent, e.g. a path); titles are only a fallback where it can't be read.
        let cli = match foreground {
            Some(cmd) if cli_from_cmdline(&cmd).is_some() => cli_from_cmdline(&cmd).unwrap().into(),
            Some(cmd) if is_shell(&cmd) => {
                vt.clear_agent_status();
                "terminal".into()
            }
            _ => self
                .cli_hint
                .clone()
                .or_else(|| {
                    vt.reported_app()
                        .and_then(cli_from_cmdline)
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| infer_cli_kind(&title)),
        };
        let status = if alive && cli != "terminal" {
            vt.agent_status(&cli, &title)
        } else {
            AgentStatus::Idle
        };
        SessionInfo {
            id: self.id,
            title,
            cli,
            cwd: self.cwd.clone(),
            alive,
            status,
        }
    }

    pub fn kill(&self) {
        self.pty.kill();
    }

    pub(crate) fn reap_if_exited(&self) {
        if !self.is_alive() {
            self.pty.reap_if_exited();
        }
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }
}

fn context_paste(text: &str) -> io::Result<Vec<u8>> {
    if text.trim().is_empty() || text.len() > 32 * 1024 {
        return Err(io::Error::other(
            "Select a non-empty snippet of at most 32 KiB.",
        ));
    }
    Ok(encode_paste(text, true))
}

fn encode_paste(text: &str, bracketed: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len() + 12);
    if bracketed {
        bytes.extend_from_slice(b"\x1b[200~");
    }
    for mut ch in text.replace("\r\n", "\n").chars() {
        if ch == '\n' && !bracketed {
            ch = '\r';
        }
        if ch == '\r' && !bracketed {
            bytes.push(b'\r');
            continue;
        }
        if ch == '\n' || ch == '\t' || !ch.is_control() {
            bytes.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
        }
    }
    if bracketed {
        bytes.extend_from_slice(b"\x1b[201~");
    }
    bytes
}

#[cfg(test)]
#[test]
fn context_is_one_paste_without_embedded_control_sequences_or_enter() {
    assert_eq!(
        context_paste("a\r\n\x1b[201~雪\r").unwrap(),
        "\x1b[200~a\n[201~雪\x1b[201~".as_bytes()
    );
    assert!(context_paste(&"x".repeat(32 * 1024 + 1)).is_err());
}

/// Splits kitty graphics APCs out of PTY output: VT bytes go to the engine in order, each APC is
/// applied at the cursor position of that moment. Returns bytes to answer on the PTY.
fn feed_with_graphics(
    vt: &Mutex<Box<dyn VtEngine>>,
    graphics: &Mutex<GraphicsState>,
    splitter: &Mutex<ApcSplitter>,
    bytes: &[u8],
) -> Option<Vec<u8>> {
    let segments = splitter.lock().ok()?.push(bytes);
    let mut vt = vt.lock().ok()?;
    let mut g = graphics.lock().ok()?;
    let mut replies = Vec::new();
    for segment in segments {
        match segment {
            Segment::Bytes(chunk) => {
                g.observe_vt_bytes(&chunk, vt.screen_top_line());
                vt.feed(&chunk);
            }
            Segment::Apc(apc) => {
                let (abs_line, col) = vt.cursor_abs();
                let result = g.apply(&apc, CursorContext { abs_line, col });
                if let Some(move_cursor) = result.vt {
                    vt.feed(&move_cursor);
                }
                if let Some(reply) = result.reply {
                    replies.extend_from_slice(&reply);
                }
            }
        }
    }
    (!replies.is_empty()).then_some(replies)
}

/// Maps a foreground argv (NUL-separated) to a known agent CLI id. Checks the program and, for
/// script launchers (`node`, `bun`, `python`), its first argument.
pub fn cli_from_cmdline(cmdline: &str) -> Option<&'static str> {
    cmdline
        .split('\0')
        .filter(|a| !a.is_empty())
        .take(2)
        .find_map(|arg| {
            let base = arg.rsplit(['/', '\\']).next().unwrap_or(arg);
            let base = base
                .strip_suffix(".exe")
                .or_else(|| base.strip_suffix(".js"))
                .unwrap_or(base);
            match base {
                "claude" => Some("claude"),
                "codex" => Some("codex"),
                "gemini" => Some("gemini"),
                "grok" | "grok-build" => Some("grok"),
                "opencode" | "opencode2" | "open-code" => Some("opencode"),
                "junie" => Some("junie"),
                "agy" | "antigravity" | "antigravity-cli" => Some("antigravity"),
                _ => None,
            }
        })
}

fn is_shell(cmd: &str) -> bool {
    let program = cmd.split('\0').next().unwrap_or_default();
    let program = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .trim_start_matches('-');
    matches!(
        program,
        "sh" | "bash"
            | "dash"
            | "fish"
            | "zsh"
            | "nu"
            | "ksh"
            | "cmd.exe"
            | "pwsh.exe"
            | "powershell.exe"
    )
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
    fn maps_foreground_cmdlines_to_cli_ids() {
        assert_eq!(cli_from_cmdline("codex\0--model\0x\0"), Some("codex"));
        assert_eq!(
            cli_from_cmdline("node\0/usr/lib/node_modules/.bin/claude\0"),
            Some("claude")
        );
        assert_eq!(
            cli_from_cmdline("C:\\tools\\opencode.exe\0"),
            Some("opencode")
        );
        assert_eq!(cli_from_cmdline("/usr/bin/fish\0"), None);
        assert_eq!(cli_from_cmdline("python3\0script.py\0codex\0"), None);
        assert_eq!(cli_from_cmdline(""), None);
    }

    #[test]
    fn anchors_kitty_image_at_the_cursor_inside_a_synchronized_redraw() {
        let vt: Mutex<Box<dyn VtEngine>> = Mutex::new(Box::new(AlacrittyVt::new(80, 24)));
        let graphics = Mutex::new(GraphicsState::default());
        let splitter = Mutex::new(ApcSplitter::default());
        // Codex saves the input cursor, moves to the thumbnail's cells, and restores the cursor.
        feed_with_graphics(
            &vt,
            &graphics,
            &splitter,
            b"\x1b[22;72H\x1b[?2026h\x1b7\x1b[18;3H\x1b_Ga=T,i=49408,f=24,s=1",
        );
        feed_with_graphics(
            &vt,
            &graphics,
            &splitter,
            b",v=1,c=6,r=3,q=2,C=1;AAAA\x1b\\\x1b8",
        );
        let images = graphics.lock().unwrap().visible(0, 24);
        assert_eq!(images.len(), 1);
        assert_eq!((images[0].row, images[0].col), (17, 2));
        let pending_cursor = vt.lock().unwrap().snapshot().cursor;
        assert_eq!((pending_cursor.row, pending_cursor.col), (17, 2));

        feed_with_graphics(&vt, &graphics, &splitter, b"\x1b[?2026l");
        let screen = vt.lock().unwrap().snapshot();
        assert_eq!((screen.cursor.row, screen.cursor.col), (21, 71));
    }

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
