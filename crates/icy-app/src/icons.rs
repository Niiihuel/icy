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

pub(crate) fn unpremultiply_rgba(rgba: &mut [u8]) {
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

const TAB_OUTLINE: &str = include_str!("../../../assets/icons/workspace-tab-outline.svg");

fn rasterize_tab_outline(scale: f32) -> Option<Picture> {
    rasterize_tab_shape("glyph:workspace-tab-outline", TAB_OUTLINE, scale)
}

fn rasterize_tab_shape(source: &str, svg: &str, scale: f32) -> Option<Picture> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let w = pixels(size.width() as u32, scale);
    let h = pixels(size.height() as u32, scale);
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    let transform =
        tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut rgba = pixmap.take();
    // Preserve coverage but keep the tint neutral even when sampling transparent texels.
    for px in rgba.as_chunks_mut::<4>().0 {
        px[..3].fill(255);
    }
    Some((source.into(), rgba, [w, h]))
}

/// Icy's "cold front": a neutral halftone mask, tinted at draw time. A curved band and a
/// finer echo frame a quiet centre; the size and opacity of each dot follow the same field.
fn workspace_halftone() -> Picture {
    const W: usize = 1080;
    const H: usize = 720;
    const PITCH: usize = 7;
    const KERNEL: usize = PITCH / 2;
    let bell = |v: f32| (-v * v).exp();
    let smooth = |v: f32| {
        let v = v.clamp(0., 1.);
        v * v * (3. - 2. * v)
    };
    let mut rgba = vec![255; W * H * 4];
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel[3] = 0;
    }
    for cy in (KERNEL..H - KERNEL).step_by(PITCH) {
        for cx in (KERNEL..W - KERNEL).step_by(PITCH) {
            let (x, y) = (cx as f32 / W as f32, cy as f32 / H as f32);
            let front = 1.08 - 0.76 * x + 0.10 * x * x + 0.07 * (x * 5.0).sin();
            let sweep = smooth((x - 0.20) / 0.74);
            let band = sweep * bell((y - front) / 0.13);
            let echo = sweep * bell((y - front - 0.20) / 0.055);
            let corner = bell((x - 0.97) / 0.40) * bell((y - 0.88) / 0.40);
            let frost = bell((x - 0.11) / 0.27) * bell((y - 0.04) / 0.25);
            let distance = ((x - 0.50) / 0.48).powi(2) + ((y - 0.45) / 0.46).powi(2);
            let quiet = 0.20 + 0.80 * smooth((distance - 0.05) / 0.85);
            let light =
                (0.04 + quiet * (0.59 * band + 0.30 * echo + 0.65 * corner + 0.23 * frost)).min(1.);
            let radius = 0.78 + 2.35 * light;
            let opacity = 30. + 207. * light.powf(0.85);
            for py in cy - KERNEL..=cy + KERNEL {
                for px in cx - KERNEL..=cx + KERNEL {
                    let dx = px.abs_diff(cx) as f32;
                    let dy = py.abs_diff(cy) as f32;
                    let coverage = (radius + 0.5 - dx.hypot(dy)).clamp(0., 1.);
                    rgba[(py * W + px) * 4 + 3] = (coverage * opacity).round() as u8;
                }
            }
        }
    }
    ("workspace:halftone".into(), rgba, [W as u32, H as u32])
}

/// Crisp Lucide/Accordee outline icons (24x24 viewBox, tinted in TSX via `paint.color`).
const UI_GLYPHS: &[(&str, &str, u32)] = &[
    (
        "monitor",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="20" height="14" x="2" y="3" rx="2"/><path d="M8 21h8M12 17v4"/></svg>"#,
        15,
    ),
    (
        "music",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>"#,
        15,
    ),
    (
        "image",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/></svg>"#,
        15,
    ),
    (
        "video",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="m16 13 5.223 3.482a.5.5 0 0 0 .777-.416V7.87a.5.5 0 0 0-.752-.432L16 10.5"/><rect x="2" y="6" width="14" height="12" rx="2"/></svg>"#,
        15,
    ),
    (
        "share",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/><path d="m8.59 13.51 6.83 3.98M15.41 6.51l-6.82 3.98"/></svg>"#,
        15,
    ),
    (
        "bookmark",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/></svg>"#,
        15,
    ),
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
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="white"><path fill-rule="evenodd" d="M6 2a.5.5 0 0 1 .47.33L10 12.036l1.53-4.208A.5.5 0 0 1 12 7.5h3.5a.5.5 0 0 1 0 1h-3.15l-1.88 5.17a.5.5 0 0 1-.94 0L6 3.964 4.47 8.171A.5.5 0 0 1 4 8.5H.5a.5.5 0 0 1 0-1h3.15l1.88-5.17A.5.5 0 0 1 6 2"/></svg>"#,
        16,
    ),
    (
        "workspaces",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="white"><path d="M4.5 6a.5.5 0 1 0 0-1 .5.5 0 0 0 0 1M6 6a.5.5 0 1 0 0-1 .5.5 0 0 0 0 1m2-.5a.5.5 0 1 1-1 0 .5.5 0 0 1 1 0"/><path d="M12 1a2 2 0 0 1 2 2 2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2 2 2 0 0 1-2-2V3a2 2 0 0 1 2-2zM2 12V5a2 2 0 0 1 2-2h9a1 1 0 0 0-1-1H2a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1m1-4v5a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V8zm12-1V5a1 1 0 0 0-1-1H4a1 1 0 0 0-1 1v2z"/></svg>"#,
        16,
    ),
    (
        "clock",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>"#,
        16,
    ),
    (
        "agent-working",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.8" stroke-linecap="round"><circle cx="12" cy="12" r="7" opacity=".25"/><path d="M5 12a7 7 0 0 1 7-7"/></svg>"#,
        16,
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
        "keyboard",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="2.5" y="5.5" width="19" height="13" rx="2"/><path d="M6.5 9.5h.01m3.66 0h.01m3.66 0h.01m3.66 0h.01m-11.02 4h.01m3.66 0h3.68m3.66 0h.01"/></svg>"#,
        15,
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
    (
        "more-horizontal",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="white"><circle cx="5" cy="12" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="19" cy="12" r="1.8"/></svg>"#,
        16,
    ),
    (
        "triangle-down",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10" fill="white"><path d="M2 3.5h6L5 7.2z"/></svg>"#,
        10,
    ),
    // Tooltip carets: the flat base sits on the bubble edge, the tip points at the anchor.
    (
        "caret-left",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10" fill="white"><path d="M10 0.5 5 5l5 4.5z"/></svg>"#,
        10,
    ),
    (
        "caret-right",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10" fill="white"><path d="M0 0.5 5 5 0 9.5z"/></svg>"#,
        10,
    ),
    (
        "caret-up",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10" fill="white"><path d="M0.5 10 5 5l4.5 5z"/></svg>"#,
        10,
    ),
    (
        "caret-down",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10" fill="white"><path d="M0.5 0 5 5l4.5-5z"/></svg>"#,
        10,
    ),
    (
        "arrow-left",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="m12 19-7-7 7-7"/><path d="M19 12H5"/></svg>"#,
        16,
    ),
    (
        "arrow-right",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="m12 5 7 7-7 7"/><path d="M5 12h14"/></svg>"#,
        16,
    ),
    (
        "arrow-up",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="m5 12 7-7 7 7M12 5v14"/></svg>"#,
        16,
    ),
    (
        "home",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="m3 10 9-7 9 7v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM9 21v-7h6v7"/></svg>"#,
        16,
    ),
    (
        "file",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/></svg>"#,
        16,
    ),
    (
        "file-diff",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M9 10h6"/><path d="M12 13V7"/><path d="M9 17h6"/></svg>"#,
        16,
    ),
    (
        "panel-right",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M15 3v18"/></svg>"#,
        16,
    ),
    (
        "copy",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>"#,
        16,
    ),
    (
        "folder-open",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/></svg>"#,
        16,
    ),
    (
        "folder-plus",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM12 11v6m-3-3h6"/></svg>"#,
        16,
    ),
    (
        "square-terminal",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="m7 11 2-2-2-2"/><path d="M11 13h4"/><rect width="18" height="18" x="3" y="3" rx="2" ry="2"/></svg>"#,
        16,
    ),
    (
        "article",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M7 8h10"/><path d="M7 12h10"/><path d="M7 16h6"/></svg>"#,
        16,
    ),
    (
        "message-square-plus",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><path d="M12 7v6"/><path d="M9 10h6"/></svg>"#,
        16,
    ),
    (
        "puzzle",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M19.439 7.85c-.049.322.059.648.289.878l1.568 1.568c.47.47.706 1.087.706 1.704s-.235 1.233-.706 1.704l-1.611 1.611a.98.98 0 0 1-.837.276c-.47-.07-.802-.48-.968-.925a2.501 2.501 0 1 0-3.214 3.214c.446.166.855.497.925.968a.979.979 0 0 1-.276.837l-1.61 1.61a2.404 2.404 0 0 1-1.705.707 2.402 2.402 0 0 1-1.704-.706l-1.568-1.568a1.026 1.026 0 0 0-.877-.29c-.493.074-.84.504-1.02.968a2.5 2.5 0 1 1-3.237-3.237c.464-.18.894-.527.967-1.02a1.026 1.026 0 0 0-.289-.877l-1.568-1.568A2.402 2.402 0 0 1 1.998 12c0-.617.236-1.234.706-1.704L4.23 8.77c.24-.24.581-.353.917-.303.515.077.877.528 1.073 1.01a2.5 2.5 0 1 0 3.259-3.259c-.482-.196-.933-.558-1.01-1.073-.05-.336.062-.676.303-.917l1.525-1.525A2.402 2.402 0 0 1 12 1.998c.617 0 1.234.236 1.704.706l1.568 1.568c.23.23.556.338.877.29.493-.074.84-.504 1.02-.968a2.5 2.5 0 1 1 3.237 3.237c-.464.18-.894.527-.967 1.02Z"/></svg>"#,
        16,
    ),
    (
        "zoom-in",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><path d="M11 8v6"/><path d="M8 11h6"/></svg>"#,
        16,
    ),
    (
        "zoom-out",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><path d="M8 11h6"/></svg>"#,
        16,
    ),
    (
        "more-vertical",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="white"><circle cx="12" cy="5" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="12" cy="19" r="1.8"/></svg>"#,
        16,
    ),
    (
        "pin",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/></svg>"#,
        14,
    ),
];

/// Colorful Accordee-style workspace & Agents marks.
const COLOR_MARKS: &[(&str, &str, u32)] = &[(
    "brand:icy",
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#f472b6"/><stop offset="50%" stop-color="#8b5cf6"/><stop offset="100%" stop-color="#38bdf8"/></linearGradient></defs><rect x="1" y="1" width="22" height="22" rx="6" fill="url(#g)"/><path d="M8 8.5v7M11.5 10l4.5 2-4.5 2" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none"/></svg>"##,
    20,
)];

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

/// Pure-vector 24x24 IDE marks (no `<text>` nodes so `resvg` renders identically without system fonts).
const EDITOR_SVGS: &[(&str, &str)] = &[
    (
        "cursor",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M12 2 3.5 6.9v10.2L12 22l8.5-4.9V6.9L12 2Z" fill="#2b2b30"/><path d="M12 2 20.5 6.9 12 11.8 3.5 6.9 12 2Z" fill="#e5e7eb"/><path d="M12 11.8 20.5 6.9v10.2L12 22V11.8Z" fill="#a1a1aa"/><path d="M12 11.8 3.5 6.9v10.2L12 22V11.8Z" fill="#52525b"/><path d="M12 6.4 17 12l-5 5.6-1.4-3.7L12 6.4Z" fill="#ffffff"/></svg>"##,
    ),
    (
        "vscode",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M17.6 2.4 7.7 11.9 3.6 8.8 2 9.6v4.8l1.6.8 4.1-3.1 9.9 9.5 4.4-2.1V4.5l-4.4-2.1Z" fill="#007acc"/><path d="M17.6 2.4 22 4.5v15l-4.4 2.1V2.4Z" fill="#1f9cf0"/><path d="M17.6 7.5 11.3 12l6.3 4.5V7.5Z" fill="#0a0a0a" fill-opacity="0.28"/></svg>"##,
    ),
    (
        "vscode-insiders",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M17.6 2.4 7.7 11.9 3.6 8.8 2 9.6v4.8l1.6.8 4.1-3.1 9.9 9.5 4.4-2.1V4.5l-4.4-2.1Z" fill="#0e9f6e"/><path d="M17.6 2.4 22 4.5v15l-4.4 2.1V2.4Z" fill="#24bfa5"/><path d="M17.6 7.5 11.3 12l6.3 4.5V7.5Z" fill="#0a0a0a" fill-opacity="0.28"/></svg>"##,
    ),
    (
        "vscodium",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M17.6 2.4 7.7 11.9 3.6 8.8 2 9.6v4.8l1.6.8 4.1-3.1 9.9 9.5 4.4-2.1V4.5l-4.4-2.1Z" fill="#2f80ed"/><path d="M17.6 2.4 22 4.5v15l-4.4 2.1V2.4Z" fill="#56ccf2"/><circle cx="15.2" cy="12" r="2.2" fill="#ffffff" fill-opacity="0.85"/></svg>"##,
    ),
    (
        "windsurf",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="ws" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#38bdf8"/><stop offset="100%" stop-color="#09b6a2"/></linearGradient></defs><path d="M3 7.5c4.5 0 6.5 4 10.5 4 2.6 0 4.5-1.2 7.5-3.5-1.2 5.2-4.4 8.5-9 8.5-4.3 0-6.2-3.5-9-9Z" fill="url(#ws)"/><path d="M3 13.5c3.5 0 5.5 3.5 9.2 3.5 2.4 0 4.3-1 6.8-2.8-1.1 3.7-3.8 6.3-7.6 6.3-3.7 0-5.8-2.4-8.4-7Z" fill="#09b6a2" fill-opacity="0.78"/></svg>"##,
    ),
    (
        "zed",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><rect x="2.5" y="2.5" width="19" height="19" rx="4" fill="#084ccf"/><path d="M7 7.5h10l-7.2 9H17" fill="none" stroke="#ffffff" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    ),
    (
        "trae",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><rect x="2.5" y="2.5" width="19" height="19" rx="5" fill="#111827"/><path d="M7 7.5h10v3h-3.5V17h-3v-6.5H7v-3Z" fill="#32f08c"/></svg>"##,
    ),
    (
        "kiro",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><rect x="2.5" y="2.5" width="19" height="19" rx="5" fill="#7c3aed"/><path d="M8.5 7v10M8.5 12l6.5-5M11.2 10l4.3 7" fill="none" stroke="#ffffff" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    ),
    (
        "sublime",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M19 4.5 5 9.1v4.2l14-4.6V4.5Z" fill="#ff9800"/><path d="M5 10.9 19 15.5v4.2L5 15.1v-4.2Z" fill="#fb8c00"/><path d="M19 10.9 5 15.5v4.2l14-4.6v-4.2Z" fill="#ffb74d"/></svg>"##,
    ),
    (
        "rustrover",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="rr" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#f97316"/><stop offset="55%" stop-color="#ef4444"/><stop offset="100%" stop-color="#eab308"/></linearGradient></defs><rect x="2" y="2" width="20" height="20" rx="4" fill="url(#rr)"/><rect x="5" y="5" width="14" height="14" rx="2" fill="#111113"/><path d="M7.5 15.5h5.5" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round"/><path d="M8 8.2v4.6M8 8.2h2.2a1.3 1.3 0 0 1 0 2.6H8m1.6 0 1.4 2M13.2 8.2v4.6m0-4.6h2.1a1.3 1.3 0 0 1 0 2.6h-2.1m1.5 0 1.3 2" fill="none" stroke="#ffffff" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    ),
    (
        "idea",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="ij" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#f43f5e"/><stop offset="55%" stop-color="#8b5cf6"/><stop offset="100%" stop-color="#3b82f6"/></linearGradient></defs><rect x="2" y="2" width="20" height="20" rx="4" fill="url(#ij)"/><rect x="5" y="5" width="14" height="14" rx="2" fill="#111113"/><path d="M7.5 15.5h5.5" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round"/><path d="M9 8.2v4.6M14.8 8.2v3.2a1.7 1.7 0 0 1-3.1.9" fill="none" stroke="#ffffff" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    ),
    (
        "webstorm",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="wsg" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#06b6d4"/><stop offset="100%" stop-color="#2563eb"/></linearGradient></defs><rect x="2" y="2" width="20" height="20" rx="4" fill="url(#wsg)"/><rect x="5" y="5" width="14" height="14" rx="2" fill="#111113"/><path d="M7.5 15.5h5.5" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round"/><path d="M7.6 8.3l1.1 4.4 1.2-3 1.2 3 1.1-4.4M16.2 8.9c-.4-.5-1-.7-1.6-.7-.8 0-1.4.4-1.4 1.1 0 1.3 3 .8 3 2.3 0 .8-.7 1.3-1.6 1.3-.7 0-1.3-.3-1.7-.7" fill="none" stroke="#ffffff" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    ),
    (
        "pycharm",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="pc" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#22c55e"/><stop offset="100%" stop-color="#eab308"/></linearGradient></defs><rect x="2" y="2" width="20" height="20" rx="4" fill="url(#pc)"/><rect x="5" y="5" width="14" height="14" rx="2" fill="#111113"/><path d="M7.5 15.5h5.5" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round"/><path d="M8.2 12.8V8.2h2.1a1.35 1.35 0 0 1 0 2.7H8.2M16 9.1a2.1 2.1 0 1 0 0 2.8" fill="none" stroke="#ffffff" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    ),
    (
        "goland",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="gl" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#06b6d4"/><stop offset="55%" stop-color="#8b5cf6"/><stop offset="100%" stop-color="#ec4899"/></linearGradient></defs><rect x="2" y="2" width="20" height="20" rx="4" fill="url(#gl)"/><rect x="5" y="5" width="14" height="14" rx="2" fill="#111113"/><path d="M7.5 15.5h5.5" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round"/><path d="M11.3 9.1a2 2 0 1 0 .2 2.8v-1.2h-1.4M14.7 8.3a2.2 2.2 0 1 0 0 4.4 2.2 2.2 0 0 0 0-4.4Z" fill="none" stroke="#ffffff" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    ),
    (
        "clion",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="cl" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#14b8a6"/><stop offset="100%" stop-color="#22c55e"/></linearGradient></defs><rect x="2" y="2" width="20" height="20" rx="4" fill="url(#cl)"/><rect x="5" y="5" width="14" height="14" rx="2" fill="#111113"/><path d="M7.5 15.5h5.5" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round"/><path d="M11.1 9.1a2.1 2.1 0 1 0 0 2.8M13.5 8.2v4.6h2.6" fill="none" stroke="#ffffff" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    ),
    (
        "phpstorm",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="ps" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#a855f7"/><stop offset="100%" stop-color="#ec4899"/></linearGradient></defs><rect x="2" y="2" width="20" height="20" rx="4" fill="url(#ps)"/><rect x="5" y="5" width="14" height="14" rx="2" fill="#111113"/><path d="M7.5 15.5h5.5" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round"/><path d="M8.2 12.8V8.2h2.1a1.35 1.35 0 0 1 0 2.7H8.2M16 8.9c-.4-.5-1-.7-1.5-.7-.8 0-1.4.4-1.4 1.1 0 1.3 2.9.8 2.9 2.3 0 .8-.7 1.3-1.6 1.3-.7 0-1.2-.3-1.6-.7" fill="none" stroke="#ffffff" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    ),
    (
        "rider",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><linearGradient id="rd" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#ec4899"/><stop offset="100%" stop-color="#f97316"/></linearGradient></defs><rect x="2" y="2" width="20" height="20" rx="4" fill="url(#rd)"/><rect x="5" y="5" width="14" height="14" rx="2" fill="#111113"/><path d="M7.5 15.5h5.5" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round"/><path d="M8.2 8.2v4.6m0-4.6h2a1.3 1.3 0 0 1 0 2.6h-2m1.4 0 1.3 2M13.2 8.2v4.6h1.4a2.3 2.3 0 0 0 0-4.6h-1.4Z" fill="none" stroke="#ffffff" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    ),
    (
        "neovide",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M4.5 5.5 8.5 3v18l-4-2.5v-13Z" fill="#38bdf8"/><path d="M19.5 5.5 15.5 3v18l4-2.5v-13Z" fill="#22c55e"/><path d="M8.5 3 15.5 14v7L8.5 10V3Z" fill="#4ade80"/></svg>"##,
    ),
    (
        "generic",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#a3a3a3" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="m9 10-2 2 2 2"/><path d="m15 10 2 2-2 2"/></svg>"##,
    ),
];

fn rasterize_editor_badge(id: &str, svg_bytes: &[u8], scale: f32) -> Option<Picture> {
    let side = pixels(16, scale);
    let mut pixmap = tiny_skia::Pixmap::new(side, side)?;
    let pad = side as f32 * 0.06;
    render_svg_centered(svg_bytes, &mut pixmap, pad, side as f32 - pad * 2.0);
    let mut rgba = pixmap.take();
    unpremultiply_rgba(&mut rgba);
    Some((format!("editor:{id}"), rgba, [side, side]))
}

/// Decodes an inline terminal image (kitty graphics) into straight RGBA for the host.
pub fn decode_term_image(data: &icy_core::graphics::ImageData) -> Option<(Vec<u8>, [u32; 2])> {
    use icy_core::graphics::ImageData;
    const MAX_SIDE: u32 = 8192;
    match data {
        ImageData::Png(bytes) => {
            let pixmap = tiny_skia::Pixmap::decode_png(bytes).ok()?;
            let size = [pixmap.width(), pixmap.height()];
            if size[0] > MAX_SIDE || size[1] > MAX_SIDE {
                return None;
            }
            let mut rgba = pixmap.take();
            unpremultiply_rgba(&mut rgba);
            Some((rgba, size))
        }
        ImageData::Rgba {
            width,
            height,
            data,
        } => (*width <= MAX_SIDE && *height <= MAX_SIDE).then(|| (data.clone(), [*width, *height])),
        ImageData::Rgb {
            width,
            height,
            data,
        } => (*width <= MAX_SIDE && *height <= MAX_SIDE).then(|| {
            let rgba = data
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|px| [px[0], px[1], px[2], 255])
                .collect();
            (rgba, [*width, *height])
        }),
    }
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Rasterizes an image file (PNG/JPEG/GIF/WebP through an SVG `<image>`, or an SVG document)
/// scaled to fit `max_side`, for file previews and the image viewer.
pub fn rasterize_image_file(path: &std::path::Path, max_side: u32) -> Option<(Vec<u8>, [u32; 2])> {
    let bytes = std::fs::read(path).ok()?;
    let is_svg = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    let dir = path.parent()?;
    let options = usvg::Options {
        resources_dir: Some(dir.to_path_buf()),
        ..usvg::Options::default()
    };
    let tree = if is_svg {
        usvg::Tree::from_data(&bytes, &options).ok()?
    } else {
        let (w, h) = icy_core::graphics::image_dimensions(&bytes)?;
        let file = xml_escape(path.file_name()?.to_str()?);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><image xlink:href="{file}" width="{w}" height="{h}"/></svg>"#
        );
        usvg::Tree::from_data(svg.as_bytes(), &options).ok()?
    };
    let size = tree.size();
    let fit = (max_side as f32 / size.width().max(size.height())).min(1.0);
    let w = (size.width() * fit).round().max(1.0) as u32;
    let h = (size.height() * fit).round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(fit, fit),
        &mut pixmap.as_mut(),
    );
    let mut rgba = pixmap.take();
    unpremultiply_rgba(&mut rgba);
    Some((rgba, [w, h]))
}

/// Rasterizes the downloaded GitHub avatar (PNG or JPEG) into a 22px circle as `avatar:github`.
pub fn rasterize_avatar(path: &std::path::Path, scale: f32) -> Option<Picture> {
    let dir = path.parent()?;
    let file = path.file_name()?.to_str()?;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="22" height="22" viewBox="0 0 22 22"><defs><clipPath id="c"><circle cx="11" cy="11" r="11"/></clipPath></defs><image xlink:href="{file}" width="22" height="22" preserveAspectRatio="xMidYMid slice" clip-path="url(#c)"/></svg>"#
    );
    let options = usvg::Options {
        resources_dir: Some(dir.to_path_buf()),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(svg.as_bytes(), &options).ok()?;
    let side = pixels(22, scale);
    let mut pixmap = tiny_skia::Pixmap::new(side, side)?;
    let fit = side as f32 / 22.0;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(fit, fit),
        &mut pixmap.as_mut(),
    );
    let mut rgba = pixmap.take();
    unpremultiply_rgba(&mut rgba);
    Some(("avatar:github".to_string(), rgba, [side, side]))
}

/// Rasterizes all static UI glyphs, brand marks, standalone SVGL CLI badges, and initial update ring.
/// Frames of the animated working spinner (`glyph:agent-working-<n>`), mirrored in app/theme.ts.
const SPINNER_FRAMES: u32 = 12;

pub fn load_static(scale: f32) -> Vec<Picture> {
    let mut out = Vec::with_capacity(UI_GLYPHS.len() + COLOR_MARKS.len() + 10);
    for &(name, svg, logical) in UI_GLYPHS {
        if let Some((rgba, size)) = rasterize_svg(svg, logical, scale, true) {
            out.push((format!("glyph:{name}"), rgba, size));
        }
    }
    out.extend(rasterize_tab_outline(scale));
    out.extend(rasterize_tab_shape(
        "glyph:workspace-tab-fill",
        &TAB_OUTLINE.replace("fill=\"none\"", "fill=\"white\""),
        scale,
    ));
    // Working spinner frames: JSNIR can't animate rotation, so the UI steps through 12 turns of
    // the `agent-working` arc. A theme overriding `agent-working` keeps these default frames.
    if let Some(&(_, svg, logical)) = UI_GLYPHS.iter().find(|g| g.0 == "agent-working") {
        for frame in 0..SPINNER_FRAMES {
            let turned = svg.replacen(
                "<path ",
                &format!(
                    r#"<path transform="rotate({} 12 12)" "#,
                    frame * 360 / SPINNER_FRAMES
                ),
                1,
            );
            if let Some((rgba, size)) = rasterize_svg(&turned, logical, scale, true) {
                out.push((format!("glyph:agent-working-{frame}"), rgba, size));
            }
        }
    }
    out.push(workspace_halftone());
    // Usage badge rings (Antigravity/Conductor-style corner ring): a full track and 21 arcs.
    let ring = |body: String| {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round">{body}</svg>"#
        )
    };
    if let Some((rgba, size)) = rasterize_svg(
        &ring(r#"<circle cx="6" cy="6" r="4.5"/>"#.into()),
        12,
        scale,
        true,
    ) {
        out.push(("glyph:uring-track".to_string(), rgba, size));
    }
    for step in 0..=20u32 {
        let circumference = 2.0 * std::f32::consts::PI * 4.5;
        let filled = circumference * step as f32 / 20.0;
        let body = format!(
            r#"<circle cx="6" cy="6" r="4.5" stroke-dasharray="{filled:.2} {circumference:.2}" transform="rotate(-90 6 6)"/>"#
        );
        if step > 0
            && let Some((rgba, size)) = rasterize_svg(&ring(body), 12, scale, true)
        {
            out.push((format!("glyph:uring-{step}"), rgba, size));
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
    if let Some(pic) = rasterize_editor_badge("antigravity", SVG_ANTIGRAVITY, scale) {
        out.push(pic);
    }
    for &(id, svg) in EDITOR_SVGS {
        if let Some(pic) = rasterize_editor_badge(id, svg.as_bytes(), scale) {
            out.push(pic);
        }
    }
    out
}

/// One colour layer of a git graph row as a white mask (tinted from the theme's lane palette).
/// Lane `x` sits at the lane's centre; `y` is in half rows (0 top, 1 dot, 2 bottom). Lane
/// changes are vertical-tangent curves so branches bend like `git log --graph` in GUIs.
pub fn rasterize_graph_layer(
    segs: &[icy_core::git_graph::Seg],
    lanes: usize,
    lane: f32,
    row: f32,
    stroke: f32,
    scale: f32,
) -> Option<(Vec<u8>, [u32; 2])> {
    let w = pixels((lanes as f32 * lane).ceil() as u32, scale);
    let h = pixels(row.ceil() as u32, scale);
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    let mut path = tiny_skia::PathBuilder::new();
    let point = |x: usize, y: u8| {
        (
            (x as f32 + 0.5) * lane * scale,
            y as f32 * 0.5 * row * scale,
        )
    };
    for seg in segs {
        let (x1, y1) = point(seg.x1, seg.y1);
        let (x2, y2) = point(seg.x2, seg.y2);
        path.move_to(x1, y1);
        if seg.x1 == seg.x2 {
            path.line_to(x2, y2);
        } else {
            let mid = (y1 + y2) * 0.5;
            path.cubic_to(x1, mid, x2, mid, x2, y2);
        }
    }
    let path = path.finish()?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.anti_alias = true;
    let stroke = tiny_skia::Stroke {
        width: stroke * scale,
        ..tiny_skia::Stroke::default()
    };
    pixmap.stroke_path(
        &path,
        &paint,
        &stroke,
        tiny_skia::Transform::identity(),
        None,
    );
    let mut rgba = pixmap.take();
    for px in rgba.as_chunks_mut::<4>().0 {
        px[..3].fill(255);
    }
    Some((rgba, [w, h]))
}

/// Theme icon overrides (`ResolvedTheme::icons`) rasterized under the same picture names as
/// the built-ins, so every `<Glyph>` / `cli:` image picks them up without TSX changes. UI
/// glyphs stay white masks tinted by the theme; CLI logos keep their own colours. Unknown
/// names, unreadable files and SVGs over 512 KiB are skipped.
pub fn theme_icon_pictures(
    icons: &std::collections::BTreeMap<String, String>,
    scale: f32,
) -> Vec<Picture> {
    const MAX_SVG: u64 = 512 * 1024;
    let mut out = Vec::new();
    for (name, source) in icons {
        let svg = if source.trim_start().starts_with('<') {
            source.clone()
        } else {
            let path = std::path::Path::new(source);
            if !std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= MAX_SVG) {
                continue;
            }
            match std::fs::read_to_string(path) {
                Ok(svg) => svg,
                Err(_) => continue,
            }
        };
        if let Some(&(_, _, logical)) = UI_GLYPHS.iter().find(|g| g.0 == name) {
            if let Some((rgba, size)) = rasterize_svg(&svg, logical, scale, true) {
                out.push((format!("glyph:{name}"), rgba, size));
            }
        } else if let Some(id) = name.strip_prefix("cli:") {
            let side = pixels(16, scale);
            let Some(mut pixmap) = tiny_skia::Pixmap::new(side, side) else {
                continue;
            };
            let pad = side as f32 * 0.08;
            if render_svg_centered(svg.as_bytes(), &mut pixmap, pad, side as f32 - pad * 2.0) {
                let mut rgba = pixmap.take();
                unpremultiply_rgba(&mut rgba);
                out.push((format!("cli:{id}"), rgba, [side, side]));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn tab_outline_reaches_the_divider_on_both_sides_without_a_bottom_border() {
        for scale in [1.0, 1.25, 2.0] {
            let (_, rgba, [w, h]) = super::rasterize_tab_outline(scale).unwrap();
            let alpha = |x, y| rgba[((y * w + x) * 4 + 3) as usize];
            assert!(alpha(0, h - 1) > 0 && alpha(w - 1, h - 1) > 0);
            assert!(alpha(w / 2, 0) > 0);
            assert_eq!(alpha(w / 2, h - 1), 0);
            for y in 0..h {
                assert!(
                    (0..w / 2).any(|x| alpha(x, y) > 0),
                    "left gap at {scale}x, row {y}"
                );
                assert!(
                    (w / 2..w).any(|x| alpha(x, y) > 0),
                    "right gap at {scale}x, row {y}"
                );
            }
        }
    }

    #[test]
    fn rasterizes_png_files_with_awkward_names() {
        // 1x1 PNG written under a name with XML-special characters.
        let png = icy_core::graphics::base64_decode(
            b"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==",
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("icy-img-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shot <1> & \"x\".png");
        std::fs::write(&path, png).unwrap();
        let (rgba, size) = super::rasterize_image_file(&path, 2048).expect("png rasterized");
        assert_eq!(size, [1, 1]);
        assert_eq!(rgba.len(), 4);
        assert!(super::rasterize_image_file(&dir.join("missing.png"), 2048).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
