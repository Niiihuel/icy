//! User-editable keyboard shortcuts: the action catalogue with its defaults, chord parsing and
//! formatting ("Ctrl+Alt+R"), per-action overrides stored in the config, and conflict handling
//! (binding a chord takes it away from whichever action had it).

use std::collections::BTreeMap;

use serde::Serialize;

pub struct ActionDef {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    pub defaults: &'static [&'static str],
}

pub const ACTIONS: &[ActionDef] = &[
    ActionDef {
        id: "session.new",
        label: "New session",
        group: "Sessions & tabs",
        description: "Choose a project folder and start a shell or agent.",
        defaults: &["Ctrl+Shift+T"],
    },
    ActionDef {
        id: "session.search",
        label: "Search sessions",
        group: "Sessions & tabs",
        description: "Find and focus a session across workspaces.",
        defaults: &["Ctrl+Shift+K"],
    },
    ActionDef {
        id: "column.new",
        label: "New column",
        group: "Sessions & tabs",
        description: "A new session next to the focused pane.",
        defaults: &["Ctrl+Shift+N"],
    },
    ActionDef {
        id: "pane.split",
        label: "Split down",
        group: "Sessions & tabs",
        description: "Stacks a new pane below the focused one.",
        defaults: &["Ctrl+Shift+D"],
    },
    ActionDef {
        id: "session.close",
        label: "Close session",
        group: "Sessions & tabs",
        description: "Kills the focused session.",
        defaults: &["Ctrl+Shift+W"],
    },
    ActionDef {
        id: "tab.next",
        label: "Next tab",
        group: "Sessions & tabs",
        description: "Focuses the next session tab.",
        defaults: &["Ctrl+Tab"],
    },
    ActionDef {
        id: "tab.prev",
        label: "Previous tab",
        group: "Sessions & tabs",
        description: "Focuses the previous session tab.",
        defaults: &["Ctrl+Shift+Tab"],
    },
    ActionDef {
        id: "focus.left",
        label: "Focus left",
        group: "Sessions & tabs",
        description: "Moves focus to the column on the left.",
        defaults: &["Alt+Left", "Alt+H"],
    },
    ActionDef {
        id: "focus.right",
        label: "Focus right",
        group: "Sessions & tabs",
        description: "Moves focus to the column on the right.",
        defaults: &["Alt+Right", "Alt+L"],
    },
    ActionDef {
        id: "focus.up",
        label: "Focus up",
        group: "Sessions & tabs",
        description: "Moves focus to the pane above.",
        defaults: &["Alt+Up", "Alt+K"],
    },
    ActionDef {
        id: "focus.down",
        label: "Focus down",
        group: "Sessions & tabs",
        description: "Moves focus to the pane below.",
        defaults: &["Alt+Down", "Alt+J"],
    },
    ActionDef {
        id: "view.fullscreen",
        label: "Focus view",
        group: "Views",
        description: "Keeps the active tab full screen in this workspace. Press again to return to the grid.",
        defaults: &["Ctrl+Shift+M"],
    },
    ActionDef {
        id: "view.overview",
        label: "Overview",
        group: "Views",
        description: "All sessions as live thumbnails.",
        defaults: &["Ctrl+Alt+G"],
    },
    // Ids keep their old `panel.*` names so saved overrides still apply.
    ActionDef {
        id: "panel.side",
        label: "Open launcher",
        group: "Windows",
        description: "Choose a session, tool, file or URL in the launcher.",
        defaults: &["Ctrl+Alt+B"],
    },
    ActionDef {
        id: "panel.files",
        label: "Files window",
        group: "Windows",
        description: "The workspace tree as a window of the tiling.",
        defaults: &["Ctrl+Alt+E"],
    },
    ActionDef {
        id: "panel.overview",
        label: "Overview window",
        group: "Windows",
        description: "Review, files and sessions summary as a window of the tiling.",
        defaults: &["Ctrl+Alt+O"],
    },
    ActionDef {
        id: "layout.wider",
        label: "Wider",
        group: "Layout",
        description: "Moves the focused column's right border.",
        defaults: &["Ctrl+Alt+Right"],
    },
    ActionDef {
        id: "layout.narrower",
        label: "Narrower",
        group: "Layout",
        description: "Moves the focused column's border back.",
        defaults: &["Ctrl+Alt+Left"],
    },
    ActionDef {
        id: "layout.taller",
        label: "Taller",
        group: "Layout",
        description: "Grows the focused pane in its column.",
        defaults: &["Ctrl+Alt+Down"],
    },
    ActionDef {
        id: "layout.shorter",
        label: "Shorter",
        group: "Layout",
        description: "Shrinks the focused pane in its column.",
        defaults: &["Ctrl+Alt+Up"],
    },
    ActionDef {
        id: "layout.cycleWidth",
        label: "Preset width",
        group: "Layout",
        description: "Cycles the focused column through ⅓ · ½ · ⅔.",
        defaults: &["Ctrl+Alt+R"],
    },
    ActionDef {
        id: "layout.maximizeColumn",
        label: "Expand column",
        group: "Layout",
        description: "In Niri, gives the focused column the full width while keeping its panes stacked.",
        defaults: &[],
    },
    ActionDef {
        id: "layout.center",
        label: "Center column",
        group: "Layout",
        description: "Centers the focused column in the scrollable workspace.",
        defaults: &["Ctrl+Alt+C"],
    },
    ActionDef {
        id: "layout.moveLeft",
        label: "Move column left",
        group: "Layout",
        description: "Swaps the focused column with the one on its left.",
        defaults: &["Alt+Shift+Left", "Ctrl+Alt+Shift+Left"],
    },
    ActionDef {
        id: "layout.moveRight",
        label: "Move column right",
        group: "Layout",
        description: "Swaps the focused column with the one on its right.",
        defaults: &["Alt+Shift+Right", "Ctrl+Alt+Shift+Right"],
    },
    ActionDef {
        id: "layout.moveUp",
        label: "Move pane up",
        group: "Layout",
        description: "Swaps the focused pane with the one above it.",
        defaults: &["Alt+Shift+Up"],
    },
    ActionDef {
        id: "layout.moveDown",
        label: "Move pane down",
        group: "Layout",
        description: "Swaps the focused pane with the one below it.",
        defaults: &["Alt+Shift+Down"],
    },
    ActionDef {
        id: "layout.consumeLeft",
        label: "Consume / expel left",
        group: "Layout",
        description: "Moves the focused pane into or out of the column on its left.",
        defaults: &["Ctrl+Alt+,"],
    },
    ActionDef {
        id: "layout.consumeRight",
        label: "Consume / expel right",
        group: "Layout",
        description: "Moves the focused pane into or out of the column on its right.",
        defaults: &["Ctrl+Alt+."],
    },
    // niri `focus-workspace-down/up` and `move-window-to-workspace-down/up`.
    ActionDef {
        id: "workspace.down",
        label: "Next workspace",
        group: "Workspaces",
        description: "The workspace below (the last one is always empty).",
        defaults: &["Ctrl+Alt+PageDown"],
    },
    ActionDef {
        id: "workspace.up",
        label: "Previous workspace",
        group: "Workspaces",
        description: "The workspace above.",
        defaults: &["Ctrl+Alt+PageUp"],
    },
    ActionDef {
        id: "workspace.moveDown",
        label: "Move window down",
        group: "Workspaces",
        description: "The focused window to the workspace below; focus follows it.",
        defaults: &["Ctrl+Alt+Shift+PageDown"],
    },
    ActionDef {
        id: "workspace.moveUp",
        label: "Move window up",
        group: "Workspaces",
        description: "The focused window to the workspace above (a new one on top from the first).",
        defaults: &["Ctrl+Alt+Shift+PageUp"],
    },
    ActionDef {
        id: "layout.cycleWidthBack",
        label: "Previous preset width",
        group: "Layout",
        description: "Cycles the preset column widths backwards (2/3 → 1/2 → 1/3).",
        defaults: &["Ctrl+Alt+Shift+R"],
    },
    ActionDef {
        id: "layout.focusFirst",
        label: "Focus first column",
        group: "Layout",
        description: "Jumps to the first column of the strip.",
        defaults: &["Alt+Home"],
    },
    ActionDef {
        id: "layout.focusLast",
        label: "Focus last column",
        group: "Layout",
        description: "Jumps to the last column of the strip.",
        defaults: &["Alt+End"],
    },
    ActionDef {
        id: "layout.moveFirst",
        label: "Move column to start",
        group: "Layout",
        description: "Moves the focused column to the start of the strip.",
        defaults: &["Alt+Shift+Home"],
    },
    ActionDef {
        id: "layout.moveLast",
        label: "Move column to end",
        group: "Layout",
        description: "Moves the focused column to the end of the strip.",
        defaults: &["Alt+Shift+End"],
    },
    ActionDef {
        id: "layout.swapLeft",
        label: "Swap window left",
        group: "Layout",
        description: "Trades the focused window with the one focused in the column on its left.",
        defaults: &[],
    },
    ActionDef {
        id: "layout.swapRight",
        label: "Swap window right",
        group: "Layout",
        description: "Trades the focused window with the one focused in the column on its right.",
        defaults: &[],
    },
    ActionDef {
        id: "layout.consumeInto",
        label: "Consume into column",
        group: "Layout",
        description: "Pulls the next column's first window into the bottom of this column.",
        defaults: &["Ctrl+Alt+["],
    },
    ActionDef {
        id: "layout.expelFrom",
        label: "Expel from column",
        group: "Layout",
        description: "Pushes this column's last window out as a new column on its right.",
        defaults: &["Ctrl+Alt+]"],
    },
    ActionDef {
        id: "layout.expandColumn",
        label: "Expand to available width",
        group: "Layout",
        description: "Grows the focused column into the space the other visible columns leave.",
        defaults: &["Ctrl+Alt+F"],
    },
    ActionDef {
        id: "workspace.previous",
        label: "Previous workspace (back)",
        group: "Workspaces",
        description: "Goes back to the workspace you were on before.",
        defaults: &["Ctrl+Alt+Tab"],
    },
    ActionDef {
        id: "workspace.moveColumnDown",
        label: "Move column down",
        group: "Workspaces",
        description: "Moves the whole focused column to the workspace below; focus follows it.",
        defaults: &[],
    },
    ActionDef {
        id: "workspace.moveColumnUp",
        label: "Move column up",
        group: "Workspaces",
        description: "Moves the whole focused column to the workspace above; focus follows it.",
        defaults: &[],
    },
    ActionDef {
        id: "focus.windowOrWorkspaceDown",
        label: "Focus down or next workspace",
        group: "Workspaces",
        description: "Moves focus down the column, then to the workspace below.",
        defaults: &[],
    },
    ActionDef {
        id: "focus.windowOrWorkspaceUp",
        label: "Focus up or previous workspace",
        group: "Workspaces",
        description: "Moves focus up the column, then to the workspace above.",
        defaults: &[],
    },
    ActionDef {
        id: "layout.reset",
        label: "Reset sizes",
        group: "Layout",
        description: "Equal columns and panes.",
        defaults: &["Ctrl+Alt+0"],
    },
    ActionDef {
        id: "editor.addToChat",
        label: "Add selection to chat",
        group: "Editor",
        description: "Pastes the editor selection into the project's agent.",
        defaults: &["Ctrl+L"],
    },
];

pub fn action(id: &str) -> Option<&'static ActionDef> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// A key with modifiers. `key` is canonical: an upper-case letter or digit, a punctuation
/// character, or a named key (`Left`, `Tab`, `Enter`, `Space`, `F5`, …).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: String,
}

const NAMED: &[&str] = &[
    "Left",
    "Right",
    "Up",
    "Down",
    "Tab",
    "Enter",
    "Escape",
    "Space",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Backspace",
    "Delete",
    "Insert",
];

pub fn canonical_key(key: &str) -> Option<String> {
    if let Some(named) = NAMED.iter().find(|n| n.eq_ignore_ascii_case(key)) {
        return Some((*named).to_string());
    }
    if let Some(n) = key.strip_prefix(['F', 'f'])
        && let Ok(n) = n.parse::<u8>()
        && (1..=24).contains(&n)
    {
        return Some(format!("F{n}"));
    }
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_some() || c.is_whitespace() || c.is_control() {
        return None;
    }
    Some(c.to_uppercase().collect())
}

impl Chord {
    pub fn parse(text: &str) -> Option<Self> {
        let mut chord = Chord {
            ctrl: false,
            alt: false,
            shift: false,
            key: String::new(),
        };
        // "+" itself is a key: "Ctrl++" ends with an empty part.
        let text = text.trim();
        let (mods, key) = match text.strip_suffix("++") {
            Some(rest) => (rest, "+"),
            None => text.rsplit_once('+').unwrap_or(("", text)),
        };
        for part in mods.split('+').filter(|p| !p.is_empty()) {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => chord.ctrl = true,
                "alt" | "option" => chord.alt = true,
                "shift" => chord.shift = true,
                _ => return None,
            }
        }
        chord.key = canonical_key(key)?;
        Some(chord)
    }

    /// Whether a user may bind this: needs Ctrl or Alt (or a function key), so typing keeps working.
    pub fn is_bindable(&self) -> bool {
        self.ctrl || self.alt || (self.key.starts_with('F') && self.key.len() > 1)
    }

    /// Ctrl+Shift chords also fire while a text field (sidebar search) has focus; the rest only
    /// when the keyboard belongs to the terminal.
    pub fn fires_in_text_fields(&self) -> bool {
        self.ctrl && self.shift
    }
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        f.write_str(&self.key)
    }
}

/// Per-action overrides as stored in the config: action id → chords (empty = unbound).
pub type Overrides = BTreeMap<String, Vec<String>>;

#[derive(Clone, Debug, Default)]
pub struct Keymap {
    bindings: Vec<(Chord, &'static str)>,
}

impl Keymap {
    pub fn new(overrides: &Overrides) -> Self {
        let mut bindings = Vec::new();
        for def in ACTIONS {
            for chord in effective(overrides, def.id) {
                if let Some(chord) = Chord::parse(&chord) {
                    bindings.push((chord, def.id));
                }
            }
        }
        Self { bindings }
    }

    pub fn resolve(&self, chord: &Chord) -> Option<&'static str> {
        self.bindings
            .iter()
            .find(|(c, _)| c == chord)
            .map(|(_, id)| *id)
    }
}

/// The chords bound to `id`: its override when present, otherwise its defaults.
pub fn effective(overrides: &Overrides, id: &str) -> Vec<String> {
    match overrides.get(id) {
        Some(chords) => chords.clone(),
        None => action(id).map_or_else(Vec::new, |a| {
            a.defaults
                .iter()
                .filter(|default| {
                    let chord = Chord::parse(default);
                    !overrides.iter().any(|(other, assigned)| {
                        other != id
                            && action(other).is_some()
                            && assigned.iter().any(|text| Chord::parse(text) == chord)
                    })
                })
                .map(|text| text.to_string())
                .collect()
        }),
    }
}

/// Binds `chord` as the only shortcut of `id` (or unbinds it with `None`). The chord is taken
/// away from any other action that had it; returns the labels of those actions.
pub fn assign(
    overrides: &mut Overrides,
    id: &str,
    chord: Option<&Chord>,
) -> Result<Vec<&'static str>, String> {
    if action(id).is_none() {
        return Err(format!("unknown action: {id}"));
    }
    let Some(chord) = chord else {
        overrides.insert(id.to_string(), Vec::new());
        return Ok(Vec::new());
    };
    if !chord.is_bindable() {
        return Err("Shortcuts need Ctrl or Alt (or a function key)".into());
    }
    let text = chord.to_string();
    let mut taken = Vec::new();
    for def in ACTIONS.iter().filter(|d| d.id != id) {
        let current = effective(overrides, def.id);
        let kept: Vec<String> = current
            .iter()
            .filter(|c| Chord::parse(c).as_ref() != Some(chord))
            .cloned()
            .collect();
        if kept.len() != current.len() {
            overrides.insert(def.id.to_string(), kept);
            taken.push(def.label);
        }
    }
    overrides.insert(id.to_string(), vec![text]);
    normalize(overrides);
    Ok(taken)
}

/// Back to the defaults for `id`.
pub fn reset(overrides: &mut Overrides, id: &str) {
    overrides.remove(id);
}

/// Drops overrides equal to the defaults, so the config only stores real changes.
fn normalize(overrides: &mut Overrides) {
    overrides.retain(|id, chords| {
        action(id).is_some_and(|a| {
            a.defaults.iter().map(|s| s.to_string()).collect::<Vec<_>>() != *chords
        })
    });
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeybindView {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    pub chords: Vec<String>,
    pub defaults: Vec<String>,
    pub customized: bool,
}

pub fn views(overrides: &Overrides) -> Vec<KeybindView> {
    ACTIONS
        .iter()
        .map(|def| KeybindView {
            id: def.id,
            label: def.label,
            group: def.group,
            description: def.description,
            chords: effective(overrides, def.id),
            defaults: def.defaults.iter().map(|s| s.to_string()).collect(),
            customized: overrides.contains_key(def.id),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(text: &str) -> Chord {
        Chord::parse(text).unwrap()
    }

    #[test]
    fn parses_and_formats_chords() {
        assert_eq!(chord("ctrl+alt+r").to_string(), "Ctrl+Alt+R");
        assert_eq!(chord("Shift+Ctrl+Tab").to_string(), "Ctrl+Shift+Tab");
        assert_eq!(chord("Ctrl+Alt+,").key, ",");
        assert_eq!(chord("Ctrl++").key, "+");
        assert_eq!(chord("f5").to_string(), "F5");
        assert!(Chord::parse("Hyper+X").is_none());
        assert!(Chord::parse("Ctrl+Nope").is_none());
        assert!(!chord("A").is_bindable());
        assert!(chord("F5").is_bindable());
        assert!(chord("Ctrl+Shift+T").fires_in_text_fields());
        assert!(!chord("Alt+H").fires_in_text_fields());
    }

    #[test]
    fn every_default_parses_and_is_unique() {
        let mut seen = std::collections::HashSet::new();
        for def in ACTIONS {
            for text in def.defaults {
                let c = Chord::parse(text).unwrap_or_else(|| panic!("bad default {text}"));
                assert!(seen.insert(c), "duplicate default {text}");
            }
        }
    }

    #[test]
    fn resolves_defaults_and_overrides() {
        let mut overrides = Overrides::new();
        let map = Keymap::new(&overrides);
        assert_eq!(map.resolve(&chord("Ctrl+Shift+T")), Some("session.new"));
        assert_eq!(map.resolve(&chord("Alt+Left")), Some("focus.left"));
        assert_eq!(map.resolve(&chord("Ctrl+Left")), None);
        assert_eq!(map.resolve(&chord("Alt+Shift+Up")), Some("layout.moveUp"));
        assert_eq!(map.resolve(&chord("Ctrl+L")), Some("editor.addToChat"));

        let taken = assign(
            &mut overrides,
            "view.overview",
            Some(&chord("Ctrl+Shift+T")),
        )
        .unwrap();
        assert_eq!(taken, vec!["New session"]);
        let map = Keymap::new(&overrides);
        assert_eq!(map.resolve(&chord("Ctrl+Shift+T")), Some("view.overview"));
        assert_eq!(map.resolve(&chord("Ctrl+Alt+G")), None);
        assert!(effective(&overrides, "session.new").is_empty());

        reset(&mut overrides, "session.new");
        reset(&mut overrides, "view.overview");
        assert!(overrides.is_empty());
        assert!(assign(&mut overrides, "pane.split", Some(&chord("D"))).is_err());
        assign(&mut overrides, "pane.split", None).unwrap();
        assert!(
            Keymap::new(&overrides)
                .resolve(&chord("Ctrl+Shift+D"))
                .is_none()
        );
        // Re-binding the default removes the override entirely.
        assign(&mut overrides, "pane.split", Some(&chord("Ctrl+Shift+D"))).unwrap();
        assert!(overrides.is_empty());
    }

    #[test]
    fn saved_custom_shortcuts_take_precedence_over_new_defaults() {
        let overrides = Overrides::from([("view.overview".into(), vec!["Alt+Shift+Up".into()])]);
        let map = Keymap::new(&overrides);
        assert_eq!(map.resolve(&chord("Alt+Shift+Up")), Some("view.overview"));
        assert!(effective(&overrides, "layout.moveUp").is_empty());
    }
}
