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

const sessionsState = signal<SessionsView>({
  activeId: 1,
  archivedCount: 0,
  items: [],
});
const screenState = signal<ScreenView>({
  cols: 120,
  rowsCount: 38,
  cursorRow: 0,
  cursorCol: 0,
  cursorVisible: true,
  rows: [],
});
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
const noticeState = signal<NoticeView | null>(null);
const sidebarOpen = signal(true);
const searchQuery = signal("");
const themesDrawerOpen = signal(false);
const activeNav = signal<"agents" | "files" | "tasks" | "metrics">("agents");
const viewportSize = signal<[number, number]>([1280, 800]);

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

function closeSession(id: number): void {
  requestHost({ type: "closeSession", id });
}

function selectSession(id: number): void {
  requestHost({ type: "selectSession", id });
}

function syncLayout(): void {
  requestHost({
    type: "layout",
    sidebarOpen: sidebarOpen.value,
    viewerOpen: viewerState.value.open,
  });
}

function detectedFilesCount(): number {
  const seen = new Set<string>();
  for (const row of screenState.value.rows) {
    for (const link of row.links) {
      if (link.kind === "file") {
        seen.add(link.target);
      }
    }
  }
  return seen.size;
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
        width: 16,
        height: 16,
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
        transitions: [smoothTransition("background", motion.fast)],
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
