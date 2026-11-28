//! Persistent workspace configuration and user-editable JSNIR JSON themes
//! (inspired by herdr's plain-file configuration in `~/.config/icy/`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::agent_notifications::NotificationPreferences;
use crate::platform;
use crate::tiling::TilingMode;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    pub auto_detect: bool,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IcyConfig {
    pub theme: String,
    pub mode: String,
    pub accent: String,
    pub density: String,
    pub font_size: u16,
    /// Transparency of the native window background as a percentage (0 is solid).
    #[serde(default, deserialize_with = "deserialize_window_transparency")]
    pub window_transparency: u8,
    /// Keep the session server (and its system tray icon) alive when closing the window.
    #[serde(default = "default_true")]
    pub keep_running_in_background: bool,
    pub tiling_mode: TilingMode,
    /// niri layout options (default column width, center-focused-column, …).
    #[serde(default)]
    pub layout: crate::tiling::LayoutOptions,
    pub check_updates: bool,
    pub auto_restart_update: bool,
    #[serde(default)]
    pub agent_notifications: NotificationPreferences,
    pub clis: Vec<CliConfig>,
    /// Project folders pinned in the sidebar (Antigravity "Projects"); sessions group under them.
    #[serde(default)]
    pub projects: Vec<String>,
    /// Keyboard shortcut overrides (action id → chords); missing actions use their defaults.
    #[serde(default)]
    pub keybinds: crate::keybinds::Overrides,
    /// Last used / preferred external IDE id (e.g. `"cursor"`, `"vscode"`, `"zed"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_editor: Option<String>,
}

impl Default for IcyConfig {
    fn default() -> Self {
        Self {
            theme: "accordee-dark".into(),
            mode: "dark".into(),
            accent: "blue".into(),
            density: "comfortable".into(),
            font_size: 13,
            window_transparency: 0,
            keep_running_in_background: true,
            tiling_mode: TilingMode::Niri,
            layout: Default::default(),
            check_updates: true,
            auto_restart_update: true,
            agent_notifications: NotificationPreferences::default(),
            clis: default_clis(),
            projects: Vec::new(),
            keybinds: Default::default(),
            preferred_editor: None,
        }
    }
}

fn default_true() -> bool {
    true
}

pub fn window_transparency_percent(value: i64) -> u8 {
    value.clamp(0, 100) as u8
}

fn deserialize_window_transparency<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u8, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(window_transparency_percent(value.as_i64().unwrap_or(0)))
}

pub fn default_clis() -> Vec<CliConfig> {
    vec![
        CliConfig {
            id: "claude".into(),
            name: "Claude Code".into(),
            command: "claude".into(),
            args: Vec::new(),
            enabled: true,
            auto_detect: true,
            description: "Anthropic CLI coding agent".into(),
        },
        CliConfig {
            id: "antigravity".into(),
            name: "Antigravity".into(),
            command: "agy".into(),
            args: Vec::new(),
            enabled: true,
            auto_detect: true,
            description: "Google Antigravity agent CLI".into(),
        },
        CliConfig {
            id: "codex".into(),
            name: "Codex".into(),
            command: "codex".into(),
            args: Vec::new(),
            enabled: true,
            auto_detect: true,
            description: "OpenAI Codex terminal agent".into(),
        },
        CliConfig {
            id: "grok".into(),
            name: "Grok CLI".into(),
            command: "grok".into(),
            args: Vec::new(),
            enabled: true,
            auto_detect: true,
            description: "xAI Grok command-line agent".into(),
        },
        CliConfig {
            id: "junie".into(),
            name: "Junie".into(),
            command: "junie".into(),
            args: Vec::new(),
            enabled: true,
            auto_detect: true,
            description: "JetBrains Junie coding agent".into(),
        },
        CliConfig {
            id: "opencode".into(),
            name: "OpenCode".into(),
            command: "opencode".into(),
            args: Vec::new(),
            enabled: true,
            auto_detect: true,
            description: "Open-source terminal coding agent".into(),
        },
        pi_cli(),
    ]
}

fn pi_cli() -> CliConfig {
    CliConfig {
        id: "pi".into(),
        name: "Pi".into(),
        command: "pi".into(),
        args: Vec::new(),
        enabled: true,
        auto_detect: true,
        description: "Extensible terminal coding agent".into(),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemeChartColors<T> {
    pub input: Option<T>,
    pub output: Option<T>,
    pub sessions: Option<T>,
    pub duration: Option<T>,
    pub axis: Option<T>,
    pub grid: Option<T>,
    pub track: Option<T>,
    pub heatmap: Vec<Option<T>>,
    pub providers: BTreeMap<String, T>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColorsHex {
    pub app_bg: String,
    pub sidebar_bg: String,
    pub sidebar_active: String,
    pub panel_bg: String,
    pub card_bg: String,
    pub kbd_bg: String,
    pub text_primary: String,
    pub text_secondary: String,
    pub text_muted: String,
    pub border: String,
    pub highlight: String,
    /// Native text selection colour; absent derives from `highlight`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charts: Option<ThemeChartColors<String>>,
    /// Git graph lane colours, cycled by lane; empty keeps the built-in palette.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graph: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColorsRgba {
    pub app_bg: u32,
    pub sidebar_bg: u32,
    pub sidebar_active: u32,
    pub panel_bg: u32,
    pub card_bg: u32,
    pub kbd_bg: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub border: u32,
    pub highlight: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charts: Option<ThemeChartColors<u32>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub graph: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserThemeFile {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub author: String,
    pub colors: ThemeColorsHex,
    /// Icon overrides: a glyph name (`git-branch`, `folder`, …) or `cli:<id>` mapped to an
    /// SVG file (relative to the theme file) or inline `<svg …>` markup.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub icons: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTheme {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub author: String,
    pub path: String,
    pub colors: ThemeColorsRgba,
    /// Icon overrides with file sources made absolute (inline SVG kept as is).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub icons: BTreeMap<String, String>,
}

/// Resolves theme icon sources: inline SVG stays, file paths become absolute against `dir`.
pub fn resolve_icons(icons: &BTreeMap<String, String>, dir: &Path) -> BTreeMap<String, String> {
    icons
        .iter()
        .map(|(name, source)| {
            let source = if source.trim_start().starts_with('<') {
                source.clone()
            } else {
                dir.join(source).display().to_string()
            };
            (name.clone(), source)
        })
        .collect()
}

pub fn parse_hex_rgba(raw: &str) -> Option<u32> {
    let hex = raw.trim().strip_prefix('#').unwrap_or(raw.trim());
    match hex.len() {
        6 => {
            let rgb = u32::from_str_radix(hex, 16).ok()?;
            Some((rgb << 8) | 0xff)
        }
        8 => u32::from_str_radix(hex, 16).ok(),
        3 => {
            let mut bytes = [0u8; 3];
            for (i, ch) in hex.chars().enumerate() {
                let v = ch.to_digit(16)? as u8;
                bytes[i] = (v << 4) | v;
            }
            Some(
                ((bytes[0] as u32) << 24)
                    | ((bytes[1] as u32) << 16)
                    | ((bytes[2] as u32) << 8)
                    | 0xff,
            )
        }
        _ => None,
    }
}

fn resolve_colors(hex: &ThemeColorsHex, fallback_dark: bool) -> ThemeColorsRgba {
    let d = |s: &str, fb: u32| parse_hex_rgba(s).unwrap_or(fb);
    let charts = hex.charts.as_ref().map(|colors| {
        let parse = |color: &Option<String>| color.as_deref().and_then(parse_hex_rgba);
        ThemeChartColors {
            input: parse(&colors.input),
            output: parse(&colors.output),
            sessions: parse(&colors.sessions),
            duration: parse(&colors.duration),
            axis: parse(&colors.axis),
            grid: parse(&colors.grid),
            track: parse(&colors.track),
            heatmap: colors.heatmap.iter().take(6).map(parse).collect(),
            providers: colors
                .providers
                .iter()
                .filter_map(|(name, color)| Some((name.clone(), parse_hex_rgba(color)?)))
                .collect(),
        }
    });
    let graph: Vec<u32> = hex.graph.iter().filter_map(|c| parse_hex_rgba(c)).collect();
    if fallback_dark {
        ThemeColorsRgba {
            app_bg: d(&hex.app_bg, 0x0a0a_0aff),
            sidebar_bg: d(&hex.sidebar_bg, 0x0f0f_10ff),
            sidebar_active: d(&hex.sidebar_active, 0x1f1f_22ff),
            panel_bg: d(&hex.panel_bg, 0x1717_17ff),
            card_bg: d(&hex.card_bg, 0x1717_17ff),
            kbd_bg: d(&hex.kbd_bg, 0x2626_26ff),
            text_primary: d(&hex.text_primary, 0xfafa_faff),
            text_secondary: d(&hex.text_secondary, 0xa3a3_a3ff),
            text_muted: d(&hex.text_muted, 0x8a8a_8aff),
            border: d(&hex.border, 0xffff_ff1a),
            highlight: d(&hex.highlight, 0x3b82_f6ff),
            selection: hex.selection.as_deref().and_then(parse_hex_rgba),
            charts,
            graph,
        }
    } else {
        ThemeColorsRgba {
            app_bg: d(&hex.app_bg, 0xffff_ffff),
            sidebar_bg: d(&hex.sidebar_bg, 0xfafa_faff),
            sidebar_active: d(&hex.sidebar_active, 0xf0f0_f1ff),
            panel_bg: d(&hex.panel_bg, 0xf4f4_f5ff),
            card_bg: d(&hex.card_bg, 0xffff_ffff),
            kbd_bg: d(&hex.kbd_bg, 0xe5e7_ebff),
            text_primary: d(&hex.text_primary, 0x0909_0bff),
            text_secondary: d(&hex.text_secondary, 0x5252_5bff),
            text_muted: d(&hex.text_muted, 0x7171_7aff),
            border: d(&hex.border, 0x0000_001a),
            highlight: d(&hex.highlight, 0x2563_ebff),
            selection: hex.selection.as_deref().and_then(parse_hex_rgba),
            charts,
            graph,
        }
    }
}

pub fn config_path() -> PathBuf {
    platform::config_dir().join("config.json")
}

pub fn themes_dir() -> PathBuf {
    platform::config_dir().join("themes")
}

pub fn load_config() -> IcyConfig {
    load_config_from(&config_path())
}

pub fn load_config_from(path: &Path) -> IcyConfig {
    let Ok(raw) = fs::read_to_string(path) else {
        let cfg = IcyConfig::default();
        let _ = save_config_to(path, &cfg);
        return cfg;
    };
    let mut cfg: IcyConfig = serde_json::from_str(&raw).unwrap_or_default();
    if !cfg.clis.iter().any(|cli| cli.id == "pi") {
        cfg.clis.push(pi_cli());
    }
    cfg
}

pub fn save_config(cfg: &IcyConfig) -> std::io::Result<()> {
    save_config_to(&config_path(), cfg)
}

pub fn save_config_to(path: &Path, cfg: &IcyConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    fs::write(path, json)
}

fn builtin_theme_files() -> Vec<UserThemeFile> {
    vec![
        UserThemeFile {
            id: "accordee-dark".into(),
            name: "Accordee Dark".into(),
            mode: "dark".into(),
            author: "icy / Accordee".into(),
            colors: ThemeColorsHex {
                app_bg: "#0a0a0a".into(),
                sidebar_bg: "#0f0f10".into(),
                sidebar_active: "#1f1f22".into(),
                panel_bg: "#171717".into(),
                card_bg: "#171717".into(),
                kbd_bg: "#262626".into(),
                text_primary: "#fafafa".into(),
                text_secondary: "#a3a3a3".into(),
                text_muted: "#8a8a8a".into(),
                border: "#ffffff1a".into(),
                highlight: "#3b82f6".into(),
                selection: None,
                charts: None,
                graph: Vec::new(),
            },
            icons: BTreeMap::new(),
        },
        UserThemeFile {
            id: "accordee-light".into(),
            name: "Accordee Light".into(),
            mode: "light".into(),
            author: "icy / Accordee".into(),
            colors: ThemeColorsHex {
                app_bg: "#ffffff".into(),
                sidebar_bg: "#fafafa".into(),
                sidebar_active: "#f0f0f1".into(),
                panel_bg: "#f4f4f5".into(),
                card_bg: "#ffffff".into(),
                kbd_bg: "#e5e7eb".into(),
                text_primary: "#09090b".into(),
                text_secondary: "#52525b".into(),
                text_muted: "#71717a".into(),
                border: "#0000001a".into(),
                highlight: "#2563eb".into(),
                selection: None,
                charts: None,
                graph: Vec::new(),
            },
            icons: BTreeMap::new(),
        },
        UserThemeFile {
            id: "tokyo-night".into(),
            name: "Tokyo Night".into(),
            mode: "dark".into(),
            author: "Community preset".into(),
            colors: ThemeColorsHex {
                app_bg: "#1a1b26".into(),
                sidebar_bg: "#16161e".into(),
                sidebar_active: "#282e44".into(),
                panel_bg: "#1f2335".into(),
                card_bg: "#1f2335".into(),
                kbd_bg: "#292e42".into(),
                text_primary: "#c0caf5".into(),
                text_secondary: "#9aa5ce".into(),
                text_muted: "#565f89".into(),
                border: "#c0caf51c".into(),
                highlight: "#7aa2f7".into(),
                selection: None,
                charts: None,
                graph: Vec::new(),
            },
            icons: BTreeMap::new(),
        },
        UserThemeFile {
            id: "catppuccin-mocha".into(),
            name: "Catppuccin Mocha".into(),
            mode: "dark".into(),
            author: "Community preset".into(),
            colors: ThemeColorsHex {
                app_bg: "#1e1e2e".into(),
                sidebar_bg: "#181825".into(),
                sidebar_active: "#313244".into(),
                panel_bg: "#242436".into(),
                card_bg: "#242436".into(),
                kbd_bg: "#313244".into(),
                text_primary: "#cdd6f4".into(),
                text_secondary: "#bac2de".into(),
                text_muted: "#7f849c".into(),
                border: "#cdd6f41a".into(),
                highlight: "#89b4fa".into(),
                selection: None,
                charts: None,
                graph: Vec::new(),
            },
            icons: BTreeMap::new(),
        },
    ]
}

/// Ensures starter JSON theme files exist under `~/.config/icy/themes/` and loads every
/// user-edited or custom `*.json` theme file from that directory.
pub fn load_themes() -> Vec<ResolvedTheme> {
    load_themes_from(&themes_dir())
}

pub fn load_themes_from(dir: &Path) -> Vec<ResolvedTheme> {
    let _ = fs::create_dir_all(dir);
    for preset in builtin_theme_files() {
        let file = dir.join(format!("{}.json", preset.id));
        if !file.exists()
            && let Ok(json) = serde_json::to_string_pretty(&preset)
        {
            let _ = fs::write(&file, json);
        }
    }

    let mut themes = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(&path)
                && let Ok(parsed) = serde_json::from_str::<UserThemeFile>(&raw)
            {
                let is_dark = parsed.mode != "light";
                themes.push(ResolvedTheme {
                    id: parsed.id,
                    name: parsed.name,
                    mode: parsed.mode,
                    author: parsed.author,
                    path: path.display().to_string(),
                    colors: resolve_colors(&parsed.colors, is_dark),
                    icons: resolve_icons(&parsed.icons, path.parent().unwrap_or(dir)),
                });
            }
        }
    }

    if themes.is_empty() {
        for preset in builtin_theme_files() {
            let is_dark = preset.mode != "light";
            themes.push(ResolvedTheme {
                id: preset.id,
                name: preset.name,
                mode: preset.mode,
                author: preset.author,
                path: String::new(),
                colors: resolve_colors(&preset.colors, is_dark),
                icons: BTreeMap::new(),
            });
        }
    }

    themes.sort_by(|a, b| a.id.cmp(&b.id));
    themes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_icons_resolve_files_against_the_theme_dir_and_keep_inline_svg() {
        let icons = BTreeMap::from([
            ("folder".to_string(), "icons/folder.svg".to_string()),
            (
                "cli:claude".to_string(),
                "<svg xmlns='http://www.w3.org/2000/svg'/>".to_string(),
            ),
        ]);
        let resolved = resolve_icons(&icons, Path::new("/themes/mine"));
        assert_eq!(
            resolved["folder"],
            Path::new("/themes/mine")
                .join("icons/folder.svg")
                .display()
                .to_string()
        );
        assert_eq!(resolved["cli:claude"], icons["cli:claude"]);
        let file: UserThemeFile = serde_json::from_str(
            r##"{"id":"t","name":"T","mode":"dark","author":"a","colors":{"appBg":"#000","sidebarBg":"#000","sidebarActive":"#000","panelBg":"#000","cardBg":"#000","kbdBg":"#000","textPrimary":"#fff","textSecondary":"#fff","textMuted":"#fff","border":"#fff","highlight":"#fff"},"icons":{"folder":"f.svg"}}"##,
        ).unwrap();
        assert_eq!(file.icons["folder"], "f.svg");
    }

    #[test]
    fn themes_resolve_partial_chart_palettes() {
        let mut theme = serde_json::to_value(&builtin_theme_files()[0]).unwrap();
        assert!(theme["colors"].get("charts").is_none());
        theme["colors"]["charts"] = serde_json::json!({
            "output": "#34a78ac0",
            "grid": "invalid",
            "heatmap": [null, "#234"],
            "providers": { "openai": "#abc", "other": "invalid" }
        });
        let parsed: UserThemeFile = serde_json::from_value(theme).unwrap();
        let resolved = resolve_colors(&parsed.colors, true);
        let charts = resolved.charts.unwrap();
        assert_eq!(charts.input, None);
        assert_eq!(charts.output, Some(0x34a7_8ac0));
        assert_eq!(charts.grid, None);
        assert_eq!(charts.heatmap, vec![None, Some(0x2233_44ff)]);
        assert_eq!(
            charts.providers,
            BTreeMap::from([("openai".into(), 0xaabb_ccff)])
        );
    }

    #[test]
    fn theme_selection_colour_is_optional() {
        let mut theme = serde_json::to_value(&builtin_theme_files()[0]).unwrap();
        theme["colors"]["selection"] = serde_json::json!("#11223344");
        let parsed: UserThemeFile = serde_json::from_value(theme).unwrap();
        assert_eq!(
            resolve_colors(&parsed.colors, true).selection,
            Some(0x1122_3344)
        );
        let parsed: UserThemeFile =
            serde_json::from_value(serde_json::to_value(&builtin_theme_files()[0]).unwrap())
                .unwrap();
        assert_eq!(resolve_colors(&parsed.colors, true).selection, None);
    }

    #[test]
    fn adds_pi_to_existing_configs_and_keeps_user_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut existing = IcyConfig::default();
        existing.clis.retain(|cli| cli.id != "pi");
        existing.clis[0].enabled = false;
        save_config_to(&path, &existing).unwrap();

        let mut loaded = load_config_from(&path);
        assert_eq!(&loaded.clis[..existing.clis.len()], &existing.clis);
        let pi = loaded.clis.last_mut().unwrap();
        assert_eq!(*pi, pi_cli());
        pi.enabled = false;
        pi.command = "custom-pi".into();
        pi.args = vec!["--no-extensions".into()];
        save_config_to(&path, &loaded).unwrap();
        assert_eq!(load_config_from(&path), loaded);
    }

    #[test]
    fn parses_hex_colors_and_roundtrips_config() {
        assert_eq!(parse_hex_rgba("#0a0a0a"), Some(0x0a0a_0aff));
        assert_eq!(parse_hex_rgba("#ffffff1a"), Some(0xffff_ff1a));
        assert_eq!(parse_hex_rgba("#fff"), Some(0xffff_ffff));
        assert_eq!(parse_hex_rgba("invalid"), None);

        let dir = std::env::temp_dir().join(format!("icy-cfg-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let cfg_path = dir.join("config.json");
        let cfg = IcyConfig {
            density: "compact".into(),
            window_transparency: 35,
            keep_running_in_background: false,
            agent_notifications: NotificationPreferences {
                needs_input: true,
                ..Default::default()
            },
            preferred_editor: Some("cursor".into()),
            ..IcyConfig::default()
        };
        save_config_to(&cfg_path, &cfg).expect("save config");
        let loaded = load_config_from(&cfg_path);
        assert_eq!(loaded.density, "compact");
        assert_eq!(loaded.window_transparency, 35);
        assert!(!loaded.keep_running_in_background);
        assert_eq!(loaded.agent_notifications, cfg.agent_notifications);
        assert_eq!(loaded.preferred_editor.as_deref(), Some("cursor"));

        // Older configs load defaults for preferences introduced after they were saved.
        let mut raw_val = serde_json::to_value(&cfg).unwrap();
        raw_val.as_object_mut().unwrap().remove("preferredEditor");
        raw_val
            .as_object_mut()
            .unwrap()
            .remove("windowTransparency");
        raw_val
            .as_object_mut()
            .unwrap()
            .remove("keepRunningInBackground");
        raw_val
            .as_object_mut()
            .unwrap()
            .remove("agentNotifications");
        let legacy: IcyConfig = serde_json::from_value(raw_val.clone()).unwrap();
        assert_eq!(legacy.preferred_editor, None);
        assert_eq!(legacy.window_transparency, 0);
        assert!(legacy.keep_running_in_background);
        assert_eq!(
            legacy.agent_notifications,
            NotificationPreferences::default()
        );
        raw_val["windowTransparency"] = 150.into();
        let clamped: IcyConfig = serde_json::from_value(raw_val).unwrap();
        assert_eq!(clamped.window_transparency, 100);

        let themes = load_themes_from(&dir.join("themes"));
        assert!(themes.iter().any(|t| t.id == "accordee-dark"));
        assert!(themes.iter().any(|t| t.id == "tokyo-night"));
        let _ = fs::remove_dir_all(&dir);
    }
}
