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
  updateBannerBg: number;
  updateBannerEdge: number;
  updateBannerAccent: number;
  mediaGradient: [number, number];
}

type AccentName = "neutral" | "blue" | "violet" | "rose" | "orange" | "green" | "teal";
type DensityMode = "comfortable" | "compact";

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
  updateBannerBg: 0x6366f11c,
  updateBannerEdge: 0x6366f148,
  updateBannerAccent: 0x818cf8ff,
  mediaGradient: [0x1e1b4bff, 0x0f172aff],
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
  updateBannerBg: 0x6366f114,
  updateBannerEdge: 0x6366f138,
  updateBannerAccent: 0x4f46e5ff,
  mediaGradient: [0x312e81ff, 0x1e293bff],
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
  medium: 440,
  semibold: 440,
  bold: 440,
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
  sidebarMenuWidth: 180,
  sidebarWidth: 224,
  paneGap: 8,
};

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

const shadows = {
  dropSm: { y: 1, blur: 2, spread: 0, color: 0x00000026 } as Shadow,
  panel: { y: 1, blur: 2, spread: 0, color: 0x0000000d } as Shadow,
  card: { y: 2, blur: 6, spread: -2, color: 0x00000014 } as Shadow,
  popover: { y: 8, blur: 24, spread: -8, color: 0x00000038 } as Shadow,
  modal: { y: 24, blur: 50, spread: -12, color: 0x0000005c } as Shadow,
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

// Exact Accordee tooltip entrance: fade-in-0 zoom-in-95 slide-in 6px, 150ms (--motion-fast).
function accordeeTooltipMotion(side: "top" | "bottom" = "top"): {
  starting: Starting;
  transitions: Transition[];
} {
  return {
    starting: { opacity: 0, scale: 0.95, y: side === "top" ? 6 : -6 },
    transitions: [
      smoothTransition("opacity", motion.fast),
      smoothTransition("scale", motion.fast),
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
  };
}

function accent(): AccentPalette {
  return themeMode.value === "dark"
    ? accentsDark[themeAccent.value]
    : accentsLight[themeAccent.value];
}
