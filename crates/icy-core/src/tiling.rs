//! Niri & Hyprland-style tiling layout engine for terminal panes.
//! Pure state, testable without a window or GPU.
//!
//! Niri: an infinite horizontal strip of columns. Each column has a width as a proportion of the
//! viewport (presets 1/3, 1/2, 2/3, `±10%`, maximize to 1.0) and the view scrolls just enough to
//! keep the focused column on screen. Hyprland: every column fits the viewport; widths are
//! fractions that sum to 1 and resizing moves the border shared with a neighbour
//! (`resizeactive`). In both modes panes inside a column have height fractions summing to 1.
//!
//! The niri behaviour (view fitting, center-focused-column, per-column focus memory, returning to
//! the previous column when a new window closes, preset cycling, consume/expel/swap, column moves
//! between workspaces) is adapted from niri's `src/layout/scrolling.rs` and `monitor.rs`
//! (<https://github.com/niri-wm/niri>, GPL-3.0-or-later, commit in `niri-ref.txt`).

use serde::{Deserialize, Serialize};

pub const PRESET_WIDTHS: [f32; 3] = [1.0 / 3.0, 0.5, 2.0 / 3.0];
pub const RESIZE_STEP: f32 = 0.1;
const MIN_FRACTION: f32 = 0.1;
const EPS: f32 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TilingMode {
    /// Scrollable horizontal columns (each column can stack vertical panes), like Niri.
    Niri,
    /// All columns fit the viewport and share its width, like Hyprland.
    Hyprland,
}

/// niri `center-focused-column`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CenterFocusedColumn {
    /// Scroll the least that brings the focused column fully on screen.
    #[default]
    Never,
    Always,
    /// Center only when the previous and the new focused column don't fit on screen together.
    OnOverflow,
}

/// niri `layout { … }` options that shape the scrolling strip.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutOptions {
    /// Width of a new column as a fraction of the viewport (niri `default-column-width`).
    pub default_column_width: f32,
    pub center_focused_column: CenterFocusedColumn,
    /// A lone column is centered whatever `center_focused_column` says.
    pub always_center_single_column: bool,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            default_column_width: 0.5,
            center_focused_column: CenterFocusedColumn::Never,
            always_center_single_column: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusDir {
    Left,
    Right,
    Up,
    Down,
}

/// Where a dragged window lands (niri interactive move / Hyprland `movewindow`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropTarget {
    /// A new column inserted before column `0..=len`.
    NewColumn(usize),
    /// Stacked into column `col`, before pane `row` (`row == len` appends).
    Into { col: usize, row: usize },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TileColumn {
    id: u64,
    width: f32,
    /// Width before `maximize_column`, restored on the next toggle.
    restore_width: Option<f32>,
    panes: Vec<u64>,
    heights: Vec<f32>,
    /// The pane focused last in this column: focusing the column returns to it (niri).
    #[serde(default)]
    active: usize,
    /// Index in `PRESET_WIDTHS` of the last preset applied, for cycling both ways.
    #[serde(default)]
    preset: Option<usize>,
}

impl TileColumn {
    fn new(id: u64, width: f32, pane: u64) -> Self {
        Self {
            id,
            width,
            restore_width: None,
            panes: vec![pane],
            heights: vec![1.0],
            active: 0,
            preset: None,
        }
    }

    fn active_pane(&self) -> u64 {
        self.panes[self.active.min(self.panes.len() - 1)]
    }
}

/// A column as laid out on the strip: `x` and `width` are fractions of the viewport width,
/// `heights` are fractions of the column height.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnView {
    pub id: u64,
    pub panes: Vec<u64>,
    pub heights: Vec<f32>,
    pub x: f32,
    pub width: f32,
    pub expanded: bool,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TilingView {
    pub mode: TilingMode,
    pub focused_session: u64,
    /// The focused pane fills the viewport (fullscreen).
    pub maximized: bool,
    /// Left edge of the viewport on the strip, in viewport widths.
    pub view_offset: f32,
    pub strip_width: f32,
    pub total_cols: usize,
    pub columns: Vec<ColumnView>,
    /// Every workspace top to bottom (filled by `Workspaces::snapshot`).
    pub workspaces: Vec<WorkspaceView>,
    pub active_workspace: usize,
}

/// One workspace in the niri-style vertical list: its windows in reading order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub panes: Vec<u64>,
    pub focused: u64,
    /// Its tiled geometry, so the overview draws each workspace as a miniature of its layout.
    pub columns: Vec<ColumnView>,
    pub view_offset: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TilingWorkspace {
    mode: TilingMode,
    columns: Vec<TileColumn>,
    next_col_id: u64,
    focused_session: u64,
    fullscreen: bool,
    view_offset: f32,
    #[serde(default)]
    options: LayoutOptions,
    /// niri `activate_prev_column_on_removal`: the view offset from before a new column took
    /// focus. Closing that column while it still has focus returns to the column on its left
    /// and to this offset; any other focus change forgets it.
    #[serde(default)]
    restore_on_close: Option<f32>,
}

/// Moves `delta` of a fraction from `values[from]` to `values[to]`, keeping both >= MIN_FRACTION.
/// Returns false when nothing could move.
fn transfer(values: &mut [f32], to: usize, from: usize, delta: f32) -> bool {
    let delta = if delta >= 0.0 {
        delta.min(values[from] - MIN_FRACTION)
    } else {
        -(-delta).min(values[to] - MIN_FRACTION)
    };
    if delta.abs() < EPS {
        return false;
    }
    values[to] += delta;
    values[from] -= delta;
    true
}

fn normalize(values: &mut [f32]) {
    let sum: f32 = values.iter().sum();
    if sum > EPS {
        for v in values.iter_mut() {
            *v /= sum;
        }
    }
}

impl TilingWorkspace {
    /// A workspace with no windows (niri keeps one at the end of the list).
    pub fn empty(mode: TilingMode) -> Self {
        Self {
            mode,
            columns: Vec::new(),
            next_col_id: 1,
            focused_session: 0,
            fullscreen: false,
            view_offset: 0.0,
            options: LayoutOptions::default(),
            restore_on_close: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn contains(&self, pane: u64) -> bool {
        self.locate(pane).is_some()
    }

    pub fn new(initial_session: u64, mode: TilingMode) -> Self {
        Self {
            mode,
            columns: vec![TileColumn::new(
                1,
                match mode {
                    TilingMode::Niri => LayoutOptions::default().default_column_width,
                    TilingMode::Hyprland => 1.0,
                },
                initial_session,
            )],
            next_col_id: 2,
            focused_session: initial_session,
            fullscreen: false,
            view_offset: 0.0,
            options: LayoutOptions::default(),
            restore_on_close: None,
        }
    }

    pub fn set_options(&mut self, options: LayoutOptions) {
        self.options = options;
        self.ensure_focused_visible();
    }

    pub fn mode(&self) -> TilingMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: TilingMode) {
        self.mode = mode;
        if mode == TilingMode::Hyprland {
            self.normalize_widths();
        }
        self.ensure_focused_visible();
    }

    pub fn focused_session(&self) -> u64 {
        self.focused_session
    }

    pub fn is_maximized(&self) -> bool {
        self.fullscreen
    }

    /// Toggles the workspace's fullscreen view, keeping its tiled sizes for when it exits.
    pub fn toggle_maximize(&mut self) {
        if !self.is_empty() {
            self.fullscreen = !self.fullscreen;
        }
    }

    /// Scrolls the niri strip by whole columns.
    pub fn scroll_columns(&mut self, delta: isize) -> bool {
        let Some((col, _)) = self.locate(self.focused_session) else {
            return false;
        };
        let target = (col as isize + delta).clamp(0, self.columns.len() as isize - 1) as usize;
        if target == col {
            return false;
        }
        self.focus_pane(self.columns[target].active_pane());
        self.reveal(Some(col));
        true
    }

    /// Adds a new column right after the focused one. niri opens it at `default_column_width`
    /// and never resizes the others; Hyprland splits the focused column's width.
    pub fn open_column(&mut self, session_id: u64) {
        let focused = self.locate(self.focused_session).map(|(c, _)| c);
        let insert_idx = focused.map_or(self.columns.len(), |c| c + 1);
        let width = match (self.mode, focused) {
            (TilingMode::Hyprland, Some(c)) => {
                self.columns[c].width /= 2.0;
                self.columns[c].width
            }
            (TilingMode::Hyprland, None) => 1.0,
            (TilingMode::Niri, _) => self.options.default_column_width,
        };
        let id = self.next_col_id;
        self.next_col_id += 1;
        self.columns
            .insert(insert_idx, TileColumn::new(id, width, session_id));
        if self.mode == TilingMode::Hyprland {
            self.normalize_widths();
        }
        let before = self.view_offset;
        self.focus_pane(session_id);
        if focused.is_some() && self.mode == TilingMode::Niri {
            self.restore_on_close = Some(before);
        }
        self.reveal(focused);
    }

    /// Stacks a new pane below the focused one, splitting its height in half.
    pub fn stack_vertical(&mut self, session_id: u64) {
        let Some((c, r)) = self.locate(self.focused_session) else {
            self.open_column(session_id);
            return;
        };
        let col = &mut self.columns[c];
        col.heights[r] /= 2.0;
        let half = col.heights[r];
        col.panes.insert(r + 1, session_id);
        col.heights.insert(r + 1, half);
        self.focus_pane(session_id);
        self.ensure_focused_visible();
    }

    /// Focuses an existing session (adding it as a column if not tiled yet).
    pub fn focus_session(&mut self, session_id: u64) {
        if self.locate(session_id).is_none() {
            self.open_column(session_id);
            return;
        }
        let from = self.locate(self.focused_session).map(|(c, _)| c);
        self.focus_pane(session_id);
        self.reveal(from);
    }

    /// Left / right land on the pane the neighbouring column had focused (niri keeps one per
    /// column); up / down move inside the column.
    pub fn focus_direction(&mut self, dir: FocusDir) -> bool {
        let Some((col, row)) = self.locate(self.focused_session) else {
            return false;
        };
        let next = match dir {
            FocusDir::Left if col > 0 => Some(self.columns[col - 1].active_pane()),
            FocusDir::Right if col + 1 < self.columns.len() => {
                Some(self.columns[col + 1].active_pane())
            }
            FocusDir::Up if row > 0 => self.columns[col].panes.get(row - 1).copied(),
            FocusDir::Down => self.columns[col].panes.get(row + 1).copied(),
            _ => None,
        };
        if let Some(id) = next {
            self.focus_pane(id);
            self.reveal(Some(col));
            true
        } else {
            false
        }
    }

    /// niri `focus-column-first/last`.
    pub fn focus_column_edge(&mut self, last: bool) -> bool {
        let Some((col, _)) = self.locate(self.focused_session) else {
            return false;
        };
        let target = if last { self.columns.len() - 1 } else { 0 };
        if target == col {
            return false;
        }
        self.focus_pane(self.columns[target].active_pane());
        self.reveal(Some(col));
        true
    }

    /// Niri `set-column-width ±10%` / Hyprland `resizeactive`: grows (or shrinks) the focused column.
    pub fn adjust_column_width(&mut self, delta: f32) -> bool {
        let Some((c, _)) = self.locate(self.focused_session) else {
            return false;
        };
        self.columns[c].restore_width = None;
        self.columns[c].preset = None;
        let changed = match self.mode {
            TilingMode::Niri => {
                let next = (self.columns[c].width + delta).clamp(MIN_FRACTION, 1.0);
                let changed = (next - self.columns[c].width).abs() > EPS;
                self.columns[c].width = next;
                changed
            }
            TilingMode::Hyprland => {
                let Some(neighbour) = self.width_neighbour(c) else {
                    return false;
                };
                let mut widths: Vec<f32> = self.columns.iter().map(|col| col.width).collect();
                let changed = transfer(&mut widths, c, neighbour, delta);
                for (col, w) in self.columns.iter_mut().zip(widths) {
                    col.width = w;
                }
                changed
            }
        };
        self.ensure_focused_visible();
        changed
    }

    /// Mouse drag on the edge after column `col` (Silica / Hyprland border drag). A border shared
    /// with a next column moves width between the two in both modes, so nothing else shifts; Niri's
    /// trailing edge (after the last column) grows or shrinks that column.
    pub fn resize_column(&mut self, col: usize, delta: f32) -> bool {
        self.resize_column_edge(col, delta, false)
    }

    /// In Niri mode the left handle (before the first column) resizes that column and keeps its
    /// opposite edge stationary as far as the scroll bounds allow.
    pub fn resize_column_edge(&mut self, col: usize, delta: f32, left: bool) -> bool {
        if col >= self.columns.len() || !delta.is_finite() || delta.abs() < EPS {
            return false;
        }
        let changed = match self.mode {
            // Shared border: trade width with the neighbour so the strip and the view stay put
            // (growing one column would scroll its neighbour out of view instead).
            TilingMode::Niri if !left && col + 1 < self.columns.len() => {
                let mut widths: Vec<f32> = self.columns.iter().map(|c| c.width).collect();
                let changed = transfer(&mut widths, col, col + 1, delta);
                for (c, w) in self.columns.iter_mut().zip(widths) {
                    c.width = w;
                }
                changed
            }
            TilingMode::Niri => {
                let old = self.columns[col].width;
                let next = (old + if left { -delta } else { delta }).clamp(MIN_FRACTION, 1.0);
                if (next - old).abs() < EPS {
                    return false;
                }
                self.columns[col].width = next;
                if left {
                    self.view_offset += next - old;
                }
                true
            }
            TilingMode::Hyprland => {
                if left || col + 1 >= self.columns.len() {
                    return false;
                }
                let mut widths: Vec<f32> = self.columns.iter().map(|c| c.width).collect();
                let total: f32 = widths.iter().sum();
                // Widths are normalized in the view; the drag delta is in view fractions.
                let changed = transfer(&mut widths, col, col + 1, delta * total);
                for (c, w) in self.columns.iter_mut().zip(widths) {
                    c.width = w;
                }
                changed
            }
        };
        if changed {
            self.columns[col].restore_width = None;
            self.columns[col].preset = None;
            self.ensure_focused_visible();
        }
        changed
    }

    pub fn resize_at(
        &mut self,
        column_id: u64,
        row: Option<usize>,
        delta: f32,
        left: bool,
    ) -> bool {
        let Some(col) = self
            .columns
            .iter()
            .position(|column| column.id == column_id)
        else {
            return false;
        };
        if self.is_maximized() || !delta.is_finite() {
            return false;
        }
        match row {
            Some(row) => self.resize_pane(col, row, delta),
            None => self.resize_column_edge(col, delta, left),
        }
    }

    /// Mouse drag on the border between panes `row` and `row + 1` of column `col`.
    pub fn resize_pane(&mut self, col: usize, row: usize, delta: f32) -> bool {
        let Some(column) = self.columns.get_mut(col) else {
            return false;
        };
        if row + 1 >= column.panes.len() {
            return false;
        }
        let total: f32 = column.heights.iter().sum();
        transfer(&mut column.heights, row, row + 1, delta * total)
    }

    /// niri `switch-preset-column-width` (`back`: `switch-preset-column-width-back`): steps the
    /// remembered preset index, or from a free width picks the next larger (smaller) preset.
    pub fn cycle_column_width(&mut self, back: bool) -> bool {
        let Some((c, _)) = self.locate(self.focused_session) else {
            return false;
        };
        let n = PRESET_WIDTHS.len();
        let col = &self.columns[c];
        let current = col.width;
        let index = match col.preset.filter(|_| col.restore_width.is_none()) {
            Some(i) if back => (i + n - 1) % n,
            Some(i) => (i + 1) % n,
            None if back => PRESET_WIDTHS
                .iter()
                .rposition(|&p| p < current - EPS)
                .unwrap_or(n - 1),
            None => PRESET_WIDTHS
                .iter()
                .position(|&p| p > current + EPS)
                .unwrap_or(0),
        };
        let changed = self.adjust_column_width(PRESET_WIDTHS[index] - current);
        self.columns[c].preset = Some(index);
        changed
    }

    /// Niri `maximize-column`: toggles the focused tiled column between full width and its last width.
    pub fn maximize_column(&mut self) -> bool {
        if self.mode != TilingMode::Niri || self.fullscreen {
            return false;
        }
        let Some((c, _)) = self.locate(self.focused_session) else {
            return false;
        };
        let col = &mut self.columns[c];
        match col.restore_width.take() {
            Some(previous) => col.width = previous,
            None if col.width >= 1.0 - EPS => return false,
            None => {
                col.restore_width = Some(col.width);
                col.width = 1.0;
            }
        }
        self.ensure_focused_visible();
        true
    }

    /// Niri `set-window-height ±10%` / Hyprland vertical `resizeactive` inside the focused column.
    pub fn adjust_pane_height(&mut self, delta: f32) -> bool {
        let Some((c, r)) = self.locate(self.focused_session) else {
            return false;
        };
        let col = &mut self.columns[c];
        let neighbour = if r + 1 < col.panes.len() {
            r + 1
        } else if r > 0 {
            r - 1
        } else {
            return false;
        };
        transfer(&mut col.heights, r, neighbour, delta)
    }

    /// Niri `center-column`: scrolls so the focused column sits in the middle of the viewport.
    pub fn center_column(&mut self) -> bool {
        if self.mode != TilingMode::Niri {
            return false;
        }
        let Some((c, _)) = self.locate(self.focused_session) else {
            return false;
        };
        let (x, w) = self.layout()[c];
        // niri never clamps the view to the ends of the strip.
        self.view_offset = x + w / 2.0 - 0.5;
        true
    }

    /// Niri `move-column-left/right`: swaps the focused column with its neighbour.
    pub fn move_column(&mut self, dir: FocusDir) -> bool {
        let Some((c, _)) = self.locate(self.focused_session) else {
            return false;
        };
        let target = match dir {
            FocusDir::Left if c > 0 => c - 1,
            FocusDir::Right if c + 1 < self.columns.len() => c + 1,
            _ => return false,
        };
        self.columns.swap(c, target);
        self.reveal(Some(c));
        true
    }

    /// niri `move-column-to-first/last`.
    pub fn move_column_to_edge(&mut self, last: bool) -> bool {
        let Some((c, _)) = self.locate(self.focused_session) else {
            return false;
        };
        let target = if last { self.columns.len() - 1 } else { 0 };
        if target == c {
            return false;
        }
        let column = self.columns.remove(c);
        self.columns.insert(target, column);
        self.reveal(Some(c));
        true
    }

    /// Reorders the focused pane within its column, retaining its size and focus.
    pub fn move_pane_in_column(&mut self, dir: FocusDir) -> bool {
        let Some((col, row)) = self.locate(self.focused_session) else {
            return false;
        };
        let column = &mut self.columns[col];
        let target = match dir {
            FocusDir::Up if row > 0 => row - 1,
            FocusDir::Down if row + 1 < column.panes.len() => row + 1,
            _ => return false,
        };
        column.panes.swap(row, target);
        column.heights.swap(row, target);
        column.active = target;
        true
    }

    /// Niri `consume-or-expel-window-left/right`: a pane sharing its column is expelled into a new
    /// column on that side; a pane alone in its column is consumed into the neighbouring column.
    pub fn consume_or_expel(&mut self, dir: FocusDir) -> bool {
        let Some((c, r)) = self.locate(self.focused_session) else {
            return false;
        };
        let pane = self.focused_session;
        if self.columns[c].panes.len() > 1 {
            self.remove_pane_at(c, r);
            let width = match self.mode {
                // niri: the expelled window keeps its column's width.
                TilingMode::Niri => self.columns[c].width,
                TilingMode::Hyprland => {
                    self.columns[c].width /= 2.0;
                    self.columns[c].width
                }
            };
            let id = self.next_col_id;
            self.next_col_id += 1;
            let at = if dir == FocusDir::Left { c } else { c + 1 };
            self.columns.insert(at, TileColumn::new(id, width, pane));
        } else {
            let target = match dir {
                FocusDir::Left if c > 0 => c - 1,
                FocusDir::Right if c + 1 < self.columns.len() => c + 1,
                _ => return false,
            };
            let width = self.columns.remove(c).width;
            let target = if target > c { target - 1 } else { target };
            let col = &mut self.columns[target];
            if self.mode == TilingMode::Hyprland {
                col.width += width;
            }
            let last = col.heights.len() - 1;
            col.heights[last] /= 2.0;
            let half = col.heights[last];
            col.panes.push(pane);
            col.heights.push(half);
        }
        if self.mode == TilingMode::Hyprland {
            self.normalize_widths();
        }
        self.focus_pane(pane);
        self.ensure_focused_visible();
        true
    }

    /// niri `consume-window-into-column`: the first window of the next column joins the bottom
    /// of the focused column.
    pub fn consume_into_column(&mut self) -> bool {
        let Some((c, _)) = self.locate(self.focused_session) else {
            return false;
        };
        if c + 1 >= self.columns.len() {
            return false;
        }
        let pane = self.columns[c + 1].panes[0];
        self.remove_pane_at(c + 1, 0);
        if self.columns[c + 1].panes.is_empty() {
            let width = self.columns.remove(c + 1).width;
            if self.mode == TilingMode::Hyprland {
                self.columns[c].width += width;
            }
        }
        Self::append_share(&mut self.columns[c], pane);
        if self.mode == TilingMode::Hyprland {
            self.normalize_widths();
        }
        self.ensure_focused_visible();
        true
    }

    /// niri `expel-window-from-column`: the focused column's last window becomes a new column on
    /// its right.
    pub fn expel_from_column(&mut self) -> bool {
        let Some((c, _)) = self.locate(self.focused_session) else {
            return false;
        };
        let last = self.columns[c].panes.len() - 1;
        if last == 0 {
            return false;
        }
        let pane = self.columns[c].panes[last];
        self.remove_pane_at(c, last);
        let width = match self.mode {
            TilingMode::Niri => self.options.default_column_width,
            TilingMode::Hyprland => {
                self.columns[c].width /= 2.0;
                self.columns[c].width
            }
        };
        let id = self.next_col_id;
        self.next_col_id += 1;
        self.columns.insert(c + 1, TileColumn::new(id, width, pane));
        if self.mode == TilingMode::Hyprland {
            self.normalize_widths();
        }
        self.focus_pane(self.focused_session);
        self.ensure_focused_visible();
        true
    }

    /// niri `swap-window-left/right`: trades the focused window with the focused one of the
    /// neighbouring column; two lone windows swap their whole columns.
    pub fn swap_window(&mut self, dir: FocusDir) -> bool {
        let Some((c, r)) = self.locate(self.focused_session) else {
            return false;
        };
        let t = match dir {
            FocusDir::Left if c > 0 => c - 1,
            FocusDir::Right if c + 1 < self.columns.len() => c + 1,
            _ => return false,
        };
        if self.columns[c].panes.len() == 1 && self.columns[t].panes.len() == 1 {
            return self.move_column(dir);
        }
        let tr = self.columns[t].active.min(self.columns[t].panes.len() - 1);
        let pane = self.columns[c].panes[r];
        self.columns[c].panes[r] = self.columns[t].panes[tr];
        self.columns[t].panes[tr] = pane;
        self.focus_pane(pane);
        self.reveal(Some(c));
        true
    }

    /// niri `expand-column-to-available-width`: the focused column takes the room the other fully
    /// visible columns leave; alone on screen it toggles full width.
    pub fn expand_column_to_available_width(&mut self) -> bool {
        if self.mode != TilingMode::Niri || self.fullscreen {
            return false;
        }
        let Some((c, _)) = self.locate(self.focused_session) else {
            return false;
        };
        let layout = self.layout();
        let offset = self.view_offset;
        let shown: Vec<usize> = (0..layout.len())
            .filter(|&i| {
                layout[i].0 >= offset - EPS && layout[i].0 + layout[i].1 <= offset + 1.0 + EPS
            })
            .collect();
        if !shown.contains(&c) {
            return false;
        }
        if shown.len() == 1 {
            return self.maximize_column();
        }
        let others: f32 = shown
            .iter()
            .filter(|&&i| i != c)
            .map(|&i| layout[i].1)
            .sum();
        let target = (1.0 - others).max(MIN_FRACTION);
        let col = &mut self.columns[c];
        if (target - col.width).abs() < EPS {
            return false;
        }
        col.width = target;
        col.preset = None;
        col.restore_width = None;
        self.view_offset = layout[shown[0]].0;
        true
    }

    /// Moves a pane to `target` (drag and drop). A pane leaving a column alone takes the column
    /// (and its width) with it; a pane stacked into a column gets an equal share of its height.
    /// Returns false for a drop on its own slot.
    pub fn move_pane(&mut self, pane: u64, target: DropTarget) -> bool {
        let Some((c, r)) = self.locate(pane) else {
            return false;
        };
        let alone = self.columns[c].panes.len() == 1;
        let noop = match target {
            DropTarget::NewColumn(i) => alone && (i == c || i == c + 1),
            DropTarget::Into { col, row } => col == c && (alone || row == r || row == r + 1),
        };
        if noop || matches!(target, DropTarget::Into { col, .. } if col >= self.columns.len()) {
            return false;
        }
        let source_column = if alone {
            Some(self.columns.remove(c))
        } else {
            self.remove_pane_at(c, r);
            None
        };
        // Indices were given against the layout before the source column disappeared.
        let shift = |i: usize| if alone && i > c { i - 1 } else { i };
        match target {
            DropTarget::NewColumn(i) => {
                let at = shift(i).min(self.columns.len());
                let column = source_column.unwrap_or_else(|| {
                    let width = match self.mode {
                        TilingMode::Niri => 0.5,
                        TilingMode::Hyprland => 1.0 / (self.columns.len() + 1) as f32,
                    };
                    let id = self.next_col_id;
                    self.next_col_id += 1;
                    TileColumn::new(id, width, pane)
                });
                self.columns.insert(at, column);
            }
            DropTarget::Into { col, row } => {
                let col = shift(col);
                let row = if !alone && col == c && row > r {
                    row - 1
                } else {
                    row
                };
                let column = &mut self.columns[col];
                let share = 1.0 / (column.panes.len() + 1) as f32;
                column.heights.iter_mut().for_each(|h| *h *= 1.0 - share);
                let row = row.min(column.panes.len());
                column.panes.insert(row, pane);
                column.heights.insert(row, share);
            }
        }
        if self.mode == TilingMode::Hyprland {
            self.normalize_widths();
        }
        self.focus_pane(pane);
        self.ensure_focused_visible();
        true
    }

    /// Equalizes every column width and pane height.
    pub fn reset_sizes(&mut self) {
        let n = self.columns.len().max(1) as f32;
        let width = match self.mode {
            TilingMode::Hyprland => 1.0 / n,
            TilingMode::Niri => self.options.default_column_width,
        };
        for col in &mut self.columns {
            col.width = width;
            col.restore_width = None;
            col.preset = None;
            let h = 1.0 / col.panes.len() as f32;
            col.heights.iter_mut().for_each(|v| *v = h);
        }
        self.ensure_focused_visible();
    }

    pub fn remove_session(&mut self, session_id: u64) {
        let Some((col, row)) = self.locate(session_id) else {
            return;
        };
        let had_focus = self.focused_session == session_id;
        let focused_col = self.locate(self.focused_session).map(|(c, _)| c);
        let restore = had_focus.then(|| self.restore_on_close.take()).flatten();
        // Fullscreen belongs to the pane shown: closing it brings the tiled layout back instead
        // of handing fullscreen to whichever pane gets focus next.
        if self.fullscreen && had_focus {
            self.fullscreen = false;
        }
        self.remove_pane_at(col, row);
        let column_gone = self.columns[col].panes.is_empty();
        if column_gone {
            let width = self.columns.remove(col).width;
            if self.mode == TilingMode::Hyprland {
                if let Some(n) = self.columns.get_mut(col.saturating_sub(1)) {
                    n.width += width;
                }
                self.normalize_widths();
            }
        }
        if self.columns.is_empty() {
            self.focused_session = 0;
            self.fullscreen = false;
            self.restore_on_close = None;
            self.view_offset = 0.0;
            return;
        }
        if !had_focus {
            // niri `remove_column_by_idx`: a column removed left of the focused one shifts the
            // column to return to, so it is forgotten.
            if column_gone && focused_col.is_some_and(|f| col < f) {
                self.restore_on_close = None;
            }
            self.ensure_focused_visible();
            return;
        }
        // niri: a column opened from `col - 1` and closed before focus moved hands focus and the
        // view back; otherwise focus goes to the right neighbour (or the new last column).
        let next_col = match restore {
            Some(offset) if column_gone && col > 0 => {
                self.view_offset = offset;
                col - 1
            }
            _ if column_gone => col.min(self.columns.len() - 1),
            _ => col,
        };
        self.focus_pane(self.columns[next_col].active_pane());
        self.ensure_focused_visible();
    }

    /// Takes the focused column out (to move it to another workspace), focusing like a close.
    fn take_focused_column(&mut self) -> Option<TileColumn> {
        let (c, _) = self.locate(self.focused_session)?;
        let column = self.columns.remove(c);
        self.restore_on_close = None;
        self.fullscreen = false;
        if self.mode == TilingMode::Hyprland {
            self.normalize_widths();
        }
        match self
            .columns
            .get(c.min(self.columns.len().saturating_sub(1)))
        {
            Some(next) => self.focus_pane(next.active_pane()),
            None => {
                self.focused_session = 0;
                self.view_offset = 0.0;
            }
        }
        self.ensure_focused_visible();
        Some(column)
    }

    /// Inserts a column after the focused one and focuses it.
    fn insert_column(&mut self, mut column: TileColumn) {
        let at = self
            .locate(self.focused_session)
            .map_or(self.columns.len(), |(c, _)| c + 1);
        column.id = self.next_col_id;
        self.next_col_id += 1;
        let pane = column.active_pane();
        self.columns.insert(at, column);
        if self.mode == TilingMode::Hyprland {
            self.normalize_widths();
        }
        self.focus_pane(pane);
        self.ensure_focused_visible();
    }

    /// Every pane in reading order (columns left to right, panes top to bottom), even while
    /// one is full screen: the order of the unified tab row.
    pub fn pane_order(&self) -> Vec<u64> {
        self.columns
            .iter()
            .flat_map(|c| c.panes.iter().copied())
            .collect()
    }

    pub fn visible_pane_ids(&self) -> Vec<u64> {
        if self.is_maximized() {
            return vec![self.focused_session];
        }
        self.snapshot()
            .columns
            .into_iter()
            .filter(|c| c.visible)
            .flat_map(|c| c.panes)
            .collect()
    }

    /// Fullscreen keeps every column with its id and tiled geometry (`maximized` + the focused
    /// session say which pane fills the view), so the renderer can animate in and out of it
    /// niri-style and the hidden panes keep their PTY size. Only the focused column is visible.
    pub fn snapshot(&self) -> TilingView {
        let layout = self.layout();
        let offset = self.view_offset;
        let maximized = self.is_maximized();
        let columns = self
            .columns
            .iter()
            .zip(&layout)
            .map(|(col, &(x, width))| {
                let mut heights = col.heights.clone();
                normalize(&mut heights);
                ColumnView {
                    id: col.id,
                    panes: col.panes.clone(),
                    heights,
                    x,
                    width,
                    expanded: col.restore_width.is_some(),
                    visible: if maximized {
                        col.panes.contains(&self.focused_session)
                    } else {
                        x < offset + 1.0 - EPS && x + width > offset + EPS
                    },
                }
            })
            .collect();

        TilingView {
            mode: self.mode,
            focused_session: self.focused_session,
            maximized,
            view_offset: offset,
            strip_width: self.strip_width(),
            total_cols: self.columns.len(),
            columns,
            workspaces: Vec::new(),
            active_workspace: 0,
        }
    }

    fn layout(&self) -> Vec<(f32, f32)> {
        let total: f32 = self.columns.iter().map(|c| c.width).sum();
        let scale = match self.mode {
            TilingMode::Hyprland if total > EPS => 1.0 / total,
            _ => 1.0,
        };
        let mut x = 0.0;
        self.columns
            .iter()
            .map(|c| {
                let w = c.width * scale;
                let start = x;
                x += w;
                (start, w)
            })
            .collect()
    }

    fn strip_width(&self) -> f32 {
        self.layout().last().map_or(1.0, |&(x, w)| x + w)
    }

    fn width_neighbour(&self, c: usize) -> Option<usize> {
        if c + 1 < self.columns.len() {
            Some(c + 1)
        } else if c > 0 {
            Some(c - 1)
        } else {
            None
        }
    }

    fn normalize_widths(&mut self) {
        let mut widths: Vec<f32> = self.columns.iter().map(|c| c.width).collect();
        normalize(&mut widths);
        for (col, w) in self.columns.iter_mut().zip(widths) {
            col.width = w;
            col.restore_width = None;
        }
    }

    /// Removes a pane and hands its height to the pane above (or below). The column's focus
    /// stays on the same pane, or moves to the one below (above for the bottom one), as in niri.
    fn remove_pane_at(&mut self, c: usize, r: usize) {
        let col = &mut self.columns[c];
        col.panes.remove(r);
        let freed = col.heights.remove(r);
        if let Some(n) = col.heights.get_mut(r.saturating_sub(1)) {
            *n += freed;
        }
        if col.active > r {
            col.active -= 1;
        }
        col.active = col.active.min(col.panes.len().saturating_sub(1));
    }

    /// Focuses `pane`, remembers it as its column's focused pane and forgets the column to
    /// return to on close (any focus change but `open_column` does, as in niri).
    fn focus_pane(&mut self, pane: u64) {
        self.focused_session = pane;
        self.restore_on_close = None;
        if let Some((c, r)) = self.locate(pane) {
            self.columns[c].active = r;
        }
    }

    /// Adds `pane` at the bottom of `column` with an equal share of its height.
    fn append_share(column: &mut TileColumn, pane: u64) {
        let share = 1.0 / (column.panes.len() + 1) as f32;
        column.heights.iter_mut().for_each(|h| *h *= 1.0 - share);
        column.panes.push(pane);
        column.heights.push(share);
    }

    fn locate(&self, session_id: u64) -> Option<(usize, usize)> {
        self.columns.iter().enumerate().find_map(|(c, col)| {
            col.panes
                .iter()
                .position(|&id| id == session_id)
                .map(|r| (c, r))
        })
    }

    fn ensure_focused_visible(&mut self) {
        self.reveal(None);
    }

    /// niri's view fitting for the focused column, `from` being the column focused before. A
    /// column as wide as the view aligns left; a centered mode centers it; otherwise the view
    /// keeps still if the column is fully visible, or moves the least to show it. Never clamped
    /// to the strip, so closing the last column can leave empty space as in niri.
    fn reveal(&mut self, from: Option<usize>) {
        if self.mode == TilingMode::Hyprland {
            self.view_offset = 0.0;
            return;
        }
        let Some((c, _)) = self.locate(self.focused_session) else {
            self.view_offset = 0.0;
            return;
        };
        let layout = self.layout();
        let (x, w) = layout[c];
        let center = match self.options.center_focused_column {
            CenterFocusedColumn::Always => true,
            CenterFocusedColumn::Never => false,
            CenterFocusedColumn::OnOverflow => from
                .filter(|&f| f != c && f < layout.len())
                .is_some_and(|f| {
                    let (fx, fw) = layout[f];
                    (x + w).max(fx + fw) - x.min(fx) > 1.0 + EPS
                }),
        } || (self.options.always_center_single_column && self.columns.len() == 1);
        let offset = self.view_offset;
        self.view_offset = if w >= 1.0 - EPS {
            x
        } else if center {
            x + w / 2.0 - 0.5
        } else if x >= offset - EPS && x + w <= offset + 1.0 + EPS {
            offset
        } else if (x - offset).abs() <= (x + w - 1.0 - offset).abs() {
            x
        } else {
            x + w - 1.0
        };
    }
}

/// niri workspaces: a vertical list of independent tilings. There is always one empty workspace
/// at the end; an empty workspace elsewhere disappears once it is no longer active. Single-window
/// operations go to the active workspace through `Deref`; the ones that must see every
/// workspace (`focus_session`, `remove_session`) are defined here and take precedence.
#[derive(Clone, Serialize, Deserialize)]
pub struct Workspaces {
    list: Vec<TilingWorkspace>,
    active: usize,
    /// The window focused on the workspace left last (0: it was empty), for niri
    /// `focus-workspace-previous`; a window survives the index shifts of `tidy`.
    #[serde(default)]
    previous: Option<u64>,
}

impl std::ops::Deref for Workspaces {
    type Target = TilingWorkspace;
    fn deref(&self) -> &TilingWorkspace {
        &self.list[self.active]
    }
}

impl std::ops::DerefMut for Workspaces {
    fn deref_mut(&mut self) -> &mut TilingWorkspace {
        &mut self.list[self.active]
    }
}

impl Workspaces {
    pub fn empty(mode: TilingMode) -> Self {
        Self {
            list: vec![TilingWorkspace::empty(mode)],
            active: 0,
            previous: None,
        }
    }

    pub fn new(initial_session: u64, mode: TilingMode) -> Self {
        let mut set = Self {
            list: vec![TilingWorkspace::new(initial_session, mode)],
            active: 0,
            previous: None,
        };
        set.tidy();
        set
    }

    pub fn set_options(&mut self, options: LayoutOptions) {
        self.list.iter_mut().for_each(|w| w.set_options(options));
    }

    /// A new empty workspace with the same mode and options as the others.
    fn blank(&self) -> TilingWorkspace {
        let mut workspace = TilingWorkspace::empty(self.list[0].mode());
        workspace.options = self.list[0].options;
        workspace
    }

    /// Switches the active workspace, remembering the one left for `focus_workspace_previous`.
    fn set_active(&mut self, index: usize) {
        if index != self.active {
            self.previous = Some(self.list[self.active].focused_session());
            self.active = index;
        }
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn is_empty_active(&self) -> bool {
        self.list[self.active].is_empty()
    }

    pub fn open_pane(&mut self, pane: u64, stack: bool) {
        let workspace = &mut self.list[self.active];
        if stack {
            workspace.stack_vertical(pane);
        } else {
            workspace.open_column(pane);
        }
        self.tidy();
    }

    pub fn set_mode(&mut self, mode: TilingMode) {
        self.list.iter_mut().for_each(|w| w.set_mode(mode));
    }

    pub fn workspace_of(&self, pane: u64) -> Option<usize> {
        self.list.iter().position(|w| w.contains(pane))
    }

    /// Activates workspace `index` (clamped).
    pub fn focus_workspace(&mut self, index: usize) -> bool {
        let index = index.min(self.list.len() - 1);
        if index == self.active {
            return false;
        }
        self.set_active(index);
        self.tidy();
        true
    }

    /// niri `focus-workspace-previous`: back to the workspace focused before this one.
    pub fn focus_workspace_previous(&mut self) -> bool {
        let index = match self.previous {
            Some(0) => self.list.len() - 1,
            Some(pane) => match self.workspace_of(pane) {
                Some(index) => index,
                None => return false,
            },
            None => return false,
        };
        self.focus_workspace(index)
    }

    /// niri `focus-window-or-workspace-down/up`: moves inside the column, and past its end to
    /// the next workspace.
    pub fn focus_window_or_workspace(&mut self, down: bool) -> bool {
        let dir = if down { FocusDir::Down } else { FocusDir::Up };
        self.list[self.active].focus_direction(dir)
            || self.focus_workspace_relative(if down { 1 } else { -1 })
    }

    /// niri `move-column-to-workspace-down/up`: the whole focused column; focus follows it.
    pub fn move_column_relative(&mut self, delta: isize) -> bool {
        let target = self.active as isize + delta;
        if target < 0 || target as usize >= self.list.len() {
            return false;
        }
        let Some(column) = self.list[self.active].take_focused_column() else {
            return false;
        };
        let target = target as usize;
        self.list[target].insert_column(column);
        self.set_active(target);
        self.tidy();
        true
    }

    /// niri `focus-workspace-down/up`.
    pub fn focus_workspace_relative(&mut self, delta: isize) -> bool {
        let target = (self.active as isize + delta).clamp(0, self.list.len() as isize - 1);
        self.focus_workspace(target as usize)
    }

    /// Moves a window to workspace `index` (`insert`: into a new workspace created at `index`,
    /// niri's drop between workspaces). The window opens as a column after the focused one there,
    /// and focus follows it.
    pub fn move_to_workspace(&mut self, pane: u64, index: usize, insert: bool) -> bool {
        let Some(from) = self.workspace_of(pane) else {
            return false;
        };
        let mut to = index.min(self.list.len() - usize::from(!insert));
        if !insert && to == from {
            return false;
        }
        if insert {
            let blank = self.blank();
            self.list.insert(to, blank);
        }
        let from = if insert && to <= from { from + 1 } else { from };
        self.list[from].remove_session(pane);
        // What stays focused there is what `focus_workspace_previous` comes back to.
        let left = self.list[from].focused_session();
        if from < to && self.list[from].is_empty() && from + 1 < self.list.len() {
            // Dropping the emptied source must not shift the target past it.
            self.list.remove(from);
            to -= 1;
        }
        self.list[to].open_column(pane);
        self.previous = Some(left);
        self.active = to;
        self.tidy();
        true
    }

    /// niri `move-window-to-workspace-down/up` for the focused window.
    pub fn move_focused_relative(&mut self, delta: isize) -> bool {
        let pane = self.focused_session();
        if pane == 0 {
            return false;
        }
        let target = self.active as isize + delta;
        if target < 0 {
            // Above the first workspace: a new one on top.
            return self.move_to_workspace(pane, 0, true);
        }
        self.move_to_workspace(pane, target as usize, false)
    }

    /// Focuses a window on whichever workspace holds it (activating that workspace); an unknown
    /// id opens as a column on the active workspace.
    pub fn focus_session(&mut self, pane: u64) {
        if let Some(w) = self.workspace_of(pane) {
            self.set_active(w);
            self.list[w].focus_session(pane);
            self.tidy();
        } else {
            self.list[self.active].focus_session(pane);
            self.tidy();
        }
    }

    pub fn remove_session(&mut self, pane: u64) {
        if let Some(w) = self.workspace_of(pane) {
            self.list[w].remove_session(pane);
            self.tidy();
        }
    }

    /// Every window on every workspace.
    pub fn all_panes(&self) -> Vec<u64> {
        self.list.iter().flat_map(|w| w.pane_order()).collect()
    }

    pub fn snapshot(&self) -> TilingView {
        let mut view = self.list[self.active].snapshot();
        view.workspaces = self
            .list
            .iter()
            .map(|w| {
                let tiled = w.snapshot();
                WorkspaceView {
                    panes: w.pane_order(),
                    focused: w.focused_session(),
                    columns: tiled.columns,
                    view_offset: tiled.view_offset,
                }
            })
            .collect();
        view.active_workspace = self.active;
        view
    }

    /// Drops empty workspaces other than the active one and the last, and keeps exactly one
    /// empty workspace at the end.
    fn tidy(&mut self) {
        let (active, last) = (self.active, self.list.len() - 1);
        let (mut i, mut removed_before_active) = (0, 0);
        self.list.retain(|w| {
            let keep = !w.is_empty() || i == active || i == last;
            if !keep && i < active {
                removed_before_active += 1;
            }
            i += 1;
            keep
        });
        self.active -= removed_before_active;
        // Two empty workspaces at the end collapse into one unless the user is on the last.
        while self.list.len() > 1
            && self.active != self.list.len() - 1
            && self.list[self.list.len() - 1].is_empty()
            && self.list[self.list.len() - 2].is_empty()
        {
            self.list.pop();
        }
        if !self.list[self.list.len() - 1].is_empty() {
            let blank = self.blank();
            self.list.push(blank);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_a_new_column_returns_to_the_previous_one() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.open_column(2);
        ws.open_column(3);
        ws.focus_direction(FocusDir::Left);
        ws.open_column(4); // between 2 and 3
        ws.remove_session(4);
        assert_eq!(
            ws.focused_session(),
            2,
            "back to where the window was opened from"
        );
        // After any focus change niri forgets it: focus goes to the right neighbour.
        ws.open_column(5);
        ws.focus_direction(FocusDir::Left);
        ws.focus_direction(FocusDir::Right);
        ws.remove_session(5);
        assert_eq!(ws.focused_session(), 3);

        // Closing a column left of the new one also forgets it, as in niri.
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.open_column(2);
        ws.open_column(3);
        ws.focus_direction(FocusDir::Left);
        ws.open_column(4); // [1, 2, 4, 3], opened from 2
        ws.remove_session(1);
        ws.remove_session(4);
        assert_eq!(
            ws.focused_session(),
            3,
            "right neighbour, not the shifted column"
        );
    }

    #[test]
    fn view_fits_like_niri_and_can_center() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.open_column(2);
        ws.open_column(3);
        // Moving left within the visible pair keeps the view still.
        let offset = ws.snapshot().view_offset;
        ws.focus_direction(FocusDir::Left);
        assert!(close(ws.snapshot().view_offset, offset));
        ws.set_options(LayoutOptions {
            center_focused_column: CenterFocusedColumn::Always,
            ..LayoutOptions::default()
        });
        assert!(
            close(ws.snapshot().view_offset, 0.25),
            "centered: 0.5 + 0.25 - 0.5"
        );
        let mut single = TilingWorkspace::new(1, TilingMode::Niri);
        single.set_options(LayoutOptions {
            always_center_single_column: true,
            ..LayoutOptions::default()
        });
        assert!(close(single.snapshot().view_offset, -0.25));
    }

    #[test]
    fn presets_cycle_both_ways_and_columns_move_across_workspaces() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        assert!(ws.cycle_column_width(true)); // 0.5 -> 1/3
        assert!(close(ws.snapshot().columns[0].width, 1.0 / 3.0));
        assert!(ws.cycle_column_width(true)); // wraps to 2/3
        assert!(close(ws.snapshot().columns[0].width, 2.0 / 3.0));

        let mut w = Workspaces::new(1, TilingMode::Niri);
        w.stack_vertical(2);
        assert!(w.move_column_relative(1));
        assert_eq!(w.active_index(), 0, "the emptied workspace is gone");
        assert_eq!(
            w.snapshot().columns[0].panes,
            vec![1, 2],
            "the whole column moved"
        );
        w.open_pane(3, false);
        assert!(w.move_focused_relative(1));
        assert!(w.focus_workspace_previous());
        assert_eq!(
            w.focused_session(),
            2,
            "previous workspace, its focused window"
        );
    }

    #[test]
    fn consume_expel_and_swap() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.open_column(2);
        ws.focus_direction(FocusDir::Left);
        assert!(ws.consume_into_column());
        assert_eq!(ws.snapshot().columns[0].panes, vec![1, 2]);
        assert!(ws.expel_from_column());
        assert_eq!(ws.pane_order(), vec![1, 2]);
        assert_eq!(ws.snapshot().columns.len(), 2);
        ws.stack_vertical(3); // column 0: [1, 3]
        assert!(ws.swap_window(FocusDir::Right));
        assert_eq!(ws.snapshot().columns[1].panes, vec![3]);
        assert_eq!(ws.focused_session(), 3);
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn niri_columns_scroll_and_stack_vertically() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.open_column(2);
        ws.open_column(3);
        // Three half-width columns: focusing 3 scrolls the strip to show [2, 3].
        assert_eq!(ws.visible_pane_ids(), vec![2, 3]);
        assert!(close(ws.snapshot().view_offset, 0.5));

        ws.stack_vertical(4);
        assert_eq!(ws.focused_session(), 4);
        assert!(ws.focus_direction(FocusDir::Up));
        assert_eq!(ws.focused_session(), 3);
        assert!(ws.focus_direction(FocusDir::Left));
        assert!(ws.focus_direction(FocusDir::Left));
        assert_eq!(ws.focused_session(), 1);
        assert_eq!(ws.visible_pane_ids(), vec![1, 2]);
    }

    #[test]
    fn niri_width_presets_steps_and_maximize() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.open_column(2);
        assert!(ws.cycle_column_width(false)); // 0.5 -> 2/3
        assert!(close(ws.snapshot().columns[1].width, 2.0 / 3.0));
        assert!(ws.cycle_column_width(false)); // 2/3 -> 1/3 (wraps)
        assert!(close(ws.snapshot().columns[1].width, 1.0 / 3.0));
        for _ in 0..3 {
            assert!(ws.adjust_column_width(-RESIZE_STEP));
        }
        assert!(close(ws.snapshot().columns[1].width, MIN_FRACTION));
        assert!(!ws.adjust_column_width(-RESIZE_STEP)); // clamped

        ws.stack_vertical(3);
        assert!(ws.maximize_column());
        let expanded = ws.snapshot();
        assert!(close(expanded.columns[1].width, 1.0));
        assert!(expanded.columns[1].expanded);
        assert!(close(expanded.view_offset, 0.5)); // full column scrolled into view
        assert_eq!(expanded.columns[1].heights, [0.5, 0.5]);
        assert_eq!(ws.visible_pane_ids(), [2, 3]);
        ws.toggle_maximize();
        assert_eq!(ws.visible_pane_ids(), [3]);
        assert!(!ws.maximize_column());
        ws.toggle_maximize();
        assert!(ws.maximize_column());
        assert!(close(ws.snapshot().columns[1].width, MIN_FRACTION));
        assert!(!ws.snapshot().columns[1].expanded);
    }

    #[test]
    fn fullscreen_snapshot_keeps_the_tiled_layout_for_animation() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.open_column(2);
        ws.stack_vertical(3);
        let tiled = ws.snapshot();
        ws.toggle_maximize();
        let full = ws.snapshot();
        assert!(full.maximized);
        // Same columns, ids and geometry; only visibility narrows to the fullscreen pane.
        assert_eq!(full.columns.len(), tiled.columns.len());
        for (a, b) in full.columns.iter().zip(&tiled.columns) {
            assert_eq!(
                (a.id, &a.panes, a.x, a.width),
                (b.id, &b.panes, b.x, b.width)
            );
        }
        assert_eq!(
            full.columns.iter().map(|c| c.visible).collect::<Vec<_>>(),
            vec![false, true]
        );
        assert_eq!(ws.visible_pane_ids(), vec![3]);
    }

    #[test]
    fn fullscreen_follows_focus_and_stays_with_the_workspace() {
        for mode in [TilingMode::Niri, TilingMode::Hyprland] {
            let mut tiling = Workspaces::new(1, mode);
            tiling.open_pane(2, false);
            tiling.open_pane(3, true);
            tiling.toggle_maximize();
            tiling.focus_session(1);
            assert_eq!(tiling.visible_pane_ids(), [1]);
            // The column remembers its focused pane (3, stacked last), as in niri.
            assert!(tiling.focus_direction(FocusDir::Right));
            assert_eq!(tiling.visible_pane_ids(), [3]);
            assert!(tiling.focus_direction(FocusDir::Up));
            assert_eq!(tiling.visible_pane_ids(), [2]);
            assert!(tiling.focus_direction(FocusDir::Down));

            assert!(tiling.move_to_workspace(2, 1, false));
            assert!(!tiling.is_maximized());
            tiling.focus_session(1);
            assert!(tiling.is_maximized());
            tiling.remove_session(1);
            assert!(
                !tiling.is_maximized(),
                "closing the fullscreen pane exits fullscreen"
            );
            assert_eq!(tiling.visible_pane_ids(), [3]);
            tiling.remove_session(3);
            assert!(!tiling.is_maximized());
            tiling.open_pane(4, false);
            assert!(!tiling.is_maximized());
            tiling.focus_session(2);
            assert!(!tiling.is_maximized());
            if mode == TilingMode::Hyprland {
                assert!(!tiling.maximize_column());
                assert!(!tiling.is_maximized());
            }
        }
    }

    #[test]
    fn niri_new_windows_open_right_of_focus_even_in_fullscreen() {
        let mut tiling = Workspaces::new(1, TilingMode::Niri);
        tiling.open_pane(2, false);
        tiling.open_pane(3, false);
        tiling.focus_session(2);
        let before = tiling.snapshot();

        tiling.open_pane(4, false);
        assert_eq!(tiling.pane_order(), [1, 2, 4, 3]);
        assert_eq!(tiling.focused_session(), 4);
        let after = tiling.snapshot();
        for existing in &before.columns {
            let column = after
                .columns
                .iter()
                .find(|col| col.id == existing.id)
                .unwrap();
            assert_eq!(column.width, existing.width);
        }

        tiling.toggle_maximize();
        tiling.open_pane(5, false);
        assert_eq!(tiling.pane_order(), [1, 2, 4, 5, 3]);
        assert!(tiling.is_maximized());
        assert_eq!(tiling.visible_pane_ids(), [5]);
    }

    #[test]
    fn closing_the_fullscreen_pane_restores_the_tiling() {
        for mode in [TilingMode::Niri, TilingMode::Hyprland] {
            let mut tiling = Workspaces::new(1, mode);
            tiling.open_pane(2, false);
            tiling.open_pane(3, false);
            tiling.focus_session(2);
            tiling.toggle_maximize();
            // Closing a pane hidden behind the fullscreen one keeps fullscreen.
            tiling.remove_session(3);
            assert!(tiling.is_maximized());
            assert_eq!(tiling.visible_pane_ids(), [2]);
            // Closing the fullscreen pane itself: the remaining panes are tiled again.
            tiling.open_pane(4, false);
            tiling.focus_session(2);
            assert!(tiling.is_maximized());
            tiling.remove_session(2);
            assert!(!tiling.is_maximized());
            let mut visible = tiling.visible_pane_ids();
            visible.sort_unstable();
            // niri keeps the view still: 4 slides into 2's place, the strip end stays empty.
            let expected: &[u64] = if mode == TilingMode::Niri {
                &[4]
            } else {
                &[1, 4]
            };
            assert_eq!(visible, expected, "{mode:?}");
        }
    }

    #[test]
    fn hyprland_widths_share_the_viewport() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Hyprland);
        ws.open_column(2); // splits 1 in half
        ws.open_column(3); // splits 2 in half
        let widths: Vec<f32> = ws.snapshot().columns.iter().map(|c| c.width).collect();
        assert!(close(widths.iter().sum(), 1.0));
        assert!(close(widths[0], 0.5) && close(widths[1], 0.25) && close(widths[2], 0.25));

        // Focused (last) column grows by taking width from its left neighbour.
        assert!(ws.adjust_column_width(RESIZE_STEP));
        let widths: Vec<f32> = ws.snapshot().columns.iter().map(|c| c.width).collect();
        assert!(close(widths[1], 0.15) && close(widths[2], 0.35));
        assert!(close(widths.iter().sum(), 1.0));
        assert!(ws.snapshot().columns.iter().all(|c| c.visible));

        ws.remove_session(3);
        let widths: Vec<f32> = ws.snapshot().columns.iter().map(|c| c.width).collect();
        assert!(close(widths.iter().sum(), 1.0));
    }

    #[test]
    fn pane_heights_resize_and_consume_expel() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.stack_vertical(2);
        assert!(ws.adjust_pane_height(RESIZE_STEP)); // pane 2 (bottom) grows from pane 1
        let h = ws.snapshot().columns[0].heights.clone();
        assert!(close(h[0], 0.4) && close(h[1], 0.6));

        // Expel pane 2 into its own column on the right, then consume it back.
        assert!(ws.consume_or_expel(FocusDir::Right));
        let snap = ws.snapshot();
        assert_eq!(snap.columns.len(), 2);
        assert_eq!(snap.columns[1].panes, vec![2]);
        assert!(close(snap.columns[0].heights[0], 1.0));
        assert!(ws.consume_or_expel(FocusDir::Left));
        let snap = ws.snapshot();
        assert_eq!(snap.columns.len(), 1);
        assert_eq!(snap.columns[0].panes, vec![1, 2]);

        ws.reset_sizes();
        assert!(close(ws.snapshot().columns[0].heights[0], 0.5));
    }

    #[test]
    fn drag_resizes_columns_and_panes() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.open_column(2);
        assert!(ws.resize_column(0, 0.2));
        let snap = ws.snapshot();
        assert!(close(snap.columns[0].width, 0.7) && close(snap.columns[1].width, 0.3));
        assert!(close(snap.columns[1].x, 0.7));
        assert!(close(snap.view_offset, 0.0)); // both stay on screen, nothing scrolls
        assert!(ws.resize_column(1, 0.2)); // trailing edge still grows the last column
        assert!(close(ws.snapshot().columns[1].width, 0.5));
        assert!(!ws.resize_column(5, 0.1));

        let mut hy = TilingWorkspace::new(1, TilingMode::Hyprland);
        hy.open_column(2);
        assert!(hy.resize_column(0, 0.1));
        let w: Vec<f32> = hy.snapshot().columns.iter().map(|c| c.width).collect();
        assert!(close(w[0], 0.6) && close(w[1], 0.4));
        assert!(!hy.resize_column(1, 0.1)); // no border after the last column

        hy.stack_vertical(3);
        let col = hy
            .snapshot()
            .columns
            .iter()
            .position(|c| c.panes.len() == 2)
            .unwrap();
        assert!(hy.resize_pane(col, 0, -0.2));
        let h = &hy.snapshot().columns[col].heights;
        assert!(close(h[0], 0.3) && close(h[1], 0.7));
        assert!(!hy.resize_pane(col, 1, 0.1));
    }

    #[test]
    fn resizing_the_last_column_clamps_scroll_and_keeps_its_right_edge_visible() {
        let mut workspace = TilingWorkspace::new(1, TilingMode::Niri);
        workspace.open_column(2);
        workspace.open_column(3);
        let before = workspace.snapshot();
        let last = &before.columns[2];
        assert!(close(before.view_offset, 0.5));

        let mut preview = workspace.clone();
        assert!(preview.resize_at(last.id, None, 0.2, true));
        let resized = preview.snapshot();
        let right = resized.columns[2].x + resized.columns[2].width - resized.view_offset;
        assert!(close(resized.columns[2].width, 0.3));
        assert!(close(resized.view_offset, 0.3));
        assert!(close(right, 1.0));
        assert_eq!(workspace.snapshot(), before);
        assert!(workspace.resize_at(last.id, None, 0.2, true));
        assert_eq!(workspace.snapshot(), resized);
    }

    #[test]
    fn moving_a_stacked_pane_keeps_the_focus_and_its_height() {
        let mut workspace = TilingWorkspace::new(1, TilingMode::Niri);
        workspace.stack_vertical(2);
        assert!(workspace.resize_pane(0, 0, -0.2));
        assert!(workspace.move_pane_in_column(FocusDir::Up));
        let column = &workspace.snapshot().columns[0];
        assert_eq!(column.panes, [2, 1]);
        assert!(close(column.heights[0], 0.7));
        assert_eq!(workspace.focused_session(), 2);
        assert!(!workspace.move_pane_in_column(FocusDir::Up));
        assert!(workspace.move_pane_in_column(FocusDir::Down));
    }

    #[test]
    fn drag_moves_panes_between_columns_and_stacks() {
        let mut t = TilingWorkspace::new(1, TilingMode::Hyprland);
        t.open_column(2);
        t.open_column(3);
        // Column 3 dropped into column 1's stack, below pane 1.
        assert!(t.move_pane(3, DropTarget::Into { col: 0, row: 1 }));
        assert_eq!(t.pane_order(), vec![1, 3, 2]);
        let s = t.snapshot();
        assert_eq!(s.columns.len(), 2);
        assert!(close(s.columns.iter().map(|c| c.width).sum(), 1.0));
        assert!(close(s.columns[0].heights.iter().sum(), 1.0));
        assert_eq!(t.focused_session(), 3);
        // Expelled to a new first column.
        assert!(t.move_pane(3, DropTarget::NewColumn(0)));
        assert_eq!(t.pane_order(), vec![3, 1, 2]);
        // A lone column moved to the end (index given against the old layout).
        assert!(t.move_pane(3, DropTarget::NewColumn(3)));
        assert_eq!(t.pane_order(), vec![1, 2, 3]);
        // Dropping on its own slot does nothing.
        assert!(!t.move_pane(3, DropTarget::NewColumn(2)));
        assert!(!t.move_pane(3, DropTarget::Into { col: 2, row: 0 }));
        // Reorder inside a stack.
        t.move_pane(2, DropTarget::Into { col: 0, row: 1 });
        t.move_pane(3, DropTarget::Into { col: 0, row: 0 });
        assert_eq!(t.pane_order(), vec![3, 1, 2]);
        assert!(t.move_pane(2, DropTarget::Into { col: 0, row: 0 }));
        assert_eq!(t.pane_order(), vec![2, 3, 1]);
        assert!(close(t.snapshot().columns[0].width, 1.0));
    }

    #[test]
    fn workspaces_follow_niri_dynamic_rules() {
        let mut w = Workspaces::new(1, TilingMode::Niri);
        w.open_column(2);
        assert_eq!(w.len(), 2, "one empty workspace at the end");
        // Move window 2 down into the empty workspace: a new empty one appears below it.
        assert!(w.move_focused_relative(1));
        assert_eq!(w.active_index(), 1);
        assert_eq!(
            w.snapshot()
                .workspaces
                .iter()
                .map(|v| v.panes.clone())
                .collect::<Vec<_>>(),
            vec![vec![1], vec![2], vec![]]
        );
        assert!(
            close(
                w.snapshot().columns[0].width,
                LayoutOptions::default().default_column_width
            ),
            "a moved window opens at niri's default column width"
        );
        // Moving 1 down empties workspace 0, which disappears (it is no longer active).
        assert!(w.move_to_workspace(1, 1, false));
        assert_eq!(
            w.snapshot()
                .workspaces
                .iter()
                .map(|v| v.panes.clone())
                .collect::<Vec<_>>(),
            vec![vec![2, 1], vec![]]
        );
        assert_eq!(w.active_index(), 0);
        // Drop between workspaces: a new workspace inserted on top.
        assert!(w.move_to_workspace(2, 0, true));
        assert_eq!(
            w.snapshot()
                .workspaces
                .iter()
                .map(|v| v.panes.clone())
                .collect::<Vec<_>>(),
            vec![vec![2], vec![1], vec![]]
        );
        assert_eq!(w.active_index(), 0);
        // Focusing a window activates its workspace; tab cycling stays per workspace.
        w.focus_session(1);
        assert_eq!(w.active_index(), 1);
        assert_eq!(w.pane_order(), vec![1]);
        // Going to the empty last workspace and back keeps the list tidy.
        assert!(w.focus_workspace_relative(1));
        assert!(w.is_empty_active());
        assert!(w.focus_workspace_relative(-1));
        assert_eq!(w.len(), 3);
        w.remove_session(2);
        assert_eq!(
            w.snapshot()
                .workspaces
                .iter()
                .map(|v| v.panes.clone())
                .collect::<Vec<_>>(),
            vec![vec![1], vec![]]
        );
        assert_eq!(w.all_panes(), vec![1]);
    }

    #[test]
    fn move_and_center_columns() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.open_column(2);
        ws.open_column(3);
        assert!(ws.move_column(FocusDir::Left));
        let order: Vec<u64> = ws.snapshot().columns.iter().map(|c| c.panes[0]).collect();
        assert_eq!(order, vec![1, 3, 2]);
        assert!(ws.center_column());
        assert!(close(ws.snapshot().view_offset, 0.25)); // column 3 (x 0.5, w 0.5) centred
    }
}
