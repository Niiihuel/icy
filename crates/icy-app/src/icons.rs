//! Accordee-style outline icons, official SVGL CLI logos, usage rings, and updater progress ring
//! rasterized with `resvg` at device scale.
use resvg::{tiny_skia, usvg};

pub type Picture = (String, Vec<u8>, [u32; 2]);

// Official SVGL CLI & platform logos embedded at compile time.
const SVG_CLAUDE: &[u8] = include_bytes!("../../../assets/icons/clis/claude.svg");
const SVG_ANTIGRAVITY: &[u8] = include_bytes!("../../../assets/icons/clis/antigravity.svg");
const SVG_CODEX: &[u8] = include_bytes!("../../../assets/icons/clis/codex.svg");
const SVG_GROK: &[u8] = include_bytes!("../../../assets/icons/clis/grok.svg");
const SVG_JUNIE: &[u8] = include_bytes!("../../../assets/icons/clis/junie.svg");
const SVG_OPENCODE: &[u8] = include_bytes!("../../../assets/icons/clis/opencode.svg");
const SVG_GEMINI: &[u8] = include_bytes!("../../../assets/icons/clis/gemini.svg");
const SVG_GITHUB: &[u8] = include_bytes!("../../../assets/icons/clis/github.svg");
const SVG_TERMINAL_FALLBACK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#a3a3a3" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="5 17 11 11 5 5"/><line x1="12" x2="19" y1="19" y2="19"/></svg>"##;

fn pixels(logical: u32, scale: f32) -> u32 {
    (logical as f32 * scale).round().max(1.) as u32
}

fn unpremultiply_rgba(rgba: &mut [u8]) {
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = px[3];
        if a > 0 && a < 255 {
            let af = a as f32 / 255.0;
            px[0] = ((px[0] as f32 / af).round() as u32).min(255) as u8;
            px[1] = ((px[1] as f32 / af).round() as u32).min(255) as u8;
            px[2] = ((px[2] as f32 / af).round() as u32).min(255) as u8;
        }
    }
}

fn render_svg_centered(
    svg_bytes: &[u8],
    pixmap: &mut tiny_skia::Pixmap,
    box_offset: f32,
    box_size: f32,
) -> bool {
    let Ok(tree) = usvg::Tree::from_data(svg_bytes, &usvg::Options::default()) else {
        return false;
    };
    let size = tree.size();
    let w = size.width().max(1.0);
    let h = size.height().max(1.0);
    let fit = (box_size / w).min(box_size / h);
    let dx = box_offset + (box_size - w * fit) * 0.5;
    let dy = box_offset + (box_size - h * fit) * 0.5;
    let transform = tiny_skia::Transform::from_scale(fit, fit).post_translate(dx, dy);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    true
}

fn rasterize_svg(
    svg: &str,
    logical: u32,
    scale: f32,
    monochrome_white: bool,
) -> Option<(Vec<u8>, [u32; 2])> {
    let side = pixels(logical, scale);
    let mut pixmap = tiny_skia::Pixmap::new(side, side)?;
    if !render_svg_centered(svg.as_bytes(), &mut pixmap, 0.0, side as f32) {
        return None;
    }
    let mut rgba = pixmap.take();
    if monochrome_white {
        for px in rgba.as_chunks_mut::<4>().0 {
            px[..3].fill(255);
        }
    } else {
        unpremultiply_rgba(&mut rgba);
    }
    Some((rgba, [side, side]))
}

/// Crisp Lucide/Accordee outline icons (24x24 viewBox, tinted in TSX via `paint.color`).
const UI_GLYPHS: &[(&str, &str, u32)] = &[
    (
        "search",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>"#,
        14,
    ),
    (
        "plus",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5v14"/><path d="M5 12h14"/></svg>"#,
        15,
    ),
    (
        "terminal",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 17 10 11 4 5"/><line x1="12" x2="20" y1="19" y2="19"/></svg>"#,
        15,
    ),
    (
        "dashboard",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="12" x="3" y="4" rx="2"/><line x1="8" x2="16" y1="20" y2="20"/><line x1="12" x2="12" y1="16" y2="20"/></svg>"#,
        15,
    ),
    (
        "inbox",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/></svg>"#,
        15,
    ),
    (
        "tasks",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="3"/><path d="m9 12 2 2 4-4"/></svg>"#,
        15,
    ),
    (
        "folder",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"#,
        15,
    ),
    (
        "file-code",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><path d="m10 13-2 2 2 2"/><path d="m14 17 2-2-2-2"/></svg>"#,
        15,
    ),
    (
        "globe",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3a14 14 0 0 1 0 18"/><path d="M12 3a14 14 0 0 0 0 18"/></svg>"#,
        15,
    ),
    (
        "palette",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r="1" fill="white"/><circle cx="17.5" cy="10.5" r="1" fill="white"/><circle cx="8.5" cy="7.5" r="1" fill="white"/><circle cx="6.5" cy="12.5" r="1" fill="white"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.93 0 1.5-.75 1.5-1.5 0-.39-.15-.74-.39-1.01-.23-.26-.38-.61-.38-.99 0-.83.67-1.5 1.5-1.5H16c3.31 0 6-2.69 6-6 0-4.96-4.49-9-10-9z"/></svg>"#,
        15,
    ),
    (
        "settings",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h.01a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v.01a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>"#,
        15,
    ),
    (
        "panel-left",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/></svg>"#,
        15,
    ),
    (
        "chevrons-up-down",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/></svg>"#,
        13,
    ),
    (
        "chevron-down",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>"#,
        12,
    ),
    (
        "chevron-left",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>"#,
        13,
    ),
    (
        "chevron-right",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>"#,
        13,
    ),
    (
        "close",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>"#,
        12,
    ),
    (
        "archive",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/></svg>"#,
        14,
    ),
    (
        "download",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/></svg>"#,
        14,
    ),
    (
        "refresh",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 16h5v5"/></svg>"#,
        14,
    ),
    (
        "play",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="white" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><polygon points="6 4 20 12 6 20 6 4"/></svg>"#,
        14,
    ),
    (
        "columns",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M12 3v18"/></svg>"#,
        14,
    ),
    (
        "split",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M3 12h18"/></svg>"#,
        14,
    ),
    (
        "maximize",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 3 21 3 21 9"/><polyline points="9 21 3 21 3 15"/><line x1="21" x2="14" y1="3" y2="10"/><line x1="3" x2="10" y1="21" y2="14"/></svg>"#,
        13,
    ),
    (
        "trash",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/></svg>"#,
        14,
    ),
    (
        "check",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>"#,
        13,
    ),
    (
        "sparkles",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="m12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3Z"/></svg>"#,
        14,
    ),
    (
        "alert",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><line x1="12" x2="12" y1="9" y2="13"/><line x1="12" x2="12.01" y1="17" y2="17"/></svg>"#,
        16,
    ),
    (
        "external-link",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/></svg>"#,
        13,
    ),
    (
        "git-branch",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><line x1="6" x2="6" y1="3" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/></svg>"#,
        13,
    ),
    (
        "key",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="7.5" cy="15.5" r="4.5"/><path d="m21 2-9.6 9.6"/><path d="m15.5 7.5 3 3L22 7l-3-3"/></svg>"#,
        14,
    ),
];

/// Colorful Accordee-style workspace & Agents marks.
const COLOR_MARKS: &[(&str, &str, u32)] = &[
    (
        "brand:icy",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#f472b6"/><stop offset="50%" stop-color="#8b5cf6"/><stop offset="100%" stop-color="#38bdf8"/></linearGradient></defs><rect x="1" y="1" width="22" height="22" rx="6" fill="url(#g)"/><path d="M8 8.5v7M11.5 10l4.5 2-4.5 2" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none"/></svg>"##,
        20,
    ),
    (
        "brand:agents",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke-width="2.4" stroke-linecap="round"><line x1="12" y1="3.5" x2="12" y2="8.5" stroke="#f97316"/><line x1="12" y1="15.5" x2="12" y2="20.5" stroke="#3b82f6"/><line x1="3.5" y1="12" x2="8.5" y2="12" stroke="#ec4899"/><line x1="15.5" y1="12" x2="20.5" y2="12" stroke="#14b8a6"/><line x1="6" y1="6" x2="9.3" y2="9.3" stroke="#eab308"/><line x1="14.7" y1="14.7" x2="18" y2="18" stroke="#8b5cf6"/><line x1="18" y1="6" x2="14.7" y2="9.3" stroke="#22c55e"/><line x1="9.3" y1="14.7" x2="6" y2="18" stroke="#f43f5e"/></svg>"##,
        16,
    ),
];

fn cli_svg_bytes(id: &str) -> &'static [u8] {
    match id {
        "claude" => SVG_CLAUDE,
        "antigravity" => SVG_ANTIGRAVITY,
        "codex" => SVG_CODEX,
        "grok" => SVG_GROK,
        "junie" => SVG_JUNIE,
        "opencode" => SVG_OPENCODE,
        "gemini" => SVG_GEMINI,
        "github" => SVG_GITHUB,
        _ => SVG_TERMINAL_FALLBACK,
    }
}

/// Generates a 28x28 circular progress ring composited with the official SVGL CLI logo centered inside.
pub fn rasterize_cli_ring(id: &str, color_hex: &str, percent: f32, scale: f32) -> Option<Picture> {
    let radius = 12.0_f32;
    let circumference = 2.0 * std::f32::consts::PI * radius;
    let clamped = percent.clamp(0.0, 100.0);
    let visible_pct = if clamped > 0.0 && clamped < 4.0 {
        4.0
    } else {
        clamped
    };
    let filled = circumference * (visible_pct / 100.0);

    let arc = if visible_pct > 0.0 {
        format!(
            r##"<circle cx="14" cy="14" r="{radius}" fill="none" stroke="{color_hex}" stroke-width="1.9" stroke-linecap="round" stroke-dasharray="{filled:.2} {circumference:.2}" transform="rotate(-90 14 14)"/>"##
        )
    } else {
        String::new()
    };

    let ring_svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 28 28"><circle cx="14" cy="14" r="{radius}" fill="none" stroke="#71717a" stroke-opacity="0.28" stroke-width="1.9"/>{arc}</svg>"##
    );

    let side = pixels(28, scale);
    let mut pixmap = tiny_skia::Pixmap::new(side, side)?;
    render_svg_centered(ring_svg.as_bytes(), &mut pixmap, 0.0, side as f32);

    // Composite the official SVGL logo neatly centered inside the ring (12.5px logical box).
    let inner_size = side as f32 * (12.5 / 28.0);
    let inner_offset = (side as f32 - inner_size) * 0.5;
    render_svg_centered(cli_svg_bytes(id), &mut pixmap, inner_offset, inner_size);

    let mut rgba = pixmap.take();
    unpremultiply_rgba(&mut rgba);
    Some((format!("ring:{id}"), rgba, [side, side]))
}

/// Generates a 16x16 standalone official SVGL logo for session tabs, sidebar rows, settings & metrics.
fn rasterize_cli_badge(id: &str, scale: f32) -> Option<Picture> {
    let side = pixels(16, scale);
    let mut pixmap = tiny_skia::Pixmap::new(side, side)?;
    let pad = side as f32 * 0.08;
    render_svg_centered(cli_svg_bytes(id), &mut pixmap, pad, side as f32 - pad * 2.0);
    let mut rgba = pixmap.take();
    unpremultiply_rgba(&mut rgba);
    Some((format!("cli:{id}"), rgba, [side, side]))
}

/// Generates the 26x26 header-left download button icon with a live circular progress ring (`0..100%`).
pub fn rasterize_update_ring(
    percent: u8,
    has_update: bool,
    active: bool,
    ready: bool,
    scale: f32,
) -> Option<Picture> {
    let radius = 11.0_f32;
    let circumference = 2.0 * std::f32::consts::PI * radius;
    let pct = if ready {
        100.0_f32
    } else if active {
        (percent as f32).clamp(6.0, 100.0)
    } else if has_update {
        28.0_f32
    } else {
        0.0_f32
    };
    let filled = circumference * (pct / 100.0);

    let ring_color = if ready {
        "#22c55e"
    } else if active || has_update {
        "#6366f1"
    } else {
        "#71717a"
    };
    let icon_color = if ready {
        "#22c55e"
    } else if active || has_update {
        "#e0e7ff"
    } else {
        "#a3a3a3"
    };

    let arc = if pct > 0.0 {
        format!(
            r##"<circle cx="13" cy="13" r="{radius}" fill="none" stroke="{ring_color}" stroke-width="1.9" stroke-linecap="round" stroke-dasharray="{filled:.2} {circumference:.2}" transform="rotate(-90 13 13)"/>"##
        )
    } else {
        String::new()
    };

    let center_glyph = if ready {
        // Ready-to-restart checkmark
        format!(
            r##"<polyline points="9.2,13.2 11.9,15.8 17.0,10.5" fill="none" stroke="{icon_color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"##
        )
    } else {
        // Crisp download arrow
        format!(
            r##"<g fill="none" stroke="{icon_color}" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><line x1="13" y1="8.3" x2="13" y2="15.3"/><polyline points="9.8,12.5 13,15.7 16.2,12.5"/><line x1="9.6" y1="17.8" x2="16.4" y2="17.8"/></g>"##
        )
    };

    let badge_dot = if has_update && !active && !ready {
        r##"<circle cx="21.5" cy="4.5" r="2.6" fill="#6366f1"/>"##
    } else {
        ""
    };

    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="26" height="26" viewBox="0 0 26 26"><circle cx="13" cy="13" r="{radius}" fill="none" stroke="#71717a" stroke-opacity="0.28" stroke-width="1.9"/>{arc}{center_glyph}{badge_dot}</svg>"##
    );

    let (rgba, size) = rasterize_svg(&svg, 26, scale, false)?;
    Some(("update:ring".to_string(), rgba, size))
}

/// Rasterizes all static UI glyphs, brand marks, standalone SVGL CLI badges, and initial update ring.
pub fn load_static(scale: f32) -> Vec<Picture> {
    let mut out = Vec::with_capacity(UI_GLYPHS.len() + COLOR_MARKS.len() + 10);
    for &(name, svg, logical) in UI_GLYPHS {
        if let Some((rgba, size)) = rasterize_svg(svg, logical, scale, true) {
            out.push((format!("glyph:{name}"), rgba, size));
        }
    }
    for &(name, svg, logical) in COLOR_MARKS {
        if let Some((rgba, size)) = rasterize_svg(svg, logical, scale, false) {
            out.push((name.to_string(), rgba, size));
        }
    }
    for id in [
        "claude",
        "antigravity",
        "codex",
        "grok",
        "junie",
        "opencode",
        "gemini",
        "github",
        "terminal",
    ] {
        if let Some(pic) = rasterize_cli_badge(id, scale) {
            out.push(pic);
        }
    }
    if let Some(pic) = rasterize_update_ring(0, false, false, false, scale) {
        out.push(pic);
    }
    out
}
