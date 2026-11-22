#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod icons;
mod usage;
mod view;
mod window;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use icy_core::links::{self, LinkRef, RefKind, ResolvedTarget};
use icy_core::session::{Session, SessionInfo};
use icy_core::vt::DEFAULT_BG;
use jsnir::host::Host;
use jsnir::text::TextEngine;
use serde::Serialize;
use serde_json::Value;

use crate::usage::CliUsage;
use crate::window::{Pusher, WindowHooks, WindowShortcut};

const APP: &str = include_str!(concat!(env!("OUT_DIR"), "/app.js"));
const GEIST_MONO_TTF: &[u8] = include_bytes!("../../../assets/fonts/GeistMono-Regular.ttf");
const INTER_REGULAR_TTF: &[u8] = include_bytes!("../../../../Jurui/assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD_TTF: &[u8] =
    include_bytes!("../../../../Jurui/assets/fonts/Inter-SemiBold.ttf");

const DEFAULT_COLS: u16 = 120;
const DEFAULT_ROWS: u16 = 38;
const MAX_VIEWER_LINES: usize = 400;
const SCREEN_FRAME_MIN_INTERVAL: Duration = Duration::from_millis(16);

// Geist Mono 13px metrics (lineHeight 1.4 -> 18.2px, advance ~7.82px).
const CELL_WIDTH_PX: f32 = 7.82;
const CELL_HEIGHT_PX: f32 = 18.2;
const SIDEBAR_WIDTH_PX: f32 = 220.0;
const VIEWER_WIDTH_PX: f32 = 460.0;
const TAB_BAR_HEIGHT_PX: f32 = 42.0;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SpanPayload {
    text: String,
    fg: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    bg: Option<u32>,
    bold: bool,
    underline: bool,
    link: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LinkPayload {
    kind: RefKind,
    target: String,
    line: Option<u32>,
    col: Option<u32>,
    resolved: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RowPayload {
    idx: usize,
    plain: String,
    spans: Vec<SpanPayload>,
    links: Vec<LinkPayload>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScreenPayload {
    cols: u16,
    rows_count: u16,
    cursor_row: u16,
    cursor_col: u16,
    cursor_visible: bool,
    rows: Vec<RowPayload>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionsPayload {
    active_id: u64,
    archived_count: u32,
    items: Vec<SessionInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewerLine {
    num: usize,
    text: String,
    highlighted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewerPayload {
    open: bool,
    path: String,
    display_path: String,
    target_line: Option<u32>,
    target_col: Option<u32>,
    error: Option<String>,
    lines: Vec<ViewerLine>,
}

impl ViewerPayload {
    fn closed() -> Self {
        Self {
            open: false,
            path: String::new(),
            display_path: String::new(),
            target_line: None,
            target_col: None,
            error: None,
            lines: Vec::new(),
        }
    }
}

enum BridgeEvent {
    Hello,
    PtyDirty,
    SessionsDirty,
    PushViewer(ViewerPayload),
    PushNotice { kind: String, text: String },
    ScaleChanged(f32),
}

struct SessionManager {
    sessions: Vec<Session>,
    active_id: u64,
    next_id: u64,
    archived_count: u32,
    cols: u16,
    rows: u16,
    viewport_w: f32,
    viewport_h: f32,
    sidebar_open: bool,
    viewer_open: bool,
    cwd: PathBuf,
    on_pty_update: Arc<dyn Fn() + Send + Sync>,
}

impl SessionManager {
    fn new(cwd: PathBuf, on_pty_update: Arc<dyn Fn() + Send + Sync>) -> Result<Self, String> {
        let initial = Session::spawn_shell(
            1,
            "shell-1",
            cwd.clone(),
            DEFAULT_COLS,
            DEFAULT_ROWS,
            Arc::clone(&on_pty_update),
        )
        .map_err(|e| format!("failed to spawn initial shell: {e}"))?;
        Ok(Self {
            sessions: vec![initial],
            active_id: 1,
            next_id: 2,
            archived_count: 0,
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            viewport_w: 1280.0,
            viewport_h: 800.0,
            sidebar_open: true,
            viewer_open: false,
            cwd,
            on_pty_update,
        })
    }

    fn active(&self) -> Option<&Session> {
        self.sessions
            .iter()
            .find(|s| s.id == self.active_id)
            .or_else(|| self.sessions.first())
    }

    fn create_session(&mut self) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;
        let title = format!("shell-{id}");
        let session = Session::spawn_shell(
            id,
            title,
            self.cwd.clone(),
            self.cols,
            self.rows,
            Arc::clone(&self.on_pty_update),
        )
        .map_err(|e| format!("failed to spawn shell: {e}"))?;
        self.sessions.push(session);
        self.active_id = id;
        Ok(id)
    }

    fn close_session(&mut self, id: u64) -> bool {
        if self.sessions.len() <= 1 {
            return false;
        }
        let Some(pos) = self.sessions.iter().position(|s| s.id == id) else {
            return false;
        };
        let removed = self.sessions.remove(pos);
        removed.kill();
        self.archived_count = self.archived_count.saturating_add(1);
        if self.active_id == id {
            let next_idx = pos.min(self.sessions.len() - 1);
            self.active_id = self.sessions[next_idx].id;
        }
        true
    }

    fn step_tab(&mut self, delta: isize) -> bool {
        if self.sessions.len() <= 1 {
            return false;
        }
        let cur = self
            .sessions
            .iter()
            .position(|s| s.id == self.active_id)
            .unwrap_or(0) as isize;
        let len = self.sessions.len() as isize;
        let next = (cur + delta).rem_euclid(len) as usize;
        self.active_id = self.sessions[next].id;
        true
    }

    fn select_session(&mut self, id: u64) -> bool {
        if self.sessions.iter().any(|s| s.id == id) {
            self.active_id = id;
            let _ = self.apply_grid_size();
            true
        } else {
            false
        }
    }

    fn update_viewport(&mut self, width: f32, height: f32) -> bool {
        self.viewport_w = width;
        self.viewport_h = height;
        self.recompute_grid_size()
    }

    fn update_layout(&mut self, sidebar_open: bool, viewer_open: bool) -> bool {
        self.sidebar_open = sidebar_open;
        self.viewer_open = viewer_open;
        self.recompute_grid_size()
    }

    fn recompute_grid_size(&mut self) -> bool {
        let sidebar_w = if self.sidebar_open {
            SIDEBAR_WIDTH_PX
        } else {
            0.0
        };
        let viewer_w = if self.viewer_open {
            VIEWER_WIDTH_PX
        } else {
            0.0
        };
        let avail_w = (self.viewport_w - sidebar_w - viewer_w - 40.0).max(240.0);
        let avail_h = (self.viewport_h - TAB_BAR_HEIGHT_PX - 36.0).max(160.0);
        let cols = ((avail_w / CELL_WIDTH_PX).floor() as u16).clamp(40, 280);
        let rows = ((avail_h / CELL_HEIGHT_PX).floor() as u16).clamp(12, 120);
        if cols == self.cols && rows == self.rows {
            return false;
        }
        self.cols = cols;
        self.rows = rows;
        let _ = self.apply_grid_size();
        true
    }

    fn apply_grid_size(&self) -> Result<(), String> {
        for session in &self.sessions {
            session
                .resize(self.cols, self.rows)
                .map_err(|e| format!("resize failed: {e}"))?;
        }
        Ok(())
    }

    fn sessions_payload(&self) -> SessionsPayload {
        SessionsPayload {
            active_id: self.active_id,
            archived_count: self.archived_count,
            items: self.sessions.iter().map(Session::info).collect(),
        }
    }
}

fn install_fonts(host: &mut Host) {
    host.add_font(INTER_REGULAR_TTF.to_vec());
    host.add_font(INTER_SEMIBOLD_TTF.to_vec());
    host.add_font(GEIST_MONO_TTF.to_vec());
    host.set_default_family("Inter".into());
}

fn install_images_on_host(host: &mut Host, cli_usage: &[CliUsage], scale: f32) {
    for (name, rgba, size) in icons::load_static(scale) {
        host.set_image(name, rgba, size);
    }
    for cli in cli_usage {
        if let Some((name, rgba, size)) =
            icons::rasterize_cli_ring(&cli.id, &cli.color, cli.percent, scale)
        {
            host.set_image(name, rgba, size);
        }
    }
}

fn push_images(pusher: &Pusher, cli_usage: &[CliUsage], scale: f32) {
    for (name, rgba, size) in icons::load_static(scale) {
        let _ = pusher.image(name, rgba, size);
    }
    for cli in cli_usage {
        if let Some((name, rgba, size)) =
            icons::rasterize_cli_ring(&cli.id, &cli.color, cli.percent, scale)
        {
            let _ = pusher.image(name, rgba, size);
        }
    }
}

fn build_screen_payload(session: &Session) -> ScreenPayload {
    let screen = session.snapshot();
    let cwd = session.cwd();
    let mut rows = Vec::with_capacity(screen.rows.len());

    for (idx, row) in screen.rows.iter().enumerate() {
        let plain = row.plain_text();
        let detected = links::detect(&plain);

        let links: Vec<LinkPayload> = detected
            .iter()
            .map(|r| {
                let resolved = links::resolve(r, cwd, |p| p.is_file()).is_some();
                LinkPayload {
                    kind: r.kind,
                    target: r.target.clone(),
                    line: r.line,
                    col: r.col,
                    resolved,
                }
            })
            .collect();

        let mut spans = Vec::new();
        if !plain.is_empty() {
            let mut cursor = 0usize;
            for run in &row.runs {
                let run_start = cursor;
                let run_end = run_start + run.text.len();
                cursor = run_end;

                if detected.is_empty() {
                    spans.push(SpanPayload {
                        text: run.text.clone(),
                        fg: run.fg,
                        bg: (run.bg != DEFAULT_BG).then_some(run.bg),
                        bold: run.bold,
                        underline: run.underline,
                        link: false,
                    });
                    continue;
                }

                let mut cuts = BTreeSet::new();
                cuts.insert(run_start);
                cuts.insert(run_end);
                for r in &detected {
                    if r.start > run_start && r.start < run_end {
                        cuts.insert(r.start);
                    }
                    if r.end > run_start && r.end < run_end {
                        cuts.insert(r.end);
                    }
                }

                let points: Vec<usize> = cuts.into_iter().collect();
                for pair in points.windows(2) {
                    let (a, b) = (pair[0], pair[1]);
                    if a >= b {
                        continue;
                    }
                    let is_link = detected.iter().any(|r| a < r.end && b > r.start);
                    spans.push(SpanPayload {
                        text: plain[a..b].to_string(),
                        fg: run.fg,
                        bg: (run.bg != DEFAULT_BG).then_some(run.bg),
                        bold: run.bold,
                        underline: run.underline || is_link,
                        link: is_link,
                    });
                }
            }
        }

        rows.push(RowPayload {
            idx,
            plain,
            spans,
            links,
        });
    }

    ScreenPayload {
        cols: screen.cols,
        rows_count: screen.rows_count,
        cursor_row: screen.cursor.row,
        cursor_col: screen.cursor.col,
        cursor_visible: screen.cursor.visible,
        rows,
    }
}

fn open_file_for_viewer(
    raw_path: &str,
    line: Option<u32>,
    col: Option<u32>,
    cwd: &Path,
) -> ViewerPayload {
    let link_ref = LinkRef {
        start: 0,
        end: raw_path.len(),
        kind: RefKind::File,
        target: raw_path.to_string(),
        line,
        col,
    };

    let resolved = links::resolve(&link_ref, cwd, |p| p.is_file());
    let target_path = match resolved {
        Some(ResolvedTarget::File { path, .. }) => path,
        _ => {
            return ViewerPayload {
                open: true,
                path: raw_path.to_string(),
                display_path: raw_path.to_string(),
                target_line: line,
                target_col: col,
                error: Some(format!("File not found from {}", cwd.display())),
                lines: Vec::new(),
            };
        }
    };

    let display_path = target_path
        .strip_prefix(cwd)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| target_path.display().to_string());

    let content = match std::fs::read_to_string(&target_path) {
        Ok(text) => text,
        Err(err) => {
            return ViewerPayload {
                open: true,
                path: target_path.display().to_string(),
                display_path,
                target_line: line,
                target_col: col,
                error: Some(format!("Unable to read file: {err}")),
                lines: Vec::new(),
            };
        }
    };

    let all_lines: Vec<&str> = content.lines().collect();
    let total = all_lines.len();
    let target_1based = line.map(|l| l.max(1) as usize);

    let (start_idx, end_idx) = if total <= MAX_VIEWER_LINES {
        (0, total)
    } else if let Some(target) = target_1based {
        let center = target.saturating_sub(1).min(total.saturating_sub(1));
        let half = MAX_VIEWER_LINES / 2;
        let start = center.saturating_sub(half);
        let end = (start + MAX_VIEWER_LINES).min(total);
        (end.saturating_sub(MAX_VIEWER_LINES), end)
    } else {
        (0, MAX_VIEWER_LINES)
    };

    let lines = all_lines[start_idx..end_idx]
        .iter()
        .enumerate()
        .map(|(offset, text)| {
            let num = start_idx + offset + 1;
            ViewerLine {
                num,
                text: (*text).to_string(),
                highlighted: target_1based == Some(num),
            }
        })
        .collect();

    ViewerPayload {
        open: true,
        path: target_path.display().to_string(),
        display_path,
        target_line: line,
        target_col: col,
        error: None,
        lines,
    }
}

fn push_view<T: Serialize>(pusher: &Pusher, view: &str, data: &T) {
    let payload = serde_json::json!({ "view": view, "data": data });
    let _ = pusher.push(payload.to_string());
}

fn handle_request(
    raw: &str,
    manager: &Arc<Mutex<SessionManager>>,
    tx: &mpsc::Sender<BridgeEvent>,
    cwd: &Path,
) -> Result<String, String> {
    let payload: Value =
        serde_json::from_str(raw).map_err(|_| "invalid JSON request".to_string())?;
    let kind = payload
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing request type".to_string())?;

    match kind {
        "hello" => {
            let _ = tx.send(BridgeEvent::Hello);
            Ok("ok".into())
        }
        "open" => {
            let path = payload
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing path".to_string())?;
            let link_kind = payload
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("file");
            if link_kind == "url" {
                let _ = tx.send(BridgeEvent::PushNotice {
                    kind: "url".into(),
                    text: path.to_string(),
                });
                return Ok("ok".into());
            }
            let line = payload
                .get("line")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok());
            let col = payload
                .get("col")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok());
            let session_cwd = {
                let mut guard = manager.lock().unwrap();
                let sidebar = guard.sidebar_open;
                guard.update_layout(sidebar, true);
                guard
                    .active()
                    .map(|s| s.cwd().to_path_buf())
                    .unwrap_or_else(|| cwd.to_path_buf())
            };
            let viewer = open_file_for_viewer(path, line, col, &session_cwd);
            let _ = tx.send(BridgeEvent::PushViewer(viewer));
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok("ok".into())
        }
        "closeViewer" => {
            {
                let mut guard = manager.lock().unwrap();
                let sidebar = guard.sidebar_open;
                guard.update_layout(sidebar, false);
            }
            let _ = tx.send(BridgeEvent::PushViewer(ViewerPayload::closed()));
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok("ok".into())
        }
        "layout" => {
            let sidebar = payload
                .get("sidebarOpen")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let viewer = payload
                .get("viewerOpen")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let changed = manager.lock().unwrap().update_layout(sidebar, viewer);
            if changed {
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        "newSession" => {
            let mut guard = manager.lock().unwrap();
            guard.create_session()?;
            let _ = tx.send(BridgeEvent::SessionsDirty);
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok("ok".into())
        }
        "closeSession" => {
            let id = payload
                .get("id")
                .and_then(Value::as_u64)
                .ok_or_else(|| "missing session id".to_string())?;
            let mut guard = manager.lock().unwrap();
            if guard.close_session(id) {
                let _ = tx.send(BridgeEvent::SessionsDirty);
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        "selectSession" => {
            let id = payload
                .get("id")
                .and_then(Value::as_u64)
                .ok_or_else(|| "missing session id".to_string())?;
            let mut guard = manager.lock().unwrap();
            if guard.select_session(id) {
                let _ = tx.send(BridgeEvent::SessionsDirty);
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        other => Err(format!("unknown request type: {other}")),
    }
}

fn run_bridge(
    pusher: Pusher,
    rx: mpsc::Receiver<BridgeEvent>,
    manager: Arc<Mutex<SessionManager>>,
    cli_usage: Vec<CliUsage>,
) {
    let mut last_screen_push = Instant::now() - SCREEN_FRAME_MIN_INTERVAL;

    while let Ok(first) = rx.recv() {
        let mut need_sessions = false;
        let mut need_screen = false;
        let mut need_usage = false;
        let mut pending_viewer: Option<ViewerPayload> = None;
        let mut pending_notice: Option<(String, String)> = None;
        let mut pending_scale: Option<f32> = None;

        let mut apply_event = |ev: BridgeEvent| match ev {
            BridgeEvent::Hello => {
                need_sessions = true;
                need_screen = true;
                need_usage = true;
            }
            BridgeEvent::PtyDirty => {
                need_screen = true;
                need_sessions = true;
            }
            BridgeEvent::SessionsDirty => need_sessions = true,
            BridgeEvent::PushViewer(v) => pending_viewer = Some(v),
            BridgeEvent::PushNotice { kind, text } => pending_notice = Some((kind, text)),
            BridgeEvent::ScaleChanged(scale) => pending_scale = Some(scale),
        };

        apply_event(first);
        while let Ok(more) = rx.try_recv() {
            apply_event(more);
        }

        if let Some(scale) = pending_scale {
            push_images(&pusher, &cli_usage, scale);
        }
        if need_usage {
            push_view(&pusher, "cliUsage", &cli_usage);
        }
        if need_sessions {
            let payload = manager.lock().unwrap().sessions_payload();
            push_view(&pusher, "sessions", &payload);
        }
        if let Some(viewer) = pending_viewer {
            push_view(&pusher, "viewer", &viewer);
        }
        if let Some((kind, text)) = pending_notice {
            let data = serde_json::json!({ "kind": kind, "text": text });
            push_view(&pusher, "notice", &data);
        }
        if need_screen {
            let elapsed = last_screen_push.elapsed();
            if elapsed < SCREEN_FRAME_MIN_INTERVAL {
                std::thread::sleep(SCREEN_FRAME_MIN_INTERVAL - elapsed);
                while let Ok(more) = rx.try_recv() {
                    if let BridgeEvent::PushViewer(v) = more {
                        push_view(&pusher, "viewer", &v);
                    }
                }
            }
            let screen = {
                let guard = manager.lock().unwrap();
                guard.active().map(build_screen_payload)
            };
            if let Some(screen) = screen {
                push_view(&pusher, "screen", &screen);
                last_screen_push = Instant::now();
            }
        }
    }
}

fn run_offscreen(
    path: &Path,
    manager: &Arc<Mutex<SessionManager>>,
    cli_usage: &[CliUsage],
    mut host: Host,
) -> ExitCode {
    std::thread::sleep(Duration::from_millis(250));

    let (sessions_payload, screen_payload) = {
        let mut guard = manager.lock().unwrap();
        guard.update_viewport(1280.0, 800.0);
        let active = guard.active().expect("initial session");
        let snapshot = active.snapshot();
        let has_output = snapshot
            .rows
            .iter()
            .any(|row| !row.plain_text().trim().is_empty());
        if !has_output {
            active.feed_local(
                b"\x1b[1micy\x1b[0m \xc2\xb7 agent terminal workspace\r\n\
                  \x1b[90m$\x1b[0m cargo test --workspace\r\n\
                  \x1b[32m   Compiling\x1b[0m icy-core v0.1.0\r\n\
                  \x1b[36mnote\x1b[0m: click file link -> crates/icy-core/src/links.rs:36:1\r\n\
                  \x1b[90mpreview:\x1b[0m http://localhost:5173\r\n",
            );
        }
        (guard.sessions_payload(), build_screen_payload(active))
    };

    host.push(serde_json::json!({ "view": "cliUsage", "data": cli_usage }).to_string());
    host.push(serde_json::json!({ "view": "sessions", "data": sessions_payload }).to_string());
    host.push(serde_json::json!({ "view": "screen", "data": screen_payload }).to_string());

    match jsnir_gpu::capture_at(host, [1280, 800], 1.0, false, path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("icy: offscreen capture failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let offscreen_path = match args.as_slice() {
        [] => None,
        [flag, path] if flag == "--offscreen" => Some(PathBuf::from(path)),
        _ => {
            eprintln!("usage: icy [--offscreen OUTPUT.ppm]");
            return ExitCode::FAILURE;
        }
    };

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cli_usage = usage::collect_cli_usage();
    let (tx, rx) = mpsc::channel();

    let pty_tx = tx.clone();
    let on_pty_update: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let _ = pty_tx.send(BridgeEvent::PtyDirty);
    });

    let manager = match SessionManager::new(cwd.clone(), on_pty_update) {
        Ok(m) => Arc::new(Mutex::new(m)),
        Err(error) => {
            eprintln!("icy: {error}");
            return ExitCode::FAILURE;
        }
    };

    let req_manager = Arc::clone(&manager);
    let req_tx = tx.clone();
    let req_cwd = cwd.clone();
    let mut host = match Host::with_request_handler(
        APP.into(),
        "{}".into(),
        TextEngine::default(),
        move |json| handle_request(&json, &req_manager, &req_tx, &req_cwd),
    ) {
        Ok(h) => h,
        Err(error) => {
            eprintln!("icy: failed to load app host: {error}");
            return ExitCode::FAILURE;
        }
    };

    install_fonts(&mut host);
    install_images_on_host(&mut host, &cli_usage, 1.0);

    if let Some(path) = offscreen_path {
        return run_offscreen(&path, &manager, &cli_usage, host);
    }

    let key_manager = Arc::clone(&manager);
    let shortcut_manager = Arc::clone(&manager);
    let shortcut_tx = tx.clone();
    let viewport_manager = Arc::clone(&manager);
    let viewport_tx = tx.clone();

    let hooks = WindowHooks {
        on_pty_input: Box::new(move |bytes| {
            let guard = key_manager.lock().unwrap();
            if let Some(session) = guard.active() {
                let _ = session.write(bytes);
            }
        }),
        on_shortcut: Box::new(move |shortcut| {
            let mut guard = shortcut_manager.lock().unwrap();
            let changed = match shortcut {
                WindowShortcut::NewSession => guard.create_session().is_ok(),
                WindowShortcut::CloseSession => {
                    let active = guard.active_id;
                    guard.close_session(active)
                }
                WindowShortcut::NextTab => guard.step_tab(1),
                WindowShortcut::PrevTab => guard.step_tab(-1),
            };
            if changed {
                let _ = shortcut_tx.send(BridgeEvent::SessionsDirty);
                let _ = shortcut_tx.send(BridgeEvent::PtyDirty);
            }
        }),
        on_viewport: Box::new(move |w, h| {
            if viewport_manager.lock().unwrap().update_viewport(w, h) {
                let _ = viewport_tx.send(BridgeEvent::PtyDirty);
            }
        }),
    };

    let bridge_manager = Arc::clone(&manager);
    let scale_tx = tx.clone();
    let result = window::open_with(host, "icy", [1280, 800], hooks, |pusher| {
        pusher.on_scale(move |scale| {
            let _ = scale_tx.send(BridgeEvent::ScaleChanged(scale));
        });
        std::thread::Builder::new()
            .name("icy-ui-bridge".into())
            .spawn(move || run_bridge(pusher, rx, bridge_manager, cli_usage))
            .expect("spawn ui bridge thread");
    });

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("icy: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_tsx_app_mounts_and_renders_frame() {
        let mut host =
            Host::with_request_handler(APP.into(), "{}".into(), TextEngine::default(), |_| {
                Ok("ok".into())
            })
            .expect("load compiled TSX bundle");
        install_fonts(&mut host);
        let usage = usage::collect_cli_usage();
        install_images_on_host(&mut host, &usage, 1.0);
        host.poll();
        let frame = host.frame(1280., 800., 1., 4096);
        assert!(!frame.items.is_empty(), "expected mounted UI scene items");
    }

    #[test]
    fn viewer_opens_workspace_file_at_target_line() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.join("../..");
        let view = open_file_for_viewer("Cargo.toml", Some(1), None, &workspace_root);
        assert!(view.open);
        assert!(
            view.error.is_none(),
            "unexpected viewer error: {:?}",
            view.error
        );
        assert!(!view.lines.is_empty());
        assert!(view.lines[0].highlighted);
        assert_eq!(view.lines[0].num, 1);
    }
}
