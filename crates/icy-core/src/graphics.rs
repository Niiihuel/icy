//! Kitty terminal graphics protocol (the subset agent CLIs such as Codex use) layered in front of
//! the VT engine: APC `ESC _ G … ESC \` commands are split out of the PTY stream, images are
//! stored per session and anchored to the cursor cell (absolute line, so they scroll with output).
//!
//! Supported: `a=q` (query), `a=t`/`a=T` (transmit / transmit+display), `a=p` (put), `a=d`
//! (delete: a/A/i/I), `t=d` (direct, chunked with `m=1`), `t=f`/`t=t` (file path), `f=100` (PNG),
//! `f=24`/`f=32` (raw RGB/RGBA with `s`,`v`). Not supported: `o=z` compression, `t=s` shared
//! memory, unicode placeholders, animation frames.

use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::path::Path;

const MAX_APC_BYTES: usize = 48 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 48 * 1024 * 1024;
const MAX_IMAGES: usize = 48;
/// Fallback cell size for sizing images that do not specify `c`/`r` (px).
const CELL_PX: (u32, u32) = (8, 16);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment {
    Bytes(Vec<u8>),
    Apc(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum SplitState {
    #[default]
    Normal,
    Esc,
    Apc,
    ApcEsc,
}

/// Splits a PTY byte stream into VT bytes and kitty APC payloads, across arbitrary chunk borders.
#[derive(Default)]
pub struct ApcSplitter {
    state: SplitState,
    apc: Vec<u8>,
    overflow: bool,
}

impl ApcSplitter {
    pub fn push(&mut self, input: &[u8]) -> Vec<Segment> {
        let mut out = Vec::new();
        let mut plain = Vec::with_capacity(input.len());
        for &b in input {
            match self.state {
                SplitState::Normal => {
                    if b == 0x1b {
                        self.state = SplitState::Esc;
                    } else {
                        plain.push(b);
                    }
                }
                SplitState::Esc => {
                    if b == b'_' {
                        if !plain.is_empty() {
                            out.push(Segment::Bytes(std::mem::take(&mut plain)));
                        }
                        self.apc.clear();
                        self.overflow = false;
                        self.state = SplitState::Apc;
                    } else {
                        plain.extend_from_slice(&[0x1b, b]);
                        self.state = if b == 0x1b {
                            plain.pop();
                            SplitState::Esc
                        } else {
                            SplitState::Normal
                        };
                    }
                }
                SplitState::Apc => {
                    if b == 0x1b {
                        self.state = SplitState::ApcEsc;
                    } else if b == 0x07 {
                        self.finish(&mut out);
                    } else {
                        self.append(b);
                    }
                }
                SplitState::ApcEsc => {
                    if b == b'\\' {
                        self.finish(&mut out);
                    } else {
                        self.append(0x1b);
                        self.append(b);
                        self.state = SplitState::Apc;
                    }
                }
            }
        }
        if !plain.is_empty() {
            out.push(Segment::Bytes(plain));
        }
        out
    }

    fn append(&mut self, b: u8) {
        if self.apc.len() < MAX_APC_BYTES {
            self.apc.push(b);
        } else {
            self.overflow = true;
        }
    }

    fn finish(&mut self, out: &mut Vec<Segment>) {
        self.state = SplitState::Normal;
        let payload = std::mem::take(&mut self.apc);
        // Only graphics APCs matter; others (and oversized ones) are dropped like the VT would.
        if !self.overflow && payload.first() == Some(&b'G') {
            out.push(Segment::Apc(payload));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ImageData {
    Png(Vec<u8>),
    Rgb {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    Rgba {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
}

impl ImageData {
    fn pixel_size(&self) -> Option<(u32, u32)> {
        match self {
            Self::Png(bytes) => png_size(bytes),
            Self::Rgb { width, height, .. } | Self::Rgba { width, height, .. } => {
                Some((*width, *height))
            }
        }
    }
}

/// Pixel size read from an image file header (PNG, JPEG, GIF, WebP) without decoding it.
pub fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if let Some(size) = png_size(bytes) {
        return Some(size);
    }
    if bytes.starts_with(b"GIF8") && bytes.len() >= 10 {
        let w = u16::from_le_bytes([bytes[6], bytes[7]]);
        let h = u16::from_le_bytes([bytes[8], bytes[9]]);
        return Some((u32::from(w), u32::from(h)));
    }
    if bytes.len() >= 30 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        let le24 = |b: &[u8]| u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16;
        return match &bytes[12..16] {
            b"VP8X" => Some((le24(&bytes[24..27]) + 1, le24(&bytes[27..30]) + 1)),
            b"VP8L" => {
                let b = &bytes[21..25];
                let w = 1 + (u32::from(b[0]) | (u32::from(b[1] & 0x3f) << 8));
                let h = 1
                    + ((u32::from(b[1]) >> 6)
                        | (u32::from(b[2]) << 2)
                        | (u32::from(b[3] & 0x0f) << 10));
                Some((w, h))
            }
            b"VP8 " => {
                let w = u32::from(u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3fff);
                let h = u32::from(u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3fff);
                Some((w, h))
            }
            _ => None,
        };
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        // Walk JPEG segments to the first SOFn marker.
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xff {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            let len = usize::from(u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]));
            if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]);
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]);
                return Some((u32::from(w), u32::from(h)));
            }
            i += 2 + len;
        }
    }
    None
}

/// Width/height from a PNG IHDR chunk.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

struct StoredImage {
    /// Unique upload key across the session lifetime (a re-transmission gets a new one).
    seq: u64,
    hash: u64,
    data: ImageData,
    last_used: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Placement {
    image_id: u32,
    line: i64,
    col: u16,
    cols: u16,
    rows: u16,
}

/// A placement in screen coordinates (row may be negative when partly scrolled off the top).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibleImage {
    pub seq: u64,
    pub row: i64,
    pub col: u16,
    pub cols: u16,
    pub rows: u16,
    /// Source pixel size, so the UI can fit the image inside its cells without distortion.
    pub width: u32,
    pub height: u32,
}

#[derive(Default)]
pub struct GraphicsState {
    serve_images_on_demand: bool,
    images: HashMap<u32, StoredImage>,
    placements: Vec<Placement>,
    pending: Option<(HashMap<u8, String>, Vec<u8>)>,
    next_auto_id: u32,
    next_seq: u64,
    clock: u64,
    uploads: Vec<(u64, ImageData)>,
}

/// Cursor context the graphics layer needs from the VT at the moment an APC arrives.
#[derive(Clone, Copy, Debug)]
pub struct CursorContext {
    /// History length + cursor row: stays fixed for a cell while output scrolls.
    pub abs_line: i64,
    pub col: u16,
}

pub struct ApplyResult {
    /// Bytes to write back to the PTY (query / error replies).
    pub reply: Option<Vec<u8>>,
    /// Bytes to feed the VT afterwards (cursor movement when `C=1` is not set).
    pub vt: Option<Vec<u8>>,
}

fn parse_params(control: &[u8]) -> HashMap<u8, String> {
    control
        .split(|&b| b == b',')
        .filter_map(|kv| {
            let (k, v) = kv.split_at(kv.iter().position(|&b| b == b'=')?);
            (k.len() == 1).then(|| (k[0], String::from_utf8_lossy(&v[1..]).into_owned()))
        })
        .collect()
}

fn num(params: &HashMap<u8, String>, key: u8) -> u32 {
    params.get(&key).and_then(|v| v.parse().ok()).unwrap_or(0)
}

pub fn base64_decode(input: &[u8]) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in input {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        acc = (acc << 6) | value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn read_image_file(path: &Path, temporary: bool) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    // Kitty only deletes `t=t` files whose name marks them as graphics temp files.
    if temporary
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("tty-graphics-protocol"))
    {
        let _ = std::fs::remove_file(path);
    }
    Some(bytes)
}

impl GraphicsState {
    /// Handles one APC payload (starting with `G`).
    pub fn apply(&mut self, apc: &[u8], cursor: CursorContext) -> ApplyResult {
        let none = ApplyResult {
            reply: None,
            vt: None,
        };
        let body = &apc[1..];
        let (control, payload) = match body.iter().position(|&b| b == b';') {
            Some(i) => (&body[..i], &body[i + 1..]),
            None => (body, &[][..]),
        };
        let mut params = parse_params(control);

        // Chunked transmission: the first chunk carries the keys, later ones only `m`.
        if let Some((first, mut data)) = self.pending.take() {
            data.extend_from_slice(payload);
            if params.get(&b'm').is_some_and(|m| m == "1") {
                self.pending = Some((first, data));
                return none;
            }
            return self.execute(first, data, cursor);
        }
        if params.get(&b'm').is_some_and(|m| m == "1") {
            params.remove(&b'm');
            self.pending = Some((params, payload.to_vec()));
            return none;
        }
        self.execute(params, payload.to_vec(), cursor)
    }

    fn execute(
        &mut self,
        params: HashMap<u8, String>,
        payload: Vec<u8>,
        cursor: CursorContext,
    ) -> ApplyResult {
        let action = params.get(&b'a').map_or("t", String::as_str);
        let quiet = num(&params, b'q');
        let id = num(&params, b'i');
        let reply = |ok: Result<(), &str>| -> Option<Vec<u8>> {
            let show = match ok {
                Ok(()) => quiet == 0,
                Err(_) => quiet < 2,
            };
            (show && id != 0).then(|| {
                let msg = ok.err().unwrap_or("OK");
                format!("\x1b_Gi={id};{msg}\x1b\\").into_bytes()
            })
        };
        self.clock += 1;

        match action {
            "q" => {
                let ok = self.decode(&params, &payload).map(|_| ());
                ApplyResult {
                    reply: reply(ok.ok_or("EINVAL:unsupported format")),
                    vt: None,
                }
            }
            "t" | "T" => {
                let Some(data) = self.decode(&params, &payload) else {
                    return ApplyResult {
                        reply: reply(Err("EINVAL:could not load image")),
                        vt: None,
                    };
                };
                let id = if id == 0 {
                    self.next_auto_id = self.next_auto_id.wrapping_add(1) | 0x8000_0000;
                    self.next_auto_id
                } else {
                    id
                };
                self.store(id, data);
                let vt = (action == "T")
                    .then(|| self.place(id, &params, cursor))
                    .flatten();
                ApplyResult {
                    reply: reply(Ok(())),
                    vt,
                }
            }
            "p" => {
                if !self.images.contains_key(&id) {
                    return ApplyResult {
                        reply: reply(Err("ENOENT:no such image")),
                        vt: None,
                    };
                }
                let vt = self.place(id, &params, cursor);
                ApplyResult {
                    reply: reply(Ok(())),
                    vt,
                }
            }
            "d" => {
                match params.get(&b'd').map_or("a", String::as_str) {
                    "a" => self.placements.clear(),
                    "A" => {
                        self.placements.clear();
                        self.images.clear();
                    }
                    "i" => self.placements.retain(|p| p.image_id != id),
                    "I" => {
                        self.placements.retain(|p| p.image_id != id);
                        self.images.remove(&id);
                    }
                    _ => {}
                }
                ApplyResult {
                    reply: None,
                    vt: None,
                }
            }
            _ => ApplyResult {
                reply: reply(Err("EINVAL:unsupported action")),
                vt: None,
            },
        }
    }

    fn decode(&self, params: &HashMap<u8, String>, payload: &[u8]) -> Option<ImageData> {
        if params.contains_key(&b'o') {
            return None; // compressed payloads are not supported
        }
        let raw = base64_decode(payload)?;
        let bytes = match params.get(&b't').map_or("d", String::as_str) {
            "d" => raw,
            medium @ ("f" | "t") => {
                let path = String::from_utf8(raw).ok()?;
                read_image_file(Path::new(&path), medium == "t")?
            }
            _ => return None,
        };
        let (width, height) = (num(params, b's'), num(params, b'v'));
        let pixels = |bpp: usize| -> Option<Vec<u8>> {
            (width > 0 && height > 0 && bytes.len() >= width as usize * height as usize * bpp)
                .then(|| bytes[..width as usize * height as usize * bpp].to_vec())
        };
        match num(params, b'f') {
            100 => png_size(&bytes).map(|_| ImageData::Png(bytes)),
            24 => pixels(3).map(|data| ImageData::Rgb {
                width,
                height,
                data,
            }),
            32 | 0 => pixels(4).map(|data| ImageData::Rgba {
                width,
                height,
                data,
            }),
            _ => None,
        }
    }

    fn store(&mut self, id: u32, data: ImageData) {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        let hash = hasher.finish();
        // Identical re-transmissions (TUI redraws) reuse the uploaded texture.
        if let Some(existing) = self.images.get_mut(&id)
            && existing.hash == hash
        {
            existing.last_used = self.clock;
            return;
        }
        if let Some((&dup_id, dup)) = self.images.iter().find(|(_, img)| img.hash == hash) {
            let seq = dup.seq;
            let data = dup.data.clone();
            if dup_id != id {
                self.images.insert(
                    id,
                    StoredImage {
                        seq,
                        hash,
                        data,
                        last_used: self.clock,
                    },
                );
            }
        } else {
            self.next_seq += 1;
            if !self.serve_images_on_demand {
                self.uploads.push((self.next_seq, data.clone()));
            }
            self.images.insert(
                id,
                StoredImage {
                    seq: self.next_seq,
                    hash,
                    data,
                    last_used: self.clock,
                },
            );
        }
        self.evict();
    }

    fn evict(&mut self) {
        while self.images.len() > MAX_IMAGES {
            let Some((&oldest, _)) = self.images.iter().min_by_key(|(_, img)| img.last_used) else {
                break;
            };
            self.images.remove(&oldest);
            self.placements.retain(|p| p.image_id != oldest);
        }
    }

    fn place(
        &mut self,
        id: u32,
        params: &HashMap<u8, String>,
        cursor: CursorContext,
    ) -> Option<Vec<u8>> {
        let image = self.images.get_mut(&id)?;
        image.last_used = self.clock;
        let (pw, ph) = image.data.pixel_size().unwrap_or((CELL_PX.0, CELL_PX.1));
        let cols = match num(params, b'c') {
            0 => pw.div_ceil(CELL_PX.0),
            c => c,
        }
        .clamp(1, 1000) as u16;
        let rows = match num(params, b'r') {
            0 => ph.div_ceil(CELL_PX.1),
            r => r,
        }
        .clamp(1, 1000) as u16;
        let placement = Placement {
            image_id: id,
            line: cursor.abs_line,
            col: cursor.col,
            cols,
            rows,
        };
        // A redraw at the same cells (or the same image re-put) replaces the old placement.
        self.placements
            .retain(|p| p.image_id != id && !(p.line == placement.line && p.col == placement.col));
        self.placements.push(placement);
        // Without C=1 the cursor moves to the cell after the image (rows-1 down, cols right).
        (num(params, b'C') != 1).then(|| {
            let mut vt = b"\n".repeat(usize::from(rows.saturating_sub(1)));
            vt.extend_from_slice(format!("\x1b[{cols}C").as_bytes());
            vt
        })
    }

    /// Screen clears (`ED 2/3`, `RIS`) drop the placements that were on screen, like kitty.
    pub fn observe_vt_bytes(&mut self, bytes: &[u8], screen_top_abs_line: i64) {
        let clears = [&b"\x1b[2J"[..], b"\x1b[3J", b"\x1bc"];
        if clears
            .iter()
            .any(|pat| bytes.windows(pat.len()).any(|w| w == *pat))
        {
            self.placements.retain(|p| p.line < screen_top_abs_line);
        }
    }

    /// New textures to upload since the last call, keyed by their unique `seq`.
    pub fn take_uploads(&mut self) -> Vec<(u64, ImageData)> {
        std::mem::take(&mut self.uploads)
    }

    pub(crate) fn serve_images_on_demand(&mut self) {
        self.serve_images_on_demand = true;
        self.uploads.clear();
    }

    pub(crate) fn image(&self, seq: u64) -> Option<ImageData> {
        self.images
            .values()
            .find(|image| image.seq == seq)
            .map(|image| image.data.clone())
    }

    /// Placements intersecting a screen whose top row is `screen_top_abs_line`.
    pub fn visible(&self, screen_top_abs_line: i64, screen_rows: u16) -> Vec<VisibleImage> {
        self.placements
            .iter()
            .filter_map(|p| {
                let row = p.line - screen_top_abs_line;
                let on_screen = row + i64::from(p.rows) > 0 && row < i64::from(screen_rows);
                let image = self.images.get(&p.image_id)?;
                let (width, height) = image.data.pixel_size().unwrap_or((0, 0));
                on_screen.then_some(VisibleImage {
                    seq: image.seq,
                    row,
                    col: p.col,
                    cols: p.cols,
                    rows: p.rows,
                    width,
                    height,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1x1 PNG.
    const PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

    fn at(line: i64, col: u16) -> CursorContext {
        CursorContext {
            abs_line: line,
            col,
        }
    }

    #[test]
    fn splits_apc_across_chunks_and_keeps_vt_bytes() {
        let mut s = ApcSplitter::default();
        let mut segs = s.push(b"ab\x1b[31mc\x1b");
        segs.extend(s.push(b"_Ga=q,i=1;AA"));
        segs.extend(s.push(b"AA\x1b"));
        segs.extend(s.push(b"\\tail\x1b_Xother\x1b\\"));
        assert_eq!(
            segs,
            vec![
                Segment::Bytes(b"ab\x1b[31mc".to_vec()),
                Segment::Apc(b"Ga=q,i=1;AAAA".to_vec()),
                Segment::Bytes(b"tail".to_vec()),
            ]
        );
    }

    #[test]
    fn reads_dimensions_from_headers() {
        let png = base64_decode(PNG_B64.as_bytes()).unwrap();
        assert_eq!(image_dimensions(&png), Some((1, 1)));
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&[0x40, 0x01, 0xf0, 0x00]);
        assert_eq!(image_dimensions(&gif), Some((320, 240)));
        // JPEG: SOI, an APP0 segment, then SOF0 with height 600 / width 800.
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x00, 0x00, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x02,
            0x58, 0x03, 0x20, 0x03, 0x00, 0x00,
        ];
        assert_eq!(image_dimensions(&jpeg), Some((800, 600)));
        assert_eq!(image_dimensions(b"not an image"), None);
    }

    #[test]
    fn decodes_base64_with_padding_and_urlsafe() {
        assert_eq!(base64_decode(b"aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode(b"aGk").unwrap(), b"hi");
        assert!(base64_decode(b"a$b").is_none());
    }

    #[test]
    fn answers_codex_style_query() {
        let mut g = GraphicsState::default();
        let r = g.apply(b"Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA", at(0, 0));
        assert_eq!(r.reply.unwrap(), b"\x1b_Gi=31;OK\x1b\\");
        let r = g.apply(b"Gi=32,a=q,t=d,f=99;AAAA", at(0, 0));
        assert!(r.reply.unwrap().starts_with(b"\x1b_Gi=32;EINVAL"));
    }

    #[test]
    fn chunked_png_transmit_and_display_with_scroll() {
        let mut g = GraphicsState::default();
        let (a, b) = PNG_B64.split_at(20);
        let first = format!("Ga=T,t=d,f=100,c=10,r=4,q=2,m=1,C=1;{a}");
        assert!(g.apply(first.as_bytes(), at(100, 3)).reply.is_none());
        let last = format!("Gm=0;{b}");
        let r = g.apply(last.as_bytes(), at(100, 3));
        assert!(r.reply.is_none() && r.vt.is_none()); // q=2, C=1
        assert_eq!(g.take_uploads().len(), 1);
        assert_eq!(
            g.visible(98, 24),
            vec![VisibleImage {
                seq: 1,
                row: 2,
                col: 3,
                cols: 10,
                rows: 4,
                width: 1,
                height: 1,
            }]
        );
        // Scrolled so the image is partly above the top: still visible with a negative row.
        assert_eq!(g.visible(102, 24)[0].row, -2);
        assert!(g.visible(104, 24).is_empty());

        // The same pixels redrawn at the same cell: no new upload, one placement.
        let whole = format!("Ga=T,t=d,f=100,c=10,r=4,q=2,C=1;{PNG_B64}");
        g.apply(whole.as_bytes(), at(100, 3));
        assert!(g.take_uploads().is_empty());
        assert_eq!(g.visible(98, 24).len(), 1);

        g.observe_vt_bytes(b"\x1b[2J", 90);
        assert!(g.visible(98, 24).is_empty());
    }

    #[test]
    fn put_delete_and_cursor_advance() {
        let mut g = GraphicsState::default();
        let t = format!("Ga=t,i=7,f=100,q=2;{PNG_B64}");
        g.apply(t.as_bytes(), at(0, 0));
        let r = g.apply(b"Ga=p,i=7,c=2,r=3", at(5, 1));
        assert_eq!(r.vt.unwrap(), b"\n\n\x1b[2C");
        assert_eq!(g.visible(0, 24).len(), 1);
        g.apply(b"Ga=d,d=i,i=7,q=2", at(0, 0));
        assert!(g.visible(0, 24).is_empty());
        let r = g.apply(b"Ga=p,i=9", at(0, 0));
        assert!(r.reply.unwrap().starts_with(b"\x1b_Gi=9;ENOENT"));
    }
}
