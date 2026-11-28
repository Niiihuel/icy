#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod browser;
mod desktop_notifications;
mod font_preview;
mod icons;
mod pixel_blast;
mod release_glow;
#[cfg(test)]
mod review_tests;
mod usage_scan;
mod window;
mod window_surface;

use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use icy_core::agent_notifications::AgentNotifications;
use icy_core::config::{self, IcyConfig, ResolvedTheme};
use icy_core::document;
use icy_core::git::{self, ChangeSet, DiffLine};
use icy_core::git_graph;
use icy_core::github::{self, GitHubStatus};
use icy_core::graphics::VisibleImage;
use icy_core::keybinds;
use icy_core::links::{self, LinkRef, RefKind, ResolvedTarget};
use icy_core::platform;
#[cfg(test)]
use icy_core::session::Session;
use icy_core::session::SessionInfo;
use icy_core::session_server::{
    self, Client as SessionClient, ServerEvent, ServerOptions, Spawn as TerminalSpawn, Terminal,
};
use icy_core::syntax::{self, HighlightRange};
use icy_core::tiling::{
    ColumnView, DropTarget, FocusDir, RESIZE_STEP, TilingMode, TilingView, Workspaces,
};
use icy_core::updater::{self, CheckSchedule, CheckStatus, UpdateState};
use icy_core::vt::{DEFAULT_BG, Row as VtRow, Screen};
use jsnir::host::Host;
use jsnir::text::TextEngine;
use serde::Serialize;
use serde_json::Value;

use crate::window::{Pusher, TerminalSelect, WindowHooks, WindowShortcut};
use icy_core::usage::{CliUsage, UsageSnapshot};
use usage_scan::UsageScan;

const APP: &str = include_str!(concat!(env!("OUT_DIR"), "/app.js"));
const GEIST_MONO_TTF: &[u8] = include_bytes!("../../../assets/fonts/GeistMono-Regular.ttf");
const GEIST_MONO_SEMIBOLD_TTF: &[u8] =
    include_bytes!("../../../assets/fonts/GeistMono-SemiBold.ttf");
const INTER_MEDIUM_TTF: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium.ttf");
const INTER_REGULAR_TTF: &[u8] = include_bytes!("../../../../Jurui/assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD_TTF: &[u8] =
    include_bytes!("../../../../Jurui/assets/fonts/Inter-SemiBold.ttf");

const DEFAULT_COLS: u16 = 120;
const DEFAULT_ROWS: u16 = 38;
// Only the newest foreground document may navigate the preview window.
static VIEWER_GENERATION: AtomicU64 = AtomicU64::new(0);
const SCREEN_FRAME_MIN_INTERVAL: Duration = Duration::from_millis(16);
const SESSION_STATUS_INTERVAL: Duration = Duration::from_millis(125);
/// How often the sidebar usage meters rescan local transcripts.
const CLI_USAGE_REFRESH: Duration = Duration::from_secs(120);
/// Live limit endpoints (Claude OAuth usage, `agy /usage`, Grok billing); Orca never polls them
/// more often than every 5 minutes.
const REMOTE_LIMITS_REFRESH: Duration = Duration::from_secs(600);

// Initial PTY geometry mirrors WorkspaceShell, TilingWorkspaceView and TerminalPane.
// The measured grid takes over once JSNIR lays out the new pane.
const CELL_ADVANCE: f32 = 0.6;
const CELL_LINE_HEIGHT: f32 = 1.4;
const TERMINAL_PADDING_X: f32 = 16.0;
const TERMINAL_PADDING_Y: f32 = 10.0;
const SIDEBAR_WIDTH_PX: f32 = 224.0;
const TAB_BAR_HEIGHT_PX: f32 = 44.0;
const WORKSPACE_MARGIN_PX: f32 = 8.0;
const FRAME_INSET_PX: f32 = 1.0;
const PANE_GAP_PX: f32 = 8.0;
const PANE_HEADER_PX: f32 = 30.0;

#[derive(PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpanPayload {
    text: String,
    col: u16,
    cells: u16,
    fg: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    bg: Option<u32>,
    bold: bool,
    underline: bool,
    link: bool,
    /// Part of a resolved image path: drawn as a tag with hover preview.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    image: bool,
    /// Inside the mouse selection.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    selected: bool,
    /// Index in the row's `links` of the link this span belongs to, so hover previews target it.
    #[serde(skip_serializing_if = "Option::is_none")]
    link_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LinkPayload {
    kind: RefKind,
    target: String,
    line: Option<u32>,
    col: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    old_line: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit: Option<links::EditPreview>,
    resolved: bool,
    image: bool,
}

#[derive(PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RowPayload {
    idx: usize,
    plain: String,
    spans: Vec<SpanPayload>,
    links: Vec<LinkPayload>,
}

#[derive(PartialEq, Eq, Serialize)]
struct ScreenPayload {
    #[serde(flatten)]
    meta: ScreenMeta,
    rows: Vec<Arc<RowPayload>>,
}

#[derive(PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScreenMeta {
    cols: u16,
    rows_count: u16,
    cursor_row: u16,
    cursor_col: u16,
    cursor_visible: bool,
    /// Lines scrolled back into the history (0 = following the output).
    display_offset: u32,
    /// Inline images (kitty graphics) placed over the cell grid.
    images: Vec<ImagePayload>,
}

#[derive(PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImagePayload {
    key: String,
    row: i64,
    col: u16,
    cols: u16,
    rows: u16,
    width: u32,
    height: u32,
}

struct TerminalSnapshot {
    id: u64,
    cwd: PathBuf,
    screen: Screen,
    placements: Vec<VisibleImage>,
}

#[derive(Default)]
struct TerminalRowCache {
    source: Vec<VtRow>,
    rendered: Vec<Arc<RowPayload>>,
}

#[derive(PartialEq, Eq)]
struct TerminalScreens {
    visible: BTreeMap<u64, ScreenPayload>,
    context_id: Option<u64>,
    // The context terminal may be outside the visible panes when a non-terminal tile has focus.
    context_screen: Option<ScreenPayload>,
}

#[derive(Serialize)]
struct ScreenUpdate<'a> {
    #[serde(flatten)]
    meta: &'a ScreenMeta,
    rows: Vec<Option<&'a RowPayload>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalFrame<'a> {
    sequence: u64,
    screens: BTreeMap<u64, ScreenUpdate<'a>>,
    context_id: Option<u64>,
    context_screen: Option<ScreenUpdate<'a>>,
}

/// A loaded commit graph waiting for the bridge loop, which rasterizes its lane layers at
/// the current device scale (sizes come from the TSX theme so themes can restyle them).
struct GitGraphRequest {
    cwd: PathBuf,
    graph: Option<git_graph::Graph>,
    lane: f32,
    row: f32,
    stroke: f32,
}

fn git_graph_view(
    request: GitGraphRequest,
    scale: f32,
    registered: &mut std::collections::HashSet<String>,
    pusher: &Pusher,
) -> serde_json::Value {
    let GitGraphRequest {
        cwd,
        graph,
        lane,
        row,
        stroke,
    } = request;
    let Some(graph) = graph else {
        return serde_json::json!({ "cwd": cwd, "error": "Not a git repository", "rows": [], "width": 0 });
    };
    let rows: Vec<_> = graph
        .rows
        .iter()
        .map(|r| {
            let mut colors: Vec<usize> = r.segs.iter().map(|s| s.color).collect();
            colors.sort_unstable();
            colors.dedup();
            let layers: Vec<_> = colors
                .into_iter()
                .filter_map(|color| {
                    let segs: Vec<_> = r
                        .segs
                        .iter()
                        .copied()
                        .filter(|s| s.color == color)
                        .collect();
                    let shape: Vec<_> = segs.iter().map(|s| (s.x1, s.y1, s.x2, s.y2)).collect();
                    let key = format!(
                        "ggraph:{scale}:{lane}:{row}:{stroke}:{}:{shape:?}",
                        graph.width
                    );
                    if !registered.contains(&key) {
                        let (rgba, size) = icons::rasterize_graph_layer(
                            &segs,
                            graph.width,
                            lane,
                            row,
                            stroke,
                            scale,
                        )?;
                        let _ = pusher.image(key.clone(), rgba, size);
                        registered.insert(key.clone());
                    }
                    Some(serde_json::json!({ "color": color, "key": key }))
                })
                .collect();
            let c = &r.commit;
            serde_json::json!({
                "hash": c.hash,
                "short": c.hash.get(..7).unwrap_or(&c.hash),
                "subject": c.subject,
                "author": c.author,
                "time": c.time,
                "refs": c.refs,
                "node": r.node,
                "color": r.color,
                "merge": c.parents.len() > 1,
                "layers": layers,
            })
        })
        .collect();
    serde_json::json!({ "cwd": cwd, "width": graph.width, "rows": rows, "limit": git_graph::LOG_LIMIT })
}

struct FileImageResult {
    path: String,
    image: Option<(String, Vec<u8>, [u32; 2])>,
}

/// Loads an image a CLI printed the path of (resolved against the session cwd) for the hover
/// preview and the viewer. Only previewable extensions and regular files up to 32 MiB.
fn resolve_link_path(link: &LinkRef, cwd: &Path) -> Option<ResolvedTarget> {
    if let Some(found) = links::resolve(link, cwd, |p| p.is_file()) {
        return Some(found);
    }
    if link.kind == RefKind::Url {
        return None;
    }
    let raw = Path::new(&link.target);
    if raw.is_absolute() || link.target.starts_with("~/") || link.target.starts_with("~\\") {
        return None;
    }
    resolve_in_workspace_cache(cwd, &link.target).map(|path| ResolvedTarget::File {
        path,
        line: link.line,
        col: link.col,
    })
}

fn resolve_in_workspace_cache(cwd: &Path, target: &str) -> Option<PathBuf> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    type CacheMap = HashMap<PathBuf, (Instant, icy_core::search::WorkspaceFileIndex)>;
    static CACHE: OnceLock<Mutex<CacheMap>> = OnceLock::new();
    const TTL: Duration = Duration::from_secs(5);

    let lookup = |root: &Path| -> Option<PathBuf> {
        if !root.is_dir() {
            return None;
        }
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = cache.lock().unwrap();
        let stale = guard
            .get(root)
            .is_none_or(|(built, _)| built.elapsed() >= TTL);
        if stale {
            let index = icy_core::search::WorkspaceFileIndex::build(root);
            guard.insert(root.to_path_buf(), (Instant::now(), index));
        }
        guard
            .get(root)
            .and_then(|(_, index)| index.resolve(root, target))
    };

    if let Some(path) = lookup(cwd) {
        return Some(path);
    }
    // If the session was started inside a build/sub folder (e.g. `target/`) of a git repo,
    // fall back to the parent repo root so printed filenames still resolve.
    if !cwd.join(".git").exists()
        && let Some(parent) = cwd.parent()
        && parent.join(".git").exists()
    {
        return lookup(parent);
    }
    None
}

fn spawn_file_image(raw: String, cwd: PathBuf, tx: mpsc::Sender<BridgeEvent>) {
    let _ = std::thread::Builder::new()
        .name("icy-file-image".into())
        .spawn(move || {
            let link = LinkRef {
                start: 0,
                end: raw.len(),
                kind: RefKind::File,
                target: raw.clone(),
                line: None,
                col: None,
            };
            let image = resolve_link_path(&link, &cwd)
                .and_then(|t| match t {
                    ResolvedTarget::File { path, .. } => Some(path),
                    _ => None,
                })
                .filter(|p| links::is_image_path(&p.display().to_string()))
                .filter(|p| std::fs::metadata(p).is_ok_and(|m| m.len() <= 32 * 1024 * 1024))
                .and_then(|path| {
                    let stamp = std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_millis());
                    let (rgba, size) = icons::rasterize_image_file(&path, 2048)?;
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::hash::Hash::hash(&(path.display().to_string(), stamp), &mut hasher);
                    let key = format!("fimg:{:x}", std::hash::Hasher::finish(&hasher));
                    Some((key, rgba, size))
                });
            let _ = tx.send(BridgeEvent::FileImage(FileImageResult { path: raw, image }));
        });
}

fn spawn_release_cover(url: String, tx: mpsc::Sender<BridgeEvent>) -> Result<(), String> {
    std::thread::Builder::new()
        .name("icy-release-cover".into())
        .spawn(move || {
            let image = updater::fetch_release_image(&url)
                .and_then(|file| icons::rasterize_image_file(&file, 1600))
                .map(|(rgba, size)| (format!("release-cover:{url}"), rgba, size));
            let _ = tx.send(BridgeEvent::FileImage(FileImageResult { path: url, image }));
        })
        .map(|_| ())
        .map_err(|error| format!("Cannot start release cover loader: {error}"))
}

fn term_image_key(session_id: u64, seq: u64) -> String {
    format!("timg:{session_id}-{seq}")
}

#[derive(PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionsPayload {
    active_id: u64,
    archived_count: u32,
    archived: Vec<ArchivedSession>,
    items: Vec<SessionInfo>,
}

/// A session hidden from the workspace, available to restore from Archived.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArchivedSession {
    #[serde(flatten)]
    session: SessionInfo,
    /// Unix seconds.
    archived_at: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewerLine {
    num: usize,
    text: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    highlighted: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    highlights: Vec<HighlightRange>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewerPayload {
    #[serde(skip)]
    generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    intent: Option<u64>,
    cwd: PathBuf,
    requested_path: String,
    background: bool,
    open: bool,
    path: String,
    display_path: String,
    target_line: Option<u32>,
    target_col: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    old_line: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit: Option<links::EditPreview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text_info: Option<document::TextInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff_focus: Option<usize>,
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<FilePreview>,
    #[serde(skip)]
    preview_pixels: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    editable_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit_unavailable: Option<&'static str>,
    lines: Vec<ViewerLine>,
    /// `file` (syntax-highlighted source) or `diff` (unified diff in `diff`).
    mode: &'static str,
    diff: Vec<DiffView>,
    additions: u32,
    deletions: u32,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum FilePreview {
    Binary,
    Font {
        name: String,
        key: String,
        size: [u32; 2],
    },
}

const CODE_PREVIEW_LINES: usize = 14;
const CODE_PREVIEW_CONTEXT: usize = 3;
const CODE_PREVIEW_MAX_BYTES: u64 = 32 * 1024 * 1024;
const CODE_PREVIEW_MAX_CHARS: usize = 240;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CodePreviewPayload {
    key: String,
    mode: &'static str,
    display_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_line: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    single_edit: bool,
    total_lines: usize,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff_focus: Option<usize>,
    additions: u32,
    deletions: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lines: Vec<ViewerLine>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diff: Vec<DiffView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl CodePreviewPayload {
    fn error(
        key: String,
        mode: &'static str,
        display_path: String,
        target_line: Option<u32>,
        single_edit: bool,
        error: String,
    ) -> Self {
        Self {
            key,
            mode,
            display_path,
            target_line,
            single_edit,
            total_lines: 0,
            truncated: false,
            diff_focus: None,
            additions: 0,
            deletions: 0,
            lines: Vec::new(),
            diff: Vec::new(),
            error: Some(error),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffView {
    #[serde(flatten)]
    line: DiffLine,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    highlights: Vec<HighlightRange>,
}

impl ViewerPayload {
    fn closed() -> Self {
        Self {
            generation: 0,
            intent: None,
            cwd: PathBuf::new(),
            requested_path: String::new(),
            background: false,
            open: false,
            path: String::new(),
            display_path: String::new(),
            target_line: None,
            target_col: None,
            old_line: false,
            edit: None,
            text_info: None,
            diff_focus: None,
            error: None,
            preview: None,
            preview_pixels: None,
            editable_text: None,
            edit_unavailable: None,
            lines: Vec::new(),
            mode: "file",
            diff: Vec::new(),
            additions: 0,
            deletions: 0,
        }
    }

    fn error(
        path: &str,
        display_path: String,
        line: Option<u32>,
        col: Option<u32>,
        error: String,
    ) -> Self {
        Self {
            open: true,
            path: path.to_string(),
            display_path,
            target_line: line,
            target_col: col,
            error: Some(error),
            ..Self::closed()
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
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

enum BridgeEvent {
    Hello,
    TerminalFrameReceived(u64),
    ScanUsage {
        tx: mpsc::Sender<BridgeEvent>,
        refresh: bool,
    },
    UsageReady(Arc<UsageSnapshot>),
    TestNotification(mpsc::Sender<BridgeEvent>),
    NotificationTested(Option<String>),
    /// Fresh sidebar meters from the periodic `icy-cli-usage` scan.
    CliUsage(Vec<CliUsage>),
    PtyDirty,
    SessionsDirty,
    TilingDirty,
    ConfigDirty,
    ThemesDirty,
    EditorsUpdated(Vec<platform::ExternalEditor>),
    GitHubUpdated(GitHubStatus),
    CheckUpdate,
    UpdateChecked(Result<Option<updater::CheckedRelease>, updater::CheckError>),
    UpdateChanged,
    PushFiles(FilesPayload),
    PushChanges(PathBuf, ChangeSet),
    PushViewer(ViewerPayload),
    EditorSaved(Value),
    PasteContext {
        id: u64,
        cwd: PathBuf,
        cli: String,
        text: String,
    },
    PushNotice {
        kind: String,
        text: String,
    },
    ScaleChanged(f32),
    AvatarReady(PathBuf),
    ModalKey(&'static str),
    FileImage(FileImageResult),
    CodePreview(CodePreviewPayload),
    FileSearch(serde_json::Value),
    GitGraph(GitGraphRequest),
    /// Blame and branch for the editor footer, answered to the page as `gitBlame`.
    GitBlame(Value),
    /// A browser tile request; the window thread owns CEF and applies it.
    Browser(browser::Command),
    /// A context-menu edit (`copy`, `cut`, `paste`, `selectAll`) for the focused editor.
    EditorCommand(String),
    CapturedKey(String),
    KeybindsDirty(Vec<&'static str>),
    FocusWindow,
    ServerQuit,
    QuitApp,
}

struct SessionManager {
    sessions: Vec<Terminal>,
    session_client: Option<Arc<SessionClient>>,
    /// niri workspaces; single-window operations reach the active one through `Deref`.
    tiling: Workspaces,
    config: IcyConfig,
    themes: Vec<ResolvedTheme>,
    editors: Vec<platform::ExternalEditor>,
    github: GitHubStatus,
    update: UpdateState,
    next_id: u64,
    /// Parked terminal IDs and their archive timestamps.
    archived: BTreeMap<u64, u64>,
    cols: u16,
    rows: u16,
    viewport_w: f32,
    viewport_h: f32,
    sidebar_w: f32,
    panel_w: f32,
    /// The niri-style overview shows every session, so all of them get screen pushes.
    overview: bool,
    /// Non-terminal tiles placed in the tiling (ids share the session counter).
    tiles: BTreeMap<u64, PathBuf>,
    terminal_sizes: BTreeMap<u64, (u16, u16)>,
    cwd: PathBuf,
    on_pty_update: Arc<dyn Fn() + Send + Sync>,
    restarting_for_update: bool,
    server_stopped: bool,
}

impl SessionManager {
    fn new(cwd: PathBuf, on_pty_update: Arc<dyn Fn() + Send + Sync>) -> Self {
        let cfg = config::load_config();
        *window::KEYMAP.write().unwrap() = keybinds::Keymap::new(&cfg.keybinds);
        let themes = config::load_themes();
        Self {
            sessions: Vec::new(),
            session_client: None,
            tiling: {
                let mut tiling = Workspaces::empty(cfg.tiling_mode);
                tiling.set_options(cfg.layout);
                tiling
            },
            config: cfg,
            themes,
            editors: Vec::new(),
            github: GitHubStatus::default(),
            update: UpdateState::initial(),
            next_id: 1,
            archived: BTreeMap::new(),
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            viewport_w: 1280.0,
            viewport_h: 800.0,
            sidebar_w: SIDEBAR_WIDTH_PX,
            panel_w: 0.0,
            overview: false,
            tiles: BTreeMap::new(),
            terminal_sizes: BTreeMap::new(),
            cwd,
            on_pty_update,
            restarting_for_update: false,
            server_stopped: false,
        }
    }

    fn detach_sessions(&mut self) {
        self.sessions.clear();
        self.session_client = None;
    }

    fn attach_sessions(&mut self, tx: &mpsc::Sender<BridgeEvent>) -> std::io::Result<()> {
        let dir = session_server::default_directory();
        let dirty = tx.clone();
        let notices = tx.clone();
        let events = tx.clone();
        let (client, attachment) = SessionClient::attach_with_events(
            &dir,
            &std::env::current_exe()?,
            move || {
                let _ = dirty.send(BridgeEvent::PtyDirty);
            },
            move || {
                let _ = notices.send(BridgeEvent::PushNotice {
                    kind: "error".into(),
                    text: "Disconnected from the session server. Reopen icy to reconnect.".into(),
                });
                let _ = notices.send(BridgeEvent::PtyDirty);
            },
            move |event| match event {
                ServerEvent::FocusWindow => {
                    let _ = events.send(BridgeEvent::FocusWindow);
                }
                ServerEvent::QuitWindow => {
                    let _ = events.send(BridgeEvent::ServerQuit);
                }
            },
        )?;
        let terminal_ids: BTreeSet<_> = attachment.terminals.iter().map(|info| info.id).collect();
        let mut archived = client.load_archive()?;
        let stored = archived.len();
        archived.retain(|id, _| terminal_ids.contains(id));
        if archived.len() != stored {
            client.save_archive(&archived)?;
        }
        self.archived = archived;
        self.sessions = attachment
            .terminals
            .into_iter()
            .map(|info| client.terminal(info))
            .collect();
        if let Some(mut layout) = attachment.layout {
            for pane in layout.all_panes() {
                if !terminal_ids.contains(&pane) || self.archived.contains_key(&pane) {
                    layout.remove_session(pane);
                }
            }
            if layout.mode() != self.config.tiling_mode {
                layout.set_mode(self.config.tiling_mode);
            }
            layout.set_options(self.config.layout);
            self.tiling = layout;
        }
        // A terminal started just before a UI crash can exist before its layout was sent.
        for session in &self.sessions {
            if !self.archived.contains_key(&session.id)
                && self.tiling.workspace_of(session.id).is_none()
            {
                self.tiling.open_pane(session.id, false);
            }
            self.next_id = self.next_id.max(session.id + 1);
        }
        self.session_client = Some(client);
        self.apply_tiled_sizes();
        Ok(())
    }

    fn save_terminal_layout(&self) {
        if let Some(client) = &self.session_client
            && let Err(error) = client.save_layout(&self.tiling)
        {
            eprintln!("icy: failed to save terminal layout: {error}");
        }
    }

    fn active_id(&self) -> u64 {
        self.tiling.focused_session()
    }

    /// The session that owns the keyboard: only when the focused tile is a terminal.
    fn focused_terminal(&self) -> Option<&Terminal> {
        let id = self.active_id();
        self.sessions.iter().find(|s| s.id == id)
    }

    fn workspace_dir(&self) -> &Path {
        if let Some(session) = self.focused_terminal() {
            session.cwd()
        } else {
            self.tiles.get(&self.active_id()).unwrap_or(&self.cwd)
        }
    }

    fn request_workspace(&self, payload: &Value) -> PathBuf {
        payload
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .map_or_else(|| self.workspace_dir().to_path_buf(), PathBuf::from)
    }

    fn context_terminal(&self) -> Option<&Terminal> {
        let dir = self.workspace_dir();
        self.focused_terminal().or_else(|| {
            self.tiling
                .pane_order()
                .into_iter()
                .find_map(|id| self.sessions.iter().find(|s| s.id == id && s.cwd() == dir))
        })
    }

    fn allocate_session_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn place_session(&mut self, session: impl Into<Terminal>, stack_vertical: bool) -> u64 {
        VIEWER_GENERATION.fetch_add(1, Ordering::Relaxed);
        let session = session.into();
        let id = session.id;
        self.sessions.push(session);
        self.tiling.open_pane(id, stack_vertical);
        let _ = self.apply_tiled_sizes();
        id
    }

    fn spawn_terminal(&self, spawn: TerminalSpawn) -> std::io::Result<Terminal> {
        if let Some(client) = &self.session_client {
            client.spawn(spawn)
        } else {
            spawn
                .start(Arc::clone(&self.on_pty_update))
                .map(Terminal::from)
        }
    }

    fn shell_spawn(id: u64, title: String, cwd: PathBuf, size: (u16, u16)) -> TerminalSpawn {
        let (program, args) = platform::default_shell();
        TerminalSpawn {
            id,
            title,
            cwd,
            program,
            args,
            size,
            cli_hint: None,
            initial_output: None,
        }
    }

    fn create_session_in(&mut self, stack_vertical: bool, cwd: PathBuf) -> Result<u64, String> {
        let id = self.allocate_session_id();
        let title = format!("shell-{id}");
        let size = self.new_terminal_size(id, stack_vertical);
        let session = self
            .spawn_terminal(Self::shell_spawn(id, title, cwd, size))
            .map_err(|e| format!("failed to spawn shell: {e}"))?;
        Ok(self.place_session(session, stack_vertical))
    }

    fn launch_cli(
        &mut self,
        cli_id: &str,
        cwd: PathBuf,
        prompt: Option<&str>,
    ) -> Result<u64, String> {
        let cli = self
            .config
            .clis
            .iter()
            .find(|c| c.id == cli_id)
            .cloned()
            .ok_or_else(|| format!("unknown CLI: {cli_id}"))?;

        let id = self.allocate_session_id();
        let size = self.new_terminal_size(id, false);
        let mut args = cli.args.clone();
        if let Some(text) = prompt.map(str::trim).filter(|t| !t.is_empty()) {
            if cli.id == "opencode" {
                args.push("--prompt".into());
            }
            args.push(text.to_string());
        }

        let (program, args) = if cli.id == "pi" {
            platform::pi_launch_command(cli.command.clone(), args, &cwd)
        } else {
            (cli.command.clone(), args)
        };
        let spawned = self.spawn_terminal(TerminalSpawn {
            id,
            title: cli.name.clone(),
            cwd: cwd.clone(),
            program,
            args,
            size,
            cli_hint: Some(cli.id),
            initial_output: None,
        });

        let session = match spawned {
            Ok(s) => s,
            Err(_) => {
                // Fallback to interactive shell with a clear hint if the CLI binary is not in PATH.
                let mut shell = Self::shell_spawn(id, cli.name.clone(), cwd, size);
                shell.initial_output = Some(format!(
                    "\x1b[33m[icy]\x1b[0m `{}` was not found in PATH — opened shell instead.\r\n",
                    cli.command
                ));
                self.spawn_terminal(shell)
                    .map_err(|e| format!("failed to launch {}: {e}", cli.name))?
            }
        };

        Ok(self.place_session(session, false))
    }

    /// Opens a non-terminal tile (file, diff, review, files tree) in the tiling,
    /// as a new column or stacked below the focused pane.
    fn open_tile(&mut self, stack_vertical: bool, cwd: PathBuf) -> u64 {
        VIEWER_GENERATION.fetch_add(1, Ordering::Relaxed);
        let id = self.allocate_session_id();
        self.tiles.insert(id, cwd);
        self.tiling.open_pane(id, stack_vertical);
        let _ = self.apply_tiled_sizes();
        id
    }

    fn close_session(&mut self, id: u64) -> bool {
        if self.tiles.remove(&id).is_some() {
            self.tiling.remove_session(id);
            let _ = self.apply_tiled_sizes();
            return true;
        }
        let Some(pos) = self.sessions.iter().position(|s| s.id == id) else {
            return false;
        };
        if self.archived.contains_key(&id) && self.sessions[pos].is_alive() {
            return false;
        }
        if self.sessions[pos].close().is_err() {
            return false;
        }
        self.sessions.remove(pos);
        self.terminal_sizes.remove(&id);
        if self.archived.remove(&id).is_some()
            && let Some(client) = &self.session_client
            && let Err(error) = client.save_archive(&self.archived)
        {
            eprintln!("icy: failed to save terminal archive: {error}");
        }
        self.tiling.remove_session(id);
        let _ = self.apply_tiled_sizes();
        true
    }

    fn set_session_archived(&mut self, id: u64, archive: bool) -> std::io::Result<bool> {
        if self.archived.contains_key(&id) == archive || !self.sessions.iter().any(|s| s.id == id) {
            return Ok(false);
        }
        let mut next = self.archived.clone();
        if archive {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            next.insert(id, now);
        } else {
            next.remove(&id);
        }
        if let Some(client) = &self.session_client {
            client.save_archive(&next)?;
        }
        self.archived = next;
        if archive {
            self.terminal_sizes.remove(&id);
            self.tiling.remove_session(id);
        } else {
            self.tiling.open_pane(id, false);
        }
        self.apply_tiled_sizes();
        Ok(true)
    }

    fn step_tab(&mut self, delta: isize) -> bool {
        // Tabs follow the tiling's reading order and include every tile, not only terminals.
        let order = self.tiling.pane_order();
        if order.len() <= 1 {
            return false;
        }
        let cur = order
            .iter()
            .position(|&id| id == self.active_id())
            .unwrap_or(0) as isize;
        let next = (cur + delta).rem_euclid(order.len() as isize) as usize;
        let next_id = order[next];
        VIEWER_GENERATION.fetch_add(1, Ordering::Relaxed);
        self.tiling.focus_session(next_id);
        let _ = self.apply_tiled_sizes();
        true
    }

    fn select_session(&mut self, id: u64) -> bool {
        if self.archived.contains_key(&id) {
            return false;
        }
        if self.tiles.contains_key(&id) || self.sessions.iter().any(|s| s.id == id) {
            VIEWER_GENERATION.fetch_add(1, Ordering::Relaxed);
            self.tiling.focus_session(id);
            let _ = self.apply_tiled_sizes();
            true
        } else {
            false
        }
    }

    /// A workspace switch or move, then the PTY sizes of the newly shown windows.
    fn workspace_step(&mut self, step: impl FnOnce(&mut Workspaces) -> bool) -> bool {
        let changed = step(&mut self.tiling);
        if changed {
            let _ = self.apply_tiled_sizes();
        }
        changed
    }

    /// Niri / Hyprland layout actions from the Layout menu and Ctrl+Alt shortcuts.
    fn tiling_action(&mut self, action: &str) -> bool {
        let t = &mut self.tiling;
        let changed = match action {
            "cycleWidth" => t.cycle_column_width(false),
            "cycleWidthBack" => t.cycle_column_width(true),
            "focusFirst" => t.focus_column_edge(false),
            "focusLast" => t.focus_column_edge(true),
            "moveFirst" => t.move_column_to_edge(false),
            "moveLast" => t.move_column_to_edge(true),
            "swapLeft" => t.swap_window(FocusDir::Left),
            "swapRight" => t.swap_window(FocusDir::Right),
            "consumeInto" => t.consume_into_column(),
            "expelFrom" => t.expel_from_column(),
            "expandColumn" => t.expand_column_to_available_width(),
            "wider" => t.adjust_column_width(RESIZE_STEP),
            "narrower" => t.adjust_column_width(-RESIZE_STEP),
            "taller" => t.adjust_pane_height(RESIZE_STEP),
            "shorter" => t.adjust_pane_height(-RESIZE_STEP),
            "maximizeColumn" => t.maximize_column(),
            "fullscreen" => {
                t.toggle_maximize();
                true
            }
            "center" => t.center_column(),
            "moveLeft" => t.move_column(FocusDir::Left),
            "moveRight" => t.move_column(FocusDir::Right),
            "moveUp" => t.move_pane_in_column(FocusDir::Up),
            "moveDown" => t.move_pane_in_column(FocusDir::Down),
            "consumeLeft" => t.consume_or_expel(FocusDir::Left),
            "consumeRight" => t.consume_or_expel(FocusDir::Right),
            "reset" => {
                t.reset_sizes();
                true
            }
            _ => false,
        };
        if changed {
            let _ = self.apply_tiled_sizes();
        }
        changed
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

    fn update_layout(&mut self, sidebar_w: f32, panel_w: f32) -> bool {
        self.sidebar_w = sidebar_w;
        self.panel_w = panel_w;
        self.apply_tiled_sizes()
    }

    fn new_terminal_size(&self, id: u64, stack_vertical: bool) -> (u16, u16) {
        let mut tiling = self.tiling.clone();
        tiling.open_pane(id, stack_vertical);
        let view = tiling.snapshot();
        view.columns
            .iter()
            .find_map(|column| {
                let index = column.panes.iter().position(|&pane| pane == id)?;
                Some(self.estimate_terminal_size(&view, column, index))
            })
            .unwrap_or((DEFAULT_COLS, DEFAULT_ROWS))
    }

    fn estimate_terminal_size(
        &self,
        view: &TilingView,
        column: &ColumnView,
        index: usize,
    ) -> (u16, u16) {
        let area_w = (self.viewport_w
            - self.sidebar_w
            - self.panel_w
            - WORKSPACE_MARGIN_PX
            - 2.0 * FRAME_INSET_PX)
            .max(0.0);
        let area_h =
            (self.viewport_h - TAB_BAR_HEIGHT_PX - 2.0 * (WORKSPACE_MARGIN_PX + FRAME_INSET_PX))
                .max(0.0);
        let tiled = !view.maximized
            && (view.columns.len() > 1 || column.panes.len() > 1 || column.width < 1.0 - 1e-3);
        let (grid_w, grid_h) = if view.maximized && column.panes[index] == view.focused_session {
            (area_w, area_h)
        } else {
            let gap = if tiled { PANE_GAP_PX } else { 0.0 };
            let inset = if tiled { FRAME_INSET_PX } else { 0.0 };
            let header = if tiled { PANE_HEADER_PX } else { 0.0 };
            let usable = (area_w - gap).max(0.0);
            let x = (column.x - view.view_offset) * usable;
            let pane_w = (x + column.width * usable).round() - (x + gap).round();
            let row_gap = if column.panes.len() > 1 {
                PANE_GAP_PX
            } else {
                0.0
            };
            let total = area_h - 2.0 * gap + row_gap;
            let start = column.heights[..index].iter().sum::<f32>();
            let pane_h = ((start + column.heights[index]) * total - row_gap).round()
                - (start * total).round();
            (pane_w - 2.0 * inset, pane_h - 2.0 * inset - header)
        };
        let font_size = f32::from(self.config.font_size.clamp(11, 18));
        let cols = ((grid_w - 2.0 * TERMINAL_PADDING_X) / (font_size * CELL_ADVANCE)) as u16;
        let rows = ((grid_h - 2.0 * TERMINAL_PADDING_Y) / (font_size * CELL_LINE_HEIGHT)) as u16;
        (cols.clamp(2, 400), rows.clamp(1, 200))
    }

    fn apply_tiled_sizes(&mut self) -> bool {
        let snapshot = self.tiling.snapshot();
        let mut focused_size = (self.cols, self.rows);
        for column in &snapshot.columns {
            for (index, &id) in column.panes.iter().enumerate() {
                if snapshot.maximized && id != snapshot.focused_session {
                    continue;
                }
                let size = if let Some(&size) = self.terminal_sizes.get(&id) {
                    // Once measured, only the grid reports future sizes. Repeating its last
                    // size here costs a synchronous round trip to an attached session server.
                    size
                } else {
                    let size = self.estimate_terminal_size(&snapshot, column, index);
                    if let Some(session) = self.sessions.iter().find(|s| s.id == id) {
                        let _ = session.resize(size.0, size.1);
                    }
                    size
                };
                if id == snapshot.focused_session {
                    focused_size = size;
                }
            }
        }
        let changed = focused_size != (self.cols, self.rows);
        (self.cols, self.rows) = focused_size;
        changed
    }

    fn sessions_payload(&self) -> SessionsPayload {
        let mut archived = Vec::with_capacity(self.archived.len());
        let mut items = Vec::new();
        for session in &self.sessions {
            let info = session.info();
            if let Some(&archived_at) = self.archived.get(&info.id) {
                archived.push(ArchivedSession {
                    session: info,
                    archived_at,
                });
            } else {
                items.push(info);
            }
        }
        archived
            .sort_unstable_by_key(|entry| std::cmp::Reverse((entry.archived_at, entry.session.id)));
        SessionsPayload {
            active_id: self.active_id(),
            archived_count: u32::try_from(archived.len()).unwrap_or(u32::MAX),
            archived,
            items,
        }
    }

    fn tiling_snapshot(&self) -> TilingView {
        self.tiling.snapshot()
    }

    fn take_image_uploads(&self) -> Vec<(String, icy_core::graphics::ImageData)> {
        self.sessions
            .iter()
            .flat_map(|s| {
                s.take_image_uploads()
                    .into_iter()
                    .map(|(seq, data)| (term_image_key(s.id, seq), data))
            })
            .collect()
    }

    fn visible_terminals(&self) -> impl Iterator<Item = &Terminal> {
        let ids: Vec<u64> = if self.overview {
            self.tiling.all_panes()
        } else {
            self.tiling.visible_pane_ids()
        };
        ids.into_iter()
            .filter_map(|id| self.sessions.iter().find(|s| s.id == id))
    }

    fn visible_screens(&self) -> BTreeMap<String, ScreenPayload> {
        self.visible_terminals()
            .map(|session| (session.id.to_string(), build_screen_payload(session)))
            .collect()
    }

    fn themes_payload(&self) -> ThemesPayload {
        ThemesPayload {
            themes_dir: config::themes_dir().display().to_string(),
            items: self.themes.clone(),
        }
    }
}

fn list_workspace_files(root: &Path, subdir: &str) -> FilesPayload {
    let mut entries = Vec::new();
    if Path::new(subdir)
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)))
        && let Ok(rd) = std::fs::read_dir(root.join(subdir))
    {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if matches!(name.as_str(), ".git" | "target" | "node_modules") {
                continue;
            }
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            let path = if subdir.is_empty() {
                name.clone()
            } else {
                format!("{subdir}/{name}")
            };
            entries.push(FileEntry { name, path, is_dir });
        }
    }

    entries
        .sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase(), entry.name.clone()));

    FilesPayload {
        cwd: root.display().to_string(),
        subdir: subdir.to_string(),
        entries,
    }
}

fn resolve_project_dir(base: &Path, raw: &str) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Enter a folder path.".into());
    }
    // TSX appends `/` when browsing folders; Windows verbatim `\\?\` paths reject forward slashes.
    let normalized = if raw.starts_with(r"\\?\") {
        std::borrow::Cow::Owned(raw.replace('/', "\\"))
    } else {
        std::borrow::Cow::Borrowed(raw)
    };
    let raw = normalized.as_ref();
    let path = if raw == "~" || raw.starts_with("~/") || raw.starts_with("~\\") {
        platform::home_dir()
            .ok_or_else(|| "Home folder is unavailable.".to_string())?
            .join(raw[1..].trim_start_matches(['/', '\\']))
    } else {
        base.join(raw)
    };
    let dir = path
        .canonicalize()
        .map_err(|err| format!("Could not open folder {raw}: {err}"))?;
    if !dir.is_dir() {
        return Err(format!("{raw} is not a folder."));
    }
    Ok(dir)
}

#[derive(Serialize)]
struct ProjectFolders {
    path: PathBuf,
    parent: Option<PathBuf>,
    /// Root-first chain above `path`, so the launcher can draw where it sits in the tree.
    ancestors: Vec<ProjectFolder>,
    /// Branch of the repository containing `path`, so the launcher shows where you stand.
    branch: Option<String>,
    folders: Vec<ProjectFolder>,
}

#[derive(Serialize)]
struct ProjectFolder {
    name: String,
    path: PathBuf,
    /// Set when the folder is a repository root.
    branch: Option<String>,
}

fn browse_project_dir(dir: PathBuf) -> Result<ProjectFolders, String> {
    let entries = std::fs::read_dir(&dir).map_err(|err| format!("Could not read folder: {err}"))?;
    let mut folders: Vec<ProjectFolder> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().into_string().ok()?;
            path.is_dir().then(|| ProjectFolder {
                name,
                branch: git::head_branch(&path),
                path,
            })
        })
        .collect();
    folders.sort_by_cached_key(|folder| folder.name.to_lowercase());
    let parent = dir.parent().map(Path::to_path_buf);
    let mut ancestors: Vec<ProjectFolder> = dir
        .ancestors()
        .skip(1)
        .map(|path| ProjectFolder {
            name: path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            ),
            path: path.to_path_buf(),
            branch: None,
        })
        .collect();
    ancestors.reverse();
    Ok(ProjectFolders {
        ancestors,
        branch: git::branch(&dir),
        path: dir,
        parent,
        folders,
    })
}

fn install_fonts(host: &mut Host) {
    host.add_font(INTER_REGULAR_TTF.to_vec());
    host.add_font(INTER_MEDIUM_TTF.to_vec());
    host.add_font(INTER_SEMIBOLD_TTF.to_vec());
    host.add_font(GEIST_MONO_TTF.to_vec());
    host.add_font(GEIST_MONO_SEMIBOLD_TTF.to_vec());
    host.set_default_family("Inter".into());
}

fn install_images_on_host(host: &mut Host, scale: f32) {
    for (name, rgba, size) in icons::load_static(scale) {
        host.set_image(name, rgba, size);
    }
    pixel_blast::install_masks(host);
}

fn push_images(pusher: &Pusher, scale: f32) {
    for (name, rgba, size) in icons::load_static(scale) {
        let _ = pusher.image(name, rgba, size);
    }
}

impl ScreenPayload {
    /// The UI keeps the previous frame's row at every index sent as `null`.
    fn update_since(&self, previous: Option<&Self>) -> ScreenUpdate<'_> {
        let previous_rows = previous.map_or(&[][..], |screen| screen.rows.as_slice());
        ScreenUpdate {
            meta: &self.meta,
            rows: self
                .rows
                .iter()
                .enumerate()
                .map(|(index, row)| (previous_rows.get(index) != Some(row)).then_some(row.as_ref()))
                .collect(),
        }
    }
}

impl TerminalScreens {
    fn update_since(&self, previous: Option<&Self>, sequence: u64) -> TerminalFrame<'_> {
        let previous_context = previous
            .filter(|old| old.context_id == self.context_id)
            .and_then(|old| old.context_screen.as_ref());
        TerminalFrame {
            sequence,
            screens: self
                .visible
                .iter()
                .map(|(&id, screen)| {
                    let previous = previous.and_then(|old| old.visible.get(&id));
                    (id, screen.update_since(previous))
                })
                .collect(),
            context_id: self.context_id,
            context_screen: self
                .context_screen
                .as_ref()
                .map(|screen| screen.update_since(previous_context)),
        }
    }
}

impl TerminalSnapshot {
    fn capture(session: &Terminal) -> Self {
        let (screen, placements) = session.frame();
        Self {
            id: session.id,
            cwd: session.cwd().to_path_buf(),
            screen,
            placements,
        }
    }

    fn into_payload(self, cache: &mut TerminalRowCache) -> ScreenPayload {
        let Self {
            id,
            cwd,
            screen,
            placements,
        } = self;
        let images = placements
            .into_iter()
            .map(|img| ImagePayload {
                key: term_image_key(id, img.seq),
                row: img.row,
                col: img.col,
                cols: img.cols,
                rows: img.rows,
                width: img.width,
                height: img.height,
            })
            .collect();
        let rows = terminal_rows_payload(&screen.rows, &cwd, cache);
        cache.source = screen.rows;
        cache.rendered.clone_from(&rows);
        ScreenPayload {
            meta: ScreenMeta {
                cols: screen.cols,
                rows_count: screen.rows_count,
                cursor_row: screen.cursor.row,
                cursor_col: screen.cursor.col,
                cursor_visible: screen.cursor.visible,
                display_offset: screen.display_offset,
                images,
            },
            rows,
        }
    }
}

fn build_screen_payload(session: &Terminal) -> ScreenPayload {
    TerminalSnapshot::capture(session).into_payload(&mut TerminalRowCache::default())
}

fn edit_preview_rows<'a>(
    start: usize,
    rows: &'a [VtRow],
    text: &'a [OnceCell<String>],
) -> impl Iterator<Item = Cow<'a, str>> {
    let mut next = start;
    std::iter::from_fn(move || {
        let start = next;
        let first = text.get(start)?.get_or_init(|| rows[start].plain_text());
        next += 1;
        if !rows[start].wrapped {
            return Some(Cow::Borrowed(first.as_str()));
        }
        let mut joined = first.clone();
        while next < rows.len() && rows[next - 1].wrapped {
            joined.push_str(text[next].get_or_init(|| rows[next].plain_text()));
            next += 1;
        }
        Some(Cow::Owned(joined))
    })
}

fn osc8_links_for_logical_line(
    screen_rows: &[VtRow],
    range: std::ops::Range<usize>,
) -> Vec<LinkRef> {
    let mut out = Vec::new();
    let mut byte_cursor = 0usize;
    let mut active: Option<(String, usize, usize)> = None;

    let flush = |active: &mut Option<(String, usize, usize)>, out: &mut Vec<LinkRef>| {
        if let Some((uri, start, end)) = active.take()
            && end > start
            && let Some(link) = links::parse_hyperlink_uri(&uri, start, end)
        {
            out.push(link);
        }
    };

    for row in &screen_rows[range] {
        for run in &row.runs {
            let run_start = byte_cursor;
            let run_end = run_start + run.text.len();
            byte_cursor = run_end;
            match (&mut active, &run.hyperlink) {
                (Some((uri, _, end)), Some(next_uri)) if uri == next_uri && *end == run_start => {
                    *end = run_end;
                }
                (_, Some(next_uri)) => {
                    flush(&mut active, &mut out);
                    active = Some((next_uri.clone(), run_start, run_end));
                }
                (_, None) => flush(&mut active, &mut out),
            }
        }
    }
    flush(&mut active, &mut out);
    out
}

fn terminal_rows_payload(
    screen_rows: &[VtRow],
    cwd: &Path,
    cache: &TerminalRowCache,
) -> Vec<Arc<RowPayload>> {
    let mut rows = Vec::with_capacity(screen_rows.len());
    let plain_rows: Vec<OnceCell<String>> = vec![OnceCell::new(); screen_rows.len()];

    let mut group_start = 0usize;
    while group_start < screen_rows.len() {
        let mut group_end = group_start;
        while group_end + 1 < screen_rows.len() && screen_rows[group_end].wrapped {
            group_end += 1;
        }
        let group_range = group_start..group_end + 1;
        // Reuse whole logical lines. Links can span rows, change as files appear, or contain
        // diff previews taken from later rows, so keep resolving those lines on every frame.
        if cache.source.get(group_range.clone()) == Some(&screen_rows[group_range.clone()])
            && (group_start == 0 || !cache.source[group_start - 1].wrapped)
            && (!screen_rows[group_end].wrapped || cache.source.len() == screen_rows.len())
            && cache.rendered[group_range.clone()]
                .iter()
                .all(|row| row.links.is_empty())
        {
            rows.extend_from_slice(&cache.rendered[group_range]);
            group_start = group_end + 1;
            continue;
        }

        let mut logical_plain = String::new();
        let mut row_ranges: Vec<(usize, usize)> = Vec::with_capacity(group_range.len());
        for idx in group_range.clone() {
            let start = logical_plain.len();
            logical_plain.push_str(plain_rows[idx].get_or_init(|| screen_rows[idx].plain_text()));
            row_ranges.push((start, logical_plain.len()));
        }

        // Prefer explicit OSC 8 hyperlinks over regex/token heuristics on overlapping ranges.
        let mut detected = osc8_links_for_logical_line(screen_rows, group_range.clone());
        let osc8_ranges: Vec<(usize, usize)> = detected.iter().map(|r| (r.start, r.end)).collect();
        for link in links::detect(&logical_plain) {
            if !osc8_ranges
                .iter()
                .any(|&(s, e)| link.start < e && link.end > s)
            {
                detected.push(link);
            }
        }
        detected.sort_by_key(|r| r.start);

        let logical_links: Vec<LinkPayload> = detected
            .iter()
            .map(|r| {
                let resolved = resolve_link_path(r, cwd).is_some();
                let edit = (r.kind == RefKind::Diff).then(|| {
                    links::edit_preview(
                        &logical_plain,
                        edit_preview_rows(group_end + 1, screen_rows, &plain_rows),
                    )
                });
                let changed = edit.as_ref().and_then(|patch| {
                    patch
                        .lines
                        .iter()
                        .find(|line| line.kind != git::DiffKind::Ctx)
                });
                let (line, old_line) = r.line.map_or_else(
                    || {
                        changed.map_or((None, false), |line| {
                            (
                                line.new_num.or(line.old_num),
                                line.kind == git::DiffKind::Del,
                            )
                        })
                    },
                    |line| (Some(line), false),
                );
                LinkPayload {
                    kind: r.kind,
                    target: r.target.clone(),
                    line,
                    col: r.col,
                    old_line,
                    edit,
                    resolved,
                    image: resolved && r.kind == RefKind::File && links::is_image_path(&r.target),
                }
            })
            .collect();

        for (offset, idx) in group_range.enumerate() {
            let row = &screen_rows[idx];
            let plain = plain_rows[idx].get().unwrap().clone();
            let (row_byte_start, row_byte_end) = row_ranges[offset];

            let mut row_detected: Vec<(usize, usize)> = Vec::new();
            let mut links: Vec<LinkPayload> = Vec::new();
            for (link_idx, r) in detected.iter().enumerate() {
                if r.start < row_byte_end && r.end > row_byte_start {
                    let local_start = r.start.saturating_sub(row_byte_start);
                    let local_end = r.end.min(row_byte_end) - row_byte_start;
                    if local_end > local_start {
                        row_detected.push((local_start, local_end));
                        links.push(logical_links[link_idx].clone());
                    }
                }
            }

            let mut spans = Vec::new();
            if !plain.is_empty() {
                let mut cursor = 0usize;
                for run in &row.runs {
                    let run_start = cursor;
                    let run_end = run_start + run.text.len();
                    cursor = run_end;

                    if row_detected.is_empty() {
                        spans.push(SpanPayload {
                            text: run.text.clone(),
                            col: run.col,
                            cells: run.cells,
                            fg: run.fg,
                            bg: (run.bg != DEFAULT_BG).then_some(run.bg),
                            bold: run.bold,
                            underline: run.underline,
                            link: false,
                            image: false,
                            selected: run.selected,
                            link_index: None,
                        });
                        continue;
                    }

                    let splittable = run.has_one_char_per_cell();
                    let mut cuts = BTreeSet::new();
                    cuts.insert(run_start);
                    cuts.insert(run_end);
                    if splittable {
                        for &(start, end) in &row_detected {
                            for cut in [start, end] {
                                if cut > run_start
                                    && cut < run_end
                                    && run.text.is_char_boundary(cut - run_start)
                                {
                                    cuts.insert(cut);
                                }
                            }
                        }
                    }

                    let points: Vec<usize> = cuts.into_iter().collect();
                    let mut col = run.col;
                    for pair in points.windows(2) {
                        let (a, b) = (pair[0], pair[1]);
                        let hit = row_detected.iter().position(|&(s, e)| a < e && b > s);
                        let is_link = hit.is_some();
                        let is_image = hit.is_some_and(|i| links[i].image);
                        let text = &plain[a..b];
                        let cells = if splittable && points.len() > 2 {
                            text.chars().count() as u16
                        } else {
                            run.cells
                        };
                        spans.push(SpanPayload {
                            text: text.to_string(),
                            col,
                            cells,
                            fg: run.fg,
                            bg: (run.bg != DEFAULT_BG).then_some(run.bg),
                            bold: run.bold,
                            underline: (run.underline || is_link) && !is_image,
                            link: is_link,
                            image: is_image,
                            selected: run.selected,
                            link_index: hit,
                        });
                        col += cells;
                    }
                }
            }

            rows.push(Arc::new(RowPayload {
                idx,
                plain,
                spans,
                links,
            }));
        }

        group_start = group_end + 1;
    }

    rows
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

    let resolved = resolve_link_path(&link_ref, cwd);
    let Some(ResolvedTarget::File {
        path: target_path, ..
    }) = resolved
    else {
        return ViewerPayload::error(
            raw_path,
            raw_path.to_string(),
            line,
            col,
            format!("File not found from {}", cwd.display()),
        );
    };

    let display_path = target_path
        .strip_prefix(cwd)
        .map(|p| p.display().to_string().replace('\\', "/"))
        .unwrap_or_else(|_| target_path.display().to_string());

    let bytes = match document::read_regular(&target_path, document::MAX_VIEW_BYTES) {
        Ok(bytes) => bytes,
        Err(err) => {
            return ViewerPayload::error(
                &target_path.display().to_string(),
                display_path,
                line,
                col,
                format!("Unable to read file: {err}"),
            );
        }
    };

    let mut view = ViewerPayload {
        open: true,
        path: target_path.display().to_string(),
        display_path,
        target_line: line,
        target_col: col,
        ..ViewerPayload::closed()
    };
    if font_preview::is_font(&target_path) {
        match font_preview::render(&bytes) {
            Some(sample) => {
                view.preview = Some(FilePreview::Font {
                    name: sample.name,
                    key: sample.key,
                    size: sample.size,
                });
                view.preview_pixels = Some(sample.pixels);
            }
            None => view.error = Some("Unsupported or invalid font file.".into()),
        }
        return view;
    }
    let Some(content) = document::text(&bytes) else {
        view.preview = Some(FilePreview::Binary);
        return view;
    };
    let source = content.strip_prefix('\u{feff}').unwrap_or(content);
    if source.lines().take(document::MAX_VIEW_LINES + 1).count() > document::MAX_VIEW_LINES {
        view.error = Some("Too many lines for the built-in viewer. Use Open in…".into());
        return view;
    }
    match document::editable(content) {
        Ok(()) => view.editable_text = Some(content.to_owned()),
        Err(reason) => view.edit_unavailable = Some(reason),
    }
    view.text_info = Some(document::text_info(&view.display_path, content));
    let lang = syntax::lang_for_path(&view.display_path);
    let target_1based = line.map(|l| l.max(1) as usize);

    // Bound highlighting and JSON size for generated/minified files. The source stays intact
    // for copy/context; the UI clips the visual representation of pathological lines.
    let highlight = bytes.len() <= 512 * 1024;
    view.lines = source
        .lines()
        .enumerate()
        .map(|(offset, text)| {
            let num = offset + 1;
            ViewerLine {
                num,
                text: text.to_string(),
                highlighted: target_1based == Some(num),
                highlights: if highlight && text.len() <= 1024 {
                    syntax::highlight_ranges(text, lang)
                } else {
                    Vec::new()
                },
            }
        })
        .collect();

    view
}

fn resolve_git_rel_path(path: &str, root: &Path) -> String {
    let link = LinkRef {
        start: 0,
        end: path.len(),
        kind: RefKind::Diff,
        target: path.to_string(),
        line: None,
        col: None,
    };
    if let Some(ResolvedTarget::File { path: abs, .. }) = resolve_link_path(&link, root)
        && let Ok(rel) = abs.strip_prefix(root)
    {
        return rel.display().to_string().replace('\\', "/");
    }
    path.replace('\\', "/")
}

fn open_diff_for_viewer(
    path: &str,
    root: &Path,
    line: Option<u32>,
    old_line: bool,
) -> ViewerPayload {
    let resolved_rel = resolve_git_rel_path(path, root);
    let Some(lines) = git::file_diff(root, &resolved_rel) else {
        return ViewerPayload::error(
            path,
            resolved_rel,
            line,
            None,
            "git diff failed for this file".into(),
        );
    };
    diff_view(&resolved_rel, root, line, old_line, lines)
}

fn open_edit_for_viewer(
    path: &str,
    root: &Path,
    line: Option<u32>,
    old_line: bool,
    edit: &links::EditPreview,
) -> ViewerPayload {
    let resolved_rel = resolve_git_rel_path(path, root);
    if !edit.lines.is_empty() {
        return diff_view(&resolved_rel, root, line, old_line, edit.lines.clone());
    }
    if let Some(counts) = edit.counts
        && let Some(lines) = git::file_diff(root, &resolved_rel)
        && let Some(fragment) = git::edit_fragment(&lines, counts, line, old_line)
    {
        return diff_view(&resolved_rel, root, line, old_line, fragment.to_vec());
    }
    ViewerPayload::error(
        path,
        resolved_rel,
        line,
        None,
        "This edit could not be isolated from the CLI output or working tree.".into(),
    )
}

fn diff_view(
    path: &str,
    root: &Path,
    line: Option<u32>,
    old_line: bool,
    mut lines: Vec<DiffLine>,
) -> ViewerPayload {
    let display_path = path.replace('\\', "/");
    let lang = syntax::lang_for_path(&display_path);
    let mut additions = 0;
    let mut deletions = 0;
    lines.retain(|line| match line.kind {
        git::DiffKind::Hunk => false,
        git::DiffKind::Add => {
            additions += 1;
            true
        }
        git::DiffKind::Del => {
            deletions += 1;
            true
        }
        git::DiffKind::Ctx => true,
    });
    let diff_focus = git::diff_focus(&lines, line, old_line);
    let diff = lines
        .into_iter()
        .map(|line| DiffView {
            highlights: if line.text.len() <= 1024 {
                syntax::highlight_ranges(&line.text, lang)
            } else {
                Vec::new()
            },
            line,
        })
        .collect();
    ViewerPayload {
        open: true,
        path: root.join(path).display().to_string(),
        display_path,
        mode: "diff",
        target_line: line,
        diff_focus,
        diff,
        additions,
        deletions,
        ..ViewerPayload::closed()
    }
}

fn spawn_changes_probe(root: PathBuf, tx: mpsc::Sender<BridgeEvent>) -> Result<(), String> {
    std::thread::Builder::new()
        .name("icy-git-changes".into())
        .spawn(move || {
            let changes = git::changes(&root);
            let _ = tx.send(BridgeEvent::PushChanges(root, changes));
        })
        .map(|_| ())
        .map_err(|error| format!("Cannot start changes loader: {error}"))
}

struct ViewerRequest {
    path: String,
    cwd: PathBuf,
    diff: bool,
    line: Option<u32>,
    col: Option<u32>,
    old_line: bool,
    edit: Option<links::EditPreview>,
    background: bool,
    intent: Option<u64>,
}

impl ViewerRequest {
    fn load(self, generation: u64) -> ViewerPayload {
        let mut view = if self.diff {
            let mut view = match &self.edit {
                Some(edit) => {
                    open_edit_for_viewer(&self.path, &self.cwd, self.line, self.old_line, edit)
                }
                None => open_diff_for_viewer(&self.path, &self.cwd, self.line, self.old_line),
            };
            // Errors must be cached under the requested mode too.
            view.mode = "diff";
            view
        } else {
            open_file_for_viewer(&self.path, self.line, self.col, &self.cwd)
        };
        view.generation = generation;
        view.intent = self.intent;
        view.old_line = self.old_line;
        view.edit = self.edit;
        view.target_col = self.col;
        view.cwd = self.cwd;
        view.requested_path = self.path;
        view.background = self.background;
        view
    }
}

fn spawn_viewer_probe(request: ViewerRequest, tx: mpsc::Sender<BridgeEvent>) -> Result<(), String> {
    let generation = if request.background {
        0
    } else {
        VIEWER_GENERATION.fetch_add(1, Ordering::Relaxed) + 1
    };
    std::thread::Builder::new()
        .name("icy-document".into())
        .spawn(move || {
            let _ = tx.send(BridgeEvent::PushViewer(request.load(generation)));
        })
        .map(|_| ())
        .map_err(|error| format!("Cannot start document loader: {error}"))
}

struct CodePreviewRequest {
    key: String,
    path: String,
    cwd: PathBuf,
    diff: bool,
    line: Option<u32>,
    old_line: bool,
    edit: Option<links::EditPreview>,
}

fn truncate_preview_line(text: &str) -> String {
    let mut chars = text.char_indices();
    if let Some((idx, _)) = chars.nth(CODE_PREVIEW_MAX_CHARS) {
        let mut clipped = text[..idx].to_string();
        clipped.push('…');
        clipped
    } else {
        text.to_string()
    }
}

fn preview_file_snippet(
    key: String,
    raw_path: &str,
    line: Option<u32>,
    cwd: &Path,
) -> CodePreviewPayload {
    let link_ref = LinkRef {
        start: 0,
        end: raw_path.len(),
        kind: RefKind::File,
        target: raw_path.to_string(),
        line,
        col: None,
    };
    let Some(ResolvedTarget::File {
        path: target_path, ..
    }) = resolve_link_path(&link_ref, cwd)
    else {
        return CodePreviewPayload::error(
            key,
            "file",
            raw_path.replace('\\', "/"),
            line,
            false,
            format!("File not found from {}", cwd.display()),
        );
    };

    let display_path = target_path
        .strip_prefix(cwd)
        .map(|p| p.display().to_string().replace('\\', "/"))
        .unwrap_or_else(|_| target_path.display().to_string().replace('\\', "/"));

    if std::fs::metadata(&target_path).is_ok_and(|meta| meta.len() > CODE_PREVIEW_MAX_BYTES) {
        return CodePreviewPayload::error(
            key,
            "file",
            display_path,
            line,
            false,
            "File too large to preview.".into(),
        );
    }

    let bytes = match std::fs::read(&target_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            return CodePreviewPayload::error(
                key,
                "file",
                display_path,
                line,
                false,
                format!("Unable to read file: {err}"),
            );
        }
    };

    if bytes.iter().take(8192).any(|&b| b == 0) {
        return CodePreviewPayload::error(
            key,
            "file",
            display_path,
            line,
            false,
            "Binary file".into(),
        );
    }

    let Ok(content) = String::from_utf8(bytes) else {
        return CodePreviewPayload::error(
            key,
            "file",
            display_path,
            line,
            false,
            "Binary or non-UTF-8 file".into(),
        );
    };

    let all_lines: Vec<&str> = content.lines().collect();
    let total_lines = all_lines.len();
    let target_1based = line.map(|l| l.max(1) as usize);
    let focus = target_1based.map(|l| l.saturating_sub(1));
    let (start, end) =
        syntax::preview_window(total_lines, focus, CODE_PREVIEW_LINES, CODE_PREVIEW_CONTEXT);
    let lang = syntax::lang_for_path(&display_path);
    let lines = all_lines[start..end]
        .iter()
        .enumerate()
        .map(|(offset, raw)| {
            let num = start + offset + 1;
            let text = truncate_preview_line(raw);
            let highlights = syntax::highlight_ranges(&text, lang);
            ViewerLine {
                num,
                text,
                highlighted: target_1based == Some(num),
                highlights,
            }
        })
        .collect();

    CodePreviewPayload {
        key,
        mode: "file",
        display_path,
        target_line: line,
        single_edit: false,
        total_lines,
        truncated: start > 0 || end < total_lines,
        diff_focus: None,
        additions: 0,
        deletions: 0,
        lines,
        diff: Vec::new(),
        error: None,
    }
}

fn preview_diff_snippet(
    key: String,
    path: &str,
    root: &Path,
    line: Option<u32>,
    old_line: bool,
    edit: Option<&links::EditPreview>,
) -> CodePreviewPayload {
    let display_path = path.replace('\\', "/");
    let single_edit = edit.is_some();
    let mut raw_lines = match edit {
        Some(edit) if !edit.lines.is_empty() => edit.lines.clone(),
        Some(edit) => {
            if let Some(counts) = edit.counts
                && let Some(lines) = git::file_diff(root, path)
                && let Some(fragment) = git::edit_fragment(&lines, counts, line, old_line)
            {
                fragment.to_vec()
            } else {
                return CodePreviewPayload::error(
                    key,
                    "diff",
                    display_path,
                    line,
                    true,
                    "This edit could not be isolated from the CLI output or working tree.".into(),
                );
            }
        }
        None => match git::file_diff(root, path) {
            Some(lines) => lines,
            None => {
                return CodePreviewPayload::error(
                    key,
                    "diff",
                    display_path,
                    line,
                    false,
                    "git diff failed for this file".into(),
                );
            }
        },
    };

    let mut additions = 0;
    let mut deletions = 0;
    raw_lines.retain(|row| match row.kind {
        git::DiffKind::Hunk => false,
        git::DiffKind::Add => {
            additions += 1;
            true
        }
        git::DiffKind::Del => {
            deletions += 1;
            true
        }
        git::DiffKind::Ctx => true,
    });

    let total_lines = raw_lines.len();
    let focus = git::diff_focus(&raw_lines, line, old_line);
    let (start, end) =
        syntax::preview_window(total_lines, focus, CODE_PREVIEW_LINES, CODE_PREVIEW_CONTEXT);
    let lang = syntax::lang_for_path(&display_path);
    let diff: Vec<DiffView> = raw_lines[start..end]
        .iter()
        .map(|row| {
            let text = truncate_preview_line(&row.text);
            let highlights = syntax::highlight_ranges(&text, lang);
            DiffView {
                line: DiffLine {
                    kind: row.kind,
                    old_num: row.old_num,
                    new_num: row.new_num,
                    text,
                },
                highlights,
            }
        })
        .collect();
    let diff_focus = focus
        .and_then(|idx| idx.checked_sub(start))
        .filter(|&idx| idx < diff.len());

    CodePreviewPayload {
        key,
        mode: "diff",
        display_path,
        target_line: line,
        single_edit,
        total_lines,
        truncated: start > 0 || end < total_lines,
        diff_focus,
        additions,
        deletions,
        lines: Vec::new(),
        diff,
        error: None,
    }
}

fn build_code_preview(request: CodePreviewRequest) -> CodePreviewPayload {
    if request.diff {
        preview_diff_snippet(
            request.key,
            &request.path,
            &request.cwd,
            request.line,
            request.old_line,
            request.edit.as_ref(),
        )
    } else {
        preview_file_snippet(request.key, &request.path, request.line, &request.cwd)
    }
}

fn spawn_code_preview(
    request: CodePreviewRequest,
    tx: mpsc::Sender<BridgeEvent>,
) -> Result<(), String> {
    std::thread::Builder::new()
        .name("icy-code-preview".into())
        .spawn(move || {
            let _ = tx.send(BridgeEvent::CodePreview(build_code_preview(request)));
        })
        .map(|_| ())
        .map_err(|error| format!("Cannot start code preview loader: {error}"))
}

/// Kept alive for the whole process: on X11/Wayland the copied text lives as long as its owner.
fn copy_to_clipboard(text: &str) {
    static CLIPBOARD: std::sync::OnceLock<Mutex<jsnir_gpu::input::Clipboard>> =
        std::sync::OnceLock::new();
    CLIPBOARD
        .get_or_init(|| Mutex::new(jsnir_gpu::input::Clipboard::default()))
        .lock()
        .unwrap()
        .set_text(text.to_string());
}

fn push_view<T: Serialize>(pusher: &Pusher, view: &str, data: &T) {
    #[derive(Serialize)]
    struct Message<'a, T> {
        view: &'a str,
        data: &'a T,
    }
    let Ok(payload) = serde_json::to_string(&Message { view, data }) else {
        return;
    };
    // A queued full session list can be replaced by a newer list.
    let _ = match view {
        "sessions" => pusher.push_keyed("sessions", payload),
        _ => pusher.push(payload),
    };
}

fn spawn_editors_probe(tx: mpsc::Sender<BridgeEvent>) {
    let _ = std::thread::Builder::new()
        .name("icy-editors-probe".into())
        .spawn(move || {
            let editors = platform::detect_editors();
            let _ = tx.send(BridgeEvent::EditorsUpdated(editors));
        });
}

fn spawn_github_probe(cwd: PathBuf, tx: mpsc::Sender<BridgeEvent>) {
    let _ = std::thread::Builder::new()
        .name("icy-github-probe".into())
        .spawn(move || {
            let status = github::detect_github(&cwd);
            let login = status.username.clone();
            let _ = tx.send(BridgeEvent::GitHubUpdated(status));
            if let Some(login) = login {
                let dest = std::env::temp_dir().join(format!("icy-avatar-{login}.img"));
                if github::fetch_avatar(&login, &dest) {
                    let _ = tx.send(BridgeEvent::AvatarReady(dest));
                }
            }
        });
}

fn spawn_update_check(etag: Option<String>, tx: mpsc::Sender<BridgeEvent>) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("icy-update-check".into())
        .spawn(move || {
            let result = updater::check_latest_release(etag.as_deref());
            let _ = tx.send(BridgeEvent::UpdateChecked(result));
        })?;
    Ok(())
}

fn spawn_update_download(manager: &Arc<Mutex<SessionManager>>, tx: &mpsc::Sender<BridgeEvent>) {
    let asset = {
        let mut guard = manager.lock().unwrap();
        if !guard.update.has_update || !guard.update.can_check() {
            return;
        }
        let Some(asset) = guard.update.asset.clone() else {
            return;
        };
        guard.update.stage = "downloading".into();
        guard.update.progress = 8;
        guard.update.status_text = "Starting download...".into();
        asset
    };
    let _ = tx.send(BridgeEvent::UpdateChanged);

    let worker_manager = Arc::clone(manager);
    let worker_tx = tx.clone();
    let worker = std::thread::Builder::new()
        .name("icy-update-download".into())
        .spawn(move || {
            let result = updater::download_and_stage(&asset, |stage, progress, status| {
                let mut guard = worker_manager.lock().unwrap();
                guard.update.stage = stage.into();
                guard.update.progress = progress;
                guard.update.status_text = status.into();
                let _ = worker_tx.send(BridgeEvent::UpdateChanged);
            });
            let mut guard = worker_manager.lock().unwrap();
            match result {
                Ok(path) => {
                    guard.update.stage = "ready".into();
                    guard.update.progress = 100;
                    guard.update.status_text = "Ready to install & restart".into();
                    guard.update.staged_path = Some(path);
                }
                Err(error) => {
                    guard.update.stage = "error".into();
                    guard.update.status_text = error;
                }
            }
            let _ = worker_tx.send(BridgeEvent::UpdateChanged);
        });
    if let Err(error) = worker {
        let mut guard = manager.lock().unwrap();
        guard.update.stage = "error".into();
        guard.update.status_text = format!("Cannot start the download: {error}");
        let _ = tx.send(BridgeEvent::UpdateChanged);
    }
}

fn handle_request(
    raw: &str,
    manager: &Arc<Mutex<SessionManager>>,
    tx: &mpsc::Sender<BridgeEvent>,
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
        "terminalFrameReceived" => {
            let sequence = payload["sequence"]
                .as_u64()
                .ok_or("Missing frame sequence")?;
            let _ = tx.send(BridgeEvent::TerminalFrameReceived(sequence));
            Ok("ok".into())
        }
        "scanUsage" => {
            tx.send(BridgeEvent::ScanUsage {
                tx: tx.clone(),
                refresh: payload["refresh"].as_bool().unwrap_or(true),
            })
            .map_err(|error| error.to_string())?;
            Ok("ok".into())
        }
        "testNotification" => {
            tx.send(BridgeEvent::TestNotification(tx.clone()))
                .map_err(|error| error.to_string())?;
            Ok("ok".into())
        }
        "saveEditor" => {
            let token = payload
                .get("token")
                .and_then(Value::as_u64)
                .ok_or("Missing save token")?;
            let path = PathBuf::from(payload["path"].as_str().ok_or("Missing file path")?);
            let expected = payload["expected"]
                .as_str()
                .ok_or("Missing original text")?
                .to_owned();
            let draft = payload["text"].as_str().ok_or("Missing draft")?.to_owned();
            if !path.is_absolute() || font_preview::is_font(&path) {
                return Err("Not an editable file path".into());
            }
            let tx = tx.clone();
            std::thread::Builder::new()
                .name("icy-editor-save".into())
                .spawn(move || {
                    let result = match document::save(&path, &expected, &draft) {
                        Ok(saved) => serde_json::json!({ "token": token, "saved": saved }),
                        Err(error) => {
                            serde_json::json!({ "token": token, "error": error.to_string() })
                        }
                    };
                    let _ = tx.send(BridgeEvent::EditorSaved(result));
                })
                .map_err(|error| error.to_string())?;
            Ok("ok".into())
        }
        "gitBlame" => {
            let raw_path = payload["path"]
                .as_str()
                .ok_or("Missing file path")?
                .to_owned();
            let path = PathBuf::from(&raw_path);
            let line = payload["line"]
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .filter(|v| *v >= 1)
                .ok_or("Missing line")?;
            if !path.is_absolute() {
                return Err("Not a file path".into());
            }
            let tx = tx.clone();
            std::thread::Builder::new()
                .name("icy-git-blame".into())
                .spawn(move || {
                    let blame = git::blame_line(&path, line);
                    let branch = path.parent().and_then(git::branch);
                    let result = serde_json::json!({
                        "path": raw_path,
                        "line": line,
                        "blame": blame,
                        "branch": branch,
                    });
                    let _ = tx.send(BridgeEvent::GitBlame(result));
                })
                .map_err(|error| error.to_string())?;
            Ok("ok".into())
        }
        "pasteEditorContext" => {
            let id = payload["id"].as_u64().ok_or("Missing CLI session")?;
            let cwd = PathBuf::from(payload["cwd"].as_str().ok_or("Missing project path")?);
            let cli = payload["cli"].as_str().ok_or("Missing CLI")?.to_owned();
            let text = payload["text"]
                .as_str()
                .ok_or("Missing selected text")?
                .to_owned();
            tx.send(BridgeEvent::PasteContext { id, cwd, cli, text })
                .map_err(|error| error.to_string())?;
            Ok("ok".into())
        }
        "open" | "openDiff" => {
            let path = payload
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing path".to_string())?;
            let link_kind = payload
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("file");
            if kind == "open" && link_kind == "url" {
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
            let session_id = payload.get("sessionId").and_then(Value::as_u64);
            let session_cwd = {
                let guard = manager.lock().unwrap();
                payload
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .or_else(|| {
                        session_id
                            .and_then(|id| guard.sessions.iter().find(|s| s.id == id))
                            .map(|s| s.cwd().to_path_buf())
                    })
                    .unwrap_or_else(|| guard.workspace_dir().to_path_buf())
            };
            // Git, file IO and tokenization must never block QuickJS's synchronous request hook.
            spawn_viewer_probe(
                ViewerRequest {
                    path: path.to_owned(),
                    cwd: session_cwd,
                    diff: kind == "openDiff",
                    line,
                    col,
                    old_line: payload
                        .get("oldLine")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    edit: payload
                        .get("edit")
                        .map(|edit| serde_json::from_value(edit.clone()))
                        .transpose()
                        .map_err(|error| error.to_string())?,
                    background: payload
                        .get("background")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    intent: payload.get("intent").and_then(Value::as_u64),
                },
                tx.clone(),
            )?;
            Ok("ok".into())
        }
        "closeViewer" => {
            VIEWER_GENERATION.fetch_add(1, Ordering::Relaxed);
            let _ = tx.send(BridgeEvent::PushViewer(ViewerPayload::closed()));
            Ok("ok".into())
        }
        "gitChanges" => {
            let root = manager.lock().unwrap().request_workspace(&payload);
            spawn_changes_probe(root, tx.clone())?;
            Ok("ok".into())
        }
        "layout" => {
            let sidebar = payload
                .get("sidebarWidth")
                .and_then(Value::as_f64)
                .map_or(SIDEBAR_WIDTH_PX, |w| (w as f32).clamp(0.0, 480.0));
            let panel = payload
                .get("panelWidth")
                .and_then(Value::as_f64)
                .map_or(0.0, |w| (w as f32).clamp(0.0, 1200.0));
            let changed = manager.lock().unwrap().update_layout(sidebar, panel);
            if changed {
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        "newSession" | "openColumn" | "stackVertical" | "launchCli" => {
            let mut guard = manager.lock().unwrap();
            let root = guard.workspace_dir();
            let dir = match payload.get("cwd").and_then(Value::as_str) {
                Some(path) => resolve_project_dir(root, path)?,
                None => root.to_path_buf(),
            };
            let prompt = payload
                .get("prompt")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty());
            if kind == "launchCli" {
                let cli_id = payload
                    .get("cliId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing cliId".to_string())?;
                guard.launch_cli(cli_id, dir, prompt)?;
            } else {
                let id = guard.create_session_in(kind == "stackVertical", dir)?;
                if let Some(text) = prompt
                    && let Some(session) = guard.sessions.iter_mut().find(|s| s.id == id)
                {
                    let _ = session.write_input(format!("{text}\r").as_bytes());
                }
            }
            let _ = tx.send(BridgeEvent::SessionsDirty);
            let _ = tx.send(BridgeEvent::TilingDirty);
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok("ok".into())
        }
        "archiveSession" | "restoreSession" => {
            let id = payload["id"].as_u64().ok_or("Missing session ID")?;
            let mut guard = manager.lock().unwrap();
            match guard.set_session_archived(id, kind == "archiveSession") {
                Ok(true) => {
                    let _ = tx.send(BridgeEvent::TilingDirty);
                    let _ = tx.send(BridgeEvent::PtyDirty);
                }
                Ok(false) => {}
                Err(error) => {
                    let message = format!("Could not update the session archive: {error}");
                    let _ = tx.send(BridgeEvent::PushNotice {
                        kind: "error".into(),
                        text: message.clone(),
                    });
                    return Err(message);
                }
            }
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
        "openTile" => {
            let stack = payload
                .get("stack")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut guard = manager.lock().unwrap();
            let root = guard.request_workspace(&payload);
            let id = guard.open_tile(stack, root);
            let _ = tx.send(BridgeEvent::SessionsDirty);
            let _ = tx.send(BridgeEvent::TilingDirty);
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok(id.to_string())
        }
        "selectSession" => {
            let id = payload
                .get("id")
                .and_then(Value::as_u64)
                .ok_or_else(|| "missing session id".to_string())?;
            let mut guard = manager.lock().unwrap();
            if guard.active_id() != id && guard.select_session(id) {
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
        "overview" => {
            let on = payload.get("on").and_then(Value::as_bool).unwrap_or(false);
            manager.lock().unwrap().overview = on;
            window::OVERVIEW_CAPTURE.store(on, Ordering::Relaxed);
            let _ = tx.send(BridgeEvent::PtyDirty);
            Ok("ok".into())
        }
        "keyCapture" => {
            let on = payload.get("on").and_then(Value::as_bool).unwrap_or(false);
            window::KEY_CAPTURE.store(on, std::sync::atomic::Ordering::Relaxed);
            Ok("ok".into())
        }
        "editorPreview" => {
            let on = payload.get("on").and_then(Value::as_bool).unwrap_or(false);
            window::EDITOR_PREVIEW.store(on, Ordering::Relaxed);
            Ok("ok".into())
        }
        "setKeybind" | "resetKeybind" => {
            let id = payload.get("id").and_then(Value::as_str).unwrap_or("");
            let mut guard = manager.lock().unwrap();
            let mut overrides = guard.config.keybinds.clone();
            let taken = if kind == "resetKeybind" {
                keybinds::reset(&mut overrides, id);
                Vec::new()
            } else {
                // `chord` empty or missing unbinds the action.
                let chord = payload
                    .get("chord")
                    .and_then(Value::as_str)
                    .filter(|c| !c.is_empty())
                    .map(|c| {
                        keybinds::Chord::parse(c).ok_or_else(|| format!("invalid shortcut: {c}"))
                    })
                    .transpose()?;
                keybinds::assign(&mut overrides, id, chord.as_ref())?
            };
            *window::KEYMAP.write().unwrap() = keybinds::Keymap::new(&overrides);
            guard.config.keybinds = overrides;
            let _ = config::save_config(&guard.config);
            let _ = tx.send(BridgeEvent::KeybindsDirty(taken));
            Ok("ok".into())
        }
        "tilingResize" => {
            let delta = payload.get("delta").and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let column = payload
                .get("column")
                .and_then(Value::as_u64)
                .ok_or_else(|| "missing resize column".to_string())?;
            let row = payload
                .get("row")
                .and_then(Value::as_u64)
                .map(|r| r as usize);
            let left = payload.get("left").and_then(Value::as_bool) == Some(true);
            let mut guard = manager.lock().unwrap();
            if payload.get("workspace").and_then(Value::as_u64)
                != Some(guard.tiling.active_index() as u64)
            {
                return Err("resize workspace changed".into());
            }
            if payload.get("preview").and_then(Value::as_bool) == Some(true) {
                let mut layout = (*guard.tiling).clone();
                layout.resize_at(column, row, delta, left);
                return serde_json::to_string(&layout.snapshot()).map_err(|e| e.to_string());
            }
            if guard.tiling.resize_at(column, row, delta, left) {
                let _ = guard.apply_tiled_sizes();
                let _ = tx.send(BridgeEvent::TilingDirty);
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            serde_json::to_string(&guard.tiling.snapshot()).map_err(|e| e.to_string())
        }
        // Overview: activate a workspace, or drop a window on one ({id, index, insert}).
        "focusWorkspace" => {
            let index = payload.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            if manager
                .lock()
                .unwrap()
                .workspace_step(|t| t.focus_workspace(index))
            {
                let _ = tx.send(BridgeEvent::SessionsDirty);
                let _ = tx.send(BridgeEvent::TilingDirty);
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        "moveToWorkspace" => {
            let id = payload
                .get("id")
                .and_then(Value::as_u64)
                .ok_or_else(|| "missing window id".to_string())?;
            let index = payload.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let insert = payload
                .get("insert")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if manager
                .lock()
                .unwrap()
                .workspace_step(|t| t.move_to_workspace(id, index, insert))
            {
                let _ = tx.send(BridgeEvent::SessionsDirty);
                let _ = tx.send(BridgeEvent::TilingDirty);
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        // Drag and drop of a window: {id, newColumn} or {id, col, row}.
        "moveWindow" => {
            let id = payload
                .get("id")
                .and_then(Value::as_u64)
                .ok_or_else(|| "missing window id".to_string())?;
            let index = |key: &str| payload.get(key).and_then(Value::as_u64).map(|v| v as usize);
            let target = match (index("newColumn"), index("col"), index("row")) {
                (Some(at), _, _) => DropTarget::NewColumn(at),
                (None, Some(col), Some(row)) => DropTarget::Into { col, row },
                _ => return Err("missing drop target".into()),
            };
            let mut guard = manager.lock().unwrap();
            if payload.get("preview").and_then(Value::as_bool) == Some(true) {
                let mut layout = (*guard.tiling).clone();
                return if layout.move_pane(id, target) {
                    serde_json::to_string(&layout.snapshot()).map_err(|e| e.to_string())
                } else {
                    Ok("null".into())
                };
            }
            if guard.tiling.move_pane(id, target) {
                let _ = guard.apply_tiled_sizes();
                let _ = tx.send(BridgeEvent::SessionsDirty);
                let _ = tx.send(BridgeEvent::TilingDirty);
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        "previewScreen" => {
            let id = payload.get("id").and_then(Value::as_u64);
            let guard = manager.lock().unwrap();
            let screen = guard
                .sessions
                .iter()
                .find(|session| Some(session.id) == id)
                .map(build_screen_payload);
            serde_json::to_string(&screen).map_err(|e| e.to_string())
        }
        "tilingAction" => {
            let action = payload.get("action").and_then(Value::as_str).unwrap_or("");
            if manager.lock().unwrap().tiling_action(action) {
                let _ = tx.send(BridgeEvent::SessionsDirty);
                let _ = tx.send(BridgeEvent::TilingDirty);
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        "scrollColumn" => {
            let delta = payload.get("delta").and_then(Value::as_i64).unwrap_or(0) as isize;
            let mut guard = manager.lock().unwrap();
            if guard.scroll_column(delta) {
                let _ = tx.send(BridgeEvent::SessionsDirty);
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
            if let Some(value) = payload.get("windowTransparency").and_then(Value::as_i64) {
                guard.config.window_transparency = config::window_transparency_percent(value);
            }
            if let Some(keep) = payload
                .get("keepRunningInBackground")
                .and_then(Value::as_bool)
            {
                guard.config.keep_running_in_background = keep;
            }
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
            if let Some(layout) = payload
                .get("layout")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
            {
                guard.config.layout = layout;
                guard.tiling.set_options(layout);
                let _ = guard.apply_tiled_sizes();
            }
            if let Some(check) = payload.get("checkUpdates").and_then(Value::as_bool) {
                guard.config.check_updates = check;
            }
            if let Some(auto_r) = payload.get("autoRestartUpdate").and_then(Value::as_bool) {
                guard.config.auto_restart_update = auto_r;
            }
            if let Some(notifications) = payload.get("agentNotifications") {
                let prefs = &mut guard.config.agent_notifications;
                if let Some(value) = notifications["finished"].as_bool() {
                    prefs.finished = value;
                }
                if let Some(value) = notifications["needsInput"].as_bool() {
                    prefs.needs_input = value;
                }
                if let Some(value) = notifications["backgroundOnly"].as_bool() {
                    prefs.background_only = value;
                }
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
        "browseProject" => {
            let root = manager.lock().unwrap().workspace_dir().to_path_buf();
            let base = payload
                .get("base")
                .and_then(Value::as_str)
                .map(Path::new)
                .unwrap_or(&root);
            let dir = match payload.get("path").and_then(Value::as_str) {
                Some(path) => resolve_project_dir(base, path)?,
                None => root,
            };
            serde_json::to_string(&browse_project_dir(dir)?).map_err(|err| err.to_string())
        }
        "projectPlaces" => {
            let (special, bookmarks) = platform::user_places();
            let entry =
                |(name, path): (String, PathBuf)| serde_json::json!({ "name": name, "path": path });
            Ok(serde_json::json!({
                "special": special.into_iter().map(entry).collect::<Vec<_>>(),
                "bookmarks": bookmarks.into_iter().map(entry).collect::<Vec<_>>(),
            })
            .to_string())
        }
        "addProject" => {
            let raw = payload
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let mut guard = manager.lock().unwrap();
            let dir = resolve_project_dir(guard.workspace_dir(), raw)?
                .display()
                .to_string();
            if !guard.config.projects.contains(&dir) {
                let mut next = guard.config.clone();
                next.projects.push(dir);
                config::save_config(&next)
                    .map_err(|err| format!("Could not save project: {err}"))?;
                guard.config = next;
                let _ = tx.send(BridgeEvent::ConfigDirty);
            }
            Ok("ok".into())
        }
        "removeProject" => {
            let path = payload.get("path").and_then(Value::as_str).unwrap_or("");
            let mut guard = manager.lock().unwrap();
            let before = guard.config.projects.len();
            guard.config.projects.retain(|p| p != path);
            if guard.config.projects.len() != before {
                let _ = config::save_config(&guard.config);
                let _ = tx.send(BridgeEvent::ConfigDirty);
            }
            Ok("ok".into())
        }
        "revealPath" => {
            if let Some(raw) = payload.get("path").and_then(Value::as_str)
                && !raw.is_empty()
            {
                let root = manager.lock().unwrap().request_workspace(&payload);
                let target = if Path::new(raw).is_absolute() {
                    PathBuf::from(raw)
                } else {
                    root.join(raw)
                };
                if target.exists() {
                    let _ = platform::reveal_in_file_manager(&target);
                }
            }
            Ok("ok".into())
        }
        "openInEditor" => {
            let editor_id = payload
                .get("editorId")
                .and_then(Value::as_str)
                .unwrap_or("system");
            let raw = payload
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing path".to_string())?;
            let line = payload
                .get("line")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok());
            let col = payload
                .get("col")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok());
            let (target, editor) = {
                let mut guard = manager.lock().unwrap();
                let root = guard.request_workspace(&payload);
                let target = if Path::new(raw).is_absolute() {
                    PathBuf::from(raw)
                } else {
                    root.join(raw)
                };
                if editor_id == "system" {
                    if guard.config.preferred_editor.is_some() {
                        guard.config.preferred_editor = None;
                        let _ = config::save_config(&guard.config);
                        let _ = tx.send(BridgeEvent::ConfigDirty);
                    }
                    (target, None)
                } else {
                    let found = guard.editors.iter().find(|e| e.id == editor_id).cloned();
                    if found.is_some()
                        && guard.config.preferred_editor.as_deref() != Some(editor_id)
                    {
                        guard.config.preferred_editor = Some(editor_id.into());
                        let _ = config::save_config(&guard.config);
                        let _ = tx.send(BridgeEvent::ConfigDirty);
                    }
                    (target, found)
                }
            };
            if let Some(editor) = editor {
                let _ = platform::open_in_editor(&editor, &target, line, col);
            } else {
                let _ = platform::open_external(&target.display().to_string());
            }
            Ok("ok".into())
        }
        "gitGraph" => {
            let root = manager.lock().unwrap().request_workspace(&payload);
            let size = |key: &str, fallback: f32| {
                payload
                    .get(key)
                    .and_then(Value::as_f64)
                    .map_or(fallback, |v| (v as f32).clamp(1.0, 96.0))
            };
            let (lane, row, stroke) = (size("lane", 14.0), size("row", 28.0), size("stroke", 2.0));
            let tx = tx.clone();
            std::thread::Builder::new()
                .name("icy-git-graph".into())
                .spawn(move || {
                    let graph = git_graph::log(&root, git_graph::LOG_LIMIT).map(git_graph::layout);
                    let _ = tx.send(BridgeEvent::GitGraph(GitGraphRequest {
                        cwd: root,
                        graph,
                        lane,
                        row,
                        stroke,
                    }));
                })
                .map_err(|error| format!("Cannot start git graph loader: {error}"))?;
            Ok("ok".into())
        }
        "searchFiles" => {
            let query = payload
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let root = manager.lock().unwrap().request_workspace(&payload);
            let tx = tx.clone();
            let _ = std::thread::Builder::new()
                .name("icy-file-search".into())
                .spawn(move || {
                    let items = icy_core::search::search_files(&root, &query, 40);
                    let _ = tx.send(BridgeEvent::FileSearch(
                        serde_json::json!({ "cwd": root, "query": query, "items": items }),
                    ));
                });
            Ok("ok".into())
        }
        "releaseCover" => {
            let url = payload
                .get("url")
                .and_then(Value::as_str)
                .ok_or("missing cover URL")?;
            let valid = manager
                .lock()
                .unwrap()
                .update
                .notes
                .media
                .iter()
                .any(|media| {
                    media.poster_url.as_deref() == Some(url)
                        || (media.kind == "image" && media.url == url)
                });
            if !valid {
                return Err("Unknown release cover URL".into());
            }
            spawn_release_cover(url.to_owned(), tx.clone())?;
            Ok("ok".into())
        }
        "previewImage" => {
            let path = payload
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing path".to_string())?;
            let session_id = payload.get("sessionId").and_then(Value::as_u64);
            let session_cwd = {
                let guard = manager.lock().unwrap();
                session_id
                    .and_then(|id| guard.sessions.iter().find(|s| s.id == id))
                    .map(|s| s.cwd().to_path_buf())
                    .unwrap_or_else(|| guard.workspace_dir().to_path_buf())
            };
            spawn_file_image(path.to_string(), session_cwd, tx.clone());
            Ok("ok".into())
        }
        "previewCode" => {
            let key = payload
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing preview key".to_string())?
                .to_owned();
            let path = payload
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing path".to_string())?
                .to_owned();
            let diff = payload
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(|k| k == "diff");
            let line = payload
                .get("line")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok());
            let old_line = payload
                .get("oldLine")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let edit = payload
                .get("edit")
                .map(|edit| serde_json::from_value(edit.clone()))
                .transpose()
                .map_err(|error| error.to_string())?;
            let session_id = payload.get("sessionId").and_then(Value::as_u64);
            let session_cwd = {
                let guard = manager.lock().unwrap();
                payload
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .or_else(|| {
                        session_id
                            .and_then(|id| guard.sessions.iter().find(|s| s.id == id))
                            .map(|s| s.cwd().to_path_buf())
                    })
                    .unwrap_or_else(|| guard.workspace_dir().to_path_buf())
            };
            spawn_code_preview(
                CodePreviewRequest {
                    key,
                    path,
                    cwd: session_cwd,
                    diff,
                    line,
                    old_line,
                    edit,
                },
                tx.clone(),
            )?;
            Ok("ok".into())
        }
        "modalKeys" => {
            let on = payload.get("on").and_then(Value::as_bool).unwrap_or(false);
            window::MODAL_CAPTURE.store(on, std::sync::atomic::Ordering::Relaxed);
            Ok("ok".into())
        }
        "paletteKeys" => {
            let on = payload.get("on").and_then(Value::as_bool).unwrap_or(false);
            window::PALETTE_CAPTURE.store(on, Ordering::Relaxed);
            Ok("ok".into())
        }
        "browser" => {
            if payload.get("op").and_then(Value::as_str) == Some("pointer") {
                // `id` 0 releases the pointer; a browser only releases its own target.
                let id = payload.get("id").and_then(Value::as_u64).unwrap_or(0);
                let mut target = window::BROWSER_POINTER.lock().unwrap();
                if id == 0 {
                    let from = payload.get("from").and_then(Value::as_u64);
                    if from.is_none_or(|from| target.is_some_and(|t| t.id == from)) {
                        *target = None;
                    }
                } else {
                    let node = payload.get("node").and_then(Value::as_u64).unwrap_or(0);
                    *target = Some(window::BrowserPointer {
                        id,
                        node: node as u16,
                    });
                }
            } else {
                let command =
                    browser::Command::from_request(&payload).ok_or("Invalid browser request")?;
                if let browser::Command::Focus(id) = command {
                    window::BROWSER_FOCUS.store(id, Ordering::Relaxed);
                }
                let _ = tx.send(BridgeEvent::Browser(command));
            }
            Ok("ok".into())
        }
        "browserAddress" => {
            // Synchronous: the palette and the address bar need the URL before they act.
            let text = payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let url = if payload
                .get("search")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                icy_core::browser::address_url(text)
            } else {
                icy_core::browser::normalize_url(text)
            };
            Ok(serde_json::json!({ "url": url }).to_string())
        }
        "terminalWheel" => {
            // `id` 0 (or none) releases the wheel; a pane only releases its own target.
            let id = payload.get("id").and_then(Value::as_u64).unwrap_or(0);
            let mut target = window::TERMINAL_WHEEL.lock().unwrap();
            if id == 0 {
                let from = payload.get("from").and_then(Value::as_u64);
                if from.is_none_or(|from| target.is_some_and(|t| t.session == from)) {
                    *target = None;
                }
            } else {
                let num =
                    |key: &str| payload.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                *target = Some(window::TerminalWheelTarget {
                    session: id,
                    origin: [num("x"), num("y")],
                    cell: [num("cellW"), num("cellH")],
                    row: payload.get("row").and_then(Value::as_u64).unwrap_or(0) as u16,
                    grid: payload.get("grid").and_then(Value::as_u64).unwrap_or(0) as u16,
                    pad: [num("padX"), num("padY")],
                });
            }
            Ok("ok".into())
        }
        "terminalSize" => {
            let id = payload["id"].as_u64().ok_or("Missing session ID")?;
            let cols = payload["cols"]
                .as_u64()
                .ok_or("Missing terminal columns")?
                .clamp(2, 400) as u16;
            let rows = payload["rows"]
                .as_u64()
                .ok_or("Missing terminal rows")?
                .clamp(1, 200) as u16;
            let mut guard = manager.lock().unwrap();
            if guard.terminal_sizes.get(&id) != Some(&(cols, rows))
                && let Some(session) = guard.sessions.iter().find(|s| s.id == id && s.is_alive())
            {
                let resized = session.resize(cols, rows).map_err(|err| err.to_string())?;
                guard.terminal_sizes.insert(id, (cols, rows));
                if guard.active_id() == id {
                    (guard.cols, guard.rows) = (cols, rows);
                }
                if resized {
                    let _ = tx.send(BridgeEvent::PtyDirty);
                }
            }
            Ok("ok".into())
        }
        "scrollToBottom" => {
            let id = payload.get("id").and_then(Value::as_u64).unwrap_or(0);
            let guard = manager.lock().unwrap();
            if let Some(session) = guard.sessions.iter().find(|s| s.id == id)
                && session.scroll_to_bottom()
            {
                let _ = tx.send(BridgeEvent::PtyDirty);
            }
            Ok("ok".into())
        }
        "horizontalScrollCapture" => {
            let on = payload.get("on").and_then(Value::as_bool).unwrap_or(false);
            window::HORIZONTAL_SCROLL_CAPTURE.store(on, std::sync::atomic::Ordering::Relaxed);
            Ok("ok".into())
        }
        "metricsPointer" => {
            let id = payload
                .get("id")
                .and_then(Value::as_u64)
                .and_then(|id| u16::try_from(id).ok())
                .unwrap_or(0);
            window::METRICS_POINTER_TARGET.store(id, std::sync::atomic::Ordering::Relaxed);
            Ok("ok".into())
        }
        "releaseGlowTarget" => release_glow::request(&payload),
        "agentShortcutCount" => {
            let count = payload["count"].as_u64().unwrap_or(0).min(9) as u16;
            window::AGENT_SHORTCUT_COUNT.store(count, Ordering::Relaxed);
            Ok("ok".into())
        }
        "editorCommand" => {
            let command = payload
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let _ = tx.send(BridgeEvent::EditorCommand(command.to_string()));
            Ok("ok".into())
        }
        "copyText" => {
            if let Some(text) = payload.get("text").and_then(Value::as_str) {
                copy_to_clipboard(text);
            }
            Ok("ok".into())
        }
        "openExternal" => {
            if let Some(url) = payload.get("url").and_then(Value::as_str) {
                let _ = platform::open_external(url);
            }
            Ok("ok".into())
        }
        "refreshGitHub" => {
            let root = manager.lock().unwrap().workspace_dir().to_path_buf();
            spawn_github_probe(root, tx.clone());
            Ok("ok".into())
        }
        "listFiles" => {
            let subdir = payload
                .get("subdir")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let root = manager.lock().unwrap().request_workspace(&payload);
            let tx = tx.clone();
            std::thread::Builder::new()
                .name("icy-files".into())
                .spawn(move || {
                    let files = list_workspace_files(&root, &subdir);
                    let _ = tx.send(BridgeEvent::PushFiles(files));
                })
                .map_err(|error| format!("Cannot start directory loader: {error}"))?;
            Ok("ok".into())
        }
        "checkUpdate" => {
            let _ = tx.send(BridgeEvent::CheckUpdate);
            Ok("ok".into())
        }
        "downloadUpdate" => {
            spawn_update_download(manager, tx);
            Ok("ok".into())
        }
        "confirmRestartUpdate" => {
            let mut guard = manager.lock().unwrap();
            if guard.restarting_for_update {
                return Ok("ok".into());
            }
            let staged = guard
                .update
                .staged_path
                .as_deref()
                .ok_or("no staged update ready")?;
            updater::apply_staged_and_restart(staged)?;
            guard.restarting_for_update = true;
            let _ = tx.send(BridgeEvent::QuitApp);
            Ok("ok".into())
        }
        other => Err(format!("unknown request type: {other}")),
    }
}

fn run_bridge(
    pusher: Pusher,
    rx: mpsc::Receiver<BridgeEvent>,
    tx: mpsc::Sender<BridgeEvent>,
    manager: Arc<Mutex<SessionManager>>,
    usage_scan: Arc<UsageScan>,
) {
    let mut cli_usage: Vec<CliUsage> = Vec::new();
    let mut last_screen_push = Instant::now() - SCREEN_FRAME_MIN_INTERVAL;
    let mut current_scale = 1.0_f32;
    let mut graph_pictures = std::collections::HashSet::new();
    // Icon overrides of the active theme currently pushed over the built-in glyphs.
    let mut applied_icons: BTreeMap<String, String> = BTreeMap::new();
    let mut sent_screens = None;
    let mut rendered_rows = BTreeMap::<u64, TerminalRowCache>::new();
    let mut screen_dirty = false;
    let mut frame_sequence = 0;
    let mut pending_frame = None;
    let mut avatar_path: Option<PathBuf> = None;
    let mut scanning_usage = false;
    let mut workspace = None;
    let mut previous_sessions = None;
    let mut next_status_refresh = Instant::now();
    let mut status_dirty = false;
    let mut agent_notifications = AgentNotifications::default();
    let mut desktop_notifications = desktop_notifications::Delivery::default();
    // A program can begin a synchronized redraw without sending the closing marker (for
    // instance if it is interrupted). vte's deadline must be driven even when no bytes arrive.
    let mut sync_deadline: Option<Instant> = None;
    let auto_check = manager.lock().unwrap().config.check_updates;
    let mut update_schedule = CheckSchedule::new(auto_check, Instant::now());
    let mut update_can_check = true;
    let mut update_etag = None;

    loop {
        let status_deadline = status_dirty.then_some(next_status_refresh);
        let update_deadline = update_can_check
            .then(|| update_schedule.deadline())
            .flatten();
        let deadline = sync_deadline
            .into_iter()
            .chain(status_deadline)
            .chain(agent_notifications.deadline())
            .chain(update_deadline)
            .min();
        let event = if let Some(deadline) = deadline {
            rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        } else {
            rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        let first = match event {
            Ok(event) => Some(event),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let mut need_avatar = false;
        let mut pending_modal: Vec<&'static str> = Vec::new();
        let mut pending_file_images: Vec<FileImageResult> = Vec::new();
        let mut pending_code_previews: Vec<CodePreviewPayload> = Vec::new();
        let mut pending_search: Option<serde_json::Value> = None;
        let mut pending_graphs: Vec<GitGraphRequest> = Vec::new();
        let mut pending_captured: Option<String> = None;
        let mut need_keybinds = false;
        let mut keybind_taken: Vec<&'static str> = Vec::new();
        let mut need_sessions = false;
        let mut need_tiling = false;
        let mut need_screen = false;
        let mut need_usage = false;
        let mut scan_usage = None;
        let mut usage_ready = None;
        let mut test_notification = None;
        let mut need_config = false;
        let mut need_themes = false;
        let mut need_editors = false;
        let mut need_github = false;
        let mut need_update = false;
        let mut manual_update_check = false;
        let mut checked_update = None;
        let mut pending_files: Vec<FilesPayload> = Vec::new();
        let mut pending_changes = BTreeMap::new();
        let mut pending_viewer = Vec::new();
        let mut pending_saves = Vec::new();
        let mut pending_contexts = Vec::new();
        let mut pending_blames = Vec::new();
        let mut pending_notice: Option<(String, String)> = None;
        let mut pending_scale: Option<f32> = None;
        let mut quit_app = false;
        let mut apply_event = |ev: BridgeEvent| match ev {
            BridgeEvent::Hello => {
                previous_sessions = None;
                sent_screens = None;
                rendered_rows.clear();
                pending_frame = None;
                need_keybinds = true;
                need_sessions = true;
                need_tiling = true;
                need_screen = true;
                need_usage = true;
                need_config = true;
                need_themes = true;
                need_editors = true;
                need_github = true;
                need_update = true;
                workspace = None;
            }
            BridgeEvent::TerminalFrameReceived(sequence) => {
                if pending_frame == Some(sequence) {
                    pending_frame = None;
                }
            }
            BridgeEvent::ScanUsage { tx, refresh } => scan_usage = Some((tx, refresh)),
            BridgeEvent::UsageReady(snapshot) => usage_ready = Some(snapshot),
            BridgeEvent::TestNotification(tx) => test_notification = Some(tx),
            BridgeEvent::NotificationTested(error) => {
                push_view(&pusher, "notificationTest", &error)
            }
            BridgeEvent::CliUsage(list) => {
                need_usage = list != cli_usage;
                cli_usage = list;
            }
            BridgeEvent::PtyDirty => {
                need_screen = true;
                status_dirty = true;
            }
            BridgeEvent::SessionsDirty => need_sessions = true,
            BridgeEvent::TilingDirty => need_tiling = true,
            BridgeEvent::ConfigDirty => need_config = true,
            BridgeEvent::ThemesDirty => need_themes = true,
            BridgeEvent::EditorsUpdated(editors) => {
                manager.lock().unwrap().editors = editors;
                need_editors = true;
            }
            BridgeEvent::GitHubUpdated(gh) => {
                manager.lock().unwrap().github = gh;
                need_github = true;
            }
            BridgeEvent::CheckUpdate => manual_update_check = true,
            BridgeEvent::UpdateChecked(result) => checked_update = Some(result),
            BridgeEvent::UpdateChanged => need_update = true,
            BridgeEvent::PushFiles(f) => pending_files.push(f),
            BridgeEvent::PushChanges(root, changes) => {
                pending_changes.insert(root, changes);
            }
            BridgeEvent::PushViewer(v) => pending_viewer.push(v),
            BridgeEvent::EditorSaved(v) => pending_saves.push(v),
            BridgeEvent::PasteContext { id, cwd, cli, text } => {
                pending_contexts.push((id, cwd, cli, text));
            }
            BridgeEvent::PushNotice { kind, text } => pending_notice = Some((kind, text)),
            BridgeEvent::ScaleChanged(scale) => pending_scale = Some(scale),
            BridgeEvent::ModalKey(action) => pending_modal.push(action),
            BridgeEvent::FileImage(result) => pending_file_images.push(result),
            BridgeEvent::CodePreview(preview) => pending_code_previews.push(preview),
            BridgeEvent::FileSearch(view) => pending_search = Some(view),
            BridgeEvent::GitGraph(request) => pending_graphs.push(request),
            BridgeEvent::GitBlame(result) => pending_blames.push(result),
            BridgeEvent::Browser(command) => {
                pusher.browser(command);
            }
            BridgeEvent::EditorCommand(command) => {
                pusher.editor_command(command);
            }
            BridgeEvent::CapturedKey(chord) => pending_captured = Some(chord),
            BridgeEvent::KeybindsDirty(taken) => {
                need_keybinds = true;
                keybind_taken = taken;
            }
            BridgeEvent::AvatarReady(path) => {
                avatar_path = Some(path);
                need_avatar = true;
            }
            BridgeEvent::FocusWindow => {
                let _ = pusher.focus();
            }
            BridgeEvent::ServerQuit => {
                manager.lock().unwrap().server_stopped = true;
                quit_app = true;
            }
            BridgeEvent::QuitApp => quit_app = true,
        };

        if let Some(first) = first {
            apply_event(first);
        }
        while let Ok(more) = rx.try_recv() {
            apply_event(more);
        }
        let now = Instant::now();
        if need_screen || sync_deadline.is_some_and(|deadline| now >= deadline) {
            sync_deadline = None;
            for session in &manager.lock().unwrap().sessions {
                let (changed, deadline) = session.update_sync_timeout(now);
                if changed {
                    need_screen = true;
                    status_dirty = true;
                }
                if let Some(deadline) = deadline {
                    sync_deadline =
                        Some(sync_deadline.map_or(deadline, |first| first.min(deadline)));
                }
            }
        }

        if quit_app {
            let _ = pusher.quit();
            break;
        }

        if need_config
            || need_update
            || manual_update_check
            || checked_update.is_some()
            || update_deadline.is_some_and(|deadline| deadline <= now)
        {
            let mut guard = manager.lock().unwrap();
            if need_config {
                update_schedule.set_enabled(guard.config.check_updates, now);
            }
            if let Some(result) = checked_update {
                update_schedule.complete(now, result.as_ref().err());
                match result {
                    Ok(Some(checked)) => {
                        update_etag = checked.etag;
                        // An unchanged release leaves the state of a retryable download intact.
                        if guard.update.stage != "error"
                            || guard.update.available_version != checked.release.available_version
                            || guard.update.asset != checked.release.asset
                        {
                            guard.update = checked.release;
                        }
                        guard.update.check_status = CheckStatus::Succeeded;
                    }
                    Ok(None) => guard.update.check_status = CheckStatus::Succeeded,
                    Err(_) => guard.update.check_status = CheckStatus::Failed,
                }
                manual_update_check = false;
                need_update = true;
            }
            let due = update_schedule
                .deadline()
                .is_some_and(|deadline| deadline <= now);
            if (manual_update_check || due)
                && guard.update.can_check()
                && update_schedule.can_check(now)
            {
                guard.update.check_status =
                    if spawn_update_check(update_etag.clone(), tx.clone()).is_ok() {
                        CheckStatus::Checking
                    } else {
                        update_schedule.complete(now, Some(&updater::CheckError::default()));
                        CheckStatus::Failed
                    };
                need_update = true;
            }
            update_can_check = guard.update.can_check();
        }

        for (id, cwd, cli, text) in pending_contexts {
            let mut guard = manager.lock().unwrap();
            let result = guard
                .sessions
                .iter()
                .find(|session| session.id == id && session.cwd() == cwd && session.is_alive())
                .filter(|session| cli != "terminal" && session.info().cli == cli)
                .ok_or_else(|| "The selected CLI is no longer running in this project.".to_owned())
                .and_then(|session| {
                    session
                        .paste_context(&text)
                        .map_err(|error| error.to_string())
                });
            match result {
                Ok(()) => {
                    guard.select_session(id);
                    need_sessions = true;
                    need_tiling = true;
                    need_screen = true;
                    pending_notice = Some((
                        "info".into(),
                        "Context pasted. Review it in the CLI and press Enter when ready.".into(),
                    ));
                }
                Err(error) => pending_notice = Some(("error".into(), error)),
            }
        }
        for result in pending_saves {
            push_view(&pusher, "editorSaved", &result);
        }
        for result in pending_blames {
            push_view(&pusher, "gitBlame", &result);
        }

        let scale_changed = pending_scale.is_some();
        if let Some(scale) = pending_scale {
            current_scale = scale;
            push_images(&pusher, scale);
            need_avatar |= avatar_path.is_some();
        }
        if let Some(chord) = pending_captured.take() {
            push_view(&pusher, "capturedKey", &chord);
        }
        if need_keybinds {
            let overrides = manager.lock().unwrap().config.keybinds.clone();
            push_view(
                &pusher,
                "keybinds",
                &serde_json::json!({ "items": keybinds::views(&overrides), "taken": keybind_taken }),
            );
        }
        for request in pending_graphs {
            let view = git_graph_view(request, current_scale, &mut graph_pictures, &pusher);
            push_view(&pusher, "gitGraph", &view);
        }
        if let Some(view) = pending_search.take() {
            push_view(&pusher, "fileSearch", &view);
        }
        for result in pending_file_images {
            let view = match result.image {
                Some((key, rgba, size)) => {
                    let _ = pusher.image(key.clone(), rgba, size);
                    serde_json::json!({ "path": result.path, "key": key, "width": size[0], "height": size[1] })
                }
                None => serde_json::json!({ "path": result.path, "error": true }),
            };
            push_view(&pusher, "fileImage", &view);
        }
        for preview in pending_code_previews {
            push_view(&pusher, "codePreview", &preview);
        }
        for action in pending_modal {
            push_view(&pusher, "modalKey", &action);
        }
        if need_avatar
            && let Some(path) = &avatar_path
            && let Some((name, rgba, size)) = icons::rasterize_avatar(path, current_scale)
        {
            let _ = pusher.image(name, rgba, size);
            push_view(&pusher, "avatar", &serde_json::json!({ "ready": true }));
        }
        if need_usage {
            push_view(&pusher, "cliUsage", &cli_usage);
        }
        if let Some(snapshot) = usage_ready {
            scanning_usage = false;
            push_view(&pusher, "metricsUsage", &snapshot);
            scan_usage = None;
        }
        if let Some((tx, refresh)) = scan_usage
            && !scanning_usage
        {
            let usage_scan = Arc::clone(&usage_scan);
            scanning_usage = std::thread::Builder::new()
                .name("icy-usage".into())
                .spawn(move || {
                    let max_age = if refresh {
                        Duration::ZERO
                    } else {
                        CLI_USAGE_REFRESH
                    };
                    let _ = tx.send(BridgeEvent::UsageReady(usage_scan.snapshot(max_age)));
                })
                .is_ok();
            if !scanning_usage {
                push_view(&pusher, "metricsUsageError", &());
            }
        }
        if let Some(tx) = test_notification
            && let Err(error) =
                desktop_notifications.send(desktop_notifications::test_notice(), Some(tx))
        {
            push_view(&pusher, "notificationTest", &Some(error.to_string()));
        }
        if need_config {
            let cfg = manager.lock().unwrap().config.clone();
            pusher.config(&cfg);
        }
        if need_themes {
            let themes = manager.lock().unwrap().themes_payload();
            push_view(&pusher, "themes", &themes);
        }
        if need_config || need_themes || scale_changed {
            let desired = {
                let guard = manager.lock().unwrap();
                let active = &guard.config.theme;
                guard
                    .themes
                    .iter()
                    .find(|t| &t.id == active)
                    .map(|t| t.icons.clone())
                    .unwrap_or_default()
            };
            // Re-read on theme reloads too: the SVG files may have changed under the same paths.
            if need_themes || scale_changed || desired != applied_icons {
                if !applied_icons.is_empty() && !scale_changed {
                    push_images(&pusher, current_scale);
                }
                for (name, rgba, size) in icons::theme_icon_pictures(&desired, current_scale) {
                    let _ = pusher.image(name, rgba, size);
                }
                applied_icons = desired;
            }
        }
        if need_editors {
            let editors = manager.lock().unwrap().editors.clone();
            push_view(&pusher, "editors", &editors);
        }
        if need_github {
            let gh = manager.lock().unwrap().github.clone();
            push_view(&pusher, "github", &gh);
        }
        if need_update {
            let up = manager.lock().unwrap().update.clone();
            push_view(&pusher, "update", &up);
        }
        // PTY output can redraw at frame rate. Agent detection and process metadata only need a
        // periodic refresh, including a final refresh after the output goes quiet.
        let now = Instant::now();
        need_sessions |= (status_dirty && now >= next_status_refresh)
            || agent_notifications.deadline().is_some_and(|due| now >= due);
        if need_tiling || need_sessions {
            status_dirty = false;
            next_status_refresh = now + SESSION_STATUS_INTERVAL;
            let (sessions, updated_layout, notices) = {
                let guard = manager.lock().unwrap();
                let updated_layout = need_tiling.then(|| {
                    guard.save_terminal_layout();
                    (guard.tiling_snapshot(), guard.workspace_dir().to_path_buf())
                });
                let sessions = guard.sessions_payload();
                let preferences = guard.config.agent_notifications;
                let notices = if preferences.finished || preferences.needs_input {
                    let focused = pusher.focused().then_some(sessions.active_id);
                    agent_notifications
                        .observe(&sessions.items, Instant::now())
                        .into_iter()
                        .filter(|(session, notice)| {
                            preferences.allows(*notice, focused == Some(session.id))
                        })
                        .map(|(session, notice)| {
                            desktop_notifications::agent_notice(session, notice, &guard.config.clis)
                        })
                        .collect()
                } else {
                    agent_notifications = AgentNotifications::default();
                    Vec::new()
                };
                (sessions, updated_layout, notices)
            };
            for notice in notices {
                if let Err(error) = desktop_notifications.send(notice, None) {
                    eprintln!("icy: cannot queue desktop notification: {error}");
                }
            }
            if previous_sessions.as_ref() != Some(&sessions) {
                push_view(&pusher, "sessions", &sessions);
                previous_sessions = Some(sessions);
            }
            if let Some((tiling, root)) = updated_layout {
                push_view(&pusher, "tiling", &tiling);
                if workspace.as_ref() != Some(&root) {
                    push_view(&pusher, "workspace", &root);
                    workspace = Some(root);
                }
            }
        }
        for files in pending_files {
            push_view(&pusher, "files", &files);
        }
        for (root, changes) in pending_changes {
            push_view(
                &pusher,
                "changes",
                &serde_json::json!({ "cwd": root, "changes": changes }),
            );
        }
        for mut viewer in pending_viewer {
            if viewer.generation == 0
                || viewer.generation == VIEWER_GENERATION.load(Ordering::Relaxed)
            {
                if let (Some(FilePreview::Font { key, size, .. }), Some(pixels)) =
                    (&viewer.preview, viewer.preview_pixels.take())
                {
                    let _ = pusher.image(key.clone(), pixels, *size);
                }
                push_view(&pusher, "viewer", &viewer);
            }
        }
        if let Some((kind, text)) = pending_notice {
            let data = serde_json::json!({ "kind": kind, "text": text });
            push_view(&pusher, "notice", &data);
        }
        screen_dirty |= need_screen;
        // A delta depends on the previously received rows. While one is queued, accumulate
        // PTY changes and send a single update from the latest snapshot once the UI has caught up.
        if screen_dirty && pending_frame.is_none() {
            screen_dirty = false;
            let elapsed = last_screen_push.elapsed();
            if elapsed < SCREEN_FRAME_MIN_INTERVAL {
                // Keep events arriving during the delay queued for the next bridge iteration.
                std::thread::sleep(SCREEN_FRAME_MIN_INTERVAL - elapsed);
            }
            let (visible, context_id, context_screen, uploads) = {
                let guard = manager.lock().unwrap();
                let visible: BTreeMap<_, _> = guard
                    .visible_terminals()
                    .map(|session| (session.id, TerminalSnapshot::capture(session)))
                    .collect();
                let context = guard.context_terminal();
                let context_id = context.map(|session| session.id);
                let context_screen = context
                    .filter(|session| !visible.contains_key(&session.id))
                    .map(TerminalSnapshot::capture);
                (
                    visible,
                    context_id,
                    context_screen,
                    guard.take_image_uploads(),
                )
            };
            // Textures first, so the screen that references them never shows a missing image.
            for (key, data) in uploads {
                if let Some((rgba, size)) = icons::decode_term_image(&data) {
                    let _ = pusher.image(key, rgba, size);
                }
            }
            // Resolve links and diff rows after releasing the session manager for input.
            rendered_rows.retain(|id, _| Some(*id) == context_id || visible.contains_key(id));
            let screens = TerminalScreens {
                visible: visible
                    .into_iter()
                    .map(|(id, screen)| {
                        (
                            id,
                            screen.into_payload(rendered_rows.entry(id).or_default()),
                        )
                    })
                    .collect(),
                context_id,
                context_screen: context_screen.map(|screen| {
                    let cache = rendered_rows.entry(screen.id).or_default();
                    screen.into_payload(cache)
                }),
            };
            if sent_screens.as_ref() != Some(&screens) {
                frame_sequence += 1;
                let update = screens.update_since(sent_screens.as_ref(), frame_sequence);
                push_view(&pusher, "terminalFrame", &update);
                pending_frame = Some(frame_sequence);
                sent_screens = Some(screens);
            }
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

    let (
        sessions_payload,
        tiling_payload,
        screen_payload,
        screens_map,
        cfg,
        themes,
        gh,
        up,
        term_uploads,
    ) = {
        let mut guard = manager.lock().unwrap();
        guard.update_viewport(1280.0, 800.0);
        // Set a realistic preview update state for offscreen visual verification.
        guard.update.has_update = true;
        guard.update.available_version = "0.2.0-preview".into();
        guard.update.status_text = "Preview v0.2.0 ready".into();
        (
            guard.sessions_payload(),
            guard.tiling_snapshot(),
            guard.focused_terminal().map(build_screen_payload),
            guard.visible_screens(),
            guard.config.clone(),
            guard.themes_payload(),
            github::detect_github(cwd),
            guard.update.clone(),
            guard.take_image_uploads(),
        )
    };

    for (key, data) in term_uploads {
        if let Some((rgba, size)) = icons::decode_term_image(&data) {
            host.set_image(key, rgba, size);
        }
    }
    host.push(serde_json::json!({ "view": "cliUsage", "data": cli_usage }).to_string());
    host.push(
        serde_json::json!({ "view": "keybinds", "data": { "items": keybinds::views(&cfg.keybinds) } })
            .to_string(),
    );
    host.push(serde_json::json!({ "view": "config", "data": cfg }).to_string());
    host.push(serde_json::json!({ "view": "themes", "data": themes }).to_string());
    host.push(serde_json::json!({ "view": "github", "data": gh }).to_string());
    host.push(serde_json::json!({ "view": "update", "data": up }).to_string());
    host.push(serde_json::json!({ "view": "workspace", "data": cwd }).to_string());
    host.push(
        serde_json::json!({ "view": "files", "data": list_workspace_files(cwd, "") }).to_string(),
    );
    host.push(serde_json::json!({ "view": "sessions", "data": sessions_payload }).to_string());
    host.push(serde_json::json!({ "view": "tiling", "data": tiling_payload }).to_string());
    host.push(serde_json::json!({ "view": "screen", "data": screen_payload }).to_string());
    host.push(serde_json::json!({ "view": "screens", "data": screens_map }).to_string());

    pixel_blast::install_capture_mask(&mut host);
    match jsnir_gpu::capture_at(host, [1280, 800], 1.0, false, path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("icy: offscreen capture failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let terminal_server = match args.as_slice() {
        [flag, path] if flag == "--session-server" => Some(session_server::serve_with_options(
            Path::new(path),
            ServerOptions {
                app_executable: None,
                tray_icon: icons::tray_icon(),
            },
        )),
        [flag, path, app_flag, app_exe] if flag == "--session-server" && app_flag == "--app" => {
            Some(session_server::serve_with_options(
                Path::new(path),
                ServerOptions {
                    app_executable: Some(PathBuf::from(app_exe)),
                    tray_icon: icons::tray_icon(),
                },
            ))
        }
        [flag] if flag == "--stop-session-server" => {
            Some(SessionClient::stop(&session_server::default_directory()))
        }
        _ => None,
    };
    if let Some(result) = terminal_server {
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("icy: session server: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if let Some(code) = browser::run_subprocess() {
        return ExitCode::from(code.clamp(0, 255) as u8);
    }
    let offscreen_path = match args.as_slice() {
        [] => None,
        [flag, path] if flag == "--offscreen" => Some(PathBuf::from(path)),
        _ => {
            eprintln!("usage: icy [--offscreen OUTPUT.ppm | --stop-session-server]");
            return ExitCode::FAILURE;
        }
    };

    let cwd = std::env::current_dir()
        .ok()
        .or_else(platform::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    let cwd = cwd.canonicalize().unwrap_or(cwd);
    let (tx, rx) = mpsc::channel();
    let usage_scan = Arc::new(UsageScan::default());
    if offscreen_path.is_none() {
        usage_scan::spawn_sidebar(Arc::clone(&usage_scan), tx.clone());
    }
    let pty_tx = tx.clone();
    let on_pty_update: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let _ = pty_tx.send(BridgeEvent::PtyDirty);
    });

    let mut session_manager = SessionManager::new(cwd.clone(), on_pty_update);
    if offscreen_path.is_none()
        && let Err(error) = session_manager.attach_sessions(&tx)
    {
        eprintln!("icy: failed to attach sessions: {error}");
        return ExitCode::FAILURE;
    }
    let manager = Arc::new(Mutex::new(session_manager));

    let req_manager = Arc::clone(&manager);
    let req_tx = tx.clone();
    let mut host = match Host::with_request_handler(
        APP.into(),
        "{}".into(),
        TextEngine::default(),
        move |json| handle_request(&json, &req_manager, &req_tx),
    ) {
        Ok(h) => h,
        Err(error) => {
            eprintln!("icy: failed to load app host: {error}");
            return ExitCode::FAILURE;
        }
    };

    install_fonts(&mut host);
    install_images_on_host(&mut host, 1.0);

    if let Some(path) = offscreen_path {
        let snapshot = icy_core::usage::collect_local();
        return run_offscreen(
            &path,
            &manager,
            &icy_core::usage::collect_cli(&snapshot, &Vec::new()),
            &cwd,
            host,
        );
    }

    // Kick off non-blocking background IDE and GitHub SSH detection.
    spawn_editors_probe(tx.clone());
    spawn_github_probe(cwd.clone(), tx.clone());

    let key_manager = Arc::clone(&manager);
    let paste_manager = Arc::clone(&manager);
    let paste_tx = tx.clone();
    let shortcut_manager = Arc::clone(&manager);
    let shortcut_tx = tx.clone();
    let viewport_manager = Arc::clone(&manager);
    let viewport_tx = tx.clone();
    let wheel_manager = Arc::clone(&manager);
    let wheel_tx = tx.clone();
    let select_manager = Arc::clone(&manager);
    let select_tx = tx.clone();
    let kitty_manager = Arc::clone(&manager);
    let input_tx = tx.clone();

    let hooks = WindowHooks {
        on_pty_input: Box::new(move |bytes| {
            let guard = key_manager.lock().unwrap();
            // A focused file / diff tile takes no terminal input.
            if let Some(session) = guard.focused_terminal()
                && session.write_input(bytes).unwrap_or(false)
            {
                let _ = input_tx.send(BridgeEvent::PtyDirty);
            }
        }),
        on_pty_paste: Box::new(move |text| {
            let guard = paste_manager.lock().unwrap();
            if let Some(session) = guard.focused_terminal()
                && session.paste_clipboard(text).unwrap_or(false)
            {
                let _ = paste_tx.send(BridgeEvent::PtyDirty);
            }
        }),
        on_shortcut: Box::new(move |shortcut| {
            let mut guard = shortcut_manager.lock().unwrap();
            let changed = match shortcut {
                WindowShortcut::Action(id) => match id {
                    "session.close" => {
                        let active = guard.active_id();
                        guard.close_session(active)
                    }
                    "tab.next" => guard.step_tab(1),
                    "tab.prev" => guard.step_tab(-1),
                    "view.fullscreen" => guard.toggle_maximize(),
                    "focus.left" => guard.focus_dir(FocusDir::Left),
                    "focus.right" => guard.focus_dir(FocusDir::Right),
                    "focus.up" => guard.focus_dir(FocusDir::Up),
                    "focus.down" => guard.focus_dir(FocusDir::Down),
                    "workspace.down" => guard.workspace_step(|t| t.focus_workspace_relative(1)),
                    "workspace.up" => guard.workspace_step(|t| t.focus_workspace_relative(-1)),
                    "workspace.moveDown" => guard.workspace_step(|t| t.move_focused_relative(1)),
                    "workspace.moveUp" => guard.workspace_step(|t| t.move_focused_relative(-1)),
                    "workspace.previous" => {
                        guard.workspace_step(Workspaces::focus_workspace_previous)
                    }
                    "workspace.moveColumnDown" => {
                        guard.workspace_step(|t| t.move_column_relative(1))
                    }
                    "workspace.moveColumnUp" => {
                        guard.workspace_step(|t| t.move_column_relative(-1))
                    }
                    "focus.windowOrWorkspaceDown" => {
                        guard.workspace_step(|t| t.focus_window_or_workspace(true))
                    }
                    "focus.windowOrWorkspaceUp" => {
                        guard.workspace_step(|t| t.focus_window_or_workspace(false))
                    }
                    // Panels and the overview are UI state: the TSX side handles them.
                    "session.new" | "column.new" | "pane.split" | "panel.overview"
                    | "panel.side" | "panel.files" | "session.search" | "view.overview"
                    | "workspace.switcher" => {
                        let _ = shortcut_tx.send(BridgeEvent::ModalKey(id));
                        false
                    }
                    "editor.addToChat" => {
                        // Ctrl+L stays the terminal's clear-screen and the browser's address bar.
                        if guard.focused_terminal().is_some()
                            || window::BROWSER_FOCUS.load(Ordering::Relaxed) != 0
                        {
                            return false;
                        }
                        let _ = shortcut_tx.send(BridgeEvent::ModalKey(id));
                        false
                    }
                    layout => match layout.strip_prefix("layout.") {
                        Some(action) => guard.tiling_action(action),
                        None => false,
                    },
                },
                WindowShortcut::Modal(action) => {
                    let _ = shortcut_tx.send(BridgeEvent::ModalKey(action));
                    false
                }
                WindowShortcut::Captured(chord) => {
                    window::KEY_CAPTURE.store(false, std::sync::atomic::Ordering::Relaxed);
                    let _ = shortcut_tx.send(BridgeEvent::CapturedKey(chord));
                    false
                }
            };
            if changed {
                let _ = shortcut_tx.send(BridgeEvent::SessionsDirty);
                let _ = shortcut_tx.send(BridgeEvent::TilingDirty);
                let _ = shortcut_tx.send(BridgeEvent::PtyDirty);
            }
            // Consumed; `false` (the early return above) hands the key to the focused pane.
            true
        }),
        on_viewport: Box::new(move |w, h| {
            if viewport_manager.lock().unwrap().update_viewport(w, h) {
                let _ = viewport_tx.send(BridgeEvent::PtyDirty);
            }
        }),
        // The terminal under the pointer, focused or not: the tiling never sees this wheel.
        on_terminal_wheel: Box::new(move |id, lines, col, row| {
            let guard = wheel_manager.lock().unwrap();
            if let Some(session) = guard.sessions.iter().find(|s| s.id == id)
                && session.wheel(lines, col, row).unwrap_or(false)
            {
                let _ = wheel_tx.send(BridgeEvent::PtyDirty);
            }
        }),
        on_terminal_select: Box::new(move |id, action| {
            let guard = select_manager.lock().unwrap();
            let session = guard.sessions.iter().find(|s| s.id == id)?;
            match action {
                TerminalSelect::Start(kind, at) => session.start_selection(kind, at),
                TerminalSelect::Extend(to) => session.extend_selection(to),
                TerminalSelect::Clear => session.clear_selection(),
                TerminalSelect::Copy => return session.selection_text(),
            }
            let _ = select_tx.send(BridgeEvent::PtyDirty);
            None
        }),
        kitty_keys: Box::new(move || {
            let guard = kitty_manager.lock().unwrap();
            guard.focused_terminal().is_some_and(Terminal::kitty_keys)
        }),
    };

    let bridge_manager = Arc::clone(&manager);
    let scale_tx = tx.clone();
    let initial_config = manager.lock().unwrap().config.clone();
    let result = window::open_with(host, "icy", [1280, 800], &initial_config, hooks, |pusher| {
        pusher.on_scale(move |scale| {
            let _ = scale_tx.send(BridgeEvent::ScaleChanged(scale));
        });
        std::thread::Builder::new()
            .name("icy-ui-bridge".into())
            .spawn(move || run_bridge(pusher, rx, tx, bridge_manager, usage_scan))
            .expect("spawn ui bridge thread");
    });
    let (keep_background, restart_for_update, server_stopped, has_active_sessions) = {
        let guard = manager.lock().unwrap();
        guard.save_terminal_layout();
        (
            guard.config.keep_running_in_background,
            guard.restarting_for_update,
            guard.server_stopped,
            guard.sessions.iter().any(Terminal::is_alive),
        )
    };
    if !server_stopped && (!keep_background || (restart_for_update && !has_active_sessions)) {
        manager.lock().unwrap().detach_sessions();
        let _ = SessionClient::stop(&session_server::default_directory());
    }

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

    fn sample_update_asset() -> updater::ReleaseAsset {
        updater::ReleaseAsset {
            url: "https://example.com/icy-linux-x86_64".into(),
            size: 123,
            sha256: "ab".repeat(32),
        }
    }

    #[test]
    fn embedded_tsx_app_mounts_and_renders_frame() {
        let mut host =
            Host::with_request_handler(APP.into(), "{}".into(), TextEngine::default(), |_| {
                Ok("ok".into())
            })
            .expect("load compiled TSX bundle");
        install_fonts(&mut host);
        install_images_on_host(&mut host, 1.0);
        host.poll();
        let frame = host.frame(1280., 800., 1., 4096);
        assert!(!frame.items.is_empty(), "expected mounted UI scene items");
    }

    #[test]
    fn appearance_controls_change_the_native_background_preference() {
        use jsnir::editor::PointSelection;
        use jsnir::host::{Frame, Item};
        use serde_json::json;

        fn frame(host: &mut Host) -> Frame {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                host.poll();
                let frame = host.frame(1280., 800., 1., 4096);
                if (!host.busy() && !frame.animating) || Instant::now() >= deadline {
                    return frame;
                }
                std::thread::sleep(Duration::from_millis(4));
            }
        }

        fn backdrop(frame: &Frame) -> u8 {
            frame
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Fill { rect, top, .. } if *rect == [0., 0., 1280., 800.] => Some(top[3]),
                    _ => None,
                })
                .expect("window backdrop")
        }

        let (requests, received) = mpsc::channel();
        let script = format!("{APP}\nactiveNav.value = 'settings';");
        let mut host =
            Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
                let request: Value = serde_json::from_str(&raw).unwrap();
                if request["type"] == "updateConfig" {
                    requests
                        .send(request["windowTransparency"].clone())
                        .unwrap();
                }
                Ok("ok".into())
            })
            .unwrap();
        install_fonts(&mut host);
        host.push(json!({"view": "windowTransparencySupport", "data": true}).to_string());
        let solid = frame(&mut host);
        assert_eq!(backdrop(&solid), 255);
        let rect = solid
            .items
            .iter()
            .find_map(|item| match item {
                Item::Text { rect, text, .. } if text == "+" => Some(rect),
                _ => None,
            })
            .expect("increase background transparency");
        host.pointer_move(Some([rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.]));
        host.pointer_down(PointSelection::Place);
        host.pointer_up();
        frame(&mut host);
        assert_eq!(received.try_recv().unwrap(), 5);

        let config = IcyConfig {
            window_transparency: 65,
            ..IcyConfig::default()
        };
        host.push(json!({"view": "config", "data": config}).to_string());
        assert_eq!(backdrop(&frame(&mut host)), 89);
        host.push(json!({"view": "windowTransparencySupport", "data": false}).to_string());
        assert_eq!(backdrop(&frame(&mut host)), 255);
    }

    #[test]
    fn settings_controls_background_mode_and_reports_update_check_status() {
        use jsnir::editor::PointSelection;
        use jsnir::host::{Frame, Item};
        use serde_json::json;

        fn frame(host: &mut Host) -> Frame {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                host.poll();
                let frame = host.frame(1280., 800., 1., 4096);
                if (!host.busy() && !frame.animating) || Instant::now() >= deadline {
                    return frame;
                }
                std::thread::sleep(Duration::from_millis(4));
            }
        }

        let (requests, received) = mpsc::channel();
        let script = format!("{APP}\nactiveNav.value = 'settings';");
        let mut host =
            Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
                let request: Value = serde_json::from_str(&raw).unwrap();
                if matches!(
                    request["type"].as_str(),
                    Some("updateConfig" | "checkUpdate" | "downloadUpdate")
                ) {
                    requests.send(request).unwrap();
                }
                Ok("ok".into())
            })
            .unwrap();
        install_fonts(&mut host);

        let initial = frame(&mut host);
        let row = initial
            .items
            .iter()
            .find_map(|item| match item {
                Item::Text { rect, text, .. } if text == "Keep running in background" => {
                    Some(*rect)
                }
                _ => None,
            })
            .expect("background mode setting");
        let toggle = initial
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fill { rect, .. }
                    if (rect[2] - 34.).abs() < 1.
                        && (rect[3] - 18.).abs() < 1.
                        && (rect[1] - row[1]).abs() < 24. =>
                {
                    Some(*rect)
                }
                _ => None,
            })
            .expect("background mode toggle");
        host.pointer_move(Some([
            toggle[0] + toggle[2] / 2.,
            toggle[1] + toggle[3] / 2.,
        ]));
        host.pointer_down(PointSelection::Place);
        host.pointer_up();
        frame(&mut host);
        let req = received.try_recv().expect("updateConfig request");
        assert_eq!(req["type"], "updateConfig");
        assert_eq!(req["keepRunningInBackground"], false);

        // A failed lookup leaves manual checks available and never reports "up to date".
        let updates_nav = initial
            .items
            .iter()
            .find_map(|item| match item {
                Item::Text { rect, text, .. } if text == "Updates" => Some(*rect),
                _ => None,
            })
            .expect("Updates nav item");
        host.pointer_move(Some([
            updates_nav[0] + updates_nav[2] / 2.,
            updates_nav[1] + updates_nav[3] / 2.,
        ]));
        host.pointer_down(PointSelection::Place);
        host.pointer_up();
        let failed = UpdateState {
            check_status: CheckStatus::Failed,
            ..UpdateState::initial()
        };
        host.push(json!({"view": "update", "data": failed}).to_string());
        let updates = frame(&mut host);
        assert!(updates.items.iter().any(|item| matches!(item,
            Item::Text { text, .. } if text == "Could not check for updates. Try again later."
        )));
        assert!(!updates.items.iter().any(|item| matches!(item,
            Item::Text { text, .. } if text.starts_with("Up to date") || text == "Download"
        )));
        let check_now = updates
            .items
            .iter()
            .find_map(|item| match item {
                Item::Text { rect, text, .. } if text == "Check now" => Some(*rect),
                _ => None,
            })
            .expect("manual check button");
        host.pointer_move(Some([
            check_now[0] + check_now[2] / 2.,
            check_now[1] + check_now[3] / 2.,
        ]));
        host.pointer_down(PointSelection::Place);
        host.pointer_up();
        frame(&mut host);
        let request = received.try_recv().expect("checkUpdate request");
        assert_eq!(request["type"], "checkUpdate");

        let checking = UpdateState {
            check_status: CheckStatus::Checking,
            ..failed
        };
        host.push(json!({"view": "update", "data": checking}).to_string());
        let updates = frame(&mut host);
        assert!(updates.items.iter().any(|item| matches!(item,
            Item::Text { text, .. } if text == "Checking for updates…"
        )));
        host.pointer_down(PointSelection::Place);
        host.pointer_up();
        frame(&mut host);
        assert!(received.try_recv().is_err());

        let update = UpdateState {
            has_update: true,
            available_version: "0.2.0".into(),
            asset: Some(sample_update_asset()),
            check_status: CheckStatus::Succeeded,
            ..UpdateState::initial()
        };
        host.push(json!({"view": "update", "data": update}).to_string());
        let available = frame(&mut host);
        let download_btn = available
            .items
            .iter()
            .find_map(|item| match item {
                Item::Text { rect, text, .. } if text == "Download" => Some(*rect),
                _ => None,
            })
            .expect("Download button when update available");
        host.pointer_move(Some([
            download_btn[0] + download_btn[2] / 2.,
            download_btn[1] + download_btn[3] / 2.,
        ]));
        host.pointer_down(PointSelection::Place);
        host.pointer_up();
        frame(&mut host);
        let req = received.try_recv().expect("downloadUpdate request");
        assert_eq!(req["type"], "downloadUpdate");
    }

    #[test]
    fn tiling_drag_previews_match_committed_geometry() {
        use jsnir::editor::PointSelection;
        use jsnir::host::{Cursor, Frame, Item};
        use serde_json::json;

        fn frame(host: &mut Host) -> Frame {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                host.poll();
                let frame = host.frame(1280., 800., 1., 4096);
                if (!host.busy() && !frame.animating) || Instant::now() >= deadline {
                    return frame;
                }
                std::thread::sleep(Duration::from_millis(4));
            }
        }

        fn flush(host: &mut Host) {
            let deadline = Instant::now() + Duration::from_secs(1);
            while host.busy() && Instant::now() < deadline {
                host.poll();
                std::thread::sleep(Duration::from_millis(2));
            }
        }

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut manager = SessionManager::new(root.clone(), Arc::new(|| {}));
        manager.tiling.set_mode(TilingMode::Niri);
        let ids = [0, 1, 2].map(|_| manager.open_tile(false, root.clone()));
        let initial = manager.tiling_snapshot();
        let manager = Arc::new(Mutex::new(manager));
        let req_manager = Arc::clone(&manager);
        let (tx, _events) = mpsc::channel();
        let (record, requests) = mpsc::channel::<(Value, String)>();
        let mut host = Host::with_request_handler(
            APP.into(),
            "{}".into(),
            TextEngine::default(),
            move |raw| {
                let result = handle_request(&raw, &req_manager, &tx)?;
                record
                    .send((serde_json::from_str(&raw).unwrap(), result.clone()))
                    .unwrap();
                Ok(result)
            },
        )
        .unwrap();
        install_fonts(&mut host);
        host.push(json!({"view": "tiling", "data": initial}).to_string());
        let sessions: Vec<_> = ids
            .iter()
            .map(|id| {
                json!({"id": id, "title": format!("shell-{id}"), "cli": "terminal",
                "cwd": format!("/project-{id}"), "alive": true})
            })
            .collect();
        host.push(json!({"view": "sessions", "data": {"activeId": ids[2], "archivedCount": 0, "items": sessions}}).to_string());
        host.push(
            json!({"view": "screens", "data": {
                (ids[1].to_string()): {
                    "cols": 80, "rowsCount": 1, "cursorRow": 0, "cursorCol": 0, "cursorVisible": false,
                    "rows": [{"idx": 0, "plain": "moving-preview-content", "spans": [], "links": []}]
                },
                (ids[2].to_string()): {
                    "cols": 80, "rowsCount": 1, "cursorRow": 0, "cursorCol": 0, "cursorVisible": false,
                    "rows": [{"idx": 0, "plain": "right-preview-content", "spans": [], "links": []}]
                }
            }})
            .to_string(),
        );
        let pane_content = |frame: &Frame, label: &str| {
            frame
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Text { text, rect, .. } if text == label => Some(*rect),
                    _ => None,
                })
                .expect("pane content")
        };
        let right_x = |frame: &Frame| pane_content(frame, "right-preview-content")[0];
        let before = right_x(&frame(&mut host));

        // Drag the border shared by the middle and the focused right column. The preview trades
        // width between the two without scrolling, leaving the source layout untouched.
        host.pointer_move(Some([748., 200.]));
        host.pointer_down(PointSelection::Place);
        host.pointer_move(Some([904., 200.]));
        flush(&mut host);
        let during = right_x(&host.frame(1280., 800., 1., 4096));
        assert!((during - before - 156.).abs() <= 1.);
        let resize = requests
            .try_iter()
            .find(|(req, _)| req["type"] == "tilingResize" && req["preview"] == true)
            .expect("resize preview request");
        assert_eq!(manager.lock().unwrap().tiling_snapshot(), initial);
        assert_eq!(resize.0["column"], initial.columns[1].id);
        assert_eq!(resize.0["left"], false);
        let preview: Value = serde_json::from_str(&resize.1).unwrap();
        let width = |i: usize| preview["columns"][i]["width"].as_f64().unwrap() as f32;
        assert!(width(1) > initial.columns[1].width && width(2) < initial.columns[2].width);
        assert!(
            (width(1) + width(2) - initial.columns[1].width - initial.columns[2].width).abs()
                < 1e-4
        );
        assert_eq!(
            preview["viewOffset"].as_f64().unwrap() as f32,
            initial.view_offset
        );
        host.pointer_up();
        flush(&mut host);
        let committed = manager.lock().unwrap().tiling_snapshot();
        host.push(json!({"view": "tiling", "data": committed}).to_string());
        flush(&mut host);
        let resized = host.frame(1280., 800., 1., 4096);
        assert_eq!(right_x(&resized), during);
        let committed_json: Value =
            serde_json::from_str(&serde_json::to_string(&committed).unwrap()).unwrap();
        assert_eq!(preview["columns"], committed_json["columns"]);
        assert_eq!(preview["viewOffset"], committed_json["viewOffset"]);

        // Drag the second tab into the right-hand column and check its thumbnail and placement.
        let tab = resized
            .items
            .iter()
            .find_map(|item| match item {
                Item::Text { text, rect, .. } if text == "project-2" && rect[1] < 52. => {
                    Some(*rect)
                }
                _ => None,
            })
            .expect("second terminal tab");
        host.pointer_move(Some([tab[0] + tab[2] / 2., tab[1] + tab[3] / 2.]));
        host.pointer_down(PointSelection::Place);
        host.pointer_move(Some([1090., 580.]));
        let moving = frame(&mut host);
        assert!(
            moving
                .items
                .iter()
                .any(|item| matches!(item, Item::Text { text, style, .. }
            if text == "moving-preview-content" && style.size < 10.))
        );
        let move_preview = requests
            .try_iter()
            .find(|(req, _)| req["type"] == "moveWindow" && req["preview"] == true)
            .expect("insert hint request");
        let preview: Value = serde_json::from_str(&move_preview.1).unwrap();
        assert_eq!(preview["focusedSession"], ids[1]);
        assert_eq!(manager.lock().unwrap().tiling_snapshot(), committed);
        host.pointer_up();
        frame(&mut host);
        let dropped = manager.lock().unwrap().tiling_snapshot();
        let dropped_json: Value =
            serde_json::from_str(&serde_json::to_string(&dropped).unwrap()).unwrap();
        assert_eq!(preview["columns"], dropped_json["columns"]);

        // The drop stacked the second pane below the right pane. A fast drag of their
        // divider can deliver both move and release before there is a frame to draw.
        host.push(json!({"view": "tiling", "data": dropped}).to_string());
        let lower_y = |frame: &Frame| pane_content(frame, "moving-preview-content")[1];
        let settled = frame(&mut host);
        let before = lower_y(&settled);
        // niri keeps the view still when the strip shrinks, so find the column where it landed.
        let x = pane_content(&settled, "moving-preview-content")[0] + 40.;
        let divider = (300..550)
            .find(|y| host.pointer_move(Some([x, *y as f32])) == Cursor::RowResize)
            .expect("stacked pane divider") as f32;
        host.pointer_down(PointSelection::Place);
        host.pointer_move(Some([x, divider + 80.]));
        host.pointer_up();
        flush(&mut host);
        let released = lower_y(&host.frame(1280., 800., 1., 4096));
        assert!((released - before - 80.).abs() <= 1.);
        assert_eq!(lower_y(&frame(&mut host)), released);
    }

    #[test]
    fn horizontal_strips_scroll_and_expose_the_active_tab() {
        use jsnir::editor::PointSelection;
        use jsnir::host::{Frame, Item};
        use serde_json::json;

        const LEFT: [u8; 4] = [1, 0, 0, 255];
        const RIGHT: [u8; 4] = [2, 0, 0, 255];
        const FIRST_CLI: [u8; 4] = [3, 0, 0, 255];
        const CUSTOM_CLI: [u8; 4] = [4, 0, 0, 255];
        const OUTLINE: [u8; 4] = [5, 0, 0, 255];
        const UPDATE: [u8; 4] = [6, 0, 0, 255];

        fn frame(host: &mut Host) -> Frame {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                host.poll();
                let frame = host.frame(1280., 800., 1., 4096);
                if (!host.busy() && !frame.animating) || Instant::now() >= deadline {
                    return frame;
                }
                std::thread::sleep(Duration::from_millis(4));
            }
        }

        fn image_point(frame: &Frame, color: [u8; 4], sidebar: bool) -> Option<[f32; 2]> {
            frame.items.iter().find_map(|item| match item {
                Item::Image {
                    image, rect, clip, ..
                } if image.rgba.as_ref() == color => {
                    let [x, y] = [rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.];
                    let in_region = if sidebar {
                        x < 224. && y > 640.
                    } else {
                        x > 224. && y < 52.
                    };
                    (in_region && x >= clip[0] && x < clip[0] + clip[2]).then_some([x, y])
                }
                _ => None,
            })
        }

        fn has_tab(frame: &Frame, title: &str) -> bool {
            frame.items.iter().any(|item| {
                matches!(item, Item::Text { text, rect, clip, .. }
                if text == title && rect[1] < 52.
                    && rect[0] >= clip[0] && rect[0] + rect[2] <= clip[0] + clip[2])
            })
        }

        fn outline_fits(frame: &Frame) -> bool {
            frame.items.iter().any(|item| {
                matches!(item, Item::Image { image, rect, clip, .. }
                    if image.rgba.as_ref() == OUTLINE
                        && rect[0] >= clip[0] - 0.01
                        && rect[0] + rect[2] <= clip[0] + clip[2] + 0.01
                        && (rect[1] + rect[3] - clip[1] - clip[3]).abs() < 0.01)
            })
        }

        let (tx, rx) = mpsc::channel();
        let mut host = Host::with_request_handler(
            APP.into(),
            "{}".into(),
            TextEngine::default(),
            move |request| {
                tx.send(request).unwrap();
                Ok("ok".into())
            },
        )
        .unwrap();
        install_fonts(&mut host);
        for (source, color) in [
            ("glyph:chevron-left", LEFT),
            ("glyph:chevron-right", RIGHT),
            ("cli:custom-3", CUSTOM_CLI),
            ("cli:claude", FIRST_CLI),
            ("glyph:workspace-tab-outline", OUTLINE),
            ("glyph:download", UPDATE),
        ] {
            host.set_image(source.into(), color.to_vec(), [1, 1]);
        }

        let mut cfg = IcyConfig::default();
        cfg.clis.truncate(6);
        let usage: Vec<Value> = cfg
            .clis
            .iter()
            .map(|cli| json!({ "id": cli.id, "percent": 15, "sections": [] }))
            .collect();
        host.push(json!({ "view": "config", "data": cfg }).to_string());
        host.push(json!({ "view": "cliUsage", "data": usage }).to_string());
        let fits = frame(&mut host);
        assert!(image_point(&fits, FIRST_CLI, true).is_some());
        assert!(
            image_point(&fits, RIGHT, true).is_none(),
            "six CLIs fit without a chevron"
        );

        cfg.clis.extend((1..=3).map(|index| config::CliConfig {
            id: format!("custom-{index}"),
            name: format!("Custom CLI {index}"),
            command: "agent".into(),
            args: Vec::new(),
            enabled: true,
            auto_detect: false,
            description: String::new(),
        }));
        host.push(json!({ "view": "config", "data": cfg }).to_string());
        let overflow = frame(&mut host);
        let right = image_point(&overflow, RIGHT, true).expect("CLI strip right chevron");
        host.pointer_move(Some(right));
        host.pointer_down(PointSelection::Place);
        host.pointer_up();
        let end = frame(&mut host);
        assert!(image_point(&end, CUSTOM_CLI, true).is_some());
        assert!(image_point(&end, FIRST_CLI, true).is_none());
        assert!(image_point(&end, LEFT, true).is_some());
        assert!(
            image_point(&end, RIGHT, true).is_some(),
            "hover keeps the disabled control in place"
        );
        host.pointer_down(PointSelection::Place);
        host.pointer_up();
        frame(&mut host);
        assert!(!rx.try_iter().any(|request| request.contains("paletteKeys")));
        host.pointer_move(None);
        let unhovered = frame(&mut host);
        assert!(image_point(&unhovered, RIGHT, true).is_none());

        host.pointer_move(image_point(&unhovered, CUSTOM_CLI, true));
        host.push(
            json!({ "view": "horizontalScroll", "data": { "delta": -400, "smooth": false } })
                .to_string(),
        );
        let start = frame(&mut host);
        assert!(image_point(&start, FIRST_CLI, true).is_some());
        assert!(image_point(&start, LEFT, true).is_none());

        let mut tiling = Workspaces::empty(TilingMode::Niri);
        let sessions: Vec<Value> = (1..=10)
            .map(|id| {
                json!({ "id": id, "title": format!("shell-{id}"), "cli": "terminal",
                "cwd": format!("/project-{id}"), "alive": true })
            })
            .collect();
        host.push(json!({ "view": "sessions", "data": { "activeId": 10, "archivedCount": 0, "items": sessions } }).to_string());
        for id in 1..=5 {
            tiling.open_pane(id, false);
        }
        host.push(json!({ "view": "tiling", "data": tiling.snapshot() }).to_string());
        let five_tabs = frame(&mut host);
        assert!(has_tab(&five_tabs, "project-1") && has_tab(&five_tabs, "project-5"));
        assert!(image_point(&five_tabs, LEFT, false).is_none());
        assert!(image_point(&five_tabs, RIGHT, false).is_none());

        let update = updater::UpdateState {
            has_update: true,
            available_version: "2.0.0".into(),
            asset: Some(sample_update_asset()),
            ..updater::UpdateState::initial()
        };
        host.push(json!({ "view": "update", "data": update }).to_string());
        let with_update = frame(&mut host);
        assert!(has_tab(&with_update, "project-5"));
        assert!(image_point(&with_update, RIGHT, false).is_none());
        assert!(image_point(&with_update, UPDATE, false).is_none());
        let update_button = image_point(&with_update, UPDATE, true).expect("sidebar update button");
        host.pointer_move(Some(update_button));
        host.pointer_down(PointSelection::Place);
        host.pointer_up();
        assert!(frame(&mut host).items.iter().any(|item| matches!(item,
            Item::Text { text, rect, clip, .. } if text == "Download update" && rect[0] >= 224. && clip[2] > 0.
        )));
        let downloading = updater::UpdateState {
            stage: "downloading".into(),
            progress: 35,
            ..update
        };
        host.push(json!({ "view": "update", "data": downloading }).to_string());
        assert!(frame(&mut host).items.iter().any(|item| matches!(item,
            Item::Text { text, clip, .. } if text == "Downloading… 35%" && clip[2] > 0.
        )));
        host.escape();

        for id in 6..=10 {
            tiling.open_pane(id, false);
        }
        host.push(json!({ "view": "tiling", "data": tiling.snapshot() }).to_string());
        let last_tab = frame(&mut host);
        assert!(
            has_tab(&last_tab, "project-10"),
            "the focused tab scrolls into view"
        );
        assert!(image_point(&last_tab, LEFT, false).is_some());
        assert!(image_point(&last_tab, RIGHT, false).is_none());
        assert!(
            outline_fits(&last_tab),
            "the divider and both curves stay visible"
        );
        host.pointer_move(image_point(&last_tab, LEFT, false));
        host.push(
            json!({ "view": "horizontalScroll", "data": { "delta": -1500, "smooth": false } })
                .to_string(),
        );
        let first_tab = frame(&mut host);
        assert!(
            has_tab(&first_tab, "project-1"),
            "manual scrolling can move away from the focused tab"
        );
        assert!(image_point(&first_tab, RIGHT, false).is_some());

        // Removing the other tabs clears both navigation gutters. No edge paint may cover
        // the remaining title, even when the strip was previously scrolled.
        host.pointer_move(None);
        host.push(
            json!({ "view": "tiling", "data": Workspaces::new(1, TilingMode::Niri).snapshot() })
                .to_string(),
        );
        let single = frame(&mut host);
        assert!(image_point(&single, LEFT, false).is_none());
        assert!(image_point(&single, RIGHT, false).is_none());
        assert!(has_tab(&single, "project-1"));
        assert!(outline_fits(&single));
        let mut after_title = false;
        let mut title_rect = [0.; 4];
        for item in single.items {
            match item {
                Item::Text { text, rect, .. } if text == "project-1" && rect[1] < 52. => {
                    title_rect = rect;
                    after_title = true;
                }
                Item::Fill {
                    rect, clip, top, ..
                } if after_title && top[3] > 0 => {
                    let intersects = |bounds: [f32; 4]| {
                        bounds[0] < title_rect[0] + title_rect[2]
                            && bounds[0] + bounds[2] > title_rect[0]
                            && bounds[1] < title_rect[1] + title_rect[3]
                            && bounds[1] + bounds[3] > title_rect[1]
                    };
                    assert!(
                        !intersects(rect) || !intersects(clip),
                        "paint overlaps the tab title"
                    );
                }
                _ => {}
            }
        }
    }

    #[test]
    fn overview_integrates_the_header_and_opens_the_trailing_workspace() {
        use jsnir::editor::PointSelection;
        use jsnir::host::{Frame, Item};
        use serde_json::json;

        const WORKSPACES_ICON: [u8; 4] = [1, 2, 3, 255];
        const LAUNCHER_ICON: [u8; 4] = [4, 5, 6, 255];

        fn frame(host: &mut Host) -> Frame {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                host.poll();
                let frame = host.frame(1280., 800., 1., 4096);
                if (!host.busy() && !frame.animating) || Instant::now() >= deadline {
                    return frame;
                }
                std::thread::sleep(Duration::from_millis(4));
            }
        }

        fn header_icon(frame: &Frame, color: [u8; 4]) -> Option<[f32; 2]> {
            frame.items.iter().find_map(|item| match item {
                Item::Image { image, rect, .. }
                    if image.rgba.as_ref() == color && rect[0] >= 224. && rect[1] < 52. =>
                {
                    Some([rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.])
                }
                _ => None,
            })
        }

        fn has_content_frame(frame: &Frame) -> bool {
            frame.items.iter().any(|item| {
                matches!(item, Item::Inset { rect, radius, spread, color, .. }
                    if rect[0] == 224. && rect[1] == 52. && rect[2] > 1000.
                        && *radius == [8., 8., 8., 8.] && *spread == 1. && color[3] > 0)
            })
        }

        fn has_header_frame(frame: &Frame) -> bool {
            frame.items.iter().any(|item| {
                matches!(item, Item::Inset { rect, color, .. }
                    if rect[0] >= 224. && rect[1] < 52. && rect[2] > 1000. && color[3] > 0)
            })
        }

        fn add_label(frame: &Frame) -> ([f32; 4], [u8; 4]) {
            frame
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Text {
                        text, rect, style, ..
                    } if text == "Add workspace" => Some((*rect, style.color)),
                    _ => None,
                })
                .expect("visible add workspace action")
        }

        fn press(host: &mut Host) {
            host.pointer_down(PointSelection::Place);
            host.pointer_up();
        }

        let (tx, rx) = mpsc::channel();
        let mut host = Host::with_request_handler(
            APP.into(),
            "{}".into(),
            TextEngine::default(),
            move |raw| {
                tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                    .unwrap();
                Ok("ok".into())
            },
        )
        .unwrap();
        install_fonts(&mut host);
        host.set_image("glyph:workspaces".into(), WORKSPACES_ICON.to_vec(), [1, 1]);
        host.set_image("glyph:plus".into(), LAUNCHER_ICON.to_vec(), [1, 1]);
        host.push(
            json!({"view": "tiling", "data": Workspaces::new(1, TilingMode::Niri).snapshot()})
                .to_string(),
        );
        let normal = frame(&mut host);
        let toggle = header_icon(&normal, WORKSPACES_ICON).expect("workspace toggle");
        assert!(header_icon(&normal, LAUNCHER_ICON).is_some());
        assert!(has_content_frame(&normal));
        assert!(!has_header_frame(&normal));

        host.pointer_move(Some(toggle));
        press(&mut host);
        host.pointer_move(None);
        let overview = frame(&mut host);
        assert!(header_icon(&overview, WORKSPACES_ICON).is_some());
        assert!(header_icon(&overview, LAUNCHER_ICON).is_none());
        assert!(!has_content_frame(&overview));
        assert!(!has_header_frame(&overview));
        assert!(!overview.items.iter().any(|item| matches!(item,
            Item::Image { image, rect, .. } if image.rgba.as_ref() == LAUNCHER_ICON && rect[0] >= 224.
        )));
        // The trailing workspace peeks in below the active one; a click anywhere on it opens it.
        let (label, muted) = add_label(&overview);
        host.pointer_move(Some([label[0] - 80., label[1] + label[3] / 2.]));
        press(&mut host);
        let restored = frame(&mut host);
        assert!(header_icon(&restored, LAUNCHER_ICON).is_some());
        assert!(has_content_frame(&restored));
        assert!(!has_header_frame(&restored));
        let focus = json!({"type": "focusWorkspace", "index": 1});
        assert!(rx.try_iter().any(|request| request == focus));

        host.pointer_move(Some(toggle));
        press(&mut host);
        host.pointer_move(None);
        host.push(json!({"view": "modalKey", "data": "down"}).to_string());
        assert_ne!(add_label(&frame(&mut host)).1, muted);
        host.push(json!({"view": "modalKey", "data": "enter"}).to_string());
        frame(&mut host);
        assert!(rx.try_iter().any(|request| request == focus));
    }

    #[test]
    fn shortcut_icons_record_cancel_unassign_and_restore() {
        use jsnir::editor::PointSelection;
        use jsnir::host::{Frame, Item};
        use serde_json::json;

        fn frame(host: &mut Host) -> Frame {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                host.poll();
                let frame = host.frame(1280., 900., 1., 4096);
                if !host.busy() && !frame.animating || Instant::now() >= deadline {
                    return frame;
                }
                std::thread::sleep(Duration::from_millis(4));
            }
        }

        fn icon(frame: &Frame, pixel: [u8; 4]) -> [f32; 2] {
            frame
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Image { image, rect, .. } if image.rgba.as_ref() == pixel => {
                        Some([rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.])
                    }
                    _ => None,
                })
                .expect("shortcut action icon")
        }

        fn click(host: &mut Host, point: [f32; 2]) {
            host.pointer_move(Some(point));
            host.pointer_down(PointSelection::Place);
            host.pointer_up();
        }

        let (tx, rx) = mpsc::channel();
        let script = format!("{APP}\nactiveNav.value = 'settings'; settingsTab.value = 'tiling';");
        let mut host =
            Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
                tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                    .unwrap();
                Ok("ok".into())
            })
            .unwrap();
        install_fonts(&mut host);
        let record = [1, 2, 3, 255];
        let close = [2, 3, 4, 255];
        let reset = [3, 4, 5, 255];
        for (source, rgba) in [
            ("glyph:keyboard", record),
            ("glyph:close", close),
            ("glyph:refresh", reset),
        ] {
            host.set_image(source.into(), rgba.to_vec(), [1, 1]);
        }
        let overrides = keybinds::Overrides::from([("session.new".into(), vec!["Alt+F".into()])]);
        host.push(
            json!({"view": "keybinds", "data": {"items": keybinds::views(&overrides)}}).to_string(),
        );
        let initial = frame(&mut host);
        assert!(
            !initial
                .items
                .iter()
                .any(|item| matches!(item, Item::Text { text, .. }
            if matches!(text.as_str(), "Record" | "Unassign" | "Reset")))
        );

        let record_point = icon(&initial, record);
        host.pointer_move(Some(record_point));
        let hovered = frame(&mut host);
        assert!(
            hovered
                .items
                .iter()
                .any(|item| matches!(item, Item::Text { text, .. } if text == "Record shortcut"))
        );
        click(&mut host, record_point);
        let recording = frame(&mut host);
        assert!(
            recording
                .items
                .iter()
                .any(|item| matches!(item, Item::Text { text, .. } if text == "Press keys…"))
        );
        click(&mut host, icon(&recording, close));
        let cancelled = frame(&mut host);
        click(&mut host, icon(&cancelled, close));
        let unassigned = frame(&mut host);
        click(&mut host, icon(&unassigned, reset));
        frame(&mut host);

        let requests: Vec<_> = rx.try_iter().collect();
        assert!(requests.contains(&json!({"type": "keyCapture", "on": true})));
        assert!(requests.contains(&json!({"type": "keyCapture", "on": false})));
        assert!(
            requests.contains(&json!({"type": "setKeybind", "id": "session.new", "chord": ""}))
        );
        assert!(requests.contains(&json!({"type": "resetKeybind", "id": "session.new"})));
    }

    #[test]
    fn archiving_and_restoring_keeps_the_original_terminal() {
        let root = tempfile::tempdir().unwrap();
        let mut manager = SessionManager::new(root.path().into(), Arc::new(|| {}));
        let id = manager
            .create_session_in(false, root.path().into())
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        manager
            .focused_terminal()
            .unwrap()
            .write_input(b"echo ARCHIVE_MARKER\r")
            .unwrap();
        let contains_history = |session: &Terminal| {
            session
                .frame()
                .0
                .rows
                .iter()
                .any(|row| row.plain_text().contains("ARCHIVE_MARKER"))
        };
        while !contains_history(manager.focused_terminal().unwrap()) {
            assert!(
                Instant::now() < deadline,
                "terminal did not echo the marker"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(manager.set_session_archived(id, true).unwrap());
        assert!(!manager.set_session_archived(id, true).unwrap());
        assert!(manager.tiling.pane_order().is_empty());
        assert!(!manager.select_session(id));
        assert!(
            !manager.close_session(id),
            "an archived running session is protected"
        );
        manager.overview = true;
        assert!(manager.visible_terminals().next().is_none());
        let payload = manager.sessions_payload();
        assert!(payload.items.is_empty());
        assert_eq!(payload.archived_count, 1);
        assert_eq!(payload.archived[0].session.id, id);

        assert!(manager.set_session_archived(id, false).unwrap());
        assert!(!manager.set_session_archived(id, false).unwrap());
        assert_eq!(manager.tiling.pane_order(), [id]);
        let restored = manager.focused_terminal().unwrap();
        assert_eq!(restored.id, id);
        assert!(restored.is_alive());
        assert!(contains_history(restored));
        assert!(manager.sessions_payload().archived.is_empty());
    }

    #[test]
    fn terminal_opens_beside_existing_tile_and_can_be_closed() {
        let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project = cwd.join("src");
        let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut manager = SessionManager::new(cwd, Arc::clone(&notify));
        let tile = manager.open_tile(false, project.clone());
        assert!(manager.toggle_maximize());
        let id = manager.allocate_session_id();
        let (program, args) = platform::echo_command("icy-prompt");
        let terminal = Session::spawn_command(
            id,
            "test shell",
            project.clone(),
            &program,
            &args,
            DEFAULT_COLS,
            DEFAULT_ROWS,
            notify,
        )
        .unwrap();
        assert_eq!(manager.place_session(terminal, false), id);
        assert_eq!(manager.tiling.pane_order(), [tile, id]);
        assert!(manager.tiling.snapshot().maximized);
        assert_eq!(manager.workspace_dir(), project);
        assert!(manager.focused_terminal().is_some());
        assert!(manager.visible_screens().contains_key(&id.to_string()));
        assert!(manager.close_session(id));
        assert_eq!(manager.tiling.pane_order(), [tile]);
        assert!(manager.visible_screens().is_empty());
    }

    #[test]
    fn new_terminals_start_at_the_destination_grid_size() {
        let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut manager = SessionManager::new(cwd.clone(), Arc::new(|| {}));
        manager.tiling = Workspaces::empty(TilingMode::Niri);
        manager.config.font_size = 13;
        let first = manager.open_tile(false, cwd.clone());
        let next = manager.next_id;
        let full = manager.new_terminal_size(next, false);
        let stacked = manager.new_terminal_size(next, true);
        assert!(
            stacked.1 < full.1,
            "stacking must account for a shorter pane"
        );

        assert!(manager.tiling.adjust_column_width(-0.25));
        manager.apply_tiled_sizes();
        let wide = manager.new_terminal_size(next, false);
        assert!(
            wide.0 > manager.cols,
            "new column is wider than the focused column"
        );
        assert_eq!(manager.active_id(), first, "sizing must not mutate focus");

        assert!(manager.toggle_maximize());
        assert_eq!(manager.new_terminal_size(next, false), (130, 39));
    }

    #[test]
    fn empty_workspace_selects_a_project_and_displays_its_terminal() {
        use jsnir::editor::{EditorEvent, PointSelection};
        use jsnir::host::{Frame, Item};
        use serde_json::json;

        fn frame(host: &mut Host) -> Frame {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                host.poll();
                let frame = host.frame(1280., 800., 1., 4096);
                if !host.busy() && !frame.animating || Instant::now() >= deadline {
                    return frame;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }

        fn text_rect(frame: &Frame, label: &str) -> Option<[f32; 4]> {
            frame.items.iter().find_map(|item| match item {
                Item::Text { text, rect, .. } if text == label => Some(*rect),
                _ => None,
            })
        }

        fn dialog_text(frame: &Frame, label: &str) -> [f32; 4] {
            frame
                .items
                .iter()
                .rev()
                .find_map(|item| match item {
                    Item::Text { text, rect, .. } if text == label && rect[0] > 224. => Some(*rect),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing dialog label {label}"))
        }

        fn glyph_rect(frame: &Frame, pictures: &[icons::Picture], source: &str) -> [f32; 4] {
            let rgba = &pictures
                .iter()
                .find(|picture| picture.0 == source)
                .unwrap()
                .1;
            frame
                .items
                .iter()
                .rev()
                .find_map(|item| match item {
                    Item::Image { image, rect, .. } if image.rgba.as_ref() == rgba => Some(*rect),
                    _ => None,
                })
                .expect("visible folder control")
        }

        fn click(host: &mut Host, rect: [f32; 4]) {
            host.pointer_move(Some([rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.]));
            host.pointer_down(PointSelection::Place);
            host.pointer_up();
        }

        let cwd = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let next = cwd.join("app");
        let home = cwd.join("assets");
        let manager = Arc::new(Mutex::new(SessionManager::new(
            cwd.clone(),
            Arc::new(|| {}),
        )));
        let (tx, rx) = mpsc::channel();
        let req_manager = Arc::clone(&manager);
        let req_tx = tx.clone();
        let (selected_tx, selected_rx) = mpsc::channel();
        let (added_tx, added_rx) = mpsc::channel();
        let mut host = Host::with_request_handler(
            APP.into(),
            "{}".into(),
            TextEngine::default(),
            move |request| {
                let request_json: Value = serde_json::from_str(&request).unwrap();
                if request_json["type"] == "newSession" {
                    let selected = PathBuf::from(request_json["cwd"].as_str().unwrap());
                    selected_tx.send(selected).unwrap();
                    return Ok("ok".into());
                }
                if request_json["type"] == "addProject" {
                    let folder = PathBuf::from(request_json["path"].as_str().unwrap());
                    added_tx.send(folder).unwrap();
                    return Ok("ok".into());
                }
                if request_json["type"] == "browseProject" && request_json["path"] == "~" {
                    return serde_json::to_string(&browse_project_dir(home.clone())?)
                        .map_err(|err| err.to_string());
                }
                assert_ne!(
                    request_json["type"], "overview",
                    "unexpected overview action"
                );
                handle_request(&request, &req_manager, &req_tx)
            },
        )
        .unwrap();
        install_fonts(&mut host);
        let pictures = icons::load_static(1.0);
        for (source, rgba, size) in &pictures {
            host.set_image(source.clone(), rgba.clone(), *size);
        }
        let cfg = IcyConfig {
            projects: vec![next.to_string_lossy().into_owned()],
            ..IcyConfig::default()
        };
        host.push(json!({"view": "config", "data": cfg}).to_string());

        let (sessions, startup_tiling) = {
            let guard = manager.lock().unwrap();
            assert!(guard.sessions.is_empty());
            assert!(guard.tiling.pane_order().is_empty());
            assert_eq!(guard.workspace_dir(), cwd.as_path());
            (guard.sessions_payload(), guard.tiling_snapshot())
        };
        host.push(json!({ "view": "sessions", "data": sessions }).to_string());
        host.push(json!({ "view": "tiling", "data": startup_tiling }).to_string());
        host.push(json!({ "view": "workspace", "data": cwd }).to_string());
        let empty = frame(&mut host);
        assert!(text_rect(&empty, "Empty workspace").is_some());
        // The tab row keeps a "New session" tab while the workspace has no sessions.
        assert!(empty.items.iter().any(|item| matches!(item,
            Item::Text { text, rect, .. } if text == "New session" && rect[0] >= 224. && rect[1] < 52.
        )));
        let primary_action = empty.items.iter().find_map(|item| match item {
            Item::Text { text, rect, .. }
                if text == "New session" && rect[0] >= 224. && rect[1] >= 52. =>
            {
                Some(*rect)
            }
            _ => None,
        });
        click(
            &mut host,
            primary_action.expect("empty workspace session button"),
        );

        let dialog = frame(&mut host);
        assert!(text_rect(&dialog, "Search agents or terminal").is_some());
        assert!(host.focused(), "the dialog should focus its search field");
        click(&mut host, dialog_text(&dialog, "Terminal"));
        let folder = frame(&mut host);
        assert!(text_rect(&folder, "Browse").is_some());
        assert!(host.focused(), "the folder path should receive focus");
        let choose =
            |frame: &Frame| text_rect(frame, "Open Terminal").expect("folder confirmation");
        // A typed path naming a file lists no folder; choosing validates it as one.
        host.edit(&[
            EditorEvent::SelectAll,
            EditorEvent::Insert(cwd.join("Cargo.toml").display().to_string()),
        ]);
        let edited = frame(&mut host);
        click(&mut host, choose(&edited));
        let invalid = frame(&mut host);
        assert!(
            invalid
                .items
                .iter()
                .any(|item| matches!(item, Item::Text { text, .. }
                if text.contains("Cargo.toml is not a folder."))),
            "validation error missing from {:?}",
            invalid
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Text { text, .. } => Some(text),
                    _ => None,
                })
                .collect::<Vec<_>>()
        );
        assert!(selected_rx.try_recv().is_err());

        host.edit(&[
            EditorEvent::SelectAll,
            EditorEvent::Insert(format!("{}{}", cwd.display(), std::path::MAIN_SEPARATOR)),
        ]);
        let listing = frame(&mut host);
        click(&mut host, dialog_text(&listing, "app/"));
        let child = frame(&mut host);
        assert!(text_rect(&child, "No subfolders").is_some());
        click(&mut host, glyph_rect(&child, &pictures, "glyph:arrow-up"));
        let parent = frame(&mut host);
        assert!(text_rect(&parent, "crates/").is_some());
        click(&mut host, glyph_rect(&parent, &pictures, "glyph:home"));
        let home = frame(&mut host);
        assert!(text_rect(&home, "icons/").is_some());
        // Search mode: the saved project is a recent.
        host.edit(&[EditorEvent::SelectAll, EditorEvent::Insert("app".into())]);
        let projects = frame(&mut host);
        assert!(text_rect(&projects, "icons/").is_none());
        click(&mut host, dialog_text(&projects, "app"));
        let selected = frame(&mut host);
        assert!(text_rect(&selected, "No subfolders").is_some());
        click(&mut host, choose(&selected));
        let opened = frame(&mut host);
        assert!(text_rect(&opened, "Open Terminal").is_none());
        assert!(!host.focused());
        assert_eq!(selected_rx.try_recv().unwrap(), next);

        let mut tiling = Workspaces::new(1, TilingMode::Niri);
        host.push(
            json!({"view": "sessions", "data": {
                "activeId": 1, "archivedCount": 0, "items": [{
                    "id": 1, "title": "shell-1", "cli": "terminal", "cwd": next, "alive": true
                }]
            }})
            .to_string(),
        );
        host.push(json!({ "view": "tiling", "data": tiling.snapshot() }).to_string());
        host.push(
            json!({"view": "screens", "data": { "1": {
                "cols": 80, "rowsCount": 1, "cursorRow": 0, "cursorCol": 1, "cursorVisible": true,
                "rows": [{ "idx": 0, "plain": "icy-prompt", "spans": [], "links": [] }]
            }}})
            .to_string(),
        );
        let terminal = frame(&mut host);
        assert!(text_rect(&terminal, "icy-prompt").is_some());
        assert!(text_rect(&terminal, "Empty workspace").is_none());

        assert_eq!(
            tiling.len(),
            2,
            "opening in the empty workspace keeps an empty workspace below"
        );
        tiling.remove_session(1);
        host.push(json!({ "view": "tiling", "data": tiling.snapshot() }).to_string());
        let empty_again = frame(&mut host);
        assert!(text_rect(&empty_again, "Empty workspace").is_some());

        handle_request(
            &json!({ "type": "listFiles", "cwd": next }).to_string(),
            &manager,
            &tx,
        )
        .unwrap();
        assert!(
            std::iter::from_fn(|| rx.recv_timeout(Duration::from_secs(2)).ok()).any(
                |event| matches!(event, BridgeEvent::PushFiles(files)
                    if Path::new(&files.cwd) == next && files.entries.iter().any(|entry| entry.name == "main.tsx"))
            )
        );

        let worker = cwd.join("assets");
        host.push(
            json!({"view": "sessions", "data": {
                "activeId": 1, "archivedCount": 0, "items": [
                    { "id": 1, "title": "shell-1", "cli": "terminal", "cwd": next, "alive": true },
                    { "id": 2, "title": "⠋ worker", "cli": "codex", "cwd": worker, "alive": true }
                ]
            }})
            .to_string(),
        );
        let all = frame(&mut host);
        click(&mut host, glyph_rect(&all, &pictures, "glyph:clock"));
        let sort = frame(&mut host);
        click(
            &mut host,
            text_rect(&sort, "Working only").expect("working filter"),
        );
        let working = frame(&mut host);
        assert!(text_rect(&working, "assets · worker").is_some());
        assert!(text_rect(&working, "Idle").is_none());
        click(&mut host, glyph_rect(&working, &pictures, "glyph:clock"));
        let sort = frame(&mut host);
        click(
            &mut host,
            text_rect(&sort, "Idle only").expect("idle filter"),
        );
        let idle = frame(&mut host);
        assert!(text_rect(&idle, "assets · worker").is_none());
        assert!(text_rect(&idle, "Idle").is_some());
        click(&mut host, glyph_rect(&idle, &pictures, "glyph:folder-plus"));
        let add = frame(&mut host);
        assert!(text_rect(&add, "Add project").is_some());
        click(&mut host, dialog_text(&add, "assets/"));
        let selected_project = frame(&mut host);
        click(&mut host, dialog_text(&selected_project, "Add project"));
        let added = frame(&mut host);
        assert!(text_rect(&added, "Add project").is_none());
        assert_eq!(added_rx.try_recv().unwrap(), worker);
    }

    #[test]
    fn metrics_loading_keeps_the_dashboard_geometry_and_stops_animating() {
        use jsnir::editor::PointSelection;
        use jsnir::host::{Frame, Item};
        use serde_json::json;

        fn label(frame: &Frame, label: &str) -> Option<[f32; 4]> {
            frame.items.iter().find_map(|item| match item {
                Item::Text { text, rect, .. } if text == label => Some(*rect),
                _ => None,
            })
        }

        for width in [1280., 480.] {
            let (requests, received) = mpsc::channel();
            let mut host = Host::with_request_handler(
                format!("{APP}\nmount(MetricsView);"),
                "{}".into(),
                TextEngine::default(),
                move |raw| {
                    let request: Value = serde_json::from_str(&raw).unwrap();
                    if request["type"] == "scanUsage" {
                        requests.send(request["refresh"].clone()).unwrap();
                    }
                    Ok("ok".into())
                },
            )
            .unwrap();
            install_fonts(&mut host);
            const REFRESH_ICON: [u8; 4] = [1, 2, 3, 255];
            host.set_image("glyph:refresh".into(), REFRESH_ICON.to_vec(), [1, 1]);
            host.push(json!({ "view": "config", "data": IcyConfig::default() }).to_string());

            let wait_for = |host: &mut Host, text| {
                let deadline = Instant::now() + Duration::from_secs(2);
                loop {
                    host.poll();
                    let frame = host.frame(width, 1800., 1., 4096);
                    if !frame.animating && label(&frame, text).is_some() {
                        break frame;
                    }
                    assert!(Instant::now() < deadline, "missing settled label {text:?}");
                    std::thread::sleep(Duration::from_millis(4));
                }
            };

            let skeleton = wait_for(&mut host, "Token volume");
            assert_eq!(received.try_recv().unwrap(), false);
            assert!(label(&skeleton, "Input tokens").is_none());

            host.push(
                json!({
                    "view": "metricsUsage", "data": { "today": 20_000, "agents": [] }
                })
                .to_string(),
            );
            let ready = wait_for(&mut host, "Input tokens");
            for section in ["Token volume", "Peak activity", "Agents", "Models"] {
                let before =
                    label(&skeleton, section).unwrap_or_else(|| panic!("skeleton {section}"));
                let after = label(&ready, section).unwrap_or_else(|| panic!("loaded {section}"));
                assert!(
                    (before[1] - after[1]).abs() <= 1.,
                    "{width}px, {section}: {before:?} -> {after:?}"
                );
            }

            let [x, y, w, h] = ready
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Image { image, rect, .. } if image.rgba.as_ref() == REFRESH_ICON => {
                        Some(*rect)
                    }
                    _ => None,
                })
                .expect("refresh control");
            host.pointer_move(Some([x + w / 2., y + h / 2.]));
            host.pointer_down(PointSelection::Place);
            host.pointer_up();
            let refreshing = wait_for(&mut host, "Refreshing local usage…");
            assert_eq!(received.try_recv().unwrap(), true);
            assert!(label(&refreshing, "Input tokens").is_some());
        }
    }

    #[test]
    fn metrics_filters_usage_and_repaints_when_the_theme_reloads() {
        use jsnir::editor::PointSelection;
        use jsnir::host::{Frame, Item};
        use serde_json::json;

        fn frame(host: &mut Host) -> Frame {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                host.poll();
                let frame = host.frame(1280., 940., 1., 4096);
                if !host.busy() || Instant::now() >= deadline {
                    return frame;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }

        fn click(host: &mut Host, frame: &Frame, label: &str) {
            let rect = frame
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Text { text, rect, .. } if text == label => Some(rect),
                    _ => None,
                })
                .expect("visible filter control");
            host.pointer_move(Some([rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.]));
            host.pointer_down(PointSelection::Place);
            host.pointer_up();
        }

        fn has_metric(frame: &Frame, expected: &str) -> bool {
            frame.items.iter().any(|item| {
                matches!(
                    item,
                    Item::Text { text, style, .. } if text == expected && style.size == 30.
                )
            })
        }

        let day = |date, input| {
            let mut hours = [0; 24];
            hours[18] = 1;
            json!({
                "day": date, "tokens": { "input": input, "output": 7, "cached": 0, "calls": 1 },
                "sessions": 1, "durationMs": 1000, "timedTurns": 1, "hours": hours,
                "models": []
            })
        };
        let periods = [
            json!({ "current": 1, "previous": 0 }),
            json!({ "current": 1, "previous": 0 }),
            json!({ "current": 1, "previous": 0 }),
            json!({ "current": 1, "previous": 0 }),
        ];
        // Metrics is a compositor window; mount its view alone to test the dashboard.
        let script = format!("{APP}\nmount(MetricsView);");
        let mut host =
            Host::with_request_handler(script, "{}".into(), TextEngine::default(), |_| {
                Ok("ok".into())
            })
            .unwrap();
        install_fonts(&mut host);
        host.push(json!({ "view": "config", "data": IcyConfig::default() }).to_string());
        host.push(json!({
            "view": "metricsUsage",
            "data": { "today": 20_000, "agents": [
                { "id": "claude", "periods": periods, "days": [day(19_980, 12_400), day(20_000, 900)] },
                { "id": "codex", "periods": periods, "days": [day(20_000, 310)] }
            ] }
        }).to_string());

        let initial = frame(&mut host);
        assert!(has_metric(&initial, "13.6K"));
        let has_heat_cell = |frame: &Frame, color: [u8; 4]| {
            frame.items.iter().any(|item| {
                matches!(item,
                Item::Fill { top, rect, .. } if *top == color && rect[3] == 20. && rect[2] < 24.)
            })
        };
        assert!(has_heat_cell(&initial, [0x3b, 0x82, 0xf6, 0xff]));
        let mut config = serde_json::to_value(IcyConfig::default()).unwrap();
        config["accent"] = json!("rose");
        host.push(json!({ "view": "config", "data": config }).to_string());
        let rose = frame(&mut host);
        assert!(has_heat_cell(&rose, [0xf4, 0x3f, 0x5e, 0xff]));
        assert!(!has_heat_cell(&rose, [0x3b, 0x82, 0xf6, 0xff]));
        click(&mut host, &initial, "7 days");
        let week = frame(&mut host);
        assert!(has_metric(&week, "1.2K"));
        click(&mut host, &week, "Claude Code");
        let claude = frame(&mut host);
        assert!(has_metric(&claude, "900"));

        host.push(json!({
            "view": "themes",
            "data": { "items": [{
                "id": "accordee-dark",
                "colors": {
                    "appBg": 0x0a0a0aff_u32, "sidebarBg": 0x0f0f10ff_u32, "sidebarActive": 0x1f1f22ff_u32,
                    "panelBg": 0x171717ff_u32, "cardBg": 0x171717ff_u32, "kbdBg": 0x262626ff_u32,
                    "textPrimary": 0xfafafaff_u32, "textSecondary": 0xa3a3a3ff_u32, "textMuted": 0x8a8a8aff_u32,
                    "border": 0xffffff1a_u32, "highlight": 0x3b82f6ff_u32,
                    "charts": { "input": 0xf5b943ff_u32 }
                }
            }] }
        }).to_string());
        let recolored = frame(&mut host);
        assert!(recolored.items.iter().any(|item| matches!(
            item,
            Item::Fill { top: [0xf5, 0xb9, 0x43, 0xff], rect, .. } if rect[2] > 0. && rect[3] > 0.
        )));
        assert!(has_metric(&recolored, "900"));
    }

    #[test]
    fn metrics_fades_the_scrolled_edge_and_keeps_chart_tooltips_stable() {
        use jsnir::host::{Frame, Item};
        use serde_json::json;
        use std::sync::atomic::{AtomicU16, Ordering};

        fn frame(host: &mut Host) -> Frame {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                host.poll();
                let frame = host.frame(1280., 940., 1., 4096);
                if (!host.busy() && !frame.animating) || Instant::now() >= deadline {
                    return frame;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }

        fn label(frame: &Frame, value: &str) -> [f32; 4] {
            frame
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Text { text, rect, .. } if text == value => Some(*rect),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing chart label {value:?}"))
        }

        fn fade(frame: &Frame) -> Option<[f32; 4]> {
            frame.items.iter().find_map(|item| match item {
                Item::Fill {
                    rect,
                    top: [10, 10, 10, 255],
                    bottom: [10, 10, 10, 0],
                    ..
                } => Some(*rect),
                _ => None,
            })
        }

        let target = Arc::new(AtomicU16::new(0));
        let requested = Arc::clone(&target);
        let mut host = Host::with_request_handler(
            format!("{APP}\nmount(MetricsView);"),
            "{}".into(),
            TextEngine::default(),
            move |request| {
                let request: Value = serde_json::from_str(&request).unwrap();
                if request["type"] == "metricsPointer" {
                    requested.store(request["id"].as_u64().unwrap() as u16, Ordering::Relaxed);
                }
                Ok("ok".into())
            },
        )
        .unwrap();
        install_fonts(&mut host);
        let tokens = |input| json!({"input": input, "output": 0, "cached": 0, "calls": 1});
        host.push(
            json!({
                "view": "metricsUsage", "data": { "today": 20_000, "agents": [{
                    "id": "claude", "periods": vec![json!({"current": 1, "previous": 0}); 4],
                    "days": [{ "day": 20_000, "tokens": tokens(100), "sessions": 1,
                        "durationMs": 0, "timedTurns": 0, "hours": ([0; 24]), "models": [
                            { "name": "Claude", "provider": "anthropic", "tokens": tokens(60) },
                            { "name": "GPT", "provider": "openai", "tokens": tokens(40) }
                        ] }]
                }] }
            })
            .to_string(),
        );

        let tooltip_rect = |frame: &Frame| {
            frame.items.iter().find_map(|item| match item {
                Item::Fill {
                    rect,
                    top: [25, 29, 36, 255],
                    ..
                } if rect[2] == 212. => Some(*rect),
                _ => None,
            })
        };
        let hover = |host: &mut Host, [x, y]: [f32; 2]| {
            host.pointer_move(Some([x, y]));
            frame(host);
            let id = target.load(Ordering::Relaxed);
            let [left, top, width, height] = host.screen_rect(id).expect("hovered chart");
            host.push(
                json!({
                    "view": "metricsPointer", "data": [id, x - left, y - top, width, height]
                })
                .to_string(),
            );
            frame(host)
        };
        let tooltip = |host: &mut Host, point| {
            tooltip_rect(&hover(host, point)).expect("visible chart tooltip")
        };

        let initial = frame(&mut host);
        assert_eq!(fade(&initial), None);
        let [_, legend_y, _, _] = label(&initial, "Independent scales");
        let y = legend_y + 40.;
        let right = tooltip(&mut host, [300., y]);
        assert_eq!(right[0], 320.);
        assert!((right[1] - (y + 20.)).abs() <= 0.5);
        let moved = tooltip(&mut host, [304., y + 6.]);
        assert_eq!(moved[0], 324.);
        assert_eq!(moved[1] - right[1], 6.);
        let id = target.load(Ordering::Relaxed);
        let [left, _, width, _] = host.screen_rect(id).unwrap();
        let boundary = left + width - 212. - 20.;
        let near_edge = tooltip(&mut host, [boundary - 1., y]);
        let clamped = tooltip(&mut host, [boundary + 1., y]);
        assert!((clamped[0] - near_edge[0]).abs() <= 2.);
        assert!((clamped[0] + clamped[2] - (left + width)).abs() <= 0.5);

        let [_, weekday_y, _, weekday_height] = label(&initial, "Sun");
        let y = weekday_y + weekday_height / 2.;
        let mut cells = initial
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fill {
                    rect,
                    top: [38, 38, 38, 255],
                    ..
                } if rect[3] == 20. && y > rect[1] && y < rect[1] + rect[3] => Some(*rect),
                _ => None,
            })
            .collect::<Vec<_>>();
        cells.sort_by(|a, b| a[0].total_cmp(&b[0]));
        assert_eq!(cells.len(), 24);
        let [cell_x, _, cell_width, _] = cells[0];
        let up = tooltip(&mut host, [cell_x + cell_width / 2., y]);
        assert!((up[1] + up[3] - (y - 20.)).abs() <= 0.5);
        let activity_id = target.load(Ordering::Relaxed);
        let gutter = (cell_x + cell_width + cells[1][0]) / 2.;
        tooltip(&mut host, [gutter, y]);
        // Even pausing in the gutter must not release the tooltip or reset its side.
        std::thread::sleep(Duration::from_millis(120));
        assert!(tooltip_rect(&frame(&mut host)).is_some());
        let next = hover(&mut host, [cells[1][0] + cells[1][2] / 2., y]);
        assert_eq!(target.load(Ordering::Relaxed), activity_id);
        assert!(tooltip_rect(&next).is_some());
        label(&next, "Sun · 01:00–02:00");

        let [_, models_y, _, _] = label(&initial, "Models");
        let mut segments = initial
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fill { rect, .. }
                    if rect[1] > models_y && rect[3] == 8. && rect[2] > 100. =>
                {
                    Some(*rect)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        segments.sort_by(|a, b| a[0].total_cmp(&b[0]));
        assert_eq!(segments.len(), 2);
        let point = |rect: [f32; 4]| [rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.];
        label(&hover(&mut host, point(segments[0])), "60% · 60");
        let provider_id = target.load(Ordering::Relaxed);
        let provider = hover(&mut host, point(segments[1]));
        label(&provider, "40% · 40");
        assert!(tooltip_rect(&provider).is_some());
        assert_eq!(target.load(Ordering::Relaxed), provider_id);

        host.pointer_move(None);
        let deadline = Instant::now() + Duration::from_secs(1);
        while target.load(Ordering::Relaxed) != 0 && Instant::now() < deadline {
            frame(&mut host);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(target.load(Ordering::Relaxed), 0);
        assert!(tooltip_rect(&frame(&mut host)).is_none());

        host.pointer_move(Some([500., 160.]));
        assert!(host.wheel(40.));
        let scrolled = frame(&mut host);
        let [_, fade_y, width, height] = fade(&scrolled).expect("top scroll fade");
        assert_eq!([width, height], [1280., 24.]);
        assert!(fade_y > label(&scrolled, "Agent usage")[1]);
        assert!(host.wheel(-40.));
        assert_eq!(fade(&frame(&mut host)), None);
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
