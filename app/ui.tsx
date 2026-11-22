// Shared UI state, host bridge helpers, and Accordee-styled primitive components.

interface TerminalSpan {
  text: string;
  fg: number;
  bg?: number;
  bold: boolean;
  underline: boolean;
  link: boolean;
}

interface DetectedLink {
  kind: "file" | "url";
  target: string;
  line?: number | null;
  col?: number | null;
  resolved: boolean;
}

interface TerminalRow {
  idx: number;
  plain: string;
  spans: TerminalSpan[];
  links: DetectedLink[];
}

interface ScreenView {
  cols: number;
  rowsCount: number;
  cursorRow: number;
  cursorCol: number;
  cursorVisible: boolean;
  rows: TerminalRow[];
}

interface SessionItem {
  id: number;
  title: string;
  cli: string;
  cwd: string;
  alive: boolean;
}

interface SessionsView {
  activeId: number;
  archivedCount: number;
  items: SessionItem[];
}

interface TilingColumnView {
  panes: number[];
}

interface TilingView {
  mode: "niri" | "hyprland";
  focusedSession: number;
  maximized: boolean;
  totalCols: number;
  scrollCol: number;
  visibleColumns: TilingColumnView[];
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

interface ConfigView {
  theme: string;
  mode: "dark" | "light";
  accent: AccentName;
  density: DensityMode;
  fontSize: number;
  tilingMode: "niri" | "hyprland";
  checkUpdates: boolean;
  autoRestartUpdate: boolean;
  clis: CliConfigItem[];
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

interface TaskItemView {
  id: number;
  title: string;
  done: boolean;
  tag: string;
}

interface FileEntryView {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
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

interface CliUsageItem {
  id: string;
  name: string;
  color: string;
  colorRgba: number;
  percent: number;
  inputTokens: string;
  outputTokens: string;
  sessions: number;
  status: string;
}

interface ViewerLine {
  num: number;
  text: string;
  highlighted: boolean;
}

interface ViewerView {
  open: boolean;
  path: string;
  displayPath: string;
  targetLine?: number | null;
  targetCol?: number | null;
  error?: string | null;
  lines: ViewerLine[];
}

interface NoticeView {
  kind: string;
  text: string;
}

type NavSection = "agents" | "files" | "tasks" | "metrics" | "settings";

const sessionsState = signal<SessionsView>({
  activeId: 1,
  archivedCount: 0,
  items: [],
});

const tilingState = signal<TilingView>({
  mode: "niri",
  focusedSession: 1,
  maximized: false,
  totalCols: 1,
  scrollCol: 0,
  visibleColumns: [{ panes: [1] }],
});

const screenState = signal<ScreenView>({
  cols: 120,
  rowsCount: 38,
  cursorRow: 0,
  cursorCol: 0,
  cursorVisible: true,
  rows: [],
});

const screensMapState = signal<Record<string, ScreenView>>({});

const viewerState = signal<ViewerView>({
  open: false,
  path: "",
  displayPath: "",
  targetLine: null,
  targetCol: null,
  error: null,
  lines: [],
});

const cliUsageState = signal<CliUsageItem[]>([]);

const configState = signal<ConfigView>({
  theme: "accordee-dark",
  mode: "dark",
  accent: "blue",
  density: "comfortable",
  fontSize: 13,
  tilingMode: "niri",
  checkUpdates: true,
  autoRestartUpdate: true,
  clis: [],
});

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

const tasksState = signal<TaskItemView[]>([]);

const filesState = signal<FilesView>({
  cwd: "",
  subdir: "",
  entries: [],
});

const updateState = signal<UpdateView>({
  currentVersion: "0.1.0",
  availableVersion: "0.1.0",
  hasUpdate: false,
  stage: "idle",
  progress: 0,
  statusText: "Checking for updates...",
  assetUrl: null,
  stagedPath: null,
  notes: {
    version: "0.1.0",
    title: "icy",
    publishedAt: "Latest",
    summary: "",
    highlights: [],
    media: [],
    releaseUrl: "https://github.com/Niiihuel/icy/releases",
  },
});

const noticeState = signal<NoticeView | null>(null);
const sidebarOpen = signal(true);
const searchQuery = signal("");
const activeNav = signal<NavSection>("agents");
const viewportSize = signal<[number, number]>([1280, 800]);
const releaseNotesOpen = signal(false);
const restartConfirmOpen = signal(false);
const installPopoverOpen = signal(false);

function requestHost(payload: Record<string, unknown>): void {
  try {
    jsnirRequest(JSON.stringify(payload));
  } catch (_) {
    // Ignore transient request failures when closing or reloading.
  }
}

function sendHello(): void {
  requestHost({ type: "hello" });
}

function openLink(link: DetectedLink): void {
  requestHost({
    type: "open",
    kind: link.kind,
    path: link.target,
    line: link.line ?? null,
    col: link.col ?? null,
  });
}

function closeViewer(): void {
  requestHost({ type: "closeViewer" });
}

function createSession(): void {
  requestHost({ type: "newSession" });
}

function openTiledColumn(): void {
  requestHost({ type: "openColumn" });
}

function stackTiledVertical(): void {
  requestHost({ type: "stackVertical" });
}

function toggleTiledMaximize(): void {
  requestHost({ type: "toggleMaximize" });
}

function setTilingMode(mode: "niri" | "hyprland"): void {
  requestHost({ type: "setTilingMode", mode });
}

function scrollTiledColumns(delta: number): void {
  requestHost({ type: "scrollColumn", delta });
}

function launchCli(cliId: string): void {
  activeNav.value = "agents";
  requestHost({ type: "launchCli", cliId });
}

function closeSession(id: number): void {
  requestHost({ type: "closeSession", id });
}

function selectSession(id: number): void {
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

function refreshGitHub(): void {
  requestHost({ type: "refreshGitHub" });
}

function listFiles(subdir = ""): void {
  requestHost({ type: "listFiles", subdir });
}

function addTask(title: string, tag = "todo"): void {
  requestHost({ type: "addTask", title, tag });
}

function toggleTask(id: number): void {
  requestHost({ type: "toggleTask", id });
}

function deleteTask(id: number): void {
  requestHost({ type: "deleteTask", id });
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
    sidebarOpen: sidebarOpen.value,
    viewerOpen: viewerState.value.open,
  });
}

function detectedFilesList(): DetectedLink[] {
  const seen = new Set<string>();
  const out: DetectedLink[] = [];
  for (const row of screenState.value.rows) {
    for (const link of row.links) {
      if (link.kind === "file" && !seen.has(link.target)) {
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

function openTasksCount(): number {
  return tasksState.value.filter((t) => !t.done).length;
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
        font={{ family: fontFamilies.sans, size: typeScale.micro, weight: fontWeights.medium }}
        paint={() => ({ color: palette().textMuted })}
      >
        {props.label}
      </Text>
    </Box>
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
        opacity: props.disabled && read(props.disabled) ? 0.55 : 1,
        hover: { brightness: 1.08 },
        active: { scale: 0.98 },
        transitions: [
          smoothTransition("background", motion.fast),
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
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: palette().panelBg,
        edge: palette().border,
        edgeWidth: spacing.hairline,
        hover: { background: palette().sidebarActive },
        active: { scale: 0.98 },
        transitions: [
          smoothTransition("background", motion.fast),
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
