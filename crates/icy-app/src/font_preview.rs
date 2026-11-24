use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use jsnir::text::{TextEngine, TextStyle};
use jsnir::tree::Span;
use ttf_parser::{Face, name_id};

pub struct FontSample {
    pub name: String,
    pub key: String,
    pub size: [u32; 2],
    pub pixels: Vec<u8>,
}

pub fn is_font(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["ttf", "otf"]
                .iter()
                .any(|kind| ext.eq_ignore_ascii_case(kind))
        })
}

pub fn render(bytes: &[u8]) -> Option<FontSample> {
    let face = Face::parse(bytes, 0).ok()?;
    let name = [
        name_id::FULL_NAME,
        name_id::TYPOGRAPHIC_FAMILY,
        name_id::FAMILY,
    ]
    .into_iter()
    .find_map(|id| {
        face.names()
            .into_iter()
            .filter(|entry| entry.name_id == id)
            .find_map(|entry| entry.to_string())
    })
    .unwrap_or_else(|| "Font preview".into());
    // A private JSNIR font database: previewing a different face of Inter or Geist must
    // neither replace the UI's own fonts nor invalidate layout/atlas caches for the window.
    let mut engine = TextEngine::with_fonts(&[bytes], [225, 229, 237]);
    let style = TextStyle {
        size: 28.0,
        line_height: 42.0,
        weight: face.weight().to_number(),
        ..TextStyle::default()
    };
    let spans = [Span {
        text: "Aa Bb Cc Dd Ee Ff Gg\n0123456789 !? @ # % &\nlet π = (x) => x + 1;".into(),
        ..Span::default()
    }];
    let bitmap = engine.rasterize_styled(&spans, [460.0, 132.0], 2.0, false, 1024, &style)?;
    let mut pixels = bitmap.rgba;
    // JSNIR text bitmaps are premultiplied; registered file images take straight alpha.
    crate::icons::unpremultiply_rgba(&mut pixels);
    let mut hash = DefaultHasher::new();
    bytes.hash(&mut hash);
    Some(FontSample {
        name,
        key: format!("font-preview:{:016x}", hash.finish()),
        size: bitmap.size.map(|n| n as u32),
        pixels,
    })
}
