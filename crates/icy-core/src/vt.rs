use std::sync::Arc;
use std::time::Instant;

use crate::agent_status::{self, AgentStatus, StatusSignals};

use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::{Config as TermConfig, TermMode, cell::Flags},
    vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb},
};
use serde::{Deserialize, Serialize};

/// Default foreground (`--ac-text-primary` dark: `#fafafa`).
pub const DEFAULT_FG: u32 = 0xfafa_faff;
/// Default background (`--ac-app-bg` dark: `#0a0a0a`).
pub const DEFAULT_BG: u32 = 0x0a0a_0aff;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorPos {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub text: String,
    pub col: u16,
    pub cells: u16,
    pub fg: u32,
    pub bg: u32,
    pub bold: bool,
    pub underline: bool,
    /// Inside the mouse selection.
    pub selected: bool,
    /// OSC 8 hyperlink URI attached to this run, if any.
    pub hyperlink: Option<String>,
}

impl Run {
    /// Whether this run can be split at character boundaries without splitting a VT cell.
    pub fn has_one_char_per_cell(&self) -> bool {
        self.text.is_ascii()
            || (self.text.chars().all(is_monospace_cell_char)
                && self.text.chars().count() == usize::from(self.cells))
    }
}

// These glyphs have the ASCII advance in both bundled weights of Geist Mono.
fn is_monospace_cell_char(ch: char) -> bool {
    matches!(ch, ' '..='~' | '\u{a0}'..='\u{ac}' | '\u{ae}'..='\u{ff}' | '•' | '…' | '\u{2500}'..='\u{259f}')
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub runs: Vec<Run>,
    /// The terminal continues this logical line in the next screen row.
    pub wrapped: bool,
}

impl Row {
    pub(crate) fn coalesce_monospace_runs(&mut self) {
        let mut previous_is_monospace = None;
        self.runs.dedup_by(|next, prev| {
            let merge = prev.col + prev.cells == next.col
                && prev.fg == next.fg
                && prev.bg == next.bg
                && prev.bold == next.bold
                && prev.underline == next.underline
                && prev.selected == next.selected
                && prev.hyperlink == next.hyperlink
                && next.has_one_char_per_cell()
                && *previous_is_monospace.get_or_insert_with(|| prev.has_one_char_per_cell());
            if merge {
                prev.text.push_str(&next.text);
                prev.cells += next.cells;
            } else {
                previous_is_monospace = None;
            }
            merge
        });
    }

    pub fn plain_text(&self) -> String {
        let total_len: usize = self.runs.iter().map(|r| r.text.len()).sum();
        let mut out = String::with_capacity(total_len);
        for run in &self.runs {
            out.push_str(&run.text);
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    pub cols: u16,
    pub rows_count: u16,
    pub rows: Vec<Row>,
    pub cursor: CursorPos,
    /// Lines the view is scrolled back into the history (0 = following the output).
    pub display_offset: u32,
}

/// Terminal modes that decide where a mouse wheel over the terminal goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WheelModes {
    /// The program asked for mouse reports (DECSET 1000 / 1002 / 1003).
    pub mouse_report: bool,
    /// Reports use the SGR encoding (DECSET 1006).
    pub sgr: bool,
    /// Full-screen program on the alternate screen (DECSET 1049).
    pub alt_screen: bool,
    /// Wheel becomes arrow keys on the alternate screen (DECSET 1007, on by default).
    pub alternate_scroll: bool,
    /// Arrow keys use SS3 (`ESC O A`) instead of CSI (DECCKM).
    pub app_cursor: bool,
}

/// Bytes a wheel of `lines` (> 0 up, < 0 down) over cell `col`, `row` (0-based) sends to the
/// program, as xterm does. `None`: the program takes no wheel, so the view scrolls the history.
pub fn wheel_input(modes: WheelModes, lines: i32, col: u16, row: u16) -> Option<Vec<u8>> {
    if lines == 0 {
        return None;
    }
    let count = lines.unsigned_abs().min(100) as usize;
    let up = lines > 0;
    if modes.mouse_report {
        let button: u16 = if up { 64 } else { 65 };
        let event = if modes.sgr {
            format!("\x1b[<{button};{};{}M", col + 1, row + 1).into_bytes()
        } else {
            // X10 encoding: every value is offset by 32 and must fit in a byte.
            let byte = |value: u16| (value + 32).min(255) as u8;
            vec![0x1b, b'[', b'M', byte(button), byte(col + 1), byte(row + 1)]
        };
        return Some(event.repeat(count));
    }
    if modes.alt_screen && modes.alternate_scroll {
        let key: &[u8] = match (up, modes.app_cursor) {
            (true, true) => b"\x1bOA",
            (true, false) => b"\x1b[A",
            (false, true) => b"\x1bOB",
            (false, false) => b"\x1b[B",
        };
        return Some(key.repeat(count));
    }
    None
}

/// How a mouse selection grows: by cell (drag), by word (double click) or by line (triple click).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectKind {
    Cell,
    Word,
    Line,
}

/// A point of the visible screen for a selection: `right` when the pointer is on the right half
/// of the cell, so a drag that never leaves one half selects nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellPoint {
    pub col: u16,
    pub row: u16,
    pub right: bool,
}

/// Bytes for Enter with modifiers. Programs that turned on the kitty keyboard protocol get
/// `CSI 13;<mods> u`; the rest get Alt+Enter (`ESC CR`), which Claude Code, Codex and zsh read as
/// a newline in the prompt instead of a submit.
pub fn modified_enter(kitty: bool, shift: bool, alt: bool, ctrl: bool) -> Vec<u8> {
    if kitty {
        let mods = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl);
        return format!("\x1b[13;{mods}u").into_bytes();
    }
    b"\x1b\r".to_vec()
}

/// Abstract virtual terminal engine so the parser/grid backend can be swapped later
/// (for instance from `alacritty_terminal` to `libghostty-vt`) without touching sessions or UI.
pub trait VtEngine: Send {
    fn feed(&mut self, bytes: &[u8]);
    fn resize(&mut self, cols: u16, rows: u16);
    fn size(&self) -> (u16, u16);
    fn snapshot(&self) -> Screen;
    fn sync_deadline(&self) -> Option<Instant>;
    fn flush_sync_due(&mut self, now: Instant) -> bool;
    fn reported_app(&self) -> Option<&str>;
    fn agent_status(&self, cli: &str, title: &str) -> AgentStatus;
    fn clear_agent_status(&mut self);
    /// Absolute line of the top screen row (= scrollback length); stable while output scrolls.
    fn screen_top_line(&self) -> i64;
    /// Cursor at the current position in the output stream, including buffered moves.
    fn cursor_abs(&mut self) -> (i64, u16);
    /// Lines the view is scrolled back into the history.
    fn display_offset(&self) -> usize;
    /// Moves the view through the history (`lines` > 0 goes back); clamped. Returns whether it moved.
    fn scroll_display(&mut self, lines: i32) -> bool;
    /// Returns the view to the live output. Returns whether it moved.
    fn scroll_to_bottom(&mut self) -> bool;
    fn wheel_modes(&self) -> WheelModes;
    /// The program turned on the kitty keyboard protocol (disambiguated escape codes).
    fn kitty_keys(&self) -> bool;
    fn bracketed_paste(&self) -> bool;
    fn start_selection(&mut self, kind: SelectKind, at: CellPoint);
    fn extend_selection(&mut self, to: CellPoint);
    fn clear_selection(&mut self);
    /// The selected text, or `None` when nothing is selected.
    fn selection_text(&self) -> Option<String>;
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
    status_parser: alacritty_terminal::vte::Parser,
    status: StatusSignals,
    reply: Option<PtyWriteCallback>,
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
            // Answers `CSI ? u`, so CLIs can ask for Shift+Enter as its own key.
            kitty_keyboard: true,
            ..TermConfig::default()
        };
        let reply = callbacks.on_pty_write.clone();
        let term = Term::new(config, &size, TermListener { callbacks });
        Self {
            term,
            parser: Processor::new(),
            status_parser: Default::default(),
            status: StatusSignals::default(),
            reply,
            cols,
            rows,
        }
    }
}

impl AlacrittyVt {
    /// A visible-screen cell as a grid point, through the history the view is scrolled into.
    fn grid_point(&self, at: CellPoint) -> (Point, Side) {
        let offset = self.term.grid().display_offset() as i32;
        let line = i32::from(at.row.min(self.rows.saturating_sub(1))) - offset;
        let col = usize::from(at.col.min(self.cols.saturating_sub(1)));
        let side = if at.right { Side::Right } else { Side::Left };
        (Point::new(Line(line), Column(col)), side)
    }
}

impl VtEngine for AlacrittyVt {
    fn feed(&mut self, bytes: &[u8]) {
        self.status_parser.advance(&mut self.status, bytes);
        let queries = std::mem::take(&mut self.status.queries);
        if let Some(reply) = &self.reply {
            for _ in 0..queries {
                reply(agent_status::STATUS_REPLY);
            }
        }
        self.parser.advance(&mut self.term, bytes);
    }

    fn flush_sync_due(&mut self, now: Instant) -> bool {
        if self.sync_deadline().is_some_and(|deadline| now >= deadline) {
            self.parser.stop_sync(&mut self.term);
            true
        } else {
            false
        }
    }

    fn sync_deadline(&self) -> Option<Instant> {
        self.parser.sync_timeout().sync_timeout()
    }

    fn reported_app(&self) -> Option<&str> {
        self.status.app()
    }

    fn clear_agent_status(&mut self) {
        self.status.clear();
    }

    fn agent_status(&self, cli: &str, title: &str) -> AgentStatus {
        if let Some(status) = self.status.reported(cli) {
            return status;
        }
        let mut tail = Vec::new();
        if matches!(
            cli,
            "opencode" | "antigravity" | "grok" | "codex" | "claude" | "junie" | "pi"
        ) {
            let grid = self.term.grid();
            // Read live coordinates, not renderable_content(), which follows the user's scrollback.
            for row in (0..self.rows).rev().take(32) {
                let cells = &grid[Line(i32::from(row))];
                let line: String = (0..usize::from(self.cols))
                    .filter_map(|col| {
                        let cell = &cells[Column(col)];
                        (!cell
                            .flags
                            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER))
                        .then_some(cell.c)
                    })
                    .collect();
                if !line.trim().is_empty() {
                    tail.push(line.to_lowercase());
                }
                if tail.len() >= 12 {
                    break;
                }
            }
        }
        agent_status::fallback(cli, title, &tail, self.status.progress)
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        let cols = cols.max(2);
        let rows = rows.max(1);
        if self.cols == cols && self.rows == rows {
            return;
        }
        self.parser.stop_sync(&mut self.term);
        self.cols = cols;
        self.rows = rows;
        self.term.resize(GridSize {
            cols: usize::from(cols),
            rows: usize::from(rows),
        });
    }

    fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    fn screen_top_line(&self) -> i64 {
        self.term.grid().history_size() as i64
    }

    fn cursor_abs(&mut self) -> (i64, u16) {
        if self.parser.sync_bytes_count() > 0 {
            // Graphics APCs are handled outside the VT parser. Apply preceding cursor moves
            // before placing the image, then keep buffering until the program ends the redraw.
            self.parser.stop_sync(&mut self.term);
            self.parser.advance(&mut self.term, b"\x1b[?2026h");
        }
        let point = self.term.grid().cursor.point;
        (
            self.screen_top_line() + i64::from(point.line.0),
            point.column.0 as u16,
        )
    }

    fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    fn scroll_display(&mut self, lines: i32) -> bool {
        let before = self.display_offset();
        self.term.scroll_display(Scroll::Delta(lines));
        self.display_offset() != before
    }

    fn scroll_to_bottom(&mut self) -> bool {
        let before = self.display_offset();
        self.term.scroll_display(Scroll::Bottom);
        before != 0
    }

    fn wheel_modes(&self) -> WheelModes {
        let mode = *self.term.mode();
        WheelModes {
            mouse_report: mode.intersects(TermMode::MOUSE_MODE),
            sgr: mode.contains(TermMode::SGR_MOUSE),
            alt_screen: mode.contains(TermMode::ALT_SCREEN),
            alternate_scroll: mode.contains(TermMode::ALTERNATE_SCROLL),
            app_cursor: mode.contains(TermMode::APP_CURSOR),
        }
    }

    fn kitty_keys(&self) -> bool {
        self.term.mode().contains(TermMode::DISAMBIGUATE_ESC_CODES)
    }

    fn bracketed_paste(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    fn start_selection(&mut self, kind: SelectKind, at: CellPoint) {
        let ty = match kind {
            SelectKind::Cell => SelectionType::Simple,
            SelectKind::Word => SelectionType::Semantic,
            SelectKind::Line => SelectionType::Lines,
        };
        let (point, side) = self.grid_point(at);
        self.term.selection = Some(Selection::new(ty, point, side));
    }

    fn extend_selection(&mut self, to: CellPoint) {
        let (point, side) = self.grid_point(to);
        if let Some(selection) = &mut self.term.selection {
            selection.update(point, side);
        }
    }

    fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    fn selection_text(&self) -> Option<String> {
        self.term
            .selection_to_string()
            .filter(|text| !text.is_empty())
    }

    fn snapshot(&self) -> Screen {
        let content = self.term.renderable_content();
        let grid = self.term.grid();
        let display_offset = content.display_offset as i32;
        let cols = usize::from(self.cols);
        let rows_count = usize::from(self.rows);
        let selection = content.selection;

        let mut rows = Vec::with_capacity(rows_count);
        for r in 0..rows_count {
            let line = Line(r as i32 - display_offset);
            let grid_row = &grid[line];
            let wrapped = grid_row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);

            // Trim trailing blank cells that carry only default background and no underline.
            let mut last_used = 0usize;
            for c in (0..cols).rev() {
                let cell = &grid_row[Column(c)];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                let (_, bg, _) = resolve_cell_colors(cell, content.colors);
                let ch = if cell.c == '\0' { ' ' } else { cell.c };
                let has_underline = cell.flags.intersects(Flags::ALL_UNDERLINES);
                let has_hyperlink = cell.hyperlink().is_some();
                let selected = selection.is_some_and(|s| {
                    s.contains(Point::new(line, Column(c)))
                        || (cell.flags.contains(Flags::WIDE_CHAR)
                            && s.contains(Point::new(line, Column(c + 1))))
                });
                if ch != ' ' || bg != DEFAULT_BG || has_underline || has_hyperlink || selected {
                    last_used = c + 1;
                    break;
                }
            }

            let last_used = if wrapped { cols } else { last_used };
            let mut runs: Vec<Run> = Vec::new();
            let mut previous_is_monospace = false;
            for c in 0..last_used {
                let cell = &grid_row[Column(c)];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                let ch = if cell.c == '\0' { ' ' } else { cell.c };
                let (fg, bg, bold) = resolve_cell_colors(cell, content.colors);
                let underline = cell.flags.intersects(Flags::ALL_UNDERLINES);
                let hyperlink = cell.hyperlink().map(|h| h.uri().to_string());
                let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                    2
                } else {
                    1
                };
                let selected = selection.is_some_and(|s| {
                    s.contains(Point::new(line, Column(c)))
                        || (width == 2 && s.contains(Point::new(line, Column(c + 1))))
                });
                // Keep fallback glyphs, wide cells and combining sequences at their own columns.
                let combining = cell.zerowidth().unwrap_or_default();
                let is_monospace = width == 1 && combining.is_empty() && is_monospace_cell_char(ch);

                if let Some(prev) = runs.last_mut()
                    && is_monospace
                    && previous_is_monospace
                    && usize::from(prev.col + prev.cells) == c
                    && prev.fg == fg
                    && prev.bg == bg
                    && prev.bold == bold
                    && prev.underline == underline
                    && prev.selected == selected
                    && prev.hyperlink == hyperlink
                {
                    prev.text.push(ch);
                    prev.cells += width;
                } else {
                    let mut text = String::new();
                    text.push(ch);
                    text.extend(combining);
                    runs.push(Run {
                        text,
                        col: c as u16,
                        cells: width,
                        fg,
                        bg,
                        bold,
                        underline,
                        selected,
                        hyperlink,
                    });
                }
                previous_is_monospace = is_monospace;
            }

            rows.push(Row { runs, wrapped });
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
            display_offset: display_offset as u32,
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
    #[test]
    fn cell_runs_coalesce_native_monospace_glyphs_with_the_same_style() {
        let mut vt = AlacrittyVt::new(120, 2);
        let logo = format!("╹{}─█┃▄·░ •… Español", "▀".repeat(75));
        vt.feed(format!("{logo}\x1b[1m││").as_bytes());
        let screen = vt.snapshot();
        let runs = &screen.rows[0].runs;
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].text, logo);
        assert_eq!(usize::from(runs[0].cells), logo.chars().count());
        assert_eq!(runs[1].col, runs[0].cells);
        assert_eq!(runs[1].text, "││");
        assert!(runs[1].bold);
    }

    #[test]
    fn old_session_frames_can_coalesce_without_shifting_cells() {
        let mut vt = AlacrittyVt::new(12, 2);
        vt.feed("│雪e\u{301}ab·▀▄\x1b[31m█".as_bytes());
        let screen = vt.snapshot();
        let mut old_frame = screen.rows[0].clone();
        old_frame.runs.splice(
            3..4,
            ["ab", "·", "▀", "▄"]
                .into_iter()
                .scan(old_frame.runs[3].clone(), |run, text| {
                    let cells = text.chars().count() as u16;
                    let piece = Run {
                        text: text.into(),
                        cells,
                        ..run.clone()
                    };
                    run.col += cells;
                    Some(piece)
                }),
        );
        old_frame.coalesce_monospace_runs();
        assert_eq!(old_frame, screen.rows[0]);
    }

    #[test]
    fn cell_runs_keep_exact_columns_after_wide_and_combining_glyphs() {
        let mut vt = AlacrittyVt::new(12, 2);
        vt.feed("\x1b[48;2;30;40;50m│雪e\u{301}  ☆\x1b[12G│".as_bytes());
        let screen = vt.snapshot();
        let runs = &screen.rows[0].runs;
        assert!(
            runs.iter()
                .any(|run| run.text == "雪" && (run.col, run.cells) == (1, 2))
        );
        assert!(
            runs.iter()
                .any(|run| run.text == "e\u{301}" && (run.col, run.cells) == (3, 1))
        );
        assert!(
            runs.iter()
                .any(|run| run.text.ends_with('│') && run.col == 11)
        );
        assert_eq!(runs.iter().map(|run| run.cells).sum::<u16>(), 12);
    }

    #[test]
    fn synchronized_redraws_flush_even_when_the_writer_does_not_finish() {
        let mut vt = AlacrittyVt::new(12, 2);
        vt.feed(b"\x1b[?2026hcomposer");
        assert!(vt.snapshot().rows[0].plain_text().is_empty());
        let later = std::time::Instant::now() + std::time::Duration::from_secs(1);
        assert!(vt.flush_sync_due(later));
        assert_eq!(vt.snapshot().rows[0].plain_text(), "composer");
        assert!(!vt.flush_sync_due(later));
    }

    #[test]
    fn status_osc_negotiates_and_only_root_updates_the_pane() {
        let output = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = output.clone();
        let mut vt = AlacrittyVt::with_callbacks(
            50,
            6,
            VtCallbacks {
                on_pty_write: Some(std::sync::Arc::new(move |reply| {
                    captured.lock().unwrap().extend_from_slice(reply)
                })),
                ..Default::default()
            },
        );
        for byte in b"\x1b]7501;?\x1b\\\x1b]7501;state=working:app=opencode\x1b\\\x1b]7501;state=blocked:id=child\x07" {
            vt.feed(&[*byte]);
        }
        assert_eq!(*output.lock().unwrap(), agent_status::STATUS_REPLY);
        assert_eq!(vt.reported_app(), Some("opencode"));
        assert_eq!(vt.agent_status("opencode", ""), AgentStatus::Working);
        assert_eq!(vt.agent_status("codex", ""), AgentStatus::Idle);
        vt.feed(b"\x1b]7501;state=blocked:app=opencode:kind=permission\x07");
        assert_eq!(vt.agent_status("opencode", ""), AgentStatus::Blocked);
        vt.feed(b"\x1b]7501;state=done:app=opencode\x07");
        assert_eq!(vt.agent_status("opencode", ""), AgentStatus::Done);
        vt.feed(b"\x1b]133;A\x07");
        assert_eq!(vt.agent_status("opencode", ""), AgentStatus::Idle);
        assert!(vt.snapshot().rows[0].plain_text().is_empty());
    }

    #[test]
    fn pi_receives_program_status_support_before_the_da1_reply() {
        let replies = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = replies.clone();
        let mut vt = AlacrittyVt::with_callbacks(
            60,
            10,
            VtCallbacks {
                on_pty_write: Some(std::sync::Arc::new(move |reply| {
                    captured.lock().unwrap().extend_from_slice(reply)
                })),
                ..Default::default()
            },
        );
        vt.feed(b"\x1b[?u\x1b]7501;?\x1b\\\x1b[c");
        let reply = replies.lock().unwrap();
        assert!(reply.starts_with(agent_status::STATUS_REPLY));
        assert!(reply.ends_with(b"\x1b[?6c"));
        drop(reply);

        for (state, expected) in [
            ("idle", AgentStatus::Idle),
            ("working", AgentStatus::Working),
            ("blocked:kind=permission", AgentStatus::Blocked),
            ("done", AgentStatus::Done),
            ("error", AgentStatus::Failed),
        ] {
            vt.feed(format!("\x1b]7501;app=pi:state={state}\x1b\\").as_bytes());
            assert_eq!(vt.reported_app(), Some("pi"));
            assert_eq!(vt.agent_status("pi", "π - project"), expected);
        }
        vt.feed(b"\x1b]7501;state=clear\x1b\\");
        assert_eq!(vt.reported_app(), None);
    }

    #[test]
    fn status_falls_back_to_live_cli_footer_not_shell_output_or_history() {
        let mut vt = AlacrittyVt::new(72, 6);
        vt.feed(b"\x1b[6;1H  esc interrupt    ctrl+p commands");
        assert_eq!(
            vt.agent_status("opencode", "OpenCode"),
            AgentStatus::Working
        );
        assert_eq!(vt.agent_status("terminal", "OpenCode"), AgentStatus::Idle);
        vt.feed(b"\r\x1b[2K  ctrl+p commands");
        assert_eq!(vt.agent_status("opencode", "OpenCode"), AgentStatus::Idle);
        vt.feed("\r\x1b[2K↑/↓ Navigate · tab Amend · f full diff".as_bytes());
        assert_eq!(vt.agent_status("antigravity", "agy"), AgentStatus::Blocked);
        vt.feed(b"\r\x1b[2K    esc to cancel");
        assert_eq!(vt.agent_status("antigravity", "agy"), AgentStatus::Working);
        vt.feed(b"\r\x1b[2K? for shortcuts\x1b]9;4;1;-1\x07");
        assert_eq!(vt.agent_status("antigravity", "agy"), AgentStatus::Idle);
        assert_eq!(vt.agent_status("grok", "grok"), AgentStatus::Working);
        vt.feed(b"\x1b]9;4;0;0\x1b\\");
        assert_eq!(vt.agent_status("grok", "grok"), AgentStatus::Idle);
    }

    #[test]
    fn recognizes_claude_and_junie_approval_dialogs_in_live_terminal() {
        let mut vt = AlacrittyVt::new(72, 8);
        vt.feed(
            b"\x1b[5;1HDo you want to proceed?\x1b[6;1H1. Yes\x1b[7;1H2. No\x1b[8;1HEsc to cancel",
        );
        assert_eq!(
            vt.agent_status("claude", "Claude Code"),
            AgentStatus::Blocked
        );
        assert_eq!(
            vt.agent_status("terminal", "Claude Code"),
            AgentStatus::Idle
        );
        vt.feed("\x1b[2J\x1b[8;1HWorking… esc to stop".as_bytes());
        assert_eq!(vt.agent_status("junie", "Junie"), AgentStatus::Working);
        vt.feed(b"\x1b[7;1HAllow once\x1b[8;1H\x1b[2KDeny");
        assert_eq!(vt.agent_status("junie", "Junie"), AgentStatus::Blocked);
        vt.feed(b"\x1b[2J\x1b[8;1H> ");
        assert_eq!(vt.agent_status("junie", "Junie"), AgentStatus::Idle);
    }

    use super::*;

    #[test]
    fn wheel_goes_to_mouse_reports_then_arrows_then_history() {
        let sgr = WheelModes {
            mouse_report: true,
            sgr: true,
            ..WheelModes::default()
        };
        assert_eq!(
            wheel_input(sgr, 2, 4, 9).unwrap(),
            b"\x1b[<64;5;10M\x1b[<64;5;10M".to_vec()
        );
        assert_eq!(
            wheel_input(sgr, -1, 0, 0).unwrap(),
            b"\x1b[<65;1;1M".to_vec()
        );
        let x10 = WheelModes {
            mouse_report: true,
            ..WheelModes::default()
        };
        assert_eq!(
            wheel_input(x10, 1, 0, 0).unwrap(),
            vec![0x1b, b'[', b'M', 96, 33, 33]
        );
        let alt = WheelModes {
            alt_screen: true,
            alternate_scroll: true,
            ..WheelModes::default()
        };
        assert_eq!(
            wheel_input(alt, -2, 0, 0).unwrap(),
            b"\x1b[B\x1b[B".to_vec()
        );
        let app = WheelModes {
            app_cursor: true,
            ..alt
        };
        assert_eq!(wheel_input(app, 1, 0, 0).unwrap(), b"\x1bOA".to_vec());
        assert_eq!(
            wheel_input(
                WheelModes {
                    alternate_scroll: true,
                    ..WheelModes::default()
                },
                3,
                0,
                0
            ),
            None
        );
    }

    #[test]
    fn scrolling_back_shows_history_and_follows_new_output_from_the_same_place() {
        let mut vt = AlacrittyVt::new(20, 3);
        for line in 0..10 {
            vt.feed(format!("line {line}\r\n").as_bytes());
        }
        assert!(vt.wheel_modes().alternate_scroll);
        assert!(vt.scroll_display(2));
        let screen = vt.snapshot();
        assert_eq!(screen.display_offset, 2);
        assert_eq!(screen.rows[0].plain_text(), "line 6");
        assert!(!screen.cursor.visible || screen.cursor.row != 2);
        // New output keeps the scrolled-back view on the same lines.
        vt.feed(b"line 10\r\n");
        assert_eq!(vt.snapshot().rows[0].plain_text(), "line 6");
        assert!(vt.scroll_to_bottom());
        assert_eq!(vt.snapshot().display_offset, 0);
        assert!(!vt.scroll_to_bottom());
    }

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

    #[test]
    fn mouse_selection_marks_cells_and_copies_text() {
        let mut vt = AlacrittyVt::new(20, 3);
        vt.feed(b"hello world");
        let at = |col, right| CellPoint { col, row: 0, right };
        vt.start_selection(SelectKind::Cell, at(0, false));
        assert_eq!(vt.selection_text(), None, "a press alone selects nothing");
        vt.extend_selection(at(4, true));
        assert_eq!(vt.selection_text().as_deref(), Some("hello"));
        let runs = &vt.snapshot().rows[0].runs;
        assert_eq!((runs[0].text.as_str(), runs[0].selected), ("hello", true));
        assert!(!runs[1].selected);

        vt.start_selection(SelectKind::Word, at(7, false));
        assert_eq!(vt.selection_text().as_deref(), Some("world"));
        vt.clear_selection();
        assert_eq!(vt.selection_text(), None);
    }

    #[test]
    fn shift_enter_uses_kitty_keys_only_when_the_program_asked() {
        let mut vt = AlacrittyVt::new(20, 3);
        assert!(!vt.kitty_keys());
        assert_eq!(modified_enter(false, true, false, false), b"\x1b\r");
        // Push "disambiguate escape codes" (kitty `CSI > 1 u`).
        vt.feed(b"\x1b[>1u");
        assert!(vt.kitty_keys());
        assert_eq!(modified_enter(true, true, false, false), b"\x1b[13;2u");
        assert_eq!(modified_enter(true, false, false, true), b"\x1b[13;5u");
    }

    #[test]
    fn preserves_osc8_hyperlinks_on_cell_runs() {
        let mut vt = AlacrittyVt::new(80, 4);
        vt.feed(b"open \x1b]8;;file:///tmp/demo.rs#L12\x07demo.rs\x1b]8;;\x07 now\r\n");
        let screen = vt.snapshot();
        let runs = &screen.rows[0].runs;
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].text, "open ");
        assert_eq!(runs[0].hyperlink, None);
        assert_eq!(runs[1].text, "demo.rs");
        assert_eq!(
            runs[1].hyperlink.as_deref(),
            Some("file:///tmp/demo.rs#L12")
        );
        assert_eq!(runs[2].text, " now");
        assert_eq!(runs[2].hyperlink, None);
    }
}
