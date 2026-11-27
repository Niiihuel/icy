// Shared UI state, host bridge helpers, and Accordee-styled primitive components.

interface TerminalSpan {
  text: string;
  col: number;
  cells: number;
  fg: number;
  bg?: number;
  bold: boolean;
  underline: boolean;
  link: boolean;
  image?: boolean;
  // Inside the mouse selection.
  selected?: boolean;
  // The row link this span is part of (index into `TerminalRow.links`).
  linkIndex?: number;
}

interface DetectedLink {
  kind: "file" | "diff" | "url";
  target: string;
  line?: number | null;
  col?: number | null;
  oldLine?: boolean;
  edit?: EditPreview;
  resolved: boolean;
  image?: boolean;
}

interface TerminalRow {
  idx: number;
  plain: string;
  spans: TerminalSpan[];
  links: DetectedLink[];
}

interface TermImageView {
  key: string;
  row: number;
  col: number;
  cols: number;
  rows: number;
  width: number;
  height: number;
}

interface ScreenView {
  cols: number;
  rowsCount: number;
  cursorRow: number;
  cursorCol: number;
  cursorVisible: boolean;
  // Lines scrolled back into the history (0 = following the output).
  displayOffset?: number;
  rows: TerminalRow[];
  images?: TermImageView[];
}

interface SessionItem {
  id: number;
  title: string;
  cli: string;
  cwd: string;
  alive: boolean;
  status?: "idle" | "working" | "blocked" | "done" | "failed";
}

interface ArchivedItem extends SessionItem {
  archivedAt: number;
}

interface SessionsView {
  activeId: number;
  archivedCount: number;
  archived?: ArchivedItem[];
  items: SessionItem[];
}

interface TilingColumnView {
  id: number;
  panes: number[];
  heights: number[];
  // Strip position and width as fractions of the tiling area width.
  x: number;
  width: number;
  expanded: boolean;
  visible: boolean;
}

interface TilingView {
  mode: "niri" | "hyprland";
  focusedSession: number;
  maximized: boolean;
  viewOffset: number;
  stripWidth: number;
  totalCols: number;
  columns: TilingColumnView[];
  // niri workspaces top to bottom (the last one is always empty); `columns` is the active one.
  workspaces: WorkspaceView[];
  activeWorkspace: number;
}

interface WorkspaceView {
  panes: number[];
  focused: number;
  // Its own tiling, drawn as a miniature in the overview.
  columns: TilingColumnView[];
  viewOffset: number;
}

interface CliConfigItem {
  id: string;
  name: string;
  command: string;
  args: string[];
  enabled: boolean;
  autoDetect: boolean;
  description: string;
}

interface ExternalEditorItem {
  id: string;
  name: string;
}

// niri `layout` options (crates/icy-core/src/tiling.rs LayoutOptions).
interface TilingLayoutOptions {
  defaultColumnWidth: number;
  centerFocusedColumn: "never" | "always" | "on-overflow";
  alwaysCenterSingleColumn: boolean;
}

interface AgentNotificationPreferences {
  finished: boolean;
  needsInput: boolean;
  backgroundOnly: boolean;
}

interface ConfigView {
  theme: string;
  mode: "dark" | "light";
  accent: AccentName;
  density: DensityMode;
  fontSize: number;
  windowTransparency: number;
  tilingMode: "niri" | "hyprland";
  layout: TilingLayoutOptions;
  checkUpdates: boolean;
  autoRestartUpdate: boolean;
  agentNotifications: AgentNotificationPreferences;
  clis: CliConfigItem[];
  projects: string[];
  preferredEditor?: string | null;
}

interface GitHubView {
  connected: boolean;
  method: string;
  username?: string | null;
  sshKeys: string[];
  sshGithubReady: boolean;
  repoSlug?: string | null;
  branch?: string | null;
  dirtyCount: number;
  remoteUrl?: string | null;
  detail: string;
}

interface FileEntryView {
  name: string;
  path: string;
  isDir: boolean;
}

interface FilesView {
  cwd: string;
  subdir: string;
  entries: FileEntryView[];
}

interface ReleaseMediaView {
  kind: string;
  title: string;
  duration: string;
  url: string;
  caption: string;
  posterUrl?: string | null;
}

interface ReleaseNotesView {
  version: string;
  title: string;
  publishedAt: string;
  summary: string;
  highlights: string[];
  media: ReleaseMediaView[];
  releaseUrl: string;
}

interface UpdateView {
  currentVersion: string;
  availableVersion: string;
  hasUpdate: boolean;
  stage: string;
  progress: number;
  statusText: string;
  assetUrl?: string | null;
  stagedPath?: string | null;
  notes: ReleaseNotesView;
}

// Sidebar meter from icy-core::usage::cli: `percent` is set only when the CLI reports a real limit.
interface UsageRowView {
  label: string;
  value: string;
  percent: number | null;
  resetsAt: number | null;
}

interface UsageSectionView {
  title: string;
  rows: UsageRowView[];
}

interface CliUsageItem {
  id: string;
  percent: number | null;
  sections: UsageSectionView[];
}

type TokenKind = "plain" | "keyword" | "string" | "comment" | "number" | "type" | "function";

type CodeHighlight = [start: number, end: number, kind: TokenKind];

interface ViewerLine {
  num: number;
  text: string;
  highlighted?: boolean;
  highlights?: CodeHighlight[];
}

interface DiffLineView {
  kind: "ctx" | "add" | "del";
  oldNum?: number | null;
  newNum?: number | null;
  text: string;
  highlights?: CodeHighlight[];
}

interface EditPreview {
  lines?: DiffLineView[];
  counts?: { additions: number; deletions: number };
}

interface TextInfoView {
  language: string;
  bom: boolean;
  crlf: boolean;
  indent?: { kind: "tabs" } | { kind: "spaces"; size: number } | null;
}

interface ViewerView {
  cwd?: string;
  requestedPath?: string;
  background?: boolean;
  intent?: number;
  open: boolean;
  path: string;
  displayPath: string;
  targetLine?: number | null;
  targetCol?: number | null;
  oldLine?: boolean;
  edit?: EditPreview;
  diffFocus?: number;
  error?: string | null;
  lines: ViewerLine[];
  preview?: { kind: "binary" } | { kind: "font"; name: string; key: string; size: [number, number] };
  editableText?: string;
  editUnavailable?: string;
  textInfo?: TextInfoView;
  mode: "file" | "diff";
  diff: DiffLineView[];
  additions: number;
  deletions: number;
}

interface FileChangeView {
  path: string;
  status: "M" | "A" | "U" | "D" | "R";
  additions: number;
  deletions: number;
}

interface ChangeSetView {
  files: FileChangeView[];
  additions: number;
  deletions: number;
}

// Compositor windows that are not terminals. Every one is a pane of the same tiling as the
// terminals (one tab row, one focus, one fullscreen / overview); the host only knows their ids.
type TileKind = "files" | "review" | "overview" | "metrics" | "archived" | "file" | "diff" | "browser";

interface TileSpec {
  kind: TileKind;
  // Workspace-relative path for "file" / "diff".
  path: string;
  // Initial address of a "browser" pane (later navigation lives in `browserStates`).
  url?: string;
  line?: number;
  col?: number;
  oldLine?: boolean;
  edit?: EditPreview;
  cwd?: string;
  // Pinned file/diff panes stay open instead of being reused by subsequent clicks.
  pinned?: boolean;
}

interface NoticeView {
  kind: string;
  text: string;
}

type NavSection = "agents" | "settings";
type SettingsTab = "appearance" | "clis" | "github" | "tiling" | "updates";

const sessionsState = signal<SessionsView>({
  activeId: 0,
  archivedCount: 0,
  items: [],
});

const tilingState = signal<TilingView>({
  mode: "niri",
  focusedSession: 0,
  maximized: false,
  viewOffset: 0,
  stripWidth: 0,
  totalCols: 0,
  columns: [],
  workspaces: [{ panes: [], focused: 0, columns: [], viewOffset: 0 }],
  activeWorkspace: 0,
});

const emptyScreen: ScreenView = {
  cols: 120,
  rowsCount: 38,
  cursorRow: 0,
  cursorCol: 0,
  cursorVisible: false,
  rows: [],
};
const screenState = signal<ScreenView>(emptyScreen);

// Terminal frames arrive up to 60 times per second while an agent's spinner turns. Keep the previous
// object for every row, screen and map entry that did not change, so the UI only re-lays out
// the rows that really changed instead of the whole grid on each frame.
function sameSpan(a: TerminalSpan, b: TerminalSpan): boolean {
  return a.text === b.text && a.col === b.col && a.cells === b.cells && a.fg === b.fg && a.bg === b.bg
    && a.bold === b.bold && a.underline === b.underline && a.link === b.link && a.image === b.image
    && a.selected === b.selected && a.linkIndex === b.linkIndex;
}

function sameRow(a: TerminalRow, b: TerminalRow): boolean {
  if (a.idx !== b.idx || a.plain !== b.plain || a.spans.length !== b.spans.length
    || a.links.length !== b.links.length) return false;
  for (let i = 0; i < a.spans.length; i++) if (!sameSpan(a.spans[i], b.spans[i])) return false;
  return a.links.length === 0 || JSON.stringify(a.links) === JSON.stringify(b.links);
}

// The host sends `null` for rows unchanged since the last received frame.
function mergeScreen(prev: ScreenView | undefined, next: ScreenView): ScreenView {
  const base = prev ?? emptyScreen;
  let changed = base.rows.length !== next.rows.length;
  const rows = (next.rows as (TerminalRow | null)[]).map((row, i) => {
    const old = base.rows[i];
    if (row === null) return old ?? { idx: i, plain: "", spans: [], links: [] };
    if (old && sameRow(old, row)) return old;
    changed = true;
    return row;
  });
  if (base === emptyScreen) return { ...next, rows };
  const sameMeta = base.cols === next.cols && base.rowsCount === next.rowsCount && base.cursorRow === next.cursorRow
    && base.cursorCol === next.cursorCol && base.cursorVisible === next.cursorVisible
    && base.displayOffset === next.displayOffset
    && JSON.stringify(base.images ?? []) === JSON.stringify(next.images ?? []);
  if (!changed && sameMeta) return base;
  return { ...next, rows: changed ? rows : base.rows };
}

function mergeScreens(prev: Record<string, ScreenView>, next: Record<string, ScreenView>): Record<string, ScreenView> {
  const out: Record<string, ScreenView> = {};
  let changed = Object.keys(prev).length !== Object.keys(next).length;
  for (const key of Object.keys(next)) {
    out[key] = mergeScreen(prev[key], next[key]);
    if (out[key] !== prev[key]) changed = true;
  }
  return changed ? out : prev;
}
const workspaceReady = signal(false);
const workspacePath = signal("");

const screensMapState = signal<Record<string, ScreenView>>({});

interface ProjectTree {
  directories: Map<string, Signal<FileEntryView[] | undefined>>;
  pending: Set<string>;
  expanded: Signal<Set<string>>;
  selected: Signal<string>;
}

interface ChangeIndex {
  byPath: Map<string, FileChangeView>;
  changedDirs: Set<string>;
}

const emptyChanges: ChangeSetView = { files: [], additions: 0, deletions: 0 };
const changesByProject = signal<Record<string, ChangeSetView>>({});
const changeIndexes = new WeakMap<ChangeSetView, ChangeIndex>();
const changesLoading = signal<Record<string, boolean>>({});

interface GitRefView { kind: "head" | "branch" | "remote" | "tag"; name: string }
interface GitGraphRowView {
  hash: string;
  short: string;
  subject: string;
  author: string;
  time: number;
  refs: GitRefView[];
  node: number;
  color: number;
  merge: boolean;
  /** One white lane-line mask per lane colour, tinted from `palette().graphLanes`. */
  layers: { color: number; key: string }[];
}
interface GitGraphView { cwd: string; width: number; rows: GitGraphRowView[]; limit?: number; error?: string }
const gitGraphs = signal<Record<string, GitGraphView>>({});
const gitGraphLoading = signal<Record<string, boolean>>({});

function requestGitGraph(cwd = workspacePath.value): void {
  if (gitGraphLoading.value[cwd]) return;
  gitGraphLoading.value = { ...gitGraphLoading.value, [cwd]: true };
  const sent = requestHost({ type: "gitGraph", cwd, lane: gitGraphLayout.lane,
    row: gitGraphLayout.rowHeight, stroke: gitGraphLayout.stroke });
  if (!sent) gitGraphLoading.value = { ...gitGraphLoading.value, [cwd]: false };
}

function receiveGitGraph(view: GitGraphView): void {
  gitGraphs.value = { ...gitGraphs.value, [view.cwd]: view };
  gitGraphLoading.value = { ...gitGraphLoading.value, [view.cwd]: false };
}
const treesByProject = new Map<string, ProjectTree>();

function projectChanges(cwd = workspacePath.value): ChangeSetView {
  return changesByProject.value[cwd] ?? emptyChanges;
}

function changeIndex(changes: ChangeSetView): ChangeIndex {
  const existing = changeIndexes.get(changes);
  if (existing) return existing;
  const byPath = new Map<string, FileChangeView>();
  const changedDirs = new Set<string>();
  for (const file of changes.files) {
    byPath.set(file.path, file);
    for (let slash = file.path.lastIndexOf("/"); slash > 0; slash = file.path.lastIndexOf("/", slash - 1)) {
      const dir = file.path.slice(0, slash);
      if (changedDirs.has(dir)) break;
      changedDirs.add(dir);
    }
  }
  const index = { byPath, changedDirs };
  changeIndexes.set(changes, index);
  return index;
}

// Keep unchanged row identities: JSNIR's For updates a row whenever its item reference changes.
function receiveChanges(cwd: string, changes: ChangeSetView): void {
  const previous = changesByProject.value[cwd];
  const byPath = previous ? changeIndex(previous).byPath : undefined;
  const files = changes.files.map((file) => {
    const old = byPath?.get(file.path);
    return old && old.status === file.status && old.additions === file.additions && old.deletions === file.deletions ? old : file;
  });
  if (!previous || previous.additions !== changes.additions || previous.deletions !== changes.deletions ||
      previous.files.length !== files.length || files.some((file, i) => file !== previous.files[i])) {
    changesByProject.value = { ...changesByProject.value, [cwd]: { ...changes, files } };
  }
  if (changesLoading.value[cwd]) changesLoading.value = { ...changesLoading.value, [cwd]: false };
}

function projectTree(cwd: string): ProjectTree {
  let tree = treesByProject.get(cwd);
  if (!tree) {
    tree = { directories: new Map(), pending: new Set(), expanded: new LocalSignal(new Set()), selected: new LocalSignal("") };
    treesByProject.set(cwd, tree);
  }
  return tree;
}

function treeDirectory(tree: ProjectTree, path: string): Signal<FileEntryView[] | undefined> {
  let directory = tree.directories.get(path);
  if (!directory) {
    directory = new LocalSignal<FileEntryView[] | undefined>(undefined);
    tree.directories.set(path, directory);
  }
  return directory;
}

function receiveFiles(files: FilesView): void {
  const tree = projectTree(files.cwd);
  tree.pending.delete(files.subdir);
  const directory = treeDirectory(tree, files.subdir);
  const previous = directory.value;
  const byPath = new Map(previous?.map((entry) => [entry.path, entry]));
  const next = files.entries.map((entry) => {
    const old = byPath.get(entry.path);
    return old && old.name === entry.name && old.isDir === entry.isDir ? old : entry;
  });
  if (!previous || previous.length !== next.length || next.some((entry, i) => entry !== previous[i])) {
    directory.value = next;
  }
}
// What each non-terminal tiling id shows, and the files / diffs they display (by `docKey`).
const tileSpecs = signal<Record<number, TileSpec>>({});
const docCache = signal<Record<string, ViewerView>>({});
const recentDocuments = 4;
const settingsTab = signal<SettingsTab>("appearance");

const cliUsageState = signal<CliUsageItem[]>([]);
const externalEditorsState = signal<ExternalEditorItem[]>([]);

const configState = signal<ConfigView>({
  theme: "accordee-dark",
  mode: "dark",
  accent: "blue",
  density: "comfortable",
  fontSize: 13,
  windowTransparency: 0,
  tilingMode: "niri",
  layout: { defaultColumnWidth: 0.5, centerFocusedColumn: "never", alwaysCenterSingleColumn: false },
  checkUpdates: true,
  autoRestartUpdate: true,
  agentNotifications: { finished: false, needsInput: false, backgroundOnly: true },
  clis: [],
  projects: [],
});
const windowTransparencySupported = signal(false);
const notificationTest = signal({ pending: false, message: "" });

const githubState = signal<GitHubView>({
  connected: false,
  method: "none",
  username: null,
  sshKeys: [],
  sshGithubReady: false,
  repoSlug: null,
  branch: null,
  dirtyCount: 0,
  remoteUrl: null,
  detail: "Detecting SSH...",
});

const updateState = signal<UpdateView>({
  currentVersion: "0.1.1",
  availableVersion: "0.1.1",
  hasUpdate: false,
  stage: "idle",
  progress: 0,
  statusText: "Checking for updates...",
  assetUrl: null,
  stagedPath: null,
  notes: {
    version: "0.1.1",
    title: "icy",
    publishedAt: "Latest",
    summary: "",
    highlights: [],
    media: [],
    releaseUrl: "https://github.com/Niiihuel/icy/releases",
  },
});

const noticeState = signal<NoticeView | null>(null);
const sidebarCompact = signal(false);
const avatarReady = signal(false);
const activeNav = signal<NavSection>("agents");
const viewportSize = signal<[number, number]>([1280, 800]);
const releaseNotesOpen = signal(false);
const restartConfirmOpen = signal(false);
const installPopoverOpen = signal(false);

function afterView(ms: number, run: () => void): void {
  const owner = currentScope;
  // JSNIR can reuse a disposed view's callback ID while its timer is still queued.
  // Keep the timer's ID in a detached scope until it fires.
  inScope(childScope(false), () => after(ms, () => {
    if (!owner.disposed) inScope(owner, run);
  }));
}

function requestHost(payload: Record<string, unknown>): boolean {
  try {
    jsnirRequest(JSON.stringify(payload));
    return true;
  } catch (_) {
    // Ignore transient request failures when closing or reloading.
    return false;
  }
}

function sendHello(): void {
  requestHost({ type: "hello" });
}

// A document can finish after another window was opened, even if its result was already queued.
let documentIntent = 0;

function requestDocument(payload: Record<string, unknown>): void {
  requestHost({ ...payload, intent: ++documentIntent });
}

function openLink(link: DetectedLink, sessionId?: number): void {
  if (link.kind === "url") {
    openBrowser(link.target, sessionsState.value.items.find((session) => session.id === sessionId)?.cwd);
    return;
  }
  requestDocument({
    type: link.kind === "diff" ? "openDiff" : "open",
    kind: link.kind,
    path: link.target,
    line: link.line ?? null,
    col: link.col ?? null,
    oldLine: link.oldLine,
    edit: link.kind === "diff" ? link.edit ?? {} : undefined,
    sessionId,
  });
}

function createSession(): void {
  showProjectDialog("session");
}

function openTiledColumn(): void {
  ++documentIntent;
  activeNav.value = "agents";
  requestHost({ type: "openColumn" });
}

function stackTiledVertical(): void {
  ++documentIntent;
  activeNav.value = "agents";
  requestHost({ type: "stackVertical" });
}

function toggleTiledMaximize(): void {
  requestHost({ type: "toggleMaximize" });
}

function setTilingMode(mode: "niri" | "hyprland"): void {
  requestHost({ type: "setTilingMode", mode });
}

function tilingAction(action: string): void {
  activeNav.value = "agents";
  requestHost({ type: "tilingAction", action });
}

function launchCli(cliId: string): void {
  showProjectDialog("session", undefined, cliId);
}

function closeSession(id: number): void {
  ++documentIntent;
  requestHost({ type: "closeSession", id });
}

function selectSession(id: number): void {
  ++documentIntent;
  requestHost({ type: "selectSession", id });
}

function updateConfig(patch: Record<string, unknown>): void {
  requestHost({ type: "updateConfig", ...patch });
}

function updateCliConfig(id: string, patch: Record<string, unknown>): void {
  requestHost({ type: "updateCli", id, ...patch });
}

function reloadThemes(): void {
  requestHost({ type: "reloadThemes" });
}

function openThemesDir(): void {
  requestHost({ type: "openThemesDir" });
}

function openExternal(url: string): void {
  requestHost({ type: "openExternal", url });
}

function orderedEditors(): ExternalEditorItem[] {
  const list = externalEditorsState.value;
  const preferred = configState.value.preferredEditor;
  if (!preferred) return list;
  const match = list.find((editor) => editor.id === preferred);
  if (!match) return list;
  return [match, ...list.filter((editor) => editor.id !== preferred)];
}

function openInEditor(
  editorId: string,
  path: string,
  cwd?: string,
  line?: number,
  col?: number,
): void {
  requestHost({
    type: "openInEditor",
    editorId,
    path,
    cwd,
    line: line ?? null,
    col: col ?? null,
  });
}

function refreshGitHub(): void {
  requestHost({ type: "refreshGitHub" });
}

function listFiles(subdir = "", cwd = workspacePath.value): void {
  const { pending } = projectTree(cwd);
  if (pending.has(subdir)) return;
  pending.add(subdir);
  if (!requestHost({ type: "listFiles", subdir, cwd })) pending.delete(subdir);
}

function checkForUpdates(): void {
  requestHost({ type: "checkUpdate" });
}

function startUpdateDownload(): void {
  requestHost({ type: "downloadUpdate" });
}

function confirmRestartUpdate(): void {
  requestHost({ type: "confirmRestartUpdate" });
}

function syncLayout(): void {
  requestHost({
    type: "layout",
    sidebarWidth: sidebarCompact.value ? spacing.sidebarRail : spacing.sidebarWidth,
    panelWidth: 0,
  });
}

// ---- Compositor windows (files tree, review, overview, file, diff) ----------------

function docKey(spec: TileSpec): string {
  const key = [spec.kind, spec.cwd ?? workspacePath.value, spec.path];
  if (!spec.edit) return JSON.stringify(key);
  const { counts, lines } = spec.edit;
  return JSON.stringify([
    ...key, spec.line, !!spec.oldLine, counts?.additions, counts?.deletions,
    lines?.map((row) => [row.kind, row.oldNum, row.newNum, row.text]),
  ]);
}

// Tab order = tiling reading order (columns left to right, panes top to bottom).
function paneOrder(): number[] {
  return tilingState.value.columns.flatMap((c) => c.panes);
}

function workspaceList(): WorkspaceView[] {
  const list = tilingState.value.workspaces;
  const view = tilingState.value;
  return list && list.length > 0 ? list
    : [{ panes: paneOrder(), focused: view.focusedSession, columns: view.columns, viewOffset: view.viewOffset }];
}

// Every window on every workspace (the tab row only shows the active workspace's).
function allWindowIds(): number[] {
  return workspaceList().flatMap((w) => w.panes);
}

// A window's short name: an agent's title, a shell's folder, a tile's title.
function windowLabel(id: number): string {
  const session = sessionsState.value.items.find((s) => s.id === id);
  if (session) return session.cli === "terminal" ? basename(session.cwd) || "Terminal" : cleanTitle(session.title);
  const spec = tileOf(id);
  return spec ? tileTitle(spec, id) : "";
}

// Workspaces have no names of their own: one reads as the folder of its focused (or first) terminal.
function workspaceLabel(index: number): string {
  const workspace = workspaceList()[index];
  if (!workspace?.panes.length) return "Empty workspace";
  const sessions = sessionsState.value.items;
  const session = [workspace.focused, ...workspace.panes].map((id) => sessions.find((s) => s.id === id))
    .find((s) => !!s);
  if (session) return basename(session.cwd) || cleanTitle(session.title);
  return windowLabel(workspace.focused || workspace.panes[0]) || `Workspace ${index + 1}`;
}

function focusWorkspace(index: number): void {
  ++documentIntent;
  activeNav.value = "agents";
  requestHost({ type: "focusWorkspace", index });
}

function isTerminalId(id: number): boolean {
  const sessions = sessionsState.value;
  return sessions.items.some((s) => s.id === id) || (sessions.archived?.some((s) => s.id === id) ?? false);
}

// A non-terminal window falls back to Files if its UI spec is missing.
function tileOf(id: number): TileSpec | undefined {
  if (id === 0 || isTerminalId(id)) return undefined;
  return tileSpecs.value[id] ?? { kind: "files", path: "" };
}

function focusedTile(): TileSpec | undefined {
  return activeNav.value === "agents" ? tileOf(tilingState.value.focusedSession) : undefined;
}

function findTile(match: (spec: TileSpec) => boolean): number | undefined {
  return paneOrder().find((id) => {
    const spec = tileSpecs.value[id];
    return !!spec && match(spec);
  });
}

// Metrics and Archived cover every project, so one window serves them all; the others belong to one.
function tileMatches(spec: TileSpec, kind: TileKind, cwd: string): boolean {
  return spec.kind === kind && (kind === "metrics" || kind === "archived" || spec.cwd === cwd);
}

// Host state of a browser pane, keyed by its tile id (the page frame is the image `browser:<id>`).
interface BrowserStateView {
  id: number;
  url: string;
  title: string;
  loading: boolean;
  canBack: boolean;
  canForward: boolean;
  error?: string | null;
}

const browserStates = signal<Record<number, BrowserStateView>>({});

function receiveBrowserState(state: BrowserStateView): void {
  if (state.id >= FIRST_URL_PREVIEW_ID && state.id < nextUrlPreviewId) {
    const preview = urlPreview.value;
    if (preview?.id !== state.id) return;
    if (preview.loading && (!state.loading || state.error)) urlPreview.value = { ...preview, loading: false };
  }
  browserStates.value = { ...browserStates.value, [state.id]: state };
}

function tileTitle(spec: TileSpec, id?: number): string {
  if (spec.kind === "file") return basename(spec.path);
  if (spec.kind === "diff") return `${basename(spec.path)} (${spec.edit ? "single edit" : "diff"})`;
  if (spec.kind === "files") return basename(spec.cwd ?? "") || "Files";
  if (spec.kind === "metrics") return "Metrics";
  if (spec.kind === "archived") return "Archived";
  if (spec.kind === "browser") {
    const state = browserStates.value[id ?? 0];
    const url = state?.url || spec.url;
    return state?.title || (url && url !== "about:blank" ? url : "Browser");
  }
  return spec.kind === "review" ? "Git" : "Overview";
}

function tileGlyph(spec: TileSpec): string {
  return {
    files: "folder",
    review: "git-branch",
    overview: "article",
    metrics: "dashboard",
    archived: "archive",
    file: "file-code",
    diff: "file-diff",
    browser: "globe",
  }[spec.kind];
}

function loadTile(spec: TileSpec): void {
  if (spec.kind === "file" || spec.kind === "diff") {
    requestHost({
      type: spec.kind === "diff" ? "openDiff" : "open",
      path: spec.path,
      line: spec.line,
      col: spec.col,
      oldLine: spec.oldLine,
      edit: spec.edit,
      cwd: spec.cwd,
      background: true,
    });
  } else if (spec.kind === "files") {
    const cwd = spec.cwd ?? workspacePath.value;
    if (treeDirectory(projectTree(cwd), "").value === undefined) listFiles("", cwd);
    requestChanges(cwd);
  } else if (spec.kind === "review" || spec.kind === "overview") requestChanges(spec.cwd);
}

function setTile(id: number, spec: TileSpec, load = true): void {
  const next = { ...spec, cwd: spec.cwd ?? tileSpecs.value[id]?.cwd ?? workspacePath.value };
  tileSpecs.value = { ...tileSpecs.value, [id]: next };
  if (load) loadTile(next);
}

// Opens a window beside the focused one (`stack`: below it) and focuses it.
function openTile(spec: TileSpec, stack = false, load = true): void {
  ++documentIntent;
  if (overviewOpen2.value) closeOverviewMode();
  activeNav.value = "agents";
  let id = 0;
  try {
    id = Number(jsnirRequest(JSON.stringify({ type: "openTile", stack, cwd: spec.cwd ?? workspacePath.value })));
  } catch (_) {
    return;
  }
  if (id > 0) setTile(id, spec, load);
}

function focusTile(id: number): void {
  activeNav.value = "agents";
  if (overviewOpen2.value) pickFromOverview(id);
  // Consecutive clicks can arrive before the host's next tiling snapshot.
  else selectSession(id);
}

type SingleTileKind = "files" | "review" | "overview" | "metrics" | "archived";

// Files / Review / Overview / Metrics are single windows: focus the open one, or open it.
function showTile(kind: SingleTileKind, cwd = workspacePath.value): void {
  const id = findTile((s) => tileMatches(s, kind, cwd));
  if (id === undefined) openTile({ kind, path: "", cwd });
  else {
    focusTile(id);
    loadTile({ kind, path: "", cwd });
  }
}

// Header / rail / shortcut toggles: close the window when it is the focused one.
function toggleTile(kind: SingleTileKind): void {
  const id = findTile((s) => tileMatches(s, kind, workspacePath.value));
  if (id !== undefined && activeNav.value === "agents" && tilingState.value.focusedSession === id) closeSession(id);
  else showTile(kind);
}

// Reuse unpinned previews within a project. Pinned panes stay open; terminal edits and Review
// keep independent previews.
function openDocTile(spec: TileSpec): void {
  const key = docKey(spec);
  const view = tilingState.value;
  const activeColumn = view.columns.findIndex((column) => column.panes.includes(view.focusedSession));
  const next = view.columns[activeColumn + 1];
  // In niri, a reused preview should be where a new column would open (or be the current preview).
  const reusable = view.mode === "niri"
    ? [view.focusedSession, ...(!view.maximized && next?.panes.length === 1 ? next.panes : [])]
    : paneOrder();
  const target =
    findTile((s) => (s.kind === "file" || s.kind === "diff") && docKey(s) === key) ??
    reusable.find((id) => {
      const s = tileSpecs.value[id];
      return s && (s.kind === "file" || s.kind === "diff") && !s.pinned && !editorDrafts.has(docKey(s))
        && s.cwd === spec.cwd && !!s.edit === !!spec.edit;
    });
  if (target === undefined) {
    openTile(spec, false, false);
    return;
  }
  const existing = tileSpecs.value[target];
  setTile(target, { ...spec, pinned: existing?.pinned }, false);
  focusTile(target);
}

// Host-side address parsing: "" when `text` is not an address (`search`: free text becomes a web search).
function resolveAddress(text: string, search: boolean): string {
  try {
    return JSON.parse(jsnirRequest(JSON.stringify({ type: "browserAddress", text, search }))).url ?? "";
  } catch (_) {
    return "";
  }
}

// Each launch gets an independent pane, including a blank page from the sidebar.
function openBrowser(url?: string, cwd = workspacePath.value): void {
  openTile({ kind: "browser", path: "", url, cwd }, false, false);
}

// URL previews are separate from user panes. A new address gets a new id so late frames
// from the previous page cannot appear in its card.
const FIRST_URL_PREVIEW_ID = 2 ** 31;
let nextUrlPreviewId = FIRST_URL_PREVIEW_ID;
const urlPreview = signal<{ id: number; url: string; loading: boolean } | null>(null);
const urlPreviewScope = currentScope;
let urlPreviewUse = 0;
let urlPreviewVisible = false;

// Only loopback addresses are loaded into a live preview on hover.
function isLocalUrl(target: string): boolean {
  return /^https?:\/\/(?:(?:[a-z\d-]+\.)*localhost|127(?:\.\d{1,3}){3}|0\.0\.0\.0|\[::1\])(?::\d+)?(?:[/?#]|$)/i.test(target);
}

function showUrlPreview(url: string): number {
  const layout = urlPreviewLayout;
  const use = ++urlPreviewUse;
  let preview = urlPreview.value;
  if (preview?.url !== url) {
    if (preview) closeBrowser(preview.id);
    const id = nextUrlPreviewId++;
    preview = { id, url, loading: true };
    urlPreview.value = preview;
    inScope(urlPreviewScope, () => afterView(layout.loadTimeout, () => {
      const current = urlPreview.value;
      if (current?.id === id && current.loading) urlPreview.value = { ...current, loading: false };
    }));
  }
  urlPreviewVisible = true;
  requestHost({ type: "browser", op: "open", id: preview.id, url, width: layout.pageWidth, height: layout.pageHeight });
  return use;
}

function hideUrlPreview(use: number): void {
  const preview = urlPreview.value;
  if (use !== urlPreviewUse || !urlPreviewVisible || !preview) return;
  urlPreviewVisible = false;
  requestHost({ type: "browser", op: "hidden", id: preview.id, hidden: true });
  // A live page keeps CEF's pump running. Release it even if the originating pane unmounts.
  inScope(urlPreviewScope, () => afterView(urlPreviewLayout.idleClose, () => {
    if (use !== urlPreviewUse) return;
    urlPreview.value = null;
    closeBrowser(preview.id);
  }));
}

// The pane is gone: release its page (the host closes the CEF view) and forget its state.
function closeBrowser(id: number): void {
  requestHost({ type: "browser", op: "close", id });
  const states = { ...browserStates.value };
  delete states[id];
  browserStates.value = states;
}

function openReviewTab(): void {
  showTile("review");
}

function openMetricsTab(): void {
  showTile("metrics");
}

function openArchivedTab(): void {
  showTile("archived");
}

function toggleFiles(): void {
  toggleTile("files");
}

function toggleOverview(): void {
  toggleTile("overview");
}

// A file or diff pushed by the host: cache it; route it to a window unless a window asked for it.
function receiveDoc(view: ViewerView): void {
  if (!view.background && view.intent !== undefined && view.intent !== documentIntent) return;
  if (!view.open && !view.error) return;
  const path = view.requestedPath ?? (view.displayPath || view.path);
  const spec: TileSpec = {
    kind: view.mode, path, cwd: view.cwd ?? workspacePath.value,
    line: view.targetLine ?? undefined, col: view.targetCol ?? undefined, oldLine: view.oldLine, edit: view.edit,
  };
  const key = docKey(spec);
  const cache = { ...docCache.value };
  delete cache[key];
  cache[key] = view;
  const open = new Set(Object.values(tileSpecs.value)
    .filter((spec) => spec.kind === "file" || spec.kind === "diff")
    .map(docKey));
  const recent = Object.keys(cache).filter((entry) => !open.has(entry));
  for (const expired of recent.slice(0, Math.max(0, recent.length - recentDocuments))) delete cache[expired];
  docCache.value = cache;
  if (!view.background) openDocTile(spec);
}

// Windows seen in a tiling push and then gone from it were closed: drop their specs.
const seenTiles = new Set<number>();

function pruneTiles(): void {
  const live = new Set(allWindowIds());
  const specs = tileSpecs.value;
  let changed = false;
  const next: Record<number, TileSpec> = {};
  for (const key of Object.keys(specs)) {
    const id = Number(key);
    if (live.has(id)) seenTiles.add(id);
    if (isTerminalId(id) || (!live.has(id) && seenTiles.has(id))) {
      seenTiles.delete(id);
      changed = true;
      if (specs[id].kind === "browser") closeBrowser(id);
    } else next[id] = specs[id];
  }
  if (changed) tileSpecs.value = next;
}

// Launcher search ("Search or enter a URL"): host-side fuzzy file search over the workspace.
const fileSearch = signal<{ cwd: string; query: string; items: string[] }>({ cwd: "", query: "", items: [] });
let fileSearchToken = 0;

function searchWorkspaceFiles(query: string, cwd: string): void {
  const mine = ++fileSearchToken;
  const q = query.trim();
  if (!q) {
    fileSearch.value = { cwd, query: "", items: [] };
    return;
  }
  afterView(120, () => {
    if (mine === fileSearchToken) requestHost({ type: "searchFiles", query: q, cwd });
  });
}

function newSessionIn(cwd: string): void {
  showProjectDialog("session", cwd);
}

function addProject(path: string): void {
  showProjectDialog("add", path);
}

function removeProject(path: string): void {
  requestHost({ type: "removeProject", path });
}

function revealPath(path: string, cwd?: string): void {
  requestHost({ type: "revealPath", path, cwd });
}

function copyText(text: string): void {
  requestHost({ type: "copyText", text });
}

function requestChanges(cwd = workspacePath.value): void {
  if (changesLoading.value[cwd]) return;
  changesLoading.value = { ...changesLoading.value, [cwd]: true };
  if (!requestHost({ type: "gitChanges", cwd })) changesLoading.value = { ...changesLoading.value, [cwd]: false };
}

function setWorkspacePath(cwd: string): void {
  if (cwd === workspacePath.value) return;
  workspacePath.value = cwd;
  requestChanges();
}

function openFile(path: string, line?: number, cwd = workspacePath.value): void {
  requestDocument({ type: "open", kind: "file", path, line: line ?? null, col: null, cwd });
}

function openDiff(path: string, cwd = workspacePath.value): void {
  requestDocument({ type: "openDiff", path, cwd });
}

function toggleTreeDir(path: string, cwd: string): void {
  const tree = projectTree(cwd);
  const expanded = new Set(tree.expanded.value);
  if (!expanded.delete(path)) {
    expanded.add(path);
    if (treeDirectory(tree, path).value === undefined) listFiles(path, cwd);
  }
  tree.expanded.value = expanded;
}

function detectedFilesList(): DetectedLink[] {
  const seen = new Set<string>();
  const out: DetectedLink[] = [];
  for (const row of screenState.value.rows) {
    for (const link of row.links) {
      if (link.kind !== "url" && !seen.has(link.target)) {
        seen.add(link.target);
        out.push(link);
      }
    }
  }
  return out;
}

function detectedFilesCount(): number {
  return detectedFilesList().length;
}

function Show(props: {
  when: Source<boolean>;
  style?: Source<Style>;
  children: () => Element;
}): Element {
  return (
    <Dynamic value={() => read(props.when)} style={props.style}>
      {(visible) => (visible ? props.children() : <Box />)}
    </Dynamic>
  );
}

function Glyph(props: {
  name: Source<string>;
  size: number;
  color: Source<number>;
  hover?: Source<number>;
}): Element {
  return (
    <Image
      source={() => "glyph:" + read(props.name)}
      style={{ width: props.size, height: props.size, shrink: 0 }}
      paint={() => ({
        color: read(props.color),
        hover: props.hover ? { color: read(props.hover) } : undefined,
        transitions: [smoothTransition("color", motion.fast)],
      })}
    />
  );
}

function StatusDot(props: { alive: Source<boolean> }): Element {
  return (
    <Box
      style={{ width: 6, height: 6, shrink: 0 }}
      paint={() => ({
        radius: 3,
        background: read(props.alive) ? palette().statusOnline : palette().statusIdle,
        transitions: [smoothTransition("background", motion.fast)],
      })}
    />
  );
}

function KbdKey(props: { label: Source<string> }): Element {
  return (
    <Box
      style={{
        minWidth: 16,
        height: 16,
        padding: [0, 4, 0, 4],
        align: "center",
        justify: "center",
        shrink: 0,
      }}
      paint={() => ({
        radius: radii.sm,
        background: palette().kbdBg,
      })}
    >
      <Text
        font={{ family: fontFamilies.mono, size: typeScale.micro, weight: fontWeights.medium }}
        paint={() => ({ color: palette().textMuted })}
      >
        {props.label}
      </Text>
    </Box>
  );
}

// Accordee icon-only ghost button: transparent at rest, `sidebarActive` on hover, scale-95 press,
// 40% when disabled. The one icon button of the app; `sm` fits 30px card headers.
function IconButton(props: {
  icon: Source<string>;
  label: Source<string>;
  detail?: Source<string>;
  keys?: Source<string[]>;
  size?: "sm" | "md";
  side?: Side;
  layer?: number;
  tooltipDisabled?: Source<boolean>;
  disabled?: Source<boolean>;
  highlighted?: Source<boolean>;
  hoverBackground?: Source<number>;
  onPress: () => void;
}): Element {
  const highlighted = () => read(props.highlighted ?? false);
  const layout = iconButtonLayout[props.size ?? "md"];
  return (
    <HintTooltip label={props.label} detail={props.detail} keys={props.keys} side={props.side ?? "top"}
      layer={props.layer} disabled={props.tooltipDisabled} style={{ shrink: 0 }}>
      <Box
        style={{ width: layout.size, height: layout.size, align: "center", justify: "center", shrink: 0 }}
        disabled={props.disabled}
        onPress={props.onPress}
        paint={() => ({
          radius: radii.md,
          background: highlighted() ? accent().brandSoft : 0,
          opacity: read(props.disabled ?? false) ? controlState.disabledOpacity : 1,
          hover: { background: highlighted() ? accent().brandSoft : read(props.hoverBackground ?? palette().sidebarActive) },
          active: { scale: controlState.iconPressScale },
          transitions: [smoothTransition("background", motion.fast), smoothTransition("scale", motion.fast)],
        })}
      >
        <Glyph name={props.icon} size={layout.icon}
          color={() => highlighted() ? accent().brandText : palette().textSecondary}
          hover={() => highlighted() ? accent().brandText : palette().textPrimary} />
      </Box>
    </HintTooltip>
  );
}

// Accordee segmented control: 28px hairline track, 2px inset, `sidebarActive` segment. With a fixed
// `width` the segments split it evenly and one indicator springs between them; without it each
// segment hugs its label.
function SegmentedControl<T extends string | number>(props: {
  width?: number;
  value: Signal<T>;
  choices: readonly { id: T; label: string; detail?: () => string }[];
  onSelect?: (id: T) => void;
}): Element {
  const layout = quietControlLayout;
  const fixed = props.width !== undefined;
  const width = fixed ? ((props.width as number) - layout.inset * 2) / props.choices.length : undefined;
  const height = layout.height - layout.inset * 2;
  return (
    <Row style={{ width: props.width, height: layout.height, padding: [layout.inset, layout.inset, layout.inset, layout.inset],
      gap: fixed ? 0 : layout.inset, align: "center", shrink: 0 }}
      paint={() => ({ radius: radii.md, background: palette().appBg, edge: palette().border, edgeWidth: spacing.hairline })}>
      {fixed ? (
        <Box style={() => ({
          position: "absolute", top: layout.inset, height, width,
          left: layout.inset + (width as number) * props.choices.findIndex((choice) => choice.id === props.value.value),
        })} paint={() => ({
          radius: radii.sm, background: palette().sidebarActive, transitions: [springTransition("left")],
        })} />
      ) : null}
      {props.choices.map((choice) => {
        const selected = () => props.value.value === choice.id;
        return (
          <Row style={{ width, height, padding: fixed ? 0 : [0, spacing.unit * 2.5, 0, spacing.unit * 2.5],
            align: "center", justify: "center", gap: layout.gap, shrink: 0 }}
            onPress={() => {
              if (selected()) return;
              props.value.value = choice.id;
              props.onSelect?.(choice.id);
            }}
            paint={() => ({
              radius: radii.sm,
              background: !fixed && selected() ? palette().sidebarActive : 0,
              hover: { background: selected() ? (fixed ? 0 : palette().sidebarActive) : palette().panelBg },
              active: { scale: controlState.pressScale },
              transitions: [smoothTransition("background", motion.fast), smoothTransition("scale", motion.fast)],
            })}>
            <Text font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
              paint={() => ({ color: selected() ? palette().textPrimary : palette().textSecondary })}>
              {choice.label}
            </Text>
            {choice.detail && <Text font={{ family: fontFamilies.sans, size: typeScale.tiny }}
              paint={() => ({ color: palette().textMuted })}>{choice.detail}</Text>}
          </Row>
        );
      })}
    </Row>
  );
}

function CountPill(props: { count: Source<string> }): Element {
  return (
    <Box
      style={{
        height: 16,
        minWidth: 16,
        padding: [0, 5, 0, 5],
        align: "center",
        justify: "center",
        shrink: 0,
      }}
      paint={() => ({
        radius: 8,
        background: palette().kbdBg,
      })}
    >
      <Text
        font={{ family: fontFamilies.sans, size: typeScale.micro, weight: fontWeights.medium }}
        paint={() => ({ color: palette().textSecondary })}
      >
        {props.count}
      </Text>
    </Box>
  );
}

function PrimaryButton(props: {
  label: Source<string>;
  onPress: () => void;
  height?: number;
  grow?: number;
  disabled?: Source<boolean>;
}): Element {
  return (
    <Box
      style={{
        height: props.height ?? spacing.button,
        grow: props.grow,
        padding: [0, 12, 0, 12],
        align: "center",
        justify: "center",
        shrink: 0,
      }}
      disabled={props.disabled}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: accent().brand,
        opacity: props.disabled && read(props.disabled) ? controlState.disabledOpacity : 1,
        hover: { opacity: controlState.hoverOpacity },
        active: { scale: controlState.pressScale },
        transitions: [
          smoothTransition("background", motion.fast),
          smoothTransition("opacity", motion.fast),
          smoothTransition("scale", motion.fast),
        ],
      })}
    >
      <Text
        font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
        paint={() => ({ color: accent().brandFg })}
      >
        {props.label}
      </Text>
    </Box>
  );
}

function SecondaryButton(props: {
  label: Source<string>;
  onPress: () => void;
  height?: number;
  grow?: number;
  disabled?: Source<boolean>;
  accessibleLabel?: string;
}): Element {
  return (
    <Box
      style={{
        height: props.height ?? spacing.control,
        grow: props.grow,
        padding: [0, 10, 0, 10],
        align: "center",
        justify: "center",
        shrink: 0,
      }}
      disabled={props.disabled}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: palette().panelBg,
        a11y: props.accessibleLabel ? { label: props.accessibleLabel } : undefined,
        opacity: read(props.disabled ?? false) ? controlState.disabledOpacity : 1,
        edge: palette().border,
        edgeWidth: spacing.hairline,
        hover: { background: palette().sidebarActive, edge: hoverBorder() },
        active: { scale: controlState.pressScale },
        transitions: [
          smoothTransition("background", motion.fast),
          smoothTransition("edge", motion.fast),
          smoothTransition("scale", motion.fast),
        ],
      })}
    >
      <Text
        font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
        paint={() => ({ color: palette().textPrimary })}
      >
        {props.label}
      </Text>
    </Box>
  );
}

// Accordee switch toggle (32x18 track + smooth sliding thumb).
function ToggleSwitch(props: {
  checked: Source<boolean>;
  onToggle: () => void;
  label?: string;
}): Element {
  return (
    <Box
      style={{
        width: 34,
        height: 18,
        direction: "row",
        align: "center",
        padding: [2, 2, 2, 2],
        shrink: 0,
      }}
      onPress={props.onToggle}
      paint={() => ({
        radius: 9,
        background: read(props.checked) ? accent().brand : palette().kbdBg,
        edge: read(props.checked) ? accent().brand : palette().border,
        edgeWidth: spacing.hairline,
        a11y: props.label ? { role: "switch", label: props.label, checked: read(props.checked) } : undefined,
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      <Box
        style={{ width: 14, height: 14 }}
        paint={() => ({
          radius: 7,
          background: read(props.checked) ? accent().brandFg : palette().textSecondary,
          x: read(props.checked) ? 16 : 0,
          transitions: [
            { property: "x", duration: motion.base, easing: "spring" },
            smoothTransition("background", motion.fast),
          ],
        })}
      />
    </Box>
  );
}


function basename(path: string): string {
  const parts = path.split(/[\\/]/).filter((p) => p.length > 0);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

// Skeleton blocks pulse in step, like the loading placeholders of Accordee's dashboard.
function skeletonPaint(color: number, radius: number = radii.sm): Paint {
  return {
    radius,
    background: color,
    animations: [{
      property: "opacity",
      keyframes: [{ offset: 0, value: 1 }, { offset: 1, value: skeletonMotion.dim }],
      duration: skeletonMotion.pulse,
      easing: "easeInOut",
      alternate: true,
    }],
  };
}

// Accordee dropdown-menu content: radius 10, padding 6, hairline border, shadow-xl,
// fade-in zoom-in-95 slide-in-from-<side>-2 over 200ms --ease-smooth.
function MenuPanel(props: {
  open: Signal<boolean>;
  anchor: () => number | undefined;
  side?: Side;
  align?: AnchorAlign;
  width?: number;
  layer?: number;
  children: Children;
}): Element {
  const side = props.side ?? "bottom";
  const motionSide = side === "top" ? "top" : side === "left" ? "left" : side === "right" ? "right" : "bottom";
  return (
    <Popover
      open={props.open}
      anchor={props.anchor}
      side={side}
      align={props.align ?? "start"}
      layer={props.layer ?? 1050}
      style={{ width: props.width ?? spacing.menuWidth, padding: [6, 6, 6, 6] }}
      paint={() => ({
        radius: radii.popover,
        background: palette().appBg,
        edge: palette().border,
        edgeWidth: spacing.hairline,
        shadows: [shadows.menu, shadows.menuTight],
        ...accordeeMenuMotion(motionSide),
      })}
    >
      <Column style={{ gap: 1 }}>{props.children}</Column>
    </Popover>
  );
}

// `DropdownMenuLabel`: small uppercase caption ("SWITCH WORKSPACE").
function MenuLabel(props: { label: Source<string> }): Element {
  return (
    <Box style={{ padding: [6, 8, 4, 8] }}>
      <Text
        lines={1}
        font={{
          family: fontFamilies.sans,
          size: typeScale.tiny,
          weight: fontWeights.medium,
          letterSpacing: 0.04,
        }}
        paint={() => ({ color: palette().textMuted })}
      >
        {() => read(props.label).toUpperCase()}
      </Text>
    </Box>
  );
}

function MenuSeparator(): Element {
  return (
    <Box
      style={{ height: 1, margin: [5, -6, 5, -6], shrink: 0 }}
      paint={() => ({ background: palette().border })}
    />
  );
}

// Detected OS editors list shared by the file/diff viewer and the sidebar project menu.
function OpenInEditorItems(props: {
  path: () => string;
  cwd?: () => string | undefined;
  line?: () => number | undefined;
  col?: () => number | undefined;
  onSelect: () => void;
}): Element {
  return (
    <For each={orderedEditors} key={(editor) => editor.id} style={{ gap: 1 }}>
      {(editor) => (
        <MenuItem
          image={() => "editor:" + editor().id}
          label={() => editor().name}
          checked={() => configState.value.preferredEditor === editor().id}
          onPress={() => {
            props.onSelect();
            openInEditor(editor().id, props.path(), props.cwd?.(), props.line?.(), props.col?.());
          }}
        />
      )}
    </For>
  );
}

function EmptyState(props: {
  title: Source<string>;
  detail?: Source<string>;
  compact?: boolean;
  centered?: boolean;
  style?: Style;
  children?: Children;
}): Element {
  const inset = props.children ? emptyStateLayout.actionInset : 0;
  return (
    <Column style={{ minWidth: 0, shrink: 0, gap: emptyStateLayout.actionGap, ...props.style }}>
      <Column style={{ padding: [0, inset, 0, inset], gap: emptyStateLayout.detailGap }}>
        <Text align={props.centered ? "center" : undefined}
          font={{
            family: fontFamilies.sans,
            size: props.compact ? typeScale.caption : typeScale.title,
            weight: props.compact ? fontWeights.normal : fontWeights.medium,
          }}
          paint={() => ({ color: props.compact ? palette().textMuted : palette().textPrimary })}
        >
          {props.title}
        </Text>
        {props.detail === undefined ? null : (
          <Text align={props.centered ? "center" : undefined}
            font={{ family: fontFamilies.sans, size: typeScale.label }}
            paint={() => ({ color: palette().textMuted })}>{props.detail}</Text>
        )}
      </Column>
      {props.children ? <Column>{props.children}</Column> : null}
    </Column>
  );
}

function SessionMenuButton(props: {
  open: Signal<boolean>;
  visible: Source<boolean>;
  side: Side;
  children: Children;
}): Element {
  let anchor: number | undefined;
  const size = iconButtonLayout.sm.size;
  return (
    <Box ref={(id) => { anchor = id; }} style={{ width: size, height: size, shrink: 0 }}>
      <Show when={() => read(props.visible) || props.open.value}>
        {() => <IconButton icon="more-horizontal" label="Session actions" size="sm" side={props.side} tooltipDisabled={props.open}
          hoverBackground={rowHoverBackground}
          highlighted={props.open} onPress={() => { props.open.value = !props.open.value; }} />}
      </Show>
      <MenuPanel open={props.open} anchor={() => anchor} side={props.side} align={props.side === "bottom" ? "end" : "start"}>
        {props.children}
      </MenuPanel>
    </Box>
  );
}

// `DropdownMenuItem`: 32px row, 16px icon + 13px label, panel hover, optional check / shortcut.
function MenuItem(props: {
  label: Source<string>;
  // Second muted line (Accordee "Quick chat with" rows: name + role); makes the row two-line.
  description?: Source<string>;
  icon?: Source<string>;
  image?: Source<string>;
  leading?: Element;
  checked?: Source<boolean>;
  selected?: Source<boolean>;
  keys?: Source<string[]>;
  trailing?: Source<string>;
  backdrop?: Element;
  ref?: (id: number) => void;
  onHover?: (inside: boolean) => void;
  onPress: () => void;
}): Element {
  const selected = () => read(props.selected ?? false);
  const twoLine = props.description !== undefined;
  const iconSize = twoLine ? 22 : 16;
  return (
    <Row
      ref={props.ref}
      onHover={props.onHover}
      style={{
        height: twoLine ? undefined : spacing.menuItem,
        padding: twoLine ? [7, 10, 7, 8] : [0, 8, 0, 8],
        align: "center",
        gap: twoLine ? 12 : 10,
        shrink: 0,
        clip: props.backdrop !== undefined,
      }}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: selected() ? palette().panelBg : 0,
        hover: { background: palette().panelBg },
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      {props.backdrop ?? null}
      {props.leading ??
        (props.image ? (
          <Image source={props.image} style={{ width: iconSize, height: iconSize, shrink: 0 }} />
        ) : props.icon ? (
          <Glyph name={props.icon} size={16} color={() => palette().textSecondary} />
        ) : null)}
      {twoLine ? (
        <Column style={{ grow: 1, minWidth: 0, gap: 1 }}>
          <Text
            lines={1}
            font={{ family: fontFamilies.sans, size: typeScale.body }}
            paint={() => ({ color: palette().textPrimary })}
          >
            {props.label}
          </Text>
          <Text
            lines={1}
            font={{ family: fontFamilies.sans, size: typeScale.label }}
            paint={() => ({ color: palette().textMuted })}
          >
            {props.description ?? ""}
          </Text>
        </Column>
      ) : (
        <Text
          lines={1}
          style={{ grow: 1, minWidth: 0 }}
          font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.normal }}
          paint={() => ({ color: palette().textPrimary })}
        >
          {props.label}
        </Text>
      )}
      {props.trailing !== undefined ? (
        <Text
          lines={1}
          font={{ family: fontFamilies.mono, size: typeScale.tiny }}
          paint={() => ({ color: palette().textMuted })}
        >
          {props.trailing}
        </Text>
      ) : null}
      {props.keys !== undefined ? (
        // Re-rendered when the user rebinds the action (keys come from the live keymap).
        <Dynamic value={() => read(props.keys ?? []).join("\u0001")} style={{ shrink: 0 }}>
          {(joined) => (
            <Row style={{ gap: 3, shrink: 0 }}>
              {(joined ? joined.split("\u0001") : []).map((k) => (
                <KbdKey label={k} />
              ))}
            </Row>
          )}
        </Dynamic>
      ) : null}
      {props.checked !== undefined ? (
        <Box style={{ width: 16, height: 16, align: "center", justify: "center", shrink: 0 }}>
          <Show when={() => read(props.checked ?? false)}>
            {() => <Glyph name="check" size={14} color={() => palette().textPrimary} />}
          </Show>
        </Box>
      ) : null}
    </Row>
  );
}

// Accordee tooltip: inverted foreground bubble with a caret, 12px label, optional muted detail and
// shortcut chips ("Inbox [G] then [I]"); fades + zooms in from the anchor side in 150ms.
function HintTooltip(props: {
  label: Source<string>;
  detail?: Source<string>;
  keys?: Source<string[]>;
  side?: Side;
  delay?: number;
  layer?: number;
  disabled?: Source<boolean>;
  style?: Source<Style>;
  // Rich content under the label (sections, rows); rebuilt on every open so it is current.
  body?: () => Element;
  children: Element;
}): Element {
  const layout = tooltipLayout;
  const open = signal(false);
  const side: Side = props.side ?? "right";
  let wrapper: number | undefined;
  let bubbleId: number | undefined;
  let hovering = false;
  let token = 0;
  const disabled = () => read(props.disabled ?? false);
  if (props.disabled !== undefined) {
    effect(() => {
      if (disabled()) {
        ++token;
        open.value = false;
      }
    });
  }

  const header = (
    <Row
      style={{ height: layout.height, align: "center", gap: layout.headerGap, shrink: 0 }}
    >
      <Text
        lines={1}
        font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
        paint={() => ({ color: palette().tooltipFg })}
      >
        {props.label}
      </Text>
      {props.detail !== undefined ? (
        <Text
          lines={1}
          font={{ family: fontFamilies.sans, size: typeScale.caption }}
          paint={() => ({ color: palette().tooltipMuted })}
        >
          {props.detail}
        </Text>
      ) : null}
      {props.keys !== undefined ? (
        <Dynamic value={() => read(props.keys ?? []).join("\u0001")} style={{ shrink: 0 }}>
          {(joined) => (
        <Row style={{ align: "center", gap: layout.keyGap, margin: [0, -layout.keyGap, 0, 0], shrink: 0 }}>
          {(joined ? joined.split("\u0001") : []).map((k) =>
            k === "then" ? (
              <Text
                font={{ family: fontFamilies.sans, size: typeScale.tiny }}
                paint={() => ({ color: palette().tooltipMuted })}
              >
                then
              </Text>
            ) : (
              <Box
                style={{ minWidth: layout.keySize, height: layout.keySize, padding: [0, spacing.unit, 0, spacing.unit],
                  align: "center", justify: "center" }}
                paint={() => ({ radius: radii.sm, background: palette().tooltipKbdBg })}
              >
                <Text
                  font={{ family: fontFamilies.sans, size: typeScale.tiny, weight: fontWeights.medium }}
                  paint={() => ({ color: palette().tooltipFg })}
                >
                  {k}
                </Text>
              </Box>
            )
          )}
        </Row>
          )}
        </Dynamic>
      ) : null}
    </Row>
  );
  const body = props.body;
  const radius = body ? radii.lg : radii.md;
  const placement = signal<PopoverPlacement | undefined>(undefined);
  const placedSide = () => placement.value?.side ?? side;
  const caretDepth = layout.caretSize / 2;
  const caretInset = caretDepth - layout.caretOverlap;
  const bubble = (
    <Column ref={(id) => { bubbleId = id; }} style={{
      padding: body ? [spacing.unit, spacing.unit * 3, spacing.unit * 3, spacing.unit * 3]
        : [0, layout.padding, 0, layout.padding],
      shrink: 0,
    }} paint={() => ({ radius, background: palette().tooltipBg })}>
      {header}
      {body ? <Dynamic value={() => open.value}>{(visible) => (visible ? body() : <Box />)}</Dynamic> : null}
    </Column>
  );

  return (
    <Box
      ref={(id) => {
        wrapper = id;
      }}
      style={props.style}
      onHover={(inside) => {
        hovering = inside;
        const mine = ++token;
        if (!inside || disabled()) {
          open.value = false;
          return;
        }
        placement.value = undefined;
        afterView(props.delay ?? layout.delay, () => {
          if (hovering && mine === token && !disabled()) open.value = true;
        });
      }}
    >
      {props.children}
      <Popover
        open={open}
        anchor={() => wrapper}
        side={side}
        align="center"
        gap={layout.anchorGap}
        layer={props.layer ?? layout.layer}
        onPlacement={(p) => {
          if (bubbleId === undefined) return;
          const vertical = p.side === "top" || p.side === "bottom";
          const edge = vertical ? jsnir.columns.width[bubbleId] : jsnir.columns.height[bubbleId];
          const clearance = Math.min(edge / 2, radius + caretDepth);
          const offset = edge > 0 ? Math.max(clearance, Math.min(edge - clearance, p.anchorOffset)) : p.anchorOffset;
          placement.value = { side: p.side, anchorOffset: offset };
        }}
        paint={() => ({ ...accordeeTooltipMotion(placedSide()), a11y: { role: "tooltip" } })}
      >
        <Dynamic value={placedSide}>
          {(at) => {
            const vertical = at === "top" || at === "bottom";
            const leading = at === "right" || at === "bottom";
            const glyph = { top: "caret-down", bottom: "caret-up", left: "caret-right", right: "caret-left" }[at];
            // The caret's base overlaps the bubble. Leave both in the same paint and motion group.
            const slot = (
              <Box style={vertical ? { height: caretInset, shrink: 0 } : { width: caretInset, shrink: 0 }}>
                <Show when={() => placement.value !== undefined} style={() => {
                  const along = (placement.value?.anchorOffset ?? caretDepth) - caretDepth;
                  const across = leading ? -caretDepth : -layout.caretOverlap;
                  return { position: "absolute", left: vertical ? along : across, top: vertical ? across : along,
                    width: layout.caretSize, height: layout.caretSize };
                }}>
                  {() => <Glyph name={glyph} size={layout.caretSize} color={() => palette().tooltipBg} />}
                </Show>
              </Box>
            );
            return <Box style={{ direction: vertical ? "column" : "row", align: "stretch" }}>
              {leading ? [slot, bubble] : [bubble, slot]}
            </Box>;
          }}
        </Dynamic>
      </Popover>
    </Box>
  );
}


// Agent CLIs put a spinner glyph in front of their OSC title (Codex braille ⠋⠙…,
// Claude Code ·✢✶✻✽ or ◐◓◑◒; ✳ is Claude idle). Keep the title and status distinct.
const SPINNER_GLYPHS = "·✢✶✻✽*◐◓◑◒";

function titleSpinner(title: string): string | null {
  const first = Array.from(title.trimStart())[0] ?? "";
  const code = first.codePointAt(0) ?? 0;
  if (first && ((code >= 0x2800 && code <= 0x28ff) || SPINNER_GLYPHS.includes(first))) return first;
  return null;
}

function isWorkingTitle(title: string): boolean {
  return titleSpinner(title) !== null;
}

function cleanTitle(title: string): string {
  const chars = Array.from(title.trimStart());
  const first = chars[0] ?? "";
  const code = first.codePointAt(0) ?? 0;
  if ((code >= 0x2800 && code <= 0x28ff) || (SPINNER_GLYPHS + "✳").includes(first)) {
    return chars.slice(1).join("").trimStart();
  }
  return title;
}

function sessionWorking(s: SessionItem | undefined): boolean {
  return !!s && s.alive && (s.status ? s.status === "working" : isWorkingTitle(s.title));
}

// Advance the status glyphs only while a session is working.
const agentSpinnerTick = signal(0);
let agentSpinnerRunning = false;

function agentSpinnerStep(): void {
  if (!sessionsState.value.items.some(sessionWorking)) {
    agentSpinnerRunning = false;
    return;
  }
  agentSpinnerTick.value = agentSpinnerTick.value + 1;
  afterView(agentListLayout.spinnerFrameMs, agentSpinnerStep);
}

effect(() => {
  if (!agentSpinnerRunning && sessionsState.value.items.some(sessionWorking)) {
    agentSpinnerRunning = true;
    afterView(agentListLayout.spinnerFrameMs, agentSpinnerStep);
  }
});

// Keep the intrinsic title in one text node and soften only the clipped edge.
function FadeText(props: {
  text: Source<string>;
  font: Font;
  color?: Source<number>;
  spans?: Source<Span[]>;
  background: Source<number>;
  layoutChanged?: () => void;
}): Element {
  const layout = titleFadeLayout;
  const color = props.color;
  const bounds = signal({ width: 0, height: 0, overflow: false });
  let boxId: number | undefined;
  let textId: number | undefined;
  let pending = false;
  let layoutVersion = 0;
  const measure = () => {
    if (boxId === undefined || textId === undefined) return;
    const width = jsnir.columns.width[boxId] ?? 0;
    const height = jsnir.columns.height[boxId] ?? 0;
    const overflow = width > 0 && (jsnir.columns.width[textId] ?? 0) > width + spacing.hairline;
    const previous = bounds.value;
    if (width !== previous.width || height !== previous.height || overflow !== previous.overflow) {
      bounds.value = { width, height, overflow };
    }
  };
  const measureAfterLayout = () => {
    if (pending) return;
    pending = true;
    afterView(layout.measureDelay, () => {
      pending = false;
      measure();
    });
  };
  effect(() => {
    void read(props.text);
    void viewportSize.value;
    void sidebarCompact.value;
    props.layoutChanged?.();
    measureAfterLayout();
    const version = ++layoutVersion;
    afterView(SPRING_MS + layout.measureDelay, () => {
      if (version === layoutVersion) measure();
    });
  });
  return (
    <Row ref={(id) => { boxId = id; measureAfterLayout(); }}
      style={{ grow: 1, shrink: 1, minWidth: 0, align: "center", clip: true }}>
      <Box ref={(id) => { textId = id; }} style={{ shrink: 0 }}>
        <Text font={props.font} spans={props.spans}
          paint={color === undefined ? undefined : () => ({ color: read(color) })}>{props.text}</Text>
      </Box>
      <Show when={() => bounds.value.overflow} style={() => {
        const { width, height } = bounds.value;
        const fadeWidth = Math.min(width, layout.width);
        return { position: "absolute", left: width - fadeWidth, top: 0, width: fadeWidth, height };
      }}>
        {() => <Image source="ui:text-fade" style={{ grow: 1 }} paint={() => ({
          color: read(props.background), a11y: { hidden: true },
          transitions: [smoothTransition("color", motion.fast)],
        })} />}
      </Show>
    </Row>
  );
}

// ---- Image paths printed by any CLI: loaded on demand by the host for hover previews and the
// image viewer. `loading` until the host answers, `error` when it could not be read/decoded.
interface FileImageEntry {
  key: string;
  width: number;
  height: number;
}

const fileImages = signal<Record<string, FileImageEntry | "loading" | "error">>({});

function requestImagePreview(path: string, sessionId: number): void {
  if (fileImages.value[path] !== undefined) return;
  fileImages.value = { ...fileImages.value, [path]: "loading" };
  requestHost({ type: "previewImage", path, sessionId });
}

function fileImage(path: string): FileImageEntry | undefined {
  const entry = fileImages.value[path];
  return typeof entry === "object" ? entry : undefined;
}

// ---- Code & diff hover previews for CLI links: compact syntax-highlighted snippets fetched on
// demand by the host; clicking opens the full tiling panel (`TilePane` / `DocView`).
interface CodePreviewEntry {
  key: string;
  mode: "file" | "diff";
  displayPath: string;
  targetLine?: number | null;
  singleEdit?: boolean;
  totalLines: number;
  truncated?: boolean;
  diffFocus?: number;
  additions: number;
  deletions: number;
  lines?: ViewerLine[];
  diff?: DiffLineView[];
  error?: string | null;
}

const codePreviews = signal<Record<string, CodePreviewEntry | "loading">>({});
const codePreviewRequestedAt = new Map<string, number>();

function codePreviewKey(link: DetectedLink, sessionId: number): string {
  const base = [sessionId, link.kind, link.target, link.line ?? null, !!link.oldLine];
  if (link.kind !== "diff") return JSON.stringify(base);
  const { counts, lines } = link.edit ?? {};
  return JSON.stringify([
    ...base,
    counts?.additions,
    counts?.deletions,
    lines?.map((row) => [row.kind, row.oldNum, row.newNum, row.text]),
  ]);
}

function requestCodePreview(link: DetectedLink, sessionId: number): void {
  const key = codePreviewKey(link, sessionId);
  const existing = codePreviews.value[key];
  const now = Date.now();
  const last = codePreviewRequestedAt.get(key) ?? 0;
  if (existing === "loading" || (existing !== undefined && now - last < codePreviewLayout.revalidateMs)) return;
  codePreviewRequestedAt.set(key, now);
  if (existing === undefined) {
    codePreviews.value = { ...codePreviews.value, [key]: "loading" };
  }
  requestHost({
    type: "previewCode",
    key,
    kind: link.kind,
    path: link.target,
    line: link.line ?? null,
    oldLine: link.oldLine,
    edit: link.kind === "diff" ? link.edit ?? {} : undefined,
    sessionId,
  });
}

function receiveCodePreview(entry: CodePreviewEntry): void {
  const next = { ...codePreviews.value };
  delete next[entry.key];
  next[entry.key] = entry;
  const keys = Object.keys(next);
  for (const expired of keys.slice(0, Math.max(0, keys.length - codePreviewLayout.maxCache))) {
    delete next[expired];
    codePreviewRequestedAt.delete(expired);
  }
  codePreviews.value = next;
}


// ---- View modes over the single tiling model (niri-like): Grid (tiled panes), Focus (the active
// pane full screen) and Overview (niri: the live workspace stack zoomed out; arrows / Enter /
// Esc, drag a window to another workspace).
type ViewMode = "grid" | "focus" | "overview";

const overviewOpen2 = signal(false);
// Keyboard selection spans windows and the trailing empty workspace.
const overviewIndex = signal<number | "addWorkspace">(0);
// The workspace the overview closed onto, until the host has made it active.
const overviewExitTarget = signal<number | null>(null);

// Overview filter: words match window titles, folders and agents; `@status` tokens match the state
// an agent reports. Windows that do not match only dim, so the tiling never moves.
const overviewFilter = signal("");
// True while the filter field is mounted (and so focused): typing goes to it.
const overviewFiltering = signal(false);
// Bumped to remount the field with a completed token (JSNIR fields only take an initial text).
const overviewFilterRevision = signal(0);

interface OverviewStatusFilter {
  token: string;
  detail: string;
  match: (session: SessionItem) => boolean;
}

const OVERVIEW_STATUS_FILTERS: OverviewStatusFilter[] = [
  { token: "@working", detail: "Busy, running or thinking", match: (s) => s.status === "working" },
  { token: "@blocked", detail: "Needs attention or input", match: (s) => s.status === "blocked" },
  { token: "@stopped", detail: "Finished, failed or exited",
    match: (s) => s.status === "done" || s.status === "failed" || !s.alive },
  { token: "@idle", detail: "Waiting for a command", match: (s) => s.alive && (s.status ?? "idle") === "idle" },
  { token: "@ai", detail: "Agent sessions (Claude, Codex…)", match: (s) => s.cli !== "terminal" },
];

function overviewFilterWords(filter: string): string[] {
  return filter.trim().toLowerCase().split(/\s+/).filter((word) => word.length > 0);
}

function overviewMatches(id: number, filter = overviewFilter.value): boolean {
  const words = overviewFilterWords(filter);
  if (words.length === 0) return true;
  const session = sessionsState.value.items.find((s) => s.id === id);
  const spec = session ? undefined : tileOf(id);
  const text = session ? [cleanTitle(session.title), session.cli, session.cwd]
    : spec ? [tileTitle(spec, id), spec.kind, spec.path, spec.cwd ?? ""] : [];
  return words.every((word) => {
    if (!word.startsWith("@")) return text.join(" ").toLowerCase().includes(word);
    // A token still being typed ("@wo") matches every status it could become.
    return !!session && OVERVIEW_STATUS_FILTERS.some((f) => f.token.startsWith(word) && f.match(session));
  });
}

// Status tokens the last word of the filter could complete to.
function overviewSuggestions(): OverviewStatusFilter[] {
  const last = overviewFilter.value.endsWith(" ") ? "" : overviewFilterWords(overviewFilter.value).pop() ?? "";
  if (!last.startsWith("@")) return [];
  return OVERVIEW_STATUS_FILTERS.filter((f) => f.token.startsWith(last) && f.token !== last);
}

// The keyboard selection follows the filter to the first window that still matches.
function setOverviewFilter(text: string): void {
  overviewFilter.value = text;
  const selection = overviewIndex.value;
  if (typeof selection === "number" && overviewMatches(selection, text)) return;
  const first = allWindowIds().find((id) => overviewMatches(id, text));
  if (first !== undefined) overviewIndex.value = first;
}

function completeOverviewFilter(token = overviewSuggestions()[0]?.token): void {
  if (!token) return;
  setOverviewFilter(overviewFilter.value.replace(/\S*$/, token + " "));
  overviewFilterRevision.value++;
}

function resetOverviewFilter(): void {
  overviewFilter.value = "";
  overviewFiltering.value = false;
}

function setViewMode(mode: ViewMode): void {
  activeNav.value = "agents";
  if (mode === "overview") {
    if (overviewOpen2.value) return;
    overviewIndex.value = tilingState.value.focusedSession;
    overviewExitTarget.value = null;
    resetOverviewFilter();
    overviewOpen2.value = true;
    requestHost({ type: "overview", on: true });
    return;
  }
  if (overviewOpen2.value) closeOverviewMode();
  const maximized = tilingState.value.maximized;
  if ((mode === "focus") !== maximized) toggleTiledMaximize();
}

function toggleOverviewMode(): void {
  if (overviewOpen2.value) closeOverviewMode();
  else setViewMode("overview");
}

function closeOverviewMode(workspace = tilingState.value.activeWorkspace): void {
  if (!overviewOpen2.value) return;
  overviewExitTarget.value = workspace;
  resetOverviewFilter();
  overviewOpen2.value = false;
  requestHost({ type: "overview", on: false });
}

function pickFromOverview(id: number): void {
  const workspace = workspaceList().findIndex((row) => row.panes.includes(id));
  closeOverviewMode(workspace >= 0 ? workspace : undefined);
  if (workspace >= 0) selectSession(id);
}

function openWorkspaceFromOverview(index: number): void {
  closeOverviewMode(index);
  focusWorkspace(index);
}

// Follow the shape of the tiling: vertical neighbors in the same column come before another workspace.
function handleOverviewKey(action: string): void {
  if (!overviewOpen2.value) return;
  const selection = overviewIndex.value;
  if (action === "escape") {
    // Esc first drops the filter, then leaves the overview.
    if (overviewFiltering.value || overviewFilter.value) resetOverviewFilter();
    else closeOverviewMode();
    return;
  }
  if (action === "filter") {
    overviewFiltering.value = true;
    return;
  }
  if (action === "tab") {
    completeOverviewFilter();
    return;
  }
  const digit = /^workspace:([1-9])$/.exec(action);
  if (digit) {
    const index = Number(digit[1]) - 1;
    if (index < workspaceList().length) openWorkspaceFromOverview(index);
    return;
  }
  if (action === "enter") {
    if (typeof selection === "number" && !overviewMatches(selection)) return;
    if (selection === "addWorkspace") openWorkspaceFromOverview(workspaceList().length - 1);
    else pickFromOverview(selection);
    return;
  }
  const workspaces = workspaceList();
  const id = selection === "addWorkspace" ? 0 : selection;
  const selected = selection === "addWorkspace" ? workspaces.length - 1
    : workspaces.findIndex((row) => row.panes.includes(id));
  const index = selected >= 0 ? selected : tilingState.value.activeWorkspace;
  const columns = workspaces[index]?.columns ?? [];
  const col = columns.findIndex((column) => column.panes.includes(id));
  const pane = columns[col]?.panes.indexOf(id) ?? 0;
  let next: typeof selection | undefined;
  if (action === "left" || action === "right") {
    const target = columns[col + (action === "left" ? -1 : 1)];
    next = target?.panes[Math.min(pane, target.panes.length - 1)];
  } else if (action === "up" || action === "down" || action === "workspaceUp" || action === "workspaceDown") {
    const step = action === "up" || action === "workspaceUp" ? -1 : 1;
    if (step > 0 && index === workspaces.length - 1 && !workspaces[index].panes.length) {
      overviewIndex.value = "addWorkspace";
      return;
    }
    // The wheel moves a whole workspace (niri); arrows first walk the panes of the column.
    next = action.startsWith("workspace") ? undefined : columns[col]?.panes[pane + step];
    if (next === undefined) {
      for (let i = index + step; i >= 0 && i < workspaces.length; i += step) {
        if (workspaces[i].panes.length) {
          next = workspaces[i].focused || workspaces[i].panes[0];
          break;
        }
        if (i === workspaces.length - 1) {
          next = "addWorkspace";
          break;
        }
      }
    }
  }
  if (next !== undefined) overviewIndex.value = next;
}


// ---- Editable shortcuts: the host pushes every action with its chords ("Ctrl+Alt+R"); the UI
// shows them as key chips everywhere and edits them in Settings › Layout & Shortcuts.
interface KeybindView {
  id: string;
  label: string;
  group: string;
  description: string;
  chords: string[];
  defaults: string[];
  customized: boolean;
}

const keybindsState = signal<KeybindView[]>([]);
// Action being recorded in Settings (the host captures the next chord), and the last notice.
const keybindRecording = signal<string | null>(null);
const keybindNotice = signal("");

const KEY_GLYPHS: Record<string, string> = {
  Shift: "⇧",
  Left: "←",
  Right: "→",
  Up: "↑",
  Down: "↓",
  Escape: "Esc",
  Enter: "↵",
};

function chordParts(chord: string): string[] {
  const parts = chord.endsWith("++") ? [...chord.slice(0, -2).split("+"), "+"] : chord.split("+");
  return parts.filter((p) => p.length > 0).map((p) => KEY_GLYPHS[p] ?? p);
}

// Key chips for an action's first chord (empty when unbound), for tooltips and menus.
function keysFor(id: string): string[] {
  const chord = keybindsState.value.find((k) => k.id === id)?.chords[0];
  return chord ? chordParts(chord) : [];
}

function startKeybindRecording(id: string): void {
  keybindRecording.value = id;
  keybindNotice.value = "";
  requestHost({ type: "keyCapture", on: true });
}

function stopKeybindRecording(): void {
  keybindRecording.value = null;
  requestHost({ type: "keyCapture", on: false });
}
