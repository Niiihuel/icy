#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod icons;
mod usage;
mod view;
mod window;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use icy_core::config::{self, IcyConfig, ResolvedTheme};
use icy_core::github::{self, GitHubStatus};
use icy_core::links::{self, LinkRef, RefKind, ResolvedTarget};
use icy_core::platform;
use icy_core::session::{Session, SessionInfo};
use icy_core::tasks::{TaskItem, TaskStore};
use icy_core::tiling::{FocusDir, TilingMode, TilingView, TilingWorkspace};
use icy_core::updater::{self, UpdateState};
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
const SIDEBAR_WIDTH_PX: f32 = 224.0;
const VIEWER_WIDTH_PX: f32 = 460.0;
const TAB_BAR_HEIGHT_PX: f32 = 44.0;
const PANE_GAP_PX: f32 = 8.0;

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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FilesPayload {
    cwd: String,
    subdir: String,
    entries: Vec<FileEntry>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThemesPayload {
    themes_dir: String,
    items: Vec<ResolvedTheme>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TasksPayload {
    items: Vec<TaskItem>,
}

enum BridgeEvent {
    Hello,
    PtyDirty,
    SessionsDirty,
    TilingDirty,
    ConfigDirty,
    ThemesDirty,
    TasksDirty,
    GitHubUpdated(GitHubStatus),
    UpdateChanged(UpdateState),
    PushFiles(FilesPayload),
    PushViewer(ViewerPayload),
    PushNotice { kind: String, text: String },
    ScaleChanged(f32),
    QuitApp,
}

struct SessionManager {
    sessions: Vec<Session>,
    tiling: TilingWorkspace,
    config: IcyConfig,
    themes: Vec<ResolvedTheme>,
    tasks: TaskStore,
    github: GitHubStatus,
    update: UpdateState,
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
        let cfg = config::load_config();
        let themes = config::load_themes();
        let tasks = TaskStore::load();
        let initial = Session::spawn_shell(
            1,
            "shell-1",
            cwd.clone(),
            DEFAULT_COLS,
            DEFAULT_ROWS,
            Arc::clone(&on_pty_update),
        )
        .map_err(|e| format!("failed to spawn initial shell: {e}"))?;

        let tiling = TilingWorkspace::new(1, cfg.tiling_mode);

        Ok(Self {
            sessions: vec![initial],
            tiling,
            config: cfg,
            themes,
            tasks,
            github: GitHubStatus::default(),
            update: UpdateState::initial(),
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

    fn active_id(&self) -> u64 {
        self.tiling.focused_session()
    }

    fn active(&self) -> Option<&Session> {
        let active_id = self.active_id();
        self.sessions
            .iter()
            .find(|s| s.id == active_id)
            .or_else(|| self.sessions.first())
    }

    fn create_session(&mut self, stack_vertical: bool) -> Result<u64, String> {
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
        if stack_vertical {
            self.tiling.stack_vertical(id);
        } else {
            self.tiling.open_column(id);
        }
        let _ = self.apply_tiled_sizes();
        Ok(id)
    }

    fn launch_cli(&mut self, cli_id: &str) -> Result<u64, String> {
        let cli = self
            .config
            .clis
            .iter()
            .find(|c| c.id == cli_id)
            .cloned()
            .ok_or_else(|| format!("unknown CLI: {cli_id}"))?;

        let id = self.next_id;
        self.next_id += 1;

        let spawned = Session::spawn_command(
            id,
            cli.name.clone(),
            self.cwd.clone(),
            &cli.command,
            &cli.args,
            self.cols,
            self.rows,
            Arc::clone(&self.on_pty_update),
        );

        let session = match spawned {
            Ok(s) => s,
            Err(_) => {
                // Fallback to interactive shell with a clear hint if the CLI binary is not in PATH.
                let s = Session::spawn_shell(
                    id,
                    cli.name.clone(),
                    self.cwd.clone(),
                    self.cols,
                    self.rows,
                    Arc::clone(&self.on_pty_update),
                )
                .map_err(|e| format!("failed to launch {}: {e}", cli.name))?;
                let hint = format!(
                    "\x1b[33m[icy]\x1b[0m `{}` was not found in PATH — opened shell instead.\r\n",
                    cli.command
                );
                s.feed_local(hint.as_bytes());
                s
            }
        };

        self.sessions.push(session);
        self.tiling.open_column(id);
        let _ = self.apply_tiled_sizes();
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
        self.tiling.remove_session(id);
        let _ = self.apply_tiled_sizes();
        true
    }

    fn step_tab(&mut self, delta: isize) -> bool {
        if self.sessions.len() <= 1 {
            return false;
        }
        let cur = self
            .sessions
            .iter()
            .position(|s| s.id == self.active_id())
            .unwrap_or(0) as isize;
        let len = self.sessions.len() as isize;
        let next = (cur + delta).rem_euclid(len) as usize;
        let next_id = self.sessions[next].id;
        self.tiling.focus_session(next_id);
        let _ = self.apply_tiled_sizes();
        true
    }

    fn select_session(&mut self, id: u64) -> bool {
        if self.sessions.iter().any(|s| s.id == id) {
            self.tiling.focus_session(id);
            let _ = self.apply_tiled_sizes();
            true
        } else {
            false
        }
    }

    fn focus_dir(&mut self, dir: FocusDir) -> bool {
        let changed = self.tiling.focus_direction(dir);
        if changed {
            let _ = self.apply_tiled_sizes();
        }
        changed
    }

    fn toggle_maximize(&mut self) -> bool {
        self.tiling.toggle_maximize();
        let _ = self.apply_tiled_sizes();
        true
    }

    fn scroll_column(&mut self, delta: isize) -> bool {
        let changed = self.tiling.scroll_columns(delta);
        if changed {
            let _ = self.apply_tiled_sizes();
        }
        changed
    }

    fn set_tiling_mode(&mut self, mode: TilingMode) {
        self.tiling.set_mode(mode);
        self.config.tiling_mode = mode;
        let _ = config::save_config(&self.config);
        let _ = self.apply_tiled_sizes();
    }

    fn update_viewport(&mut self, width: f32, height: f32) -> bool {
        self.viewport_w = width;
        self.viewport_h = height;
        self.apply_tiled_sizes()
    }

    fn update_layout(&mut self, sidebar_open: bool, viewer_open: bool) -> bool {
        self.sidebar_open = sidebar_open;
        self.viewer_open = viewer_open;
        self.apply_tiled_sizes()
    }

    fn apply_tiled_sizes(&mut self) -> bool {
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
        let avail_w = (self.viewport_w - sidebar_w - viewer_w - 28.0).max(240.0);
        let avail_h = (self.viewport_h - TAB_BAR_HEIGHT_PX - 28.0).max(160.0);

        let scale = (self.config.font_size as f32 / 13.0).clamp(0.8, 1.5);
        let cell_w = CELL_WIDTH_PX * scale;
        let cell_h = CELL_HEIGHT_PX * scale;

        let snapshot = self.tiling.snapshot();
        let col_count = snapshot.visible_columns.len().max(1) as f32;
        let col_w = ((avail_w - (col_count - 1.0) * PANE_GAP_PX) / col_count).max(180.0);

        let base_cols = (((col_w - 24.0) / cell_w).floor() as u16).clamp(32, 280);
        let base_rows = (((avail_h - 16.0) / cell_h).floor() as u16).clamp(10, 120);
        let changed = base_cols != self.cols || base_rows != self.rows;
        self.cols = base_cols;
        self.rows = base_rows;

        for col in &snapshot.visible_columns {
            let pane_count = col.panes.len().max(1) as f32;
            let pane_h = ((avail_h - (pane_count - 1.0) * PANE_GAP_PX) / pane_count).max(110.0);
            let pane_rows = (((pane_h - 16.0) / cell_h).floor() as u16).clamp(8, 120);
            for &sid in &col.panes {
                if let Some(session) = self.sessions.iter().find(|s| s.id == sid) {
                    let _ = session.resize(base_cols, pane_rows);
                }
            }
        }
        changed
    }

    fn sessions_payload(&self) -> SessionsPayload {
        SessionsPayload {
            active_id: self.active_id(),
            archived_count: self.archived_count,
            items: self.sessions.iter().map(Session::info).collect(),
        }
    }

    fn tiling_snapshot(&self) -> TilingView {
        self.tiling.snapshot()
    }

    fn visible_screens(&self) -> BTreeMap<String, ScreenPayload> {
        let mut map = BTreeMap::new();
        for sid in self.tiling.visible_pane_ids() {
            if let Some(session) = self.sessions.iter().find(|s| s.id == sid) {
                map.insert(sid.to_string(), build_screen_payload(session));
            }
        }
        if map.is_empty()
            && let Some(active) = self.active()
        {
            map.insert(active.id.to_string(), build_screen_payload(active));
        }
        map
    }

    fn themes_payload(&self) -> ThemesPayload {
        ThemesPayload {
            themes_dir: config::themes_dir().display().to_string(),
            items: self.themes.clone(),
        }
    }

    fn tasks_payload(&self) -> TasksPayload {
        TasksPayload {
            items: self.tasks.items().to_vec(),
        }
    }
}

fn list_workspace_files(root: &Path, subdir: &str) -> FilesPayload {
    let clean_sub = subdir.trim_matches(['/', '\\']);
    let target = if clean_sub.is_empty() {
        root.to_path_buf()
    } else {
        let candidate = root.join(clean_sub);
        if candidate.starts_with(root) && candidate.is_dir() {
            candidate
        } else {
            root.to_path_buf()
        }
    };

    let mut entries = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&target) {
        for entry in rd.flatten().take(250) {
            let name = entry.file_name().to_string_lossy().to_string();
            if matches!(name.as_str(), ".git" | "target" | "node_modules") {
                continue;
            }
            let path = entry.path();
            let meta = entry.metadata().ok();
            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let rel = path
                .strip_prefix(root)
                .map(|p| p.display().to_string().replace('\\', "/"))
                .unwrap_or_else(|_| name.clone());
            entries.push(FileEntry {
                name,
                path: rel,
                is_dir,
                size,
            });
        }
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    FilesPayload {
        cwd: root.display().to_string(),
        subdir: clean_sub.to_string(),
        entries,
    }
}

fn install_fonts(host: &mut Host) {
    host.add_font(INTER_REGULAR_TTF.to_vec());
    host.add_font(INTER_SEMIBOLD_TTF.to_vec());
    host.add_font(GEIST_MONO_TTF.to_vec());
    host.set_default_family("Inter".into());
}

fn install_images_on_host(
    host: &mut Host,
    cli_usage: &[CliUsage],
    update: &UpdateState,
    scale: f32,
) {
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
    if let Some((name, rgba, size)) = icons::rasterize_update_ring(
        update.progress,
        update.has_update,
        matches!(update.stage.as_str(), "downloading" | "verifying"),
        update.stage == "ready",
        scale,
    ) {
        host.set_image(name, rgba, size);
    }
}

fn push_images(pusher: &Pusher, cli_usage: &[CliUsage], update: &UpdateState, scale: f32) {
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
    push_update_ring(pusher, update, scale);
}

fn push_update_ring(pusher: &Pusher, update: &UpdateState, scale: f32) {
    if let Some((name, rgba, size)) = icons::rasterize_update_ring(
        update.progress,
        update.has_update,
        matches!(update.stage.as_str(), "downloading" | "verifying"),
        update.stage == "ready",
        scale,
    ) {
        let _ = pusher.image(name, rgba, size);
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

fn spawn_github_probe(cwd: PathBuf, tx: mpsc::Sender<BridgeEvent>) {
    let _ = std::thread::Builder::new()
        .name("icy-github-probe".into())
        .spawn(move || {
            let status = github::detect_github(&cwd);
            let _ = tx.send(BridgeEvent::GitHubUpdated(status));
        });
}

fn spawn_update_check(tx: mpsc::Sender<BridgeEvent>) {
    let _ = std::thread::Builder::new()
        .name("icy-update-check".into())
        .spawn(move || {
            let state = updater::check_latest_release();
            let _ = tx.send(BridgeEvent::UpdateChanged(state));
        });
}

fn spawn_update_download(manager: Arc<Mutex<SessionManager>>, tx: mpsc::Sender<BridgeEvent>) {
    let snapshot = {
        let mut guard = manager.lock().unwrap();
        if matches!(guard.update.stage.as_str(), "downloading" | "verifying") {
            return;
        }
        guard.update.stage = "downloading".into();
        guard.update.progress = 8;
        guard.update.status_text = "Starting download...".into();
        guard.update.clone()
    };
    let _ = tx.send(BridgeEvent::UpdateChanged(snapshot.clone()));

    let _ = std::thread::Builder::new()
        .name("icy-update-download".into())
        .spawn(move || {
            let mut working = snapshot.clone();
            let asset = snapshot.asset_url.clone();
            let version = snapshot.available_version.clone();
            let result = updater::download_and_stage(asset.as_deref(), &version, |st, pct, msg| {
                working.stage = st.into();
                working.progress = pct;
                working.status_text = msg.into();
                if let Ok(mut guard) = manager.lock() {
                    guard.update = working.clone();
                }
                let _ = tx.send(BridgeEvent::UpdateChanged(working.clone()));
            });

            match result {
                Ok(path) => {
                    working.stage = "ready".into();
                    working.progress = 100;
                    working.status_text = "Ready to install & restart".into();
                    working.staged_path = Some(path.display().to_string());
                }
                Err(err) => {
                    working.stage = "error".into();
                    working.status_text = err;
                }
            }
            if let Ok(mut guard) = manager.lock() {
                guard.update = working.clone();
            }
            let _ = tx.send(BridgeEvent::UpdateChanged(working));
        });
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
                let _ = platform::open_external(path);
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
        "newSession" | "openColumn" => {
            let mut guard = manager.lock().unwrap();
            guard.create_session(false)?;
            let _ = tx.send(BridgeEvent::SessionsDirty);
            let _ = tx.send(BridgeEvent::TilingDirty);
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok("ok".into())
        }
        "stackVertical" => {
            let mut guard = manager.lock().unwrap();
            guard.create_session(true)?;
            let _ = tx.send(BridgeEvent::SessionsDirty);
            let _ = tx.send(BridgeEvent::TilingDirty);
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok("ok".into())
        }
        "launchCli" => {
            let cli_id = payload
                .get("cliId")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing cliId".to_string())?;
            let mut guard = manager.lock().unwrap();
            guard.launch_cli(cli_id)?;
            let _ = tx.send(BridgeEvent::SessionsDirty);
            let _ = tx.send(BridgeEvent::TilingDirty);
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
                let _ = tx.send(BridgeEvent::TilingDirty);
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
                let _ = tx.send(BridgeEvent::TilingDirty);
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        "focusDir" => {
            let dir = match payload.get("dir").and_then(Value::as_str).unwrap_or("") {
                "left" => FocusDir::Left,
                "right" => FocusDir::Right,
                "up" => FocusDir::Up,
                "down" => FocusDir::Down,
                _ => return Err("invalid direction".into()),
            };
            let mut guard = manager.lock().unwrap();
            if guard.focus_dir(dir) {
                let _ = tx.send(BridgeEvent::SessionsDirty);
                let _ = tx.send(BridgeEvent::TilingDirty);
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        "toggleMaximize" => {
            let mut guard = manager.lock().unwrap();
            guard.toggle_maximize();
            let _ = tx.send(BridgeEvent::TilingDirty);
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok("ok".into())
        }
        "scrollColumn" => {
            let delta = payload.get("delta").and_then(Value::as_i64).unwrap_or(0) as isize;
            let mut guard = manager.lock().unwrap();
            if guard.scroll_column(delta) {
                let _ = tx.send(BridgeEvent::TilingDirty);
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        "setTilingMode" => {
            let mode = match payload.get("mode").and_then(Value::as_str) {
                Some("hyprland") => TilingMode::Hyprland,
                _ => TilingMode::Niri,
            };
            let mut guard = manager.lock().unwrap();
            guard.set_tiling_mode(mode);
            let _ = tx.send(BridgeEvent::ConfigDirty);
            let _ = tx.send(BridgeEvent::TilingDirty);
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok("ok".into())
        }
        "updateConfig" => {
            let mut guard = manager.lock().unwrap();
            if let Some(theme) = payload.get("theme").and_then(Value::as_str) {
                guard.config.theme = theme.into();
                if let Some(found) = guard.themes.iter().find(|t| t.id == theme) {
                    guard.config.mode = found.mode.clone();
                }
            }
            if let Some(mode) = payload.get("mode").and_then(Value::as_str) {
                guard.config.mode = mode.into();
            }
            if let Some(accent) = payload.get("accent").and_then(Value::as_str) {
                guard.config.accent = accent.into();
            }
            if let Some(density) = payload.get("density").and_then(Value::as_str) {
                guard.config.density = density.into();
            }
            if let Some(fs) = payload.get("fontSize").and_then(Value::as_u64) {
                guard.config.font_size = (fs as u16).clamp(11, 18);
                let _ = guard.apply_tiled_sizes();
            }
            if let Some(mode) = payload.get("tilingMode").and_then(Value::as_str) {
                let tm = if mode == "hyprland" {
                    TilingMode::Hyprland
                } else {
                    TilingMode::Niri
                };
                guard.config.tiling_mode = tm;
                guard.tiling.set_mode(tm);
                let _ = guard.apply_tiled_sizes();
            }
            if let Some(check) = payload.get("checkUpdates").and_then(Value::as_bool) {
                guard.config.check_updates = check;
            }
            if let Some(auto_r) = payload.get("autoRestartUpdate").and_then(Value::as_bool) {
                guard.config.auto_restart_update = auto_r;
            }
            let _ = config::save_config(&guard.config);
            let _ = tx.send(BridgeEvent::ConfigDirty);
            let _ = tx.send(BridgeEvent::TilingDirty);
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok("ok".into())
        }
        "updateCli" => {
            let id = payload
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing cli id".to_string())?;
            let mut guard = manager.lock().unwrap();
            if let Some(cli) = guard.config.clis.iter_mut().find(|c| c.id == id) {
                if let Some(enabled) = payload.get("enabled").and_then(Value::as_bool) {
                    cli.enabled = enabled;
                }
                if let Some(auto_detect) = payload.get("autoDetect").and_then(Value::as_bool) {
                    cli.auto_detect = auto_detect;
                }
                if let Some(cmd) = payload.get("command").and_then(Value::as_str) {
                    cli.command = cmd.into();
                }
                if let Some(args) = payload.get("args").and_then(Value::as_str) {
                    cli.args = args.split_whitespace().map(str::to_string).collect();
                }
                let _ = config::save_config(&guard.config);
                let _ = tx.send(BridgeEvent::ConfigDirty);
            }
            Ok("ok".into())
        }
        "reloadThemes" => {
            let mut guard = manager.lock().unwrap();
            guard.themes = config::load_themes();
            let _ = tx.send(BridgeEvent::ThemesDirty);
            let _ = tx.send(BridgeEvent::PushNotice {
                kind: "info".into(),
                text: format!("Loaded {} themes from themes/", guard.themes.len()),
            });
            Ok("ok".into())
        }
        "openThemesDir" => {
            let dir = config::themes_dir();
            let _ = std::fs::create_dir_all(&dir);
            let _ = platform::open_external(&dir.display().to_string());
            Ok("ok".into())
        }
        "openExternal" => {
            if let Some(url) = payload.get("url").and_then(Value::as_str) {
                let _ = platform::open_external(url);
            }
            Ok("ok".into())
        }
        "refreshGitHub" => {
            spawn_github_probe(cwd.to_path_buf(), tx.clone());
            Ok("ok".into())
        }
        "listFiles" => {
            let subdir = payload.get("subdir").and_then(Value::as_str).unwrap_or("");
            let files = list_workspace_files(cwd, subdir);
            let _ = tx.send(BridgeEvent::PushFiles(files));
            Ok("ok".into())
        }
        "addTask" => {
            let title = payload.get("title").and_then(Value::as_str).unwrap_or("");
            let tag = payload.get("tag").and_then(Value::as_str).unwrap_or("todo");
            let mut guard = manager.lock().unwrap();
            if guard.tasks.add(title, tag).is_some() {
                let _ = tx.send(BridgeEvent::TasksDirty);
            }
            Ok("ok".into())
        }
        "toggleTask" => {
            let id = payload
                .get("id")
                .and_then(Value::as_u64)
                .ok_or_else(|| "missing task id".to_string())?;
            let mut guard = manager.lock().unwrap();
            if guard.tasks.toggle(id) {
                let _ = tx.send(BridgeEvent::TasksDirty);
            }
            Ok("ok".into())
        }
        "deleteTask" => {
            let id = payload
                .get("id")
                .and_then(Value::as_u64)
                .ok_or_else(|| "missing task id".to_string())?;
            let mut guard = manager.lock().unwrap();
            if guard.tasks.remove(id) {
                let _ = tx.send(BridgeEvent::TasksDirty);
            }
            Ok("ok".into())
        }
        "checkUpdate" => {
            spawn_update_check(tx.clone());
            Ok("ok".into())
        }
        "downloadUpdate" => {
            spawn_update_download(Arc::clone(manager), tx.clone());
            Ok("ok".into())
        }
        "confirmRestartUpdate" => {
            let staged = {
                let guard = manager.lock().unwrap();
                guard.update.staged_path.clone()
            };
            let Some(staged_str) = staged else {
                return Err("no staged update ready".into());
            };
            updater::apply_staged_and_restart(Path::new(&staged_str))?;
            let _ = tx.send(BridgeEvent::QuitApp);
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
    cwd: PathBuf,
) {
    let mut last_screen_push = Instant::now() - SCREEN_FRAME_MIN_INTERVAL;
    let mut current_scale = 1.0_f32;

    while let Ok(first) = rx.recv() {
        let mut need_sessions = false;
        let mut need_tiling = false;
        let mut need_screen = false;
        let mut need_usage = false;
        let mut need_config = false;
        let mut need_themes = false;
        let mut need_tasks = false;
        let mut need_github = false;
        let mut need_update = false;
        let mut pending_files: Option<FilesPayload> = None;
        let mut pending_viewer: Option<ViewerPayload> = None;
        let mut pending_notice: Option<(String, String)> = None;
        let mut pending_scale: Option<f32> = None;
        let mut quit_app = false;

        let mut apply_event = |ev: BridgeEvent| match ev {
            BridgeEvent::Hello => {
                need_sessions = true;
                need_tiling = true;
                need_screen = true;
                need_usage = true;
                need_config = true;
                need_themes = true;
                need_tasks = true;
                need_github = true;
                need_update = true;
                pending_files = Some(list_workspace_files(&cwd, ""));
            }
            BridgeEvent::PtyDirty => {
                need_screen = true;
                need_sessions = true;
            }
            BridgeEvent::SessionsDirty => need_sessions = true,
            BridgeEvent::TilingDirty => need_tiling = true,
            BridgeEvent::ConfigDirty => need_config = true,
            BridgeEvent::ThemesDirty => need_themes = true,
            BridgeEvent::TasksDirty => need_tasks = true,
            BridgeEvent::GitHubUpdated(gh) => {
                manager.lock().unwrap().github = gh;
                need_github = true;
            }
            BridgeEvent::UpdateChanged(up) => {
                manager.lock().unwrap().update = up;
                need_update = true;
            }
            BridgeEvent::PushFiles(f) => pending_files = Some(f),
            BridgeEvent::PushViewer(v) => pending_viewer = Some(v),
            BridgeEvent::PushNotice { kind, text } => pending_notice = Some((kind, text)),
            BridgeEvent::ScaleChanged(scale) => pending_scale = Some(scale),
            BridgeEvent::QuitApp => quit_app = true,
        };

        apply_event(first);
        while let Ok(more) = rx.try_recv() {
            apply_event(more);
        }

        if quit_app {
            let _ = pusher.quit();
            break;
        }

        if let Some(scale) = pending_scale {
            current_scale = scale;
            let update = manager.lock().unwrap().update.clone();
            push_images(&pusher, &cli_usage, &update, scale);
        }
        if need_usage {
            push_view(&pusher, "cliUsage", &cli_usage);
        }
        if need_config {
            let cfg = manager.lock().unwrap().config.clone();
            push_view(&pusher, "config", &cfg);
        }
        if need_themes {
            let themes = manager.lock().unwrap().themes_payload();
            push_view(&pusher, "themes", &themes);
        }
        if need_tasks {
            let tasks = manager.lock().unwrap().tasks_payload();
            push_view(&pusher, "tasks", &tasks);
        }
        if need_github {
            let gh = manager.lock().unwrap().github.clone();
            push_view(&pusher, "github", &gh);
        }
        if need_update {
            let up = manager.lock().unwrap().update.clone();
            push_update_ring(&pusher, &up, current_scale);
            push_view(&pusher, "update", &up);
        }
        if let Some(files) = pending_files {
            push_view(&pusher, "files", &files);
        }
        if need_sessions {
            let payload = manager.lock().unwrap().sessions_payload();
            push_view(&pusher, "sessions", &payload);
        }
        if need_tiling {
            let tiling = manager.lock().unwrap().tiling_snapshot();
            push_view(&pusher, "tiling", &tiling);
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
            let (focused_screen, screens_map) = {
                let guard = manager.lock().unwrap();
                (
                    guard.active().map(build_screen_payload),
                    guard.visible_screens(),
                )
            };
            if let Some(screen) = focused_screen {
                push_view(&pusher, "screen", &screen);
            }
            push_view(&pusher, "screens", &screens_map);
            last_screen_push = Instant::now();
        }
    }
}

fn run_offscreen(
    path: &Path,
    manager: &Arc<Mutex<SessionManager>>,
    cli_usage: &[CliUsage],
    cwd: &Path,
    mut host: Host,
) -> ExitCode {
    std::thread::sleep(Duration::from_millis(250));

    let (sessions_payload, tiling_payload, screen_payload, screens_map, cfg, themes, tasks, gh, up) = {
        let mut guard = manager.lock().unwrap();
        guard.update_viewport(1280.0, 800.0);
        // Set a realistic preview update state for offscreen visual verification.
        guard.update.has_update = true;
        guard.update.available_version = "0.2.0-preview".into();
        guard.update.status_text = "Preview v0.2.0 ready".into();
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
        (
            guard.sessions_payload(),
            guard.tiling_snapshot(),
            build_screen_payload(active),
            guard.visible_screens(),
            guard.config.clone(),
            guard.themes_payload(),
            guard.tasks_payload(),
            github::detect_github(cwd),
            guard.update.clone(),
        )
    };

    install_images_on_host(&mut host, cli_usage, &up, 1.0);
    host.push(serde_json::json!({ "view": "cliUsage", "data": cli_usage }).to_string());
    host.push(serde_json::json!({ "view": "config", "data": cfg }).to_string());
    host.push(serde_json::json!({ "view": "themes", "data": themes }).to_string());
    host.push(serde_json::json!({ "view": "tasks", "data": tasks }).to_string());
    host.push(serde_json::json!({ "view": "github", "data": gh }).to_string());
    host.push(serde_json::json!({ "view": "update", "data": up }).to_string());
    host.push(
        serde_json::json!({ "view": "files", "data": list_workspace_files(cwd, "") }).to_string(),
    );
    host.push(serde_json::json!({ "view": "sessions", "data": sessions_payload }).to_string());
    host.push(serde_json::json!({ "view": "tiling", "data": tiling_payload }).to_string());
    host.push(serde_json::json!({ "view": "screen", "data": screen_payload }).to_string());
    host.push(serde_json::json!({ "view": "screens", "data": screens_map }).to_string());

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
    {
        let up = manager.lock().unwrap().update.clone();
        install_images_on_host(&mut host, &cli_usage, &up, 1.0);
    }

    if let Some(path) = offscreen_path {
        return run_offscreen(&path, &manager, &cli_usage, &cwd, host);
    }

    // Kick off non-blocking background GitHub SSH & release checks.
    spawn_github_probe(cwd.clone(), tx.clone());
    if manager.lock().unwrap().config.check_updates {
        spawn_update_check(tx.clone());
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
                WindowShortcut::NewSession | WindowShortcut::OpenColumn => {
                    guard.create_session(false).is_ok()
                }
                WindowShortcut::StackVertical => guard.create_session(true).is_ok(),
                WindowShortcut::CloseSession => {
                    let active = guard.active_id();
                    guard.close_session(active)
                }
                WindowShortcut::NextTab => guard.step_tab(1),
                WindowShortcut::PrevTab => guard.step_tab(-1),
                WindowShortcut::ToggleMaximize => guard.toggle_maximize(),
                WindowShortcut::FocusLeft => guard.focus_dir(FocusDir::Left),
                WindowShortcut::FocusRight => guard.focus_dir(FocusDir::Right),
                WindowShortcut::FocusUp => guard.focus_dir(FocusDir::Up),
                WindowShortcut::FocusDown => guard.focus_dir(FocusDir::Down),
            };
            if changed {
                let _ = shortcut_tx.send(BridgeEvent::SessionsDirty);
                let _ = shortcut_tx.send(BridgeEvent::TilingDirty);
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
    let bridge_cwd = cwd.clone();
    let result = window::open_with(host, "icy", [1280, 800], hooks, |pusher| {
        pusher.on_scale(move |scale| {
            let _ = scale_tx.send(BridgeEvent::ScaleChanged(scale));
        });
        std::thread::Builder::new()
            .name("icy-ui-bridge".into())
            .spawn(move || run_bridge(pusher, rx, bridge_manager, cli_usage, bridge_cwd))
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
        let up = UpdateState::initial();
        install_images_on_host(&mut host, &usage, &up, 1.0);
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
