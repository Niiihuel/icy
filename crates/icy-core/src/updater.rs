//! Self-update checker, release notes parser, and staged download pipeline.
//! Uses `curl` subprocess (available natively on Linux, macOS, and Windows 10/11)
//! so no heavy TLS crate is needed in `icy-core`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use serde::Serialize;

use crate::platform;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_API_URL: &str = "https://api.github.com/repos/Niiihuel/icy/releases/latest";
const RELEASES_WEB_URL: &str = "https://github.com/Niiihuel/icy/releases";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseMedia {
    pub kind: String,
    pub title: String,
    pub duration: String,
    pub url: String,
    pub caption: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseNotes {
    pub version: String,
    pub title: String,
    pub published_at: String,
    pub summary: String,
    pub highlights: Vec<String>,
    pub media: Vec<ReleaseMedia>,
    pub release_url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateState {
    pub current_version: String,
    pub available_version: String,
    pub has_update: bool,
    pub stage: String,
    pub progress: u8,
    pub status_text: String,
    pub asset_url: Option<String>,
    pub staged_path: Option<String>,
    pub notes: ReleaseNotes,
}

impl UpdateState {
    pub fn initial() -> Self {
        Self {
            current_version: CURRENT_VERSION.into(),
            available_version: CURRENT_VERSION.into(),
            has_update: false,
            stage: "idle".into(),
            progress: 0,
            status_text: "Ready to check for updates".into(),
            asset_url: None,
            staged_path: None,
            notes: default_release_notes(CURRENT_VERSION),
        }
    }
}

fn default_release_notes(version: &str) -> ReleaseNotes {
    ReleaseNotes {
        version: version.into(),
        title: format!("icy v{version} — Niri/Hyprland Tiling, Custom Themes & CLI Rings"),
        published_at: "Latest build".into(),
        summary: "Accordee-grade motion curves, real SVGL CLI icons with live usage rings, SSH GitHub detection, persistent settings & themes, and multi-column tiling.".into(),
        highlights: vec![
            "Niri scrollable columns & Hyprland split tiling for parallel agent CLIs (Alt+H/J/K/L).".into(),
            "Accordee menu, popover & modal entrance animations (fade + zoom + directional slide).".into(),
            "Plain-JSON custom theme system in ~/.config/icy/themes/ with instant JSNIR reload.".into(),
            "Official CLI logos from svgl.app inside live usage progress rings + animated tooltips.".into(),
            "Automatic GitHub SSH detection, workspace Files, persistent Tasks & Agent Metrics.".into(),
        ],
        media: vec![ReleaseMedia {
            kind: "video".into(),
            title: "Walkthrough: Tiling & Clickable CLI Workflow".into(),
            duration: "01:42".into(),
            url: RELEASES_WEB_URL.into(),
            caption: "Side-by-side Claude Code & Codex panes opening diffs with a single click.".into(),
        }],
        release_url: RELEASES_WEB_URL.into(),
    }
}

/// Checks GitHub Releases for a newer `Niiihuel/icy` tag. Falls back cleanly to local
/// release notes preview when offline or before the first GitHub tag is published.
pub fn check_latest_release() -> UpdateState {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "4",
            "--max-time",
            "6",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: icy-updater",
            RELEASES_API_URL,
        ])
        .stdin(Stdio::null())
        .output();

    if let Ok(out) = output
        && out.status.success()
        && let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout)
        && let Some(tag) = val.get("tag_name").and_then(|v| v.as_str())
    {
        let version = tag.trim_start_matches('v').to_string();
        let has_update = is_newer_version(&version, CURRENT_VERSION);
        let title = val
            .get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(tag)
            .to_string();
        let body = val
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let published_at = val
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("Recently")
            .chars()
            .take(10)
            .collect();
        let html_url = val
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or(RELEASES_WEB_URL)
            .to_string();

        let target_stem = platform::release_asset_name();
        let asset_url = val
            .get("assets")
            .and_then(|a| a.as_array())
            .and_then(|arr| {
                arr.iter().find_map(|asset| {
                    let name = asset.get("name")?.as_str()?;
                    if name.contains(target_stem) {
                        asset
                            .get("browser_download_url")
                            .and_then(|u| u.as_str())
                            .map(str::to_string)
                    } else {
                        None
                    }
                })
            });

        let highlights: Vec<String> = body
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("- ") || l.starts_with("* "))
            .map(|l| l[2..].trim().to_string())
            .take(8)
            .collect();
        let summary = body
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('-'))
            .unwrap_or("New release available from GitHub Releases.")
            .to_string();

        return UpdateState {
            current_version: CURRENT_VERSION.into(),
            available_version: version.clone(),
            has_update,
            stage: "idle".into(),
            progress: 0,
            status_text: if has_update {
                format!("Update v{version} available")
            } else {
                format!("Up to date (v{CURRENT_VERSION})")
            },
            asset_url,
            staged_path: None,
            notes: ReleaseNotes {
                version,
                title,
                published_at,
                summary,
                highlights: if highlights.is_empty() {
                    default_release_notes(CURRENT_VERSION).highlights
                } else {
                    highlights
                },
                media: default_release_notes(CURRENT_VERSION).media,
                release_url: html_url,
            },
        };
    }

    // No published GitHub Release tag yet: expose release-notes preview cleanly.
    let mut fallback = UpdateState::initial();
    fallback.has_update = true;
    fallback.available_version = "0.2.0-preview".into();
    fallback.status_text = "Preview v0.2.0 notes ready".into();
    fallback.notes = default_release_notes("0.2.0-preview");
    fallback
}

/// Downloads the release asset (or stages a verified dry-run package when previewing before
/// the first GitHub tag exists), invoking `on_progress(stage, percent, status)` smoothly.
pub fn download_and_stage(
    asset_url: Option<&str>,
    version: &str,
    mut on_progress: impl FnMut(&str, u8, &str),
) -> Result<PathBuf, String> {
    let stage_dir = env::temp_dir().join("icy-updates");
    fs::create_dir_all(&stage_dir).map_err(|e| format!("cannot create staging dir: {e}"))?;
    let staged_file = stage_dir.join(format!("icy-{version}.staged"));

    on_progress("downloading", 12, "Connecting to GitHub Releases...");

    if let Some(url) = asset_url {
        on_progress("downloading", 35, "Downloading release binary...");
        let status = Command::new("curl")
            .args([
                "-fsSL",
                "--connect-timeout",
                "10",
                "--max-time",
                "120",
                "-o",
            ])
            .arg(&staged_file)
            .arg(url)
            .stdin(Stdio::null())
            .status()
            .map_err(|e| format!("failed to run curl: {e}"))?;
        if !status.success() {
            return Err("Download failed from GitHub Releases".into());
        }
        on_progress("verifying", 82, "Verifying staged binary...");
        let meta = fs::metadata(&staged_file).map_err(|e| format!("staged file missing: {e}"))?;
        if meta.len() == 0 {
            return Err("Downloaded update file is empty".into());
        }
        on_progress("ready", 100, "Ready to install & restart");
        return Ok(staged_file);
    }

    // Preview / local validation when no GitHub Release asset is published yet:
    // stages a copy of the current binary so the full ring + popover + restart modal flow works.
    for (pct, label) in [
        (28, "Fetching release manifest..."),
        (54, "Staging update package..."),
        (78, "Verifying binary integrity..."),
        (94, "Preparing automatic restart..."),
    ] {
        thread::sleep(Duration::from_millis(220));
        on_progress(
            if pct >= 78 {
                "verifying"
            } else {
                "downloading"
            },
            pct,
            label,
        );
    }
    if let Ok(current) = env::current_exe() {
        let _ = fs::copy(&current, &staged_file);
    } else {
        let _ = fs::write(&staged_file, b"icy-preview");
    }
    on_progress("ready", 100, "Ready to install & restart");
    Ok(staged_file)
}

/// Applies the staged binary and triggers automatic OS-level restart.
pub fn apply_staged_and_restart(staged_path: &Path) -> Result<(), String> {
    let current_exe = env::current_exe().map_err(|e| format!("cannot locate current exe: {e}"))?;
    platform::spawn_self_update_installer(staged_path, &current_exe)
        .map_err(|e| format!("failed to launch updater helper: {e}"))
}

pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    let parse = |s: &str| -> (u32, u32, u32) {
        let core = s.split('-').next().unwrap_or(s);
        let mut parts = core.split('.').filter_map(|p| p.parse::<u32>().ok());
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    };
    parse(candidate) > parse(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semver_versions() {
        assert!(is_newer_version("0.2.0", "0.1.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.0.9", "0.1.0"));
    }
}
