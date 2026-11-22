//! Persistent workspace configuration and user-editable JSNIR JSON themes
//! (inspired by herdr's plain-file configuration in `~/.config/icy/`).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IcyConfig {
    pub theme: String,
    pub mode: String,
    pub accent: String,
    pub density: String,
    pub font_size: u16,
    pub tiling_mode: TilingMode,
    pub check_updates: bool,
    pub auto_restart_update: bool,
    pub clis: Vec<CliConfig>,
}

impl Default for IcyConfig {
    fn default() -> Self {
        Self {
            theme: "accordee-dark".into(),
            mode: "dark".into(),
            accent: "blue".into(),
            density: "comfortable".into(),
            font_size: 13,
            tiling_mode: TilingMode::Niri,
            check_updates: true,
            auto_restart_update: true,
            clis: default_clis(),
        }
    }
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
    ]
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserThemeFile {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub author: String,
    pub colors: ThemeColorsHex,
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
    serde_json::from_str(&raw).unwrap_or_default()
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
            },
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
            },
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
            },
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
            },
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
            ..IcyConfig::default()
        };
        save_config_to(&cfg_path, &cfg).expect("save config");
        let loaded = load_config_from(&cfg_path);
        assert_eq!(loaded.density, "compact");

        let themes = load_themes_from(&dir.join("themes"));
        assert!(themes.iter().any(|t| t.id == "accordee-dark"));
        assert!(themes.iter().any(|t| t.id == "tokyo-night"));
        let _ = fs::remove_dir_all(&dir);
    }
}
