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
  cwd: string;
  alive: boolean;
}

interface SessionsView {
  activeId: number;
  items: SessionItem[];
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

const sessionsState = signal<SessionsView>({ activeId: 1, items: [] });
const screenState = signal<ScreenView>({
  cols: 104,
  rowsCount: 34,
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
const noticeState = signal<NoticeView | null>(null);

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

function sendInput(text: string): void {
  requestHost({ type: "input", text });
}

function sendKey(key: "ctrl-c" | "esc" | "tab" | "up" | "down"): void {
  requestHost({ type: "key", key });
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

function selectSession(id: number): void {
  requestHost({ type: "selectSession", id });
}

function Show(props: { when: Source<boolean>; children: () => Element }): Element {
  return (
    <Dynamic value={() => read(props.when)}>
      {(visible) => (visible ? props.children() : <Box />)}
    </Dynamic>
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

function PrimaryButton(props: {
  label: Source<string>;
  onPress: () => void;
  height?: number;
  disabled?: Source<boolean>;
}): Element {
  return (
    <Box
      style={{
        height: props.height ?? spacing.button,
        padding: [0, 12, 0, 12],
        align: "center",
        justify: "center",
        shrink: 0,
      }}
      disabled={props.disabled}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.lg,
        background: accent().brand,
        opacity: read(props.disabled ?? false) ? 0.5 : 1,
        hover: { brightness: 0.92 },
        active: { scale: 0.98 },
        transitions: [smoothTransition("brightness", motion.fast), smoothTransition("scale", motion.fast)],
      })}
    >
      <Text
        font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.medium }}
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

function KbdChip(props: {
  label: Source<string>;
  onPress?: () => void;
  accented?: Source<boolean>;
}): Element {
  return (
    <Box
      style={{
        height: 22,
        padding: [0, 7, 0, 7],
        align: "center",
        justify: "center",
        shrink: 0,
      }}
      onPress={props.onPress}
      paint={() => {
        const isAccented = read(props.accented ?? false);
        return {
          radius: radii.chip,
          background: isAccented ? accent().brandSoft : palette().kbdBg,
          edge: isAccented ? accent().ring : palette().border,
          edgeWidth: spacing.hairline,
          hover: props.onPress ? { background: palette().sidebarActive } : undefined,
          transitions: [smoothTransition("background", motion.fast)],
        };
      }}
    >
      <Text
        font={{ family: fontFamilies.mono, size: typeScale.tiny, weight: fontWeights.normal }}
        paint={() => ({
          color: read(props.accented ?? false) ? accent().brandText : palette().textSecondary,
        })}
      >
        {props.label}
      </Text>
    </Box>
  );
}
