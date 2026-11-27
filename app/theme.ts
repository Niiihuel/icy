// Accordee design tokens mapped to JSNIR primitives. Colors are 0xRRGGBBAA.
// Source of truth: design/accordee-tokens.css, design/reference/accordee/raw/1tyd3ebsyb3wb.css & DESIGN.md.

interface ThemePalette {
  appBg: number;
  panelBg: number;
  cardBg: number;
  kbdBg: number;
  textPrimary: number;
  textSecondary: number;
  textMuted: number;
  textSubtle: number;
  border: number;
  sidebarBg: number;
  sidebarActive: number;
  sendIdle: number;
  highlight: number;
  destructive: number;
  ring: number;
  statusOnline: number;
  statusIdle: number;
  backdrop: number;
  warningBg: number;
  warningText: number;
  successBg: number;
  successText: number;
  tooltipBg: number;
  tooltipFg: number;
  tooltipMuted: number;
  tooltipKbdBg: number;
  syntax: Record<TokenKind, number>;
  diffAddText: number;
  diffDelText: number;
  diffAddBg: number;
  diffDelBg: number;
  diffHunkBg: number;
  vcsModified: number;
  vcsAdded: number;
  vcsUntracked: number;
  vcsDeleted: number;
  mediaGradient: [number, number];
  series: [number, number, number, number];
  /** Git graph lane colours, cycled by lane. */
  graphLanes: number[];
  charts?: ChartTheme;
}

type AccentName = "neutral" | "blue" | "violet" | "rose" | "orange" | "green" | "teal";
type DensityMode = "comfortable" | "compact";
type ChartProvider = "anthropic" | "google" | "openai" | "xai" | "other";

interface ChartTheme {
  input?: number | null;
  output?: number | null;
  sessions?: number | null;
  duration?: number | null;
  axis?: number | null;
  grid?: number | null;
  track?: number | null;
  heatmap?: (number | null)[];
  providers?: Partial<Record<ChartProvider, number>>;
}

interface AccentPalette {
  brand: number;
  brandFg: number;
  brandText: number;
  brandSoft: number;
  ring: number;
}

interface CustomThemeColors {
  appBg: number;
  sidebarBg: number;
  sidebarActive: number;
  panelBg: number;
  cardBg: number;
  kbdBg: number;
  textPrimary: number;
  textSecondary: number;
  textMuted: number;
  border: number;
  highlight: number;
  selection?: number | null;
  charts?: ChartTheme;
  graph?: number[];
}

interface ResolvedThemeItem {
  id: string;
  name: string;
  mode: string;
  author: string;
  path: string;
  colors: CustomThemeColors;
}

const darkPalette: ThemePalette = {
  appBg: 0x0a0a0aff,
  panelBg: 0x171717ff,
  cardBg: 0x171717ff,
  kbdBg: 0x262626ff,
  textPrimary: 0xfafafaff,
  textSecondary: 0xa3a3a3ff,
  textMuted: 0x8a8a8aff,
  textSubtle: 0xe5e5e5ff,
  border: 0xffffff1a,
  sidebarBg: 0x0f0f10ff,
  sidebarActive: 0x1f1f22ff,
  sendIdle: 0x3f3f46ff,
  highlight: 0x3b82f6ff,
  destructive: 0xf87171ff,
  ring: 0x52525bff,
  statusOnline: 0x4ade80ff,
  statusIdle: 0x71717aff,
  backdrop: 0x00000099,
  warningBg: 0xf59e0b1f,
  warningText: 0xfbbf24ff,
  successBg: 0x22c55e24,
  successText: 0x4ade80ff,
  tooltipBg: 0xfafafaff,
  tooltipFg: 0x0a0a0aff,
  tooltipMuted: 0x737373ff,
  tooltipKbdBg: 0x00000014,
  // GitHub Dark syntax colors; diff + VCS dots follow Antigravity (green/red-500, amber/emerald/teal/rose).
  syntax: {
    plain: 0xe6edf3ff,
    keyword: 0xff7b72ff,
    string: 0xa5d6ffff,
    comment: 0x8b949eff,
    number: 0x79c0ffff,
    type: 0xffa657ff,
    function: 0xd2a8ffff,
  },
  diffAddText: 0x22c55eff,
  diffDelText: 0xef4444ff,
  diffAddBg: 0x22c55e1f,
  diffDelBg: 0xef44441f,
  diffHunkBg: 0x3b82f614,
  vcsModified: 0xf59e0bff,
  vcsAdded: 0x10b981ff,
  vcsUntracked: 0x2dd4bfff,
  vcsDeleted: 0xf43f5eff,
  mediaGradient: [0x1e1b4bff, 0x0f172aff],
  series: [0x3987e5ff, 0xd95926ff, 0x199e70ff, 0xc98500ff],
  graphLanes: [0x60a5faff, 0x2dd4bfff, 0xa78bfaff, 0xfbbf24ff, 0xf472b6ff, 0x4ade80ff, 0xfb923cff, 0x22d3eeff],
};

const lightPalette: ThemePalette = {
  appBg: 0xffffffff,
  panelBg: 0xf4f4f5ff,
  cardBg: 0xffffffff,
  kbdBg: 0xe5e7ebff,
  textPrimary: 0x09090bff,
  textSecondary: 0x52525bff,
  textMuted: 0x71717aff,
  textSubtle: 0x3f3f46ff,
  border: 0x0000001a,
  sidebarBg: 0xfafafaff,
  sidebarActive: 0xf0f0f1ff,
  sendIdle: 0x8e8e93ff,
  highlight: 0x2563ebff,
  destructive: 0xef4444ff,
  ring: 0xa1a1aaff,
  statusOnline: 0x16a34aff,
  statusIdle: 0x71717aff,
  backdrop: 0x09090b66,
  warningBg: 0xf59e0b22,
  warningText: 0xb45309ff,
  successBg: 0x16a34a1f,
  successText: 0x15803dff,
  tooltipBg: 0x09090bff,
  tooltipFg: 0xfafafaff,
  tooltipMuted: 0xa1a1aaff,
  tooltipKbdBg: 0xffffff29,
  syntax: {
    plain: 0x1f2328ff,
    keyword: 0xcf222eff,
    string: 0x0a3069ff,
    comment: 0x6e7781ff,
    number: 0x0550aeff,
    type: 0x953800ff,
    function: 0x8250dfff,
  },
  diffAddText: 0x16a34aff,
  diffDelText: 0xdc2626ff,
  diffAddBg: 0x16a34a1a,
  diffDelBg: 0xdc26261a,
  diffHunkBg: 0x2563eb0f,
  vcsModified: 0xf59e0bff,
  vcsAdded: 0x10b981ff,
  vcsUntracked: 0x14b8a6ff,
  vcsDeleted: 0xf43f5eff,
  mediaGradient: [0x312e81ff, 0x1e293bff],
  series: [0x2a78d6ff, 0xeb6834ff, 0x1baf7aff, 0xeda100ff],
  graphLanes: [0x2563ebff, 0x0d9488ff, 0x7c3aedff, 0xd97706ff, 0xdb2777ff, 0x16a34aff, 0xea580cff, 0x0891b2ff],
};

const accentsDark: Record<AccentName, AccentPalette> = {
  neutral: {
    brand: 0xfafafaff,
    brandFg: 0x0a0a0aff,
    brandText: 0xfafafaff,
    brandSoft: 0xffffff14,
    ring: 0x52525bff,
  },
  blue: {
    brand: 0x3b82f6ff,
    brandFg: 0xffffffff,
    brandText: 0x60a5faff,
    brandSoft: 0x3b82f633,
    ring: 0x3b82f68c,
  },
  violet: {
    brand: 0x8b5cf6ff,
    brandFg: 0xffffffff,
    brandText: 0xa78bfaff,
    brandSoft: 0x8b5cf633,
    ring: 0x8b5cf68c,
  },
  rose: {
    brand: 0xf43f5eff,
    brandFg: 0xffffffff,
    brandText: 0xfb7185ff,
    brandSoft: 0xf43f5e33,
    ring: 0xf43f5e8c,
  },
  orange: {
    brand: 0xf97316ff,
    brandFg: 0xffffffff,
    brandText: 0xfb923cff,
    brandSoft: 0xf9731633,
    ring: 0xf973168c,
  },
  green: {
    brand: 0x16a34aff,
    brandFg: 0xffffffff,
    brandText: 0x4ade80ff,
    brandSoft: 0x16a34a33,
    ring: 0x16a34a8c,
  },
  teal: {
    brand: 0x14b8a6ff,
    brandFg: 0xffffffff,
    brandText: 0x2dd4bfff,
    brandSoft: 0x14b8a633,
    ring: 0x14b8a68c,
  },
};

const accentsLight: Record<AccentName, AccentPalette> = {
  neutral: {
    brand: 0x09090bff,
    brandFg: 0xffffffff,
    brandText: 0x09090bff,
    brandSoft: 0x0000000f,
    ring: 0xa1a1aaff,
  },
  blue: {
    brand: 0x2563ebff,
    brandFg: 0xffffffff,
    brandText: 0x1d4ed8ff,
    brandSoft: 0x2563eb1f,
    ring: 0x2563eb8c,
  },
  violet: {
    brand: 0x7c3aedff,
    brandFg: 0xffffffff,
    brandText: 0x6d28d9ff,
    brandSoft: 0x7c3aed1f,
    ring: 0x7c3aed8c,
  },
  rose: {
    brand: 0xe11d48ff,
    brandFg: 0xffffffff,
    brandText: 0xbe123cff,
    brandSoft: 0xe11d481f,
    ring: 0xe11d488c,
  },
  orange: {
    brand: 0xea580cff,
    brandFg: 0xffffffff,
    brandText: 0xc2410cff,
    brandSoft: 0xea580c1f,
    ring: 0xea580c8c,
  },
  green: {
    brand: 0x16a34aff,
    brandFg: 0xffffffff,
    brandText: 0x15803dff,
    brandSoft: 0x16a34a1f,
    ring: 0x16a34a8c,
  },
  teal: {
    brand: 0x0d9488ff,
    brandFg: 0xffffffff,
    brandText: 0x0f766eff,
    brandSoft: 0x0d94881f,
    ring: 0x0d94888c,
  },
};

const typeScale = {
  micro: 10,
  tiny: 11,
  caption: 12,
  label: 13,
  body: 14,
  subtitle: 15,
  title: 16,
  headingSm: 18,
  heading: 20,
  metric: 24,
  displaySm: 26,
  display: 30,
};

const fontWeights = {
  light: 300,
  normal: 400,
  // Accordee caps emphasis at a medium weight; Inter-Medium (500) is the embedded match.
  medium: 500,
  semibold: 500,
  bold: 500,
};

const fontFamilies = {
  sans: "Inter",
  mono: "Geist Mono",
};

const spacing = {
  unit: 4,
  compactUnit: 3.36,
  hairline: 0.5,
  control: 28,
  button: 30,
  controlLg: 32,
  toolbar: 44,
  tabbar: 44,
  workspaceFrame: 8,
  sidebarMenuWidth: 180,
  sidebarWidth: 224,
  sidebarRail: 52,
  railButton: 36,
  menuItem: 32,
  menuWidth: 232,
  panelWidth: 300,
  codePanelMin: 520,
  headerHeight: 44,
  // niri `gaps 16`: between windows and around them, also for a lone maximized column.
  paneGap: 16,
};

// Folder view inside the launcher, with Nautilus-style places.
const launcherFolderLayout = {
  placesWidth: 196,
  compactAt: 540,
  placeHeight: 30,
  // Tree level width: the guide runs under the parent's icon (see LauncherTreeGuide).
  indent: 20,
  // Deepest ancestors kept above the current folder.
  treeDepth: 6,
  listInset: spacing.unit * 2,
  rowHeight: 34,
  rowPadding: [0, spacing.unit * 3, 0, spacing.unit * 3] as [number, number, number, number],
  gap: spacing.unit * 2,
  icon: 15,
};

const quietControlLayout = {
  height: spacing.control,
  inset: spacing.unit / 2,
  gap: spacing.unit,
};

// One icon-button scale: `sm` for in-card headers and dense toolbars, `md` = Accordee `size-button`.
const iconButtonLayout = {
  sm: { size: spacing.unit * 6, icon: 14 },
  md: { size: spacing.button, icon: 16 },
};

const titleFadeLayout = {
  width: spacing.unit * 4,
  measureDelay: 16,
};

const tooltipLayout = {
  height: 26,
  padding: spacing.unit * 2,
  headerGap: spacing.unit * 1.5,
  keyGap: 3,
  keySize: 18,
  caretSize: 10,
  caretOverlap: spacing.hairline * 2,
  anchorGap: spacing.unit,
  delay: 250,
  layer: 1000,
};

// Accordee control states (verified in the served CSS): `hover:opacity-90`, `disabled:opacity-40`,
// `active:scale-[0.98]` on labelled buttons and `active:scale-95` on icon-only buttons.
const controlState = {
  hoverOpacity: 0.9,
  disabledOpacity: 0.4,
  pressScale: 0.98,
  iconPressScale: 0.95,
  // `hover:border-textMuted/40` = color-mix(in oklab, var(--ac-text-muted) 40%, transparent).
  hoverBorderMix: 0.4,
  // Floating-chrome rows: the selected one gets a half-strength accent line, a hovered one a faint
  // line, so a selected row next to a hovered one never reads as two equal boxes.
  selectedLineMix: 0.5,
  hoverLineMix: 0.18,
  // Row hover wash: `bg-textPrimary/5`, lighter than `kbdBg` so keycaps stay visible on it.
  rowHoverMix: 0.05,
};

// Scales a 0xRRGGBBAA color's alpha, the JSNIR stand-in for `color-mix(..., transparent)`.
function withAlpha(color: number, fraction: number): number {
  return ((color & 0xffffff00) | Math.round(fraction * (color & 0xff))) >>> 0;
}

const windowAppearance = {
  maxTransparency: 100,
  transparencyStep: 5,
  valueWidth: spacing.unit * 9,
};

function windowTransparency(): number {
  return windowTransparencySupported.value ? configState.value.windowTransparency : 0;
}

function windowBackdrop(): number {
  return withAlpha(palette().sidebarBg, 1 - windowTransparency() / windowAppearance.maxTransparency);
}

// In translucent mode the base tint is shared by the shell and the terminal panes.
function windowUnderlay(color: number): number {
  return windowTransparency() > 0 ? 0 : color;
}

function mixColor(a: number, b: number, t: number): number {
  const ch = (c: number, shift: number) => (c >>> shift) & 255;
  const lerp = (shift: number) => Math.round(ch(a, shift) + (ch(b, shift) - ch(a, shift)) * t);
  return ((lerp(24) << 24) | (lerp(16) << 16) | (lerp(8) << 8) | lerp(0)) >>> 0;
}

function hoverBorder(): number {
  return withAlpha(palette().textMuted, controlState.hoverBorderMix);
}

// The one surface of the floating chrome (`floatingLayout`): search bars, results panels,
// suggestions and pills.
function floatingSurface(): Paint {
  return { radius: radii.lg, background: withAlpha(palette().cardBg, floatingLayout.surfaceAlpha),
    backdropBlur: floatingLayout.surfaceBlur, backdropSaturation: floatingLayout.saturation,
    edge: palette().border, edgeWidth: spacing.hairline, shadows: [shadows.card] };
}

// A selectable row inside the floating chrome: selection and hover are a line, never a fill.
function selectableRowPaint(selected: boolean): Paint {
  const line = withAlpha(accent().brand, controlState.selectedLineMix);
  // Rows sit `spacing.unit * 2` inside a `radii.lg` panel: a smaller radius keeps the curves concentric.
  return { radius: radii.md, edge: selected ? line : 0, edgeWidth: spacing.hairline,
    hover: { edge: selected ? line : withAlpha(palette().textMuted, controlState.hoverLineMix) } };
}

function rowHoverBackground(): number {
  return withAlpha(palette().textPrimary, controlState.rowHoverMix);
}

const projectSidebarLayout = {
  inset: spacing.unit * 2,
};

const agentListLayout = {
  headerHeight: spacing.toolbar - spacing.unit,
  rowHeight: spacing.button,
  rowGap: spacing.unit,
  sectionHeight: spacing.unit * 7,
  padding: spacing.unit * 2,
  gap: spacing.unit * 1.5,
  icon: 16,
  statusSize: 14,
  dot: 7,
  // Rotated frames of the working spinner rasterized by the host (icons.rs SPINNER_FRAMES).
  spinnerFrames: 12,
  spinnerFrameMs: 70,
};

const emptyStateLayout = {
  actionInset: spacing.unit * 2,
  actionGap: spacing.unit * 3,
  detailGap: spacing.unit,
  padding: spacing.unit * 4,
};

const emptyWorkspaceLayout = {
  contentWidth: 280,
  shortcutMinWidth: 248,
};

const workspacePixelBlast = {
  mask: "workspace:pixel-blast",
  pausedMask: "workspace:pixel-blast-paused",
  releaseMask: "release:pixel-blast",
  baseOpacity: 0.32,
  accentOpacity: 0.57,
};

const workspaceTiling = {
  resizeHitSlop: spacing.unit,
  resizeHoverOpacity: 0.55,
  guideWidth: spacing.unit / 2,
  guideInset: spacing.unit * 6,
  dragThreshold: spacing.unit * 1.5,
  dragSourceOpacity: 0.32,
  dragPreviewWidth: 272,
  dragPreviewHeight: 156,
  previewHeaderHeight: spacing.controlLg,
  previewPadding: spacing.unit * 3,
  previewLineHeight: 11,
  previewTypeSize: 8,
  // Smallest font a scaled terminal grid uses in overview thumbnails.
  previewMinTypeSize: 3,
  insertionEdgeWidth: spacing.unit / 2,
  insertionZoneFraction: 0.22,
  insertionZoneMax: 90,
};

const workspaceTabLayout = {
  layer: 2,
  // Outline geometry matches assets/icons/workspace-tab-outline.svg.
  height: spacing.headerHeight - spacing.unit * 1.5,
  overhang: spacing.unit * 2.5,
  stroke: spacing.hairline * 2,
  // Inactive tabs recede (image #4 reference): dimmed icon, muted title.
  dimOpacity: 0.55,
  width: 156,
  minWidth: 96,
  maxWidth: 400,
  capWidth: spacing.unit * 5,
  resizeHitWidth: spacing.unit * 1.5,
  closeSize: 20,
  gap: spacing.unit * 2,
  stripGap: spacing.unit / 2,
  icon: 15,
  inset: spacing.hairline * 2,
};

const horizontalStripLayout = {
  controlWidth: spacing.unit * 6,
  controlHeight: spacing.unit * 6,
  chevronSize: 14,
  endOpacity: controlState.disabledOpacity,
  layoutDelay: 16,
};

const settingsLayout = {
  sidebarWidth: 212,
  narrowSidebarWidth: 168,
  stackedAt: 820,
  pageWidth: 700,
  pagePadding: [spacing.unit * 9, spacing.unit * 6, spacing.unit * 12, spacing.unit * 6] as [number, number, number, number],
  sectionGap: spacing.unit * 6,
  fieldGap: spacing.unit * 3,
  rowPadding: [spacing.unit * 2.5, spacing.unit * 3.5, spacing.unit * 2.5, spacing.unit * 3.5] as [number, number, number, number],
  inset: spacing.unit * 2,
  icon: 15,
  swatchSize: 12,
  accentSize: 18,
};

// Rich usage tooltip: sections split by hairlines, limit rows with a used-share bar.
const usageTooltipLayout = {
  width: 256,
  sectionGap: spacing.unit * 2,
  rowGap: spacing.unit * 1.5,
  barHeight: spacing.unit,
  warnAt: 75,
  criticalAt: 90,
};

const cliUsageLayout = {
  chipSize: spacing.button,
  iconSize: 18,
  badgeSize: spacing.unit * 3,
  badgeLeft: spacing.unit * 4,
  badgeTop: spacing.unit / 2,
  gap: spacing.unit,
  padding: [spacing.unit * 2, spacing.unit / 2, spacing.unit * 1.5, spacing.unit / 2] as [number, number, number, number],
};

// Terminal cell grid inside its pane. Geometry is shared by the PTY size, cells, caret and images.
const terminalGridLayout = {
  top: 10,
  left: 16,
  advance: 0.6,
  lineHeight: 1.4,
  boldWeight: 600,
  textSlack: 2,
  caretInset: 0.2,
  settleDelay: 40,
};

// Loading placeholders for a terminal or browser before its first frame.
const skeletonMotion = {
  pulse: 900,
  dim: 0.45,
};

// A terminal waiting for its first frame: placeholder lines at fixed widths (fractions of the
// grid), so it reads as output without jumping around.
const terminalSkeleton = {
  lines: [0.62, 0.38, 0.84, 0.5, 0.71, 0.28, 0.56],
  lineHeight: 8,
  gap: spacing.unit * 3,
};

// Mouse selection in a terminal: the theme's highlight at 35%, readable over any cell colour.
function terminalSelectionBg(): number {
  return ((palette().highlight & 0xffffff00) | 0x59) >>> 0;
}

// Archived window and its centred empty state.
const archivedLayout = {
  maxWidth: 720,
  emptyWidth: 320,
  padding: spacing.unit * 5,
  headerGap: spacing.unit * 4,
  rowPadding: [spacing.unit * 2, spacing.unit * 3, spacing.unit * 2, spacing.unit * 3] as [number, number, number, number],
  icon: 20,
};

// Review uses the same compact table rhythm as Accordee, with room for a path subtitle.
// Review › Changes | Graph share one minimal, single-line rhythm with the git graph.
const reviewLayout = {
  inset: spacing.unit * 2,
  headerPadding: spacing.unit * 3,
  rowHeight: 28,
  rowHeightCompact: 24,
  statusWidth: 10,
  nameMaxWidth: 260,
  icon: 16,
  statusDot: 6,
};

// Git graph (Review › Graph): lane column and commit rows. The host rasterizes lane lines at
// these sizes, so a theme that changes them restyles the whole graph.
const gitGraphLayout = {
  lane: 14,
  rowHeight: 28,
  stroke: 2,
  dot: 8,
  dotRing: 2,
  maxLanes: 12,
  refHeight: 18,
  refPadding: spacing.unit * 1.5,
  inset: spacing.unit * 2,
};

const editorLayout = {
  overscan: 600,
  fallbackWidth: 600,
  fallbackHeight: 800,
  measureDelay: 16,
  settleDelay: 600,
  revealDelay: 60,
  paddingTop: spacing.unit * 2,
  paddingBottom: spacing.unit * 4,
  codeSize: 12.5,
  codeLineHeight: 1.5,
  monoAdvance: 0.6,
  minWrapColumns: 8,
  tabColumns: 4,
  maxDrawColumns: 512,
  gutterWidth: 48,
  gutterGap: 14,
  lineEndPadding: 14,
  diffMarker: 3,
  foldHeight: 52,
  scrollbarWidth: 8,
  scrollbarInset: 2,
  scrollbarMinThumb: 24,
  sessionListHeight: 280,
  sampleScale: 2,
  sampleBackground: 0x20242cff,
  selectionMix: 0.35,
  selectionRadius: 3,
  footerHeight: spacing.unit * 6,
  footerPadding: [0, spacing.unit * 3, 0, spacing.unit * 3] as [number, number, number, number],
  footerGap: spacing.unit * 4,
  footerIcon: 12,
  chipHeight: spacing.unit * 6,
  chipWidth: 150,
  chipGap: spacing.unit * 2,
  chipPadding: [0, spacing.unit * 2, 0, spacing.unit * 2] as [number, number, number, number],
  chipLayer: 1,
  caretPollMs: 150,
  doubleClickMs: 400,
  blameDelay: 400,
};

// URL preview: local pages are rendered at a desktop size before scaling into the card.
const urlPreviewLayout = {
  width: 480,
  height: 300,
  pageWidth: 1280,
  pageHeight: 800,
  headerGap: 6,
  icon: 13,
  loadTimeout: 4000,
  leaveDelay: 200,
  // Hidden this long, the preview page is closed (it would keep CEF's pump waking the window).
  idleClose: 30000,
};

const codePreviewLayout = {
  width: 480,
  maxLines: 14,
  padding: [6, 6, 8, 6] as [number, number, number, number],
  gap: 6,
  codePadding: [6, 0, 6, 0] as [number, number, number, number],
  codeSize: 11.5,
  codeLineHeight: 1.45,
  rowHeight: 17,
  gutterWidth: 36,
  gutterGap: 10,
  signWidth: 10,
  lineEndPadding: 10,
  headerIcon: 13,
  loadingHeight: 112,
  hoverDelay: 220,
  // Grace before closing when the pointer leaves a link span (it may enter the link's next span).
  leaveDelay: 80,
  // Keep terminal previews below the workspace tab bar and above inline terminal content.
  layer: 1,
  maxCache: 24,
  revalidateMs: 1500,
};

const launcherLayout = {
  width: 700,
  height: 576,
  margin: spacing.unit * 3,
  gutter: spacing.unit * 4,
  rowInset: spacing.unit * 2,
  footerHeight: spacing.toolbar,
  headingHeight: spacing.control,
  rowHeight: spacing.toolbar,
  rowGap: spacing.unit,
  sectionGap: spacing.unit * 2,
  recentGap: spacing.unit * 2,
  recentHeight: spacing.toolbar + spacing.unit,
  recentCardMin: 180,
  recentColumns: 3,
  recentLimit: 6,
  narrowHintsAt: 520,
  scrollGutter: editorLayout.scrollbarWidth + editorLayout.scrollbarInset + spacing.unit,
  icon: 16,
  layer: 2200,
};

// Floating chrome: every modal and the overview float the same separate pieces over the workspace
// (a search bar at a fixed height, a results panel sized to its content, hints at the bottom of the
// window) instead of one large fixed card. Selection and hover are drawn with line borders.
const floatingLayout = {
  searchHeight: spacing.toolbar,
  // Where the search bar sits, as a fraction of the window height (never closer than `minTop`).
  topFraction: 0.14,
  minTop: spacing.unit * 6,
  gap: spacing.unit * 2,
  // Smallest results panel: an empty state on one line.
  emptyHeight: spacing.toolbar + spacing.unit * 4,
  hintsHeight: spacing.control,
  hintsBottom: spacing.unit * 4,
  icon: 16,
  // JSNIR `paint.backdropBlur` (dual Kawase, niri's non-xray blur): the modal backdrop blurs the
  // whole workspace; each floating surface blurs what is under it, tinted by a translucent `cardBg`.
  backdropBlur: 24,
  // The blur carries the separation; the theme's backdrop dim is kept at half so it stays visible.
  backdropDim: 0.5,
  surfaceBlur: 16,
  saturation: 1.4,
  surfaceAlpha: 0.72,
};

// Workspace switcher: a wheel whose selected row stays at the vertical centre while the list
// slides under it; rows dim and shrink with their distance from the centre.
const workspaceSwitcherLayout = {
  width: 520,
  rowHeight: spacing.controlLg + spacing.unit * 2,
  rowGap: spacing.unit,
  // Rows beyond this distance from the centre are not drawn.
  visibleRows: 4,
  opacityStep: 0.28,
  scaleStep: 0.06,
  minScale: 0.82,
};

// "Workspace 2 / 6 · name" pill shown at the bottom after a workspace switch.
const workspacePillLayout = {
  height: spacing.control,
  bottom: spacing.unit * 4,
  dot: spacing.unit * 1.5,
  holdMs: 1200,
  layer: 2100,
};

// Overview filter bar and the per-workspace labels in the gap above each workspace.
const overviewFilterLayout = {
  width: 460,
  top: spacing.unit * 3,
  suggestionWidth: 300,
  labelHeight: spacing.unit * 5,
  // Windows that do not match the filter stay in place, dimmed.
  dimOpacity: 0.22,
  layer: 40,
};

const treeLayout = {
  rowHeight: 24,
  rowGap: 0,
  inset: spacing.unit,
  paddingBottom: spacing.unit * 2,
  indent: 10,
  maxGuides: 12,
  rowPadding: [0, 8, 0, 6] as [number, number, number, number],
  rowIcon: 14,
  iconGap: 6,
  searchPadding: [spacing.unit * 1.5, spacing.unit * 2, spacing.unit * 1.5, spacing.unit * 2] as [number, number, number, number],
  searchHeight: spacing.control,
  searchRowHeight: 34,
};

const metricsLayout = {
  errorIcon: 20,
  padding: spacing.unit * 5,
  // Content column cap, so a fullscreen window keeps charts readable instead of stretched.
  maxContentWidth: 1360,
  gap: spacing.unit * 3,
  sectionGap: spacing.unit * 4,
  scrollFadeHeight: spacing.unit * 6,
  cardPadding: spacing.unit * 4,
  cardVerticalPadding: spacing.unit * 3,
  cardHeight: 144,
  cardMinWidth: 168,
  panelMinWidth: 352,
  tableRow: 40,
  tableRowCompact: 32,
  tableNumber: 64,
  mark: 8,
  sparkHeight: 32,
  // Token volume: input grows up from the baseline, output down; a hidden series gives its height away.
  plotInputHeight: 128,
  plotOutputHeight: 80,
  plotScaleWidth: 44,
  gridSpacing: 24,
  // ECharts `barCategoryGap: 20%`: each side of a bar gives up a tenth of its slot.
  barGapFraction: 0.1,
  barFade: 0.55,
  blurOpacity: 0.4,
  pointerMix: 0.08,
  axisLabelHeight: 14,
  axisLabelWidth: 72,
  axisLabels: 5,
  heatmapCellHeight: 20,
  heatmapGap: spacing.unit / 2,
  heatmapLabelWidth: spacing.controlLg,
  tooltipWidth: 212,
  tooltipPadding: spacing.unit * 2.5,
  tooltipRow: 18,
  // ECharts' default distance from the pointer, in either direction.
  tooltipOffset: spacing.unit * 5,
  tooltipLeaveDelay: 90,
  providerHoverBrightness: 1.2,
  toolSize: 24,
  iconSize: 14,
  plotBars: 30,
  inlineBarWidth: 112,
  inlineBarHeight: 6,
  inlineBarPanelMin: 452,
  shareBarWidth: 96,
  shareColumnMin: 640,
  maxVisibleModels: 8,
  refreshingOpacity: 0.6,
};

const metricsSkeletonLayout = {
  delay: 120,
  bars: [0.38, 0.62, 0.47, 0.82, 0.56, 0.73, 0.42, 0.61],
  modelRows: 4,
};

const metricsColorMix = {
  input: 0.62,
  provider: 0.22,
  tooltip: 0.06,
  heatmap: [0, 0.28, 0.46, 0.64, 0.82, 1],
};

function metricsColors() {
  const p = palette();
  const output = p.charts?.output ?? accent().brand;
  const input = p.charts?.input ?? mixColor(p.cardBg, output, metricsColorMix.input);
  return {
    input,
    output,
    sessions: p.charts?.sessions ?? input,
    duration: p.charts?.duration ?? output,
    axis: p.charts?.axis ?? p.textMuted,
    grid: p.charts?.grid ?? p.border,
    track: p.charts?.track ?? p.kbdBg,
    tooltip: mixColor(p.cardBg, accent().brand, metricsColorMix.tooltip),
  };
}

function metricsHeatColor(level: number): number {
  const p = palette();
  const base = metricsColors();
  const color = p.charts?.heatmap?.[level];
  if (color != null) return color;
  if (level === 0) return base.track;
  return mixColor(p.cardBg, base.output, metricsColorMix.heatmap[level]);
}

function metricsProviderColor(provider: ChartProvider): number {
  const p = palette();
  const color = {
    anthropic: p.series[1],
    google: p.series[0],
    openai: p.series[2],
    xai: p.series[3],
    other: p.textMuted,
  }[provider];
  return p.charts?.providers?.[provider] ?? mixColor(color, accent().brand, metricsColorMix.provider);
}

// Accordee workspace avatar gradients (`.ws-avatar` blurred blobs), picked by name hash.
const avatarGradients: [number, number][] = [
  [0xf472b6ff, 0x94a3b8ff],
  [0xfda4afff, 0xfb7185ff],
  [0x60a5faff, 0x1d4ed8ff],
  [0xf9a8d4ff, 0x93c5fdff],
  [0xfdba74ff, 0x312e81ff],
  [0x86efacff, 0x0d9488ff],
  [0xc4b5fdff, 0x7c3aedff],
];

function avatarGradient(name: string): [number, number] {
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) >>> 0;
  return avatarGradients[h % avatarGradients.length];
}

const radii = {
  xs: 2,
  sm: 4,
  chip: 4,
  md: 6,
  lg: 8,
  popover: 10,
  base: 10,
  xl: 12,
  panel: 12,
  xxl: 16,
};

// Browser window: address row above the page.
const browserLayout = {
  barHeight: spacing.button + spacing.unit * 2,
  barPadding: [0, spacing.unit * 2, 0, spacing.unit * 2] as [number, number, number, number],
  barGap: spacing.unit,
  fieldHeight: spacing.control,
  fieldPadding: [0, spacing.unit * 2, 0, spacing.unit * 2] as [number, number, number, number],
  fieldRadius: radii.md,
  settleDelay: terminalGridLayout.settleDelay,
};

const shadows = {
  dropSm: { y: 1, blur: 2, spread: 0, color: 0x00000026 } as Shadow,
  panel: { y: 1, blur: 2, spread: 0, color: 0x0000000d } as Shadow,
  card: { y: 2, blur: 6, spread: -2, color: 0x00000014 } as Shadow,
  popover: { y: 8, blur: 24, spread: -8, color: 0x00000038 } as Shadow,
  // `shadow-xl` is two layers: 0 20px 25px -5px + 0 8px 10px -6px (alphas raised for dark surfaces).
  menu: { y: 20, blur: 25, spread: -5, color: 0x00000033 } as Shadow,
  menuTight: { y: 8, blur: 10, spread: -6, color: 0x0000001a } as Shadow,
  modal: { y: 24, blur: 50, spread: -12, color: 0x0000005c } as Shadow,
  searchModal: { y: 32, blur: 100, spread: -24, color: 0x00000059 } as Shadow,
  dragGhost: { y: 10, blur: 28, spread: -6, color: 0x00000038 } as Shadow,
};

const motion = {
  fast: 150,
  base: 200,
  enter: 240,
  enterSlow: 280,
  panel: 300,
  easeSmooth: [0.16, 1, 0.3, 1] as [number, number, number, number],
  easeOut: [0, 0, 0.2, 1] as [number, number, number, number],
  easeInOut: [0.4, 0, 0.2, 1] as [number, number, number, number],
  easeIn: [0.4, 0, 1, 1] as [number, number, number, number],
};

// The Release notes row hover is jsnircord's Missions row (release_glow.rs); its palette is the accent.
const releaseNotesLayout = {
  width: 520,
  maxHeight: 640,
  viewportInset: spacing.unit * 6,
  layer: 2000,
  padding: spacing.unit * 5,
  compactPadding: spacing.unit * 3,
  sectionGap: spacing.unit * 3,
  icon: 16,
  coverRatio: 1.85,
  coverHeightFraction: 0.42,
  coverAccentMix: 0.12,
  coverCloseInset: spacing.unit * 3,
  coverPlaySize: spacing.unit * 13,
  coverPlayIcon: 20,
  coverPlayHoverScale: 1.05,
  coverTitleSize: typeScale.heading,
  compactAt: 420,
  bulletSize: spacing.unit,
  bulletTop: spacing.unit * 2,
  mediaIconSize: spacing.button,
  // Missions row: hover fade, the inner/outer stops' darkening of the accent, and where the
  // label's ink ends (menu padding + icon + gap, then ~px per character) for the dimmer.
  glowFade: 350,
  glowInner: 0.55,
  glowOuter: 0.82,
  glowInkStart: 34,
  glowCharWidth: 7.2,
};

// niri window-open (150 ms ease-out-expo, scale 0.5 → 1) and window-close (scale 1 → 0.8);
// easeSmooth is the ease-out-expo curve. Workspaces stack with a gap of 10% of their height
// (niri monitor.rs `workspace_gap`) and the overview zooms the stack to niri's default 0.5.
const tilingMotion = {
  openScale: 0.5,
  closeScale: 0.8,
  duration: 150,
  ease: motion.easeSmooth,
  workspaceGap: 0.1,
  overviewZoom: 0.5,
};

// niri's springs (niri-config animations.rs defaults, src/animation/spring.rs): damping ratio 1
// (critically damped, no bounce), mass 1, ε 1e-4; niri ends them at -ln(ε)/ω = 9.21/√stiffness:
// 326 ms at stiffness 800 (window and view movement, resize, overview), 291 ms at 1000 (workspace
// switch). JSNIR transitions take a duration + curve, so the normalised curve
// 1 - (1 + ωt)·e^(-ωt) is fitted as a cubic-bezier (max error 2%, never overshoots). Like niri's
// keyboard path, a retarget restarts from the on-screen value at zero velocity.
const niriMotion = {
  curve: [0.29, 0.483, 0.032, 1] as [number, number, number, number],
  workspaceMs: 291,
  windowMs: 326,
};

// Compositor spring shared by the sidebar, the right panel and tiled columns, so every layout change
// (collapse, open, resize, niri scroll) settles with the same physics.
const SPRING_MS = niriMotion.windowMs;

const sidebarMotion = {
  // Shorter than the width spring: the old content is gone before the rail settles.
  fade: motion.enter,
};

function springTransition(property: Property, duration = SPRING_MS): Transition {
  return { property, duration, easing: "bezier", bezier: niriMotion.curve };
}

function workspaceTransition(property: Property): Transition {
  // niri's workspace-switch spring: stiffness 1000, critically damped.
  return springTransition(property, niriMotion.workspaceMs);
}

function smoothTransition(property: Property, duration = motion.base): Transition {
  return {
    property,
    duration,
    easing: "bezier",
    bezier: motion.easeSmooth,
  };
}

// Exact Accordee Radix/shadcn menu & popover entrance:
// animate-in fade-in-0 zoom-in-95 slide-in-from-<side>-2 (8px), 200ms --ease-smooth.
function accordeeMenuMotion(side: "top" | "bottom" | "left" | "right" = "bottom"): {
  starting: Starting;
  transitions: Transition[];
} {
  const starting: Starting =
    side === "top"
      ? { opacity: 0, scale: 0.95, y: 8 }
      : side === "left"
        ? { opacity: 0, scale: 0.95, x: 8 }
        : side === "right"
          ? { opacity: 0, scale: 0.95, x: -8 }
          : { opacity: 0, scale: 0.95, y: -8 };
  return {
    starting,
    transitions: [
      smoothTransition("opacity", motion.base),
      smoothTransition("scale", motion.base),
      smoothTransition("x", motion.base),
      smoothTransition("y", motion.base),
    ],
  };
}

// Exact Accordee dialog/modal entrance:
// fade-in-0 zoom-in-[0.98] y: 8->0, 300ms (--motion-panel) --ease-smooth.
function accordeeDialogMotion(): {
  starting: Starting;
  transitions: Transition[];
} {
  return {
    starting: { opacity: 0, scale: 0.98, y: 8 },
    transitions: [
      smoothTransition("opacity", motion.panel),
      smoothTransition("scale", motion.panel),
      smoothTransition("y", motion.panel),
    ],
  };
}

// Exact Accordee tooltip entrance: fade-in-0 zoom-in-95 slide-in 4px from the anchor, 150ms (--motion-fast).
function accordeeTooltipMotion(side: Side = "top"): {
  starting: Starting;
  transitions: Transition[];
} {
  const offset = 4;
  const starting: Starting =
    side === "top"
      ? { opacity: 0, scale: 0.95, y: offset }
      : side === "bottom"
        ? { opacity: 0, scale: 0.95, y: -offset }
        : side === "right"
          ? { opacity: 0, scale: 0.95, x: -offset }
          : { opacity: 0, scale: 0.95, x: offset };
  return {
    starting,
    transitions: [
      smoothTransition("opacity", motion.fast),
      smoothTransition("scale", motion.fast),
      smoothTransition("x", motion.fast),
      smoothTransition("y", motion.fast),
    ],
  };
}

// Accordee panel/card fade-in-up entrance: opacity 0->1, y 6->0, 240ms (--motion-enter).
function accordeeFadeUpMotion(): {
  starting: Starting;
  transitions: Transition[];
} {
  return {
    starting: { opacity: 0, y: 6 },
    transitions: [
      smoothTransition("opacity", motion.enter),
      smoothTransition("y", motion.enter),
    ],
  };
}

const themeMode = signal<"dark" | "light">("dark");
const themeAccent = signal<AccentName>("blue");
const activeThemeId = signal<string>("accordee-dark");
const themesList = signal<ResolvedThemeItem[]>([]);
const themesDirPath = signal<string>("~/.config/icy/themes");
const densityMode = signal<DensityMode>("comfortable");
const terminalFontSize = signal<number>(13);

function rowControlHeight(): number {
  return densityMode.value === "compact" ? 26 : 30;
}

function palette(): ThemePalette {
  const base = themeMode.value === "dark" ? darkPalette : lightPalette;
  const custom = themesList.value.find((t) => t.id === activeThemeId.value);
  if (!custom) {
    return base;
  }
  return {
    ...base,
    appBg: custom.colors.appBg,
    sidebarBg: custom.colors.sidebarBg,
    sidebarActive: custom.colors.sidebarActive,
    panelBg: custom.colors.panelBg,
    cardBg: custom.colors.cardBg,
    kbdBg: custom.colors.kbdBg,
    textPrimary: custom.colors.textPrimary,
    textSecondary: custom.colors.textSecondary,
    textMuted: custom.colors.textMuted,
    border: custom.colors.border,
    highlight: custom.colors.highlight,
    charts: custom.colors.charts,
    graphLanes: custom.colors.graph?.length ? custom.colors.graph : base.graphLanes,
  };
}

// Text selection: the theme's own color when it sets one, otherwise the accent, so it follows
// the accent picker like every other highlight.
function selectionFill(): number {
  const custom = themesList.value.find((t) => t.id === activeThemeId.value)?.colors.selection;
  return custom ?? withAlpha(accent().brand, editorLayout.selectionMix);
}

function accent(): AccentPalette {
  return themeMode.value === "dark"
    ? accentsDark[themeAccent.value]
    : accentsLight[themeAccent.value];
}
