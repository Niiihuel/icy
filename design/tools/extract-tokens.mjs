#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");
const rawDir = path.join(root, "design/reference/accordee/raw");

const cssFiles = fs
  .readdirSync(rawDir)
  .filter((f) => f.endsWith(".css"))
  .sort();

const allCss = cssFiles.map((f) => fs.readFileSync(path.join(rawDir, f), "utf8")).join("\n");

// ---------- Color math (sRGB <-> Linear sRGB <-> OKLab) ----------
function srgbToLinear(c) {
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}
function linearToSrgb(c) {
  c = Math.max(0, Math.min(1, c));
  return c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055;
}
function rgbToOklab(r, g, b) {
  const lr = srgbToLinear(r);
  const lg = srgbToLinear(g);
  const lb = srgbToLinear(b);
  const l = 0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb;
  const m = 0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb;
  const s = 0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb;
  const l_ = Math.cbrt(l);
  const m_ = Math.cbrt(m);
  const s_ = Math.cbrt(s);
  return [
    0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_,
    1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_,
    0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_,
  ];
}
function oklabToRgb(L, a, b) {
  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;
  const l = l_ * l_ * l_;
  const m = m_ * m_ * m_;
  const s = s_ * s_ * s_;
  const lr = +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
  const lg = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
  const lb = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;
  return [linearToSrgb(lr), linearToSrgb(lg), linearToSrgb(lb)];
}
function parseHex(hex) {
  let h = hex.trim().replace(/^#/, "");
  if (h.length === 3) h = h[0] + h[0] + h[1] + h[1] + h[2] + h[2] + "ff";
  else if (h.length === 4) h = h[0] + h[0] + h[1] + h[1] + h[2] + h[2] + h[3] + h[3];
  else if (h.length === 6) h = h + "ff";
  const r = parseInt(h.slice(0, 2), 16) / 255;
  const g = parseInt(h.slice(2, 4), 16) / 255;
  const b = parseInt(h.slice(4, 6), 16) / 255;
  const a = parseInt(h.slice(6, 8), 16) / 255;
  return [r, g, b, a];
}
function mixOklab(rgba1, p1, rgba2) {
  const w1 = p1 / 100;
  const w2 = 1 - w1;
  const a1 = rgba1[3];
  const a2 = rgba2[3];
  const aOut = w1 * a1 + w2 * a2;
  if (aOut === 0) return [0, 0, 0, 0];
  if (a2 === 0) {
    // Mix with transparent preserves source color and scales alpha.
    return [rgba1[0], rgba1[1], rgba1[2], w1 * a1];
  }
  const lab1 = rgbToOklab(rgba1[0], rgba1[1], rgba1[2]);
  const lab2 = rgbToOklab(rgba2[0], rgba2[1], rgba2[2]);
  // CSS Color 4 premultiplied interpolation in OKLab
  const L = (lab1[0] * a1 * w1 + lab2[0] * a2 * w2) / aOut;
  const a = (lab1[1] * a1 * w1 + lab2[1] * a2 * w2) / aOut;
  const b = (lab1[2] * a1 * w1 + lab2[2] * a2 * w2) / aOut;
  const [r, g, bOut] = oklabToRgb(L, a, b);
  return [r, g, bOut, aOut];
}
function toJsnirHex([r, g, b, a]) {
  const b8 = (v) => Math.max(0, Math.min(255, Math.round(v * 255))).toString(16).padStart(2, "0");
  return `0x${b8(r)}${b8(g)}${b8(b)}${b8(a)}`;
}

// ---------- Extract literal CSS blocks ----------
function extractMatchingBlocks(css, selectorFilter) {
  const blocks = [];
  const re = /([^{}@;]+)\{([^{}]+)\}/g;
  let m;
  while ((m = re.exec(css)) !== null) {
    const sel = m[1].trim();
    const body = m[2].trim();
    if (!body.includes("--")) continue;
    if (selectorFilter(sel, body)) {
      const props = body
        .split(";")
        .map((s) => s.trim())
        .filter(Boolean)
        .filter((s) => !s.startsWith("--tw-") && !/^--color-(red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose|slate|gray|zinc|neutral|stone)-/.test(s));
      if (props.length) {
        blocks.push({ sel, props });
      }
    }
  }
  return blocks;
}

const rootHostBlocks = extractMatchingBlocks(allCss, (sel) => sel === ":root,:host" || sel === ":root");
const darkBlocks = extractMatchingBlocks(allCss, (sel) => sel === ".dark" || sel === "html.dark");
const accentBlocks = extractMatchingBlocks(allCss, (sel) => sel.includes("data-accent"));
const fontBlocks = extractMatchingBlocks(allCss, (sel) => sel.startsWith("html[data-font") || sel.startsWith(".") && sel.includes("variable"));
const motionBlocks = extractMatchingBlocks(allCss, (sel, body) => (body.includes("--motion-") && sel !== ":root") || sel.includes("data-density") || sel.includes("data-layout"));

const keyframes = [...allCss.matchAll(/@keyframes\s+[^{]+\{(?:[^{}]|\{[^}]*\})*\}/g)].map((m) => m[0]);

function formatBlock({ sel, props }) {
  return `${sel} {\n  ${props.join(";\n  ")};\n}`;
}

const outCss = [
  "/* ============================================================================",
  " * Accordee Design System Tokens (verbatim custom properties & keyframes)",
  " * Source: https://demo.accordee.app/",
  " * Generated by design/tools/extract-tokens.mjs from raw CSS chunks.",
  " * ============================================================================ */",
  "",
  "/* --- Typography, Radii, Spacing, Motion, Semantic Mapping (:root) --- */",
  ...rootHostBlocks.map(formatBlock),
  "",
  "/* --- Dark Theme (.dark / html.dark) --- */",
  ...darkBlocks.map(formatBlock),
  "",
  "/* --- Accent Themes (html[data-accent=...]) --- */",
  ...accentBlocks.map(formatBlock),
  "",
  "/* --- Font & Layout / Density Overrides --- */",
  ...fontBlocks.map(formatBlock),
  ...motionBlocks.map(formatBlock),
  "",
  "/* --- Keyframes --- */",
  ...keyframes,
  "",
].join("\n");

fs.writeFileSync(path.join(root, "design/accordee-tokens.css"), outCss, "utf8");
console.log("Wrote design/accordee-tokens.css (" + outCss.length + " bytes, " + keyframes.length + " keyframes)");

// ---------- Resolve color tokens for app/theme.ts ----------
const lightVars = {
  appBg: "#ffffff",
  panelBg: "#f4f4f5",
  cardBg: "#ffffff",
  kbdBg: "#e5e7eb",
  textPrimary: "#09090b",
  textSecondary: "#52525b",
  textMuted: "#71717a",
  textSubtle: "#3f3f46",
  border: "#0000001a",
  sidebarBg: "#fafafa",
  sidebarActive: "#f0f0f1",
  sendIdle: "#8e8e93",
  brand: "#09090b",
  brandFg: "#ffffff",
  brandText: "#09090b",
  brandSoft: "#0000000f",
  highlight: "#3b82f6",
  destructive: "#ef4444",
  ring: "#a1a1aa",
};
const darkVars = {
  appBg: "#0a0a0a",
  panelBg: "#171717",
  cardBg: "#171717",
  kbdBg: "#262626",
  textPrimary: "#fafafa",
  textSecondary: "#a3a3a3",
  textMuted: "#8a8a8a",
  textSubtle: "#e5e5e5",
  border: "#ffffff1a",
  sidebarBg: "#0f0f10",
  sidebarActive: "#1f1f22",
  sendIdle: "#3f3f46",
  brand: "#fafafa",
  brandFg: "#0a0a0a",
  brandText: "#fafafa",
  brandSoft: "#ffffff14",
  highlight: "#3b82f6",
  destructive: "#f87171",
  ring: "#52525b",
};

const accentsLight = {
  neutral: { brand: "#09090b", brandFg: "#ffffff", brandText: "#09090b", brandSoft: "#0000000f" },
  blue: { brand: "#2563eb", brandFg: "#ffffff", brandText: "#1d4ed8" },
  violet: { brand: "#7c3aed", brandFg: "#ffffff", brandText: "#6d28d9" },
  rose: { brand: "#e11d48", brandFg: "#ffffff", brandText: "#be123c" },
  orange: { brand: "#ea580c", brandFg: "#ffffff", brandText: "#c2410c" },
  green: { brand: "#16a34a", brandFg: "#ffffff", brandText: "#15803d" },
  teal: { brand: "#0d9488", brandFg: "#ffffff", brandText: "#0f766e" },
};
const accentsDark = {
  neutral: { brand: "#fafafa", brandFg: "#0a0a0a", brandText: "#fafafa", brandSoft: "#ffffff14" },
  blue: { brand: "#3b82f6", brandFg: "#ffffff", brandText: "#60a5fa" },
  violet: { brand: "#8b5cf6", brandFg: "#ffffff", brandText: "#a78bfa" },
  rose: { brand: "#f43f5e", brandFg: "#ffffff", brandText: "#fb7185" },
  orange: { brand: "#f97316", brandFg: "#ffffff", brandText: "#fb923c" },
  green: { brand: "#16a34a", brandFg: "#ffffff", brandText: "#4ade80" },
  teal: { brand: "#14b8a6", brandFg: "#ffffff", brandText: "#2dd4bf" },
};

console.log("\n=== Resolved Light ===");
for (const [k, v] of Object.entries(lightVars)) console.log(k, v, "->", toJsnirHex(parseHex(v)));
console.log("\n=== Resolved Dark ===");
for (const [k, v] of Object.entries(darkVars)) console.log(k, v, "->", toJsnirHex(parseHex(v)));

console.log("\n=== Resolved Accents (Dark with 20% OKLab soft) ===");
for (const [name, obj] of Object.entries(accentsDark)) {
  const brandRgba = parseHex(obj.brand);
  const softRgba = obj.brandSoft ? parseHex(obj.brandSoft) : mixOklab(brandRgba, 20, [0, 0, 0, 0]);
  const ringRgba = name === "neutral" ? parseHex("#52525b") : mixOklab(brandRgba, 55, [0, 0, 0, 0]);
  console.log(name, {
    brand: toJsnirHex(brandRgba),
    brandFg: toJsnirHex(parseHex(obj.brandFg)),
    brandText: toJsnirHex(parseHex(obj.brandText)),
    brandSoft: toJsnirHex(softRgba),
    ring: toJsnirHex(ringRgba),
  });
}

console.log("\n=== Resolved Accents (Light with 12% OKLab soft) ===");
for (const [name, obj] of Object.entries(accentsLight)) {
  const brandRgba = parseHex(obj.brand);
  const softRgba = obj.brandSoft ? parseHex(obj.brandSoft) : mixOklab(brandRgba, 12, [0, 0, 0, 0]);
  const ringRgba = name === "neutral" ? parseHex("#a1a1aa") : mixOklab(brandRgba, 55, [0, 0, 0, 0]);
  console.log(name, {
    brand: toJsnirHex(brandRgba),
    brandFg: toJsnirHex(parseHex(obj.brandFg)),
    brandText: toJsnirHex(parseHex(obj.brandText)),
    brandSoft: toJsnirHex(softRgba),
    ring: toJsnirHex(ringRgba),
  });
}
