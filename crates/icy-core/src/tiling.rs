//! Hyprland & Niri-inspired tiling layout engine for terminal panes.
//! Pure state testable without a window or GPU.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TilingMode {
    /// Scrollable horizontal columns (each column can stack vertical panes), like Niri.
    Niri,
    /// Dynamic master-stack / split grid fitting all panes inside the viewport, like Hyprland.
    Hyprland,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusDir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileColumn {
    pub id: u64,
    pub width_weight: f32,
    pub panes: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TilingView {
    pub mode: TilingMode,
    pub focused_session: u64,
    pub maximized: bool,
    pub scroll_col: usize,
    pub total_cols: usize,
    pub visible_columns: Vec<TileColumn>,
}

pub struct TilingWorkspace {
    mode: TilingMode,
    columns: Vec<TileColumn>,
    next_col_id: u64,
    focused_session: u64,
    maximized: bool,
    scroll_col: usize,
    max_visible_cols: usize,
}

impl TilingWorkspace {
    pub fn new(initial_session: u64, mode: TilingMode) -> Self {
        Self {
            mode,
            columns: vec![TileColumn {
                id: 1,
                width_weight: 1.0,
                panes: vec![initial_session],
            }],
            next_col_id: 2,
            focused_session: initial_session,
            maximized: false,
            scroll_col: 0,
            max_visible_cols: 2,
        }
    }

    pub fn mode(&self) -> TilingMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: TilingMode) {
        self.mode = mode;
        self.ensure_focused_visible();
    }

    pub fn focused_session(&self) -> u64 {
        self.focused_session
    }

    pub fn is_maximized(&self) -> bool {
        self.maximized
    }

    pub fn toggle_maximize(&mut self) {
        self.maximized = !self.maximized;
    }

    pub fn scroll_columns(&mut self, delta: isize) -> bool {
        if self.columns.len() <= self.max_visible_cols {
            return false;
        }
        let max_scroll = self.columns.len().saturating_sub(self.max_visible_cols);
        let next = (self.scroll_col as isize + delta).clamp(0, max_scroll as isize) as usize;
        if next == self.scroll_col {
            return false;
        }
        self.scroll_col = next;
        true
    }

    /// Adds a new column right after the focused column (Niri style) or splits horizontally (Hyprland).
    pub fn open_column(&mut self, session_id: u64) {
        self.maximized = false;
        let insert_idx = self
            .locate(self.focused_session)
            .map(|(c, _)| c + 1)
            .unwrap_or(self.columns.len());
        let id = self.next_col_id;
        self.next_col_id += 1;
        self.columns.insert(
            insert_idx,
            TileColumn {
                id,
                width_weight: 1.0,
                panes: vec![session_id],
            },
        );
        self.focused_session = session_id;
        self.ensure_focused_visible();
    }

    /// Stacks a new pane vertically inside the currently focused column.
    pub fn stack_vertical(&mut self, session_id: u64) {
        self.maximized = false;
        if let Some((c, r)) = self.locate(self.focused_session) {
            self.columns[c].panes.insert(r + 1, session_id);
        } else if let Some(first) = self.columns.first_mut() {
            first.panes.push(session_id);
        } else {
            self.open_column(session_id);
            return;
        }
        self.focused_session = session_id;
        self.ensure_focused_visible();
    }

    /// Focuses an existing session (adding it as a column if not tiled yet).
    pub fn focus_session(&mut self, session_id: u64) {
        if self.locate(session_id).is_none() {
            self.open_column(session_id);
            return;
        }
        self.focused_session = session_id;
        self.ensure_focused_visible();
    }

    pub fn focus_direction(&mut self, dir: FocusDir) -> bool {
        let Some((col, row)) = self.locate(self.focused_session) else {
            return false;
        };
        let next = match dir {
            FocusDir::Left if col > 0 => {
                let target_col = &self.columns[col - 1];
                target_col
                    .panes
                    .get(row.min(target_col.panes.len() - 1))
                    .copied()
            }
            FocusDir::Right if col + 1 < self.columns.len() => {
                let target_col = &self.columns[col + 1];
                target_col
                    .panes
                    .get(row.min(target_col.panes.len() - 1))
                    .copied()
            }
            FocusDir::Up if row > 0 => self.columns[col].panes.get(row - 1).copied(),
            FocusDir::Down if row + 1 < self.columns[col].panes.len() => {
                self.columns[col].panes.get(row + 1).copied()
            }
            _ => None,
        };
        if let Some(id) = next {
            self.focused_session = id;
            self.ensure_focused_visible();
            true
        } else {
            false
        }
    }

    pub fn remove_session(&mut self, session_id: u64) {
        let Some((col, row)) = self.locate(session_id) else {
            return;
        };
        self.columns[col].panes.remove(row);
        if self.columns[col].panes.is_empty() {
            self.columns.remove(col);
        }
        if self.focused_session == session_id
            && let Some(next_col) = self
                .columns
                .get(col.min(self.columns.len().saturating_sub(1)))
            && let Some(&next_id) = next_col
                .panes
                .get(row.min(next_col.panes.len().saturating_sub(1)))
        {
            self.focused_session = next_id;
        }
        self.ensure_focused_visible();
    }

    pub fn visible_pane_ids(&self) -> Vec<u64> {
        self.snapshot()
            .visible_columns
            .into_iter()
            .flat_map(|c| c.panes)
            .collect()
    }

    pub fn snapshot(&self) -> TilingView {
        if self.maximized {
            return TilingView {
                mode: self.mode,
                focused_session: self.focused_session,
                maximized: true,
                scroll_col: self.scroll_col,
                total_cols: self.columns.len(),
                visible_columns: vec![TileColumn {
                    id: 0,
                    width_weight: 1.0,
                    panes: vec![self.focused_session],
                }],
            };
        }

        let visible_columns = match self.mode {
            TilingMode::Niri => {
                let start = self.scroll_col.min(self.columns.len().saturating_sub(1));
                let end = (start + self.max_visible_cols).min(self.columns.len());
                self.columns[start..end].to_vec()
            }
            TilingMode::Hyprland => {
                // Hyprland mode shows up to 3 columns side-by-side simultaneously inside viewport.
                let limit = self.columns.len().min(3);
                let start = self
                    .scroll_col
                    .min(self.columns.len().saturating_sub(limit));
                self.columns[start..start + limit].to_vec()
            }
        };

        TilingView {
            mode: self.mode,
            focused_session: self.focused_session,
            maximized: false,
            scroll_col: self.scroll_col,
            total_cols: self.columns.len(),
            visible_columns,
        }
    }

    fn locate(&self, session_id: u64) -> Option<(usize, usize)> {
        for (c, col) in self.columns.iter().enumerate() {
            if let Some(r) = col.panes.iter().position(|&id| id == session_id) {
                return Some((c, r));
            }
        }
        None
    }

    fn ensure_focused_visible(&mut self) {
        let Some((col, _)) = self.locate(self.focused_session) else {
            self.scroll_col = 0;
            return;
        };
        let window = match self.mode {
            TilingMode::Niri => self.max_visible_cols,
            TilingMode::Hyprland => 3,
        };
        if col < self.scroll_col {
            self.scroll_col = col;
        } else if col >= self.scroll_col + window {
            self.scroll_col = col + 1 - window;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn niri_columns_scroll_and_stack_vertically() {
        let mut ws = TilingWorkspace::new(1, TilingMode::Niri);
        ws.open_column(2);
        ws.open_column(3);
        // Focused on 3 -> Niri 2-col viewport scrolls to [2, 3].
        assert_eq!(ws.visible_pane_ids(), vec![2, 3]);

        ws.stack_vertical(4);
        assert_eq!(ws.focused_session(), 4);
        assert!(ws.focus_direction(FocusDir::Up));
        assert_eq!(ws.focused_session(), 3);
        assert!(ws.focus_direction(FocusDir::Left));
        assert_eq!(ws.focused_session(), 2);
        assert!(ws.focus_direction(FocusDir::Left));
        assert_eq!(ws.focused_session(), 1);
        assert_eq!(ws.visible_pane_ids(), vec![1, 2]);
    }
}
