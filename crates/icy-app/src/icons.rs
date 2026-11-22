//! Accordee-style outline icons and CLI usage rings rasterized with `resvg` at device scale.
use resvg::{tiny_skia, usvg};

pub type Picture = (String, Vec<u8>, [u32; 2]);

fn pixels(logical: u32, scale: f32) -> u32 {
    (logical as f32 * scale).round().max(1.) as u32
}

fn rasterize_svg(
    svg: &str,
    logical: u32,
    scale: f32,
    monochrome_white: bool,
) -> Option<(Vec<u8>, [u32; 2])> {
    let tree = usvg::Tree::from_data(svg.as_bytes(), &usvg::Options::default()).ok()?;
    let side = pixels(logical, scale);
    let mut pixmap = tiny_skia::Pixmap::new(side, side)?;
    let zoom = side as f32 / tree.size().width();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(zoom, zoom),
        &mut pixmap.as_mut(),
    );
    let mut rgba = pixmap.take();
    if monochrome_white {
        for px in rgba.as_chunks_mut::<4>().0 {
            px[..3].fill(255);
        }
    } else {
        // tiny-skia produces premultiplied RGBA; convert to straight RGBA8 for JSNIR.
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
    Some((rgba, [side, side]))
}

/// Crisp Lucide/Accordee 1.5px stroke icons (24x24 viewBox, tinted in TSX via `paint.color`).
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

/// Returns inner SVG elements (positioned inside a 28x28 ring viewBox) for each CLI brand.
fn cli_inner_svg(id: &str) -> &'static str {
    match id {
        "claude" => {
            // Coral sunburst asterisk
            r##"<g stroke="#e07a5f" stroke-width="1.65" stroke-linecap="round"><line x1="14" y1="8.2" x2="14" y2="19.8"/><line x1="8.2" y1="14" x2="19.8" y2="14"/><line x1="9.9" y1="9.9" x2="18.1" y2="18.1"/><line x1="18.1" y1="9.9" x2="9.9" y2="18.1"/></g>"##
        }
        "antigravity" => {
            // Blue sleek A-delta mark
            r##"<path d="M14 8.2 18.8 19.2h-2.3l-2.5-6-2.5 6H9.2L14 8.2Z" fill="#60a5fa"/>"##
        }
        "codex" => {
            // OpenAI-inspired geometric hex knot
            r##"<g fill="none" stroke="#e4e4e7" stroke-width="1.45" stroke-linejoin="round"><polygon points="14,8.2 19,11.1 19,16.9 14,19.8 9,16.9 9,11.1"/><circle cx="14" cy="14" r="2.2"/></g>"##
        }
        "grok" => {
            // Slashed orbit mark
            r##"<g fill="none" stroke="#d4d4d8" stroke-width="1.55" stroke-linecap="round"><circle cx="14" cy="14" r="4.6"/><line x1="10.2" y1="18.2" x2="18.2" y2="9.8"/></g>"##
        }
        "junie" => {
            // Emerald 4-point star
            r##"<path d="M14 8.2 15.6 12.4 19.8 14 15.6 15.6 14 19.8 12.4 15.6 8.2 14 12.4 12.4Z" fill="#4ade80"/>"##
        }
        "opencode" => {
            // Minimal framed terminal block
            r##"<g fill="none" stroke="#a78bfa" stroke-width="1.6"><rect x="9.5" y="9" width="9" height="10" rx="1.6"/><rect x="12" y="11.5" width="4" height="5" fill="#a78bfa" stroke="none"/></g>"##
        }
        _ => {
            // Generic terminal prompt
            r##"<g fill="none" stroke="#a3a3a3" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><polyline points="9.5,11 12.5,14 9.5,17"/><line x1="14" y1="17" x2="18.5" y2="17"/></g>"##
        }
    }
}

/// Generates a 28x28 circular progress ring SVG with centered CLI icon.
pub fn rasterize_cli_ring(id: &str, color_hex: &str, percent: f32, scale: f32) -> Option<Picture> {
    let radius = 12.0_f32;
    let circumference = 2.0 * std::f32::consts::PI * radius;
    let clamped = percent.clamp(0.0, 100.0);
    // Show a subtle minimum arc when > 0% so active usage is always readable.
    let visible_pct = if clamped > 0.0 && clamped < 4.0 {
        4.0
    } else {
        clamped
    };
    let filled = circumference * (visible_pct / 100.0);
    let inner = cli_inner_svg(id);

    let arc = if visible_pct > 0.0 {
        format!(
            r##"<circle cx="14" cy="14" r="{radius}" fill="none" stroke="{color_hex}" stroke-width="1.9" stroke-linecap="round" stroke-dasharray="{filled:.2} {circumference:.2}" transform="rotate(-90 14 14)"/>"##
        )
    } else {
        String::new()
    };

    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 28 28"><circle cx="14" cy="14" r="{radius}" fill="none" stroke="#71717a" stroke-opacity="0.28" stroke-width="1.9"/>{arc}{inner}</svg>"##
    );
    let (rgba, size) = rasterize_svg(&svg, 28, scale, false)?;
    Some((format!("ring:{id}"), rgba, size))
}

/// Generates a 15x15 standalone CLI icon for session tabs and sidebar rows.
fn rasterize_cli_badge(id: &str, scale: f32) -> Option<Picture> {
    let inner = cli_inner_svg(id);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="5 5 18 18">{inner}</svg>"#
    );
    let (rgba, size) = rasterize_svg(&svg, 15, scale, false)?;
    Some((format!("cli:{id}"), rgba, size))
}

/// Rasterizes all static UI glyphs, brand marks, and standalone CLI badges at `scale`.
pub fn load_static(scale: f32) -> Vec<Picture> {
    let mut out = Vec::with_capacity(UI_GLYPHS.len() + COLOR_MARKS.len() + 7);
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
        "terminal",
    ] {
        if let Some(pic) = rasterize_cli_badge(id, scale) {
            out.push(pic);
        }
    }
    out
}
