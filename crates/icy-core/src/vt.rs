use std::sync::Arc;

use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::Dimensions,
    index::{Column, Line},
    term::{Config as TermConfig, cell::Flags},
    vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb},
};
use serde::Serialize;

/// Default foreground (`--ac-text-primary` dark: `#fafafa`).
pub const DEFAULT_FG: u32 = 0xfafa_faff;
/// Default background (`--ac-app-bg` dark: `#0a0a0a`).
pub const DEFAULT_BG: u32 = 0x0a0a_0aff;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CursorPos {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Run {
    pub text: String,
    pub fg: u32,
    pub bg: u32,
    pub bold: bool,
    pub underline: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Row {
    pub runs: Vec<Run>,
}

impl Row {
    pub fn plain_text(&self) -> String {
        let total_len: usize = self.runs.iter().map(|r| r.text.len()).sum();
        let mut out = String::with_capacity(total_len);
        for run in &self.runs {
            out.push_str(&run.text);
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Screen {
    pub cols: u16,
    pub rows_count: u16,
    pub rows: Vec<Row>,
    pub cursor: CursorPos,
}

/// Abstract virtual terminal engine so the parser/grid backend can be swapped later
/// (for instance from `alacritty_terminal` to `libghostty-vt`) without touching sessions or UI.
pub trait VtEngine: Send {
    fn feed(&mut self, bytes: &[u8]);
    fn resize(&mut self, cols: u16, rows: u16);
    fn snapshot(&self) -> Screen;
}

struct GridSize {
    cols: usize,
    rows: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

pub type PtyWriteCallback = Arc<dyn Fn(&[u8]) + Send + Sync>;
pub type TitleCallback = Arc<dyn Fn(Option<String>) + Send + Sync>;

#[derive(Clone, Default)]
pub struct VtCallbacks {
    pub on_pty_write: Option<PtyWriteCallback>,
    pub on_title: Option<TitleCallback>,
}

struct TermListener {
    callbacks: VtCallbacks,
}

impl EventListener for TermListener {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                if let Some(cb) = &self.callbacks.on_pty_write {
                    cb(text.as_bytes());
                }
            }
            Event::ColorRequest(index, formatter) => {
                if let Some(cb) = &self.callbacks.on_pty_write {
                    let rgba = match index {
                        256 | 258 => DEFAULT_FG,
                        257 => DEFAULT_BG,
                        0..=255 => indexed_color(index as u8),
                        _ => DEFAULT_FG,
                    };
                    let rgb = Rgb {
                        r: ((rgba >> 24) & 0xff) as u8,
                        g: ((rgba >> 16) & 0xff) as u8,
                        b: ((rgba >> 8) & 0xff) as u8,
                    };
                    let reply = formatter(rgb);
                    cb(reply.as_bytes());
                }
            }
            Event::Title(title) => {
                if let Some(cb) = &self.callbacks.on_title {
                    cb(Some(title));
                }
            }
            Event::ResetTitle => {
                if let Some(cb) = &self.callbacks.on_title {
                    cb(None);
                }
            }
            _ => {}
        }
    }
}

pub struct AlacrittyVt {
    term: Term<TermListener>,
    parser: Processor,
    cols: u16,
    rows: u16,
}

impl AlacrittyVt {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self::with_callbacks(cols, rows, VtCallbacks::default())
    }

    pub fn with_callbacks(cols: u16, rows: u16, callbacks: VtCallbacks) -> Self {
        let cols = cols.max(2);
        let rows = rows.max(1);
        let size = GridSize {
            cols: usize::from(cols),
            rows: usize::from(rows),
        };
        let config = TermConfig {
            scrolling_history: 5000,
            ..TermConfig::default()
        };
        let term = Term::new(config, &size, TermListener { callbacks });
        Self {
            term,
            parser: Processor::new(),
            cols,
            rows,
        }
    }
}

impl VtEngine for AlacrittyVt {
    fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        let cols = cols.max(2);
        let rows = rows.max(1);
        if self.cols == cols && self.rows == rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        self.term.resize(GridSize {
            cols: usize::from(cols),
            rows: usize::from(rows),
        });
    }

    fn snapshot(&self) -> Screen {
        let content = self.term.renderable_content();
        let grid = self.term.grid();
        let display_offset = content.display_offset as i32;
        let cols = usize::from(self.cols);
        let rows_count = usize::from(self.rows);

        let mut rows = Vec::with_capacity(rows_count);
        for r in 0..rows_count {
            let line = Line(r as i32 - display_offset);
            let grid_row = &grid[line];

            // Trim trailing blank cells that carry only default background and no underline.
            let mut last_used = 0usize;
            for c in (0..cols).rev() {
                let cell = &grid_row[Column(c)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                let (_, bg, _) = resolve_cell_colors(cell, content.colors);
                let ch = if cell.c == '\0' { ' ' } else { cell.c };
                let has_underline = cell.flags.intersects(Flags::ALL_UNDERLINES);
                if ch != ' ' || bg != DEFAULT_BG || has_underline {
                    last_used = c + 1;
                    break;
                }
            }

            let mut runs: Vec<Run> = Vec::new();
            for c in 0..last_used {
                let cell = &grid_row[Column(c)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                let ch = if cell.c == '\0' { ' ' } else { cell.c };
                let (fg, bg, bold) = resolve_cell_colors(cell, content.colors);
                let underline = cell.flags.intersects(Flags::ALL_UNDERLINES);

                if let Some(prev) = runs.last_mut()
                    && prev.fg == fg
                    && prev.bg == bg
                    && prev.bold == bold
                    && prev.underline == underline
                {
                    prev.text.push(ch);
                    if let Some(zw) = cell.zerowidth() {
                        for &extra_ch in zw {
                            prev.text.push(extra_ch);
                        }
                    }
                } else {
                    let mut text = String::new();
                    text.push(ch);
                    if let Some(zw) = cell.zerowidth() {
                        for &extra_ch in zw {
                            text.push(extra_ch);
                        }
                    }
                    runs.push(Run {
                        text,
                        fg,
                        bg,
                        bold,
                        underline,
                    });
                }
            }

            rows.push(Row { runs });
        }

        let cursor_line = content.cursor.point.line.0 + display_offset;
        let cursor_visible = content.cursor.shape != CursorShape::Hidden
            && cursor_line >= 0
            && (cursor_line as usize) < rows_count;
        let cursor = CursorPos {
            row: cursor_line.clamp(0, (rows_count.saturating_sub(1)) as i32) as u16,
            col: (content.cursor.point.column.0 as u16).min(self.cols.saturating_sub(1)),
            visible: cursor_visible,
        };

        Screen {
            cols: self.cols,
            rows_count: self.rows,
            rows,
            cursor,
        }
    }
}

fn resolve_cell_colors(
    cell: &alacritty_terminal::term::cell::Cell,
    colors: &alacritty_terminal::term::color::Colors,
) -> (u32, u32, bool) {
    let bold = cell.flags.contains(Flags::BOLD);
    let dim = cell.flags.contains(Flags::DIM);
    let mut fg = resolve_color(cell.fg, colors, bold, true);
    let mut bg = resolve_color(cell.bg, colors, false, false);

    if cell.flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    if dim {
        fg = dim_rgba(fg);
    }
    if cell.flags.contains(Flags::HIDDEN) {
        fg = bg;
    }
    (fg, bg, bold)
}

fn resolve_color(
    color: Color,
    colors: &alacritty_terminal::term::color::Colors,
    bold: bool,
    is_fg: bool,
) -> u32 {
    match color {
        Color::Spec(rgb) => rgba(rgb.r, rgb.g, rgb.b),
        Color::Indexed(idx) => {
            if let Some(rgb) = colors[idx as usize] {
                rgba(rgb.r, rgb.g, rgb.b)
            } else {
                indexed_color(idx)
            }
        }
        Color::Named(named) => {
            let effective = if bold && is_fg {
                named.to_bright()
            } else {
                named
            };
            if let Some(rgb) = colors[effective] {
                rgba(rgb.r, rgb.g, rgb.b)
            } else {
                named_color(effective)
            }
        }
    }
}

fn named_color(named: NamedColor) -> u32 {
    match named {
        NamedColor::Black => 0x1818_1bff,
        NamedColor::Red => 0xf871_71ff,
        NamedColor::Green => 0x4ade_80ff,
        NamedColor::Yellow => 0xfbbf_24ff,
        NamedColor::Blue => 0x60a5_faff,
        NamedColor::Magenta => 0xc084_fcff,
        NamedColor::Cyan => 0x22d3_eeff,
        NamedColor::White => 0xe4e4_e7ff,
        NamedColor::BrightBlack => 0x7171_7aff,
        NamedColor::BrightRed => 0xfca5_a5ff,
        NamedColor::BrightGreen => 0x86ef_acff,
        NamedColor::BrightYellow => 0xfde0_47ff,
        NamedColor::BrightBlue => 0x93c5_fdff,
        NamedColor::BrightMagenta => 0xd8b4_feff,
        NamedColor::BrightCyan => 0x67e8_f9ff,
        NamedColor::BrightWhite => 0xfafa_faff,
        NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::Cursor => DEFAULT_FG,
        NamedColor::Background => DEFAULT_BG,
        NamedColor::DimBlack => 0x2727_2aff,
        NamedColor::DimRed => 0xef44_44ff,
        NamedColor::DimGreen => 0x22c5_5eff,
        NamedColor::DimYellow => 0xeab3_08ff,
        NamedColor::DimBlue => 0x3b82_f6ff,
        NamedColor::DimMagenta => 0xa855_f7ff,
        NamedColor::DimCyan => 0x06b6_d4ff,
        NamedColor::DimWhite => 0xa1a1_aaff,
        NamedColor::DimForeground => 0xa3a3_a3ff,
    }
}

fn indexed_color(idx: u8) -> u32 {
    match idx {
        0 => named_color(NamedColor::Black),
        1 => named_color(NamedColor::Red),
        2 => named_color(NamedColor::Green),
        3 => named_color(NamedColor::Yellow),
        4 => named_color(NamedColor::Blue),
        5 => named_color(NamedColor::Magenta),
        6 => named_color(NamedColor::Cyan),
        7 => named_color(NamedColor::White),
        8 => named_color(NamedColor::BrightBlack),
        9 => named_color(NamedColor::BrightRed),
        10 => named_color(NamedColor::BrightGreen),
        11 => named_color(NamedColor::BrightYellow),
        12 => named_color(NamedColor::BrightBlue),
        13 => named_color(NamedColor::BrightMagenta),
        14 => named_color(NamedColor::BrightCyan),
        15 => named_color(NamedColor::BrightWhite),
        16..=231 => {
            let n = idx - 16;
            let r = cube_component(n / 36);
            let g = cube_component((n / 6) % 6);
            let b = cube_component(n % 6);
            rgba(r, g, b)
        }
        232..=255 => {
            let v = 8 + (idx - 232) * 10;
            rgba(v, v, v)
        }
    }
}

fn cube_component(step: u8) -> u8 {
    if step == 0 { 0 } else { 55 + step * 40 }
}

const fn rgba(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | 0xff
}

fn dim_rgba(color: u32) -> u32 {
    let rgb = Rgb {
        r: (((color >> 24) & 0xff) * 2 / 3) as u8,
        g: (((color >> 16) & 0xff) * 2 / 3) as u8,
        b: (((color >> 8) & 0xff) * 2 / 3) as u8,
    };
    ((rgb.r as u32) << 24) | ((rgb.g as u32) << 16) | ((rgb.b as u32) << 8) | (color & 0xff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feeds_ansi_text_and_colors() {
        let mut vt = AlacrittyVt::new(80, 24);
        vt.feed(b"\x1b[32mhello\x1b[0m world\r\n");
        let screen = vt.snapshot();
        assert_eq!(screen.rows[0].plain_text(), "hello world");
        assert_eq!(screen.rows[0].runs.len(), 2);
        assert_eq!(screen.rows[0].runs[0].text, "hello");
        assert_eq!(screen.rows[0].runs[0].fg, named_color(NamedColor::Green));
        assert_eq!(screen.rows[0].runs[1].text, " world");
        assert_eq!(screen.rows[0].runs[1].fg, DEFAULT_FG);
    }
}
