//! Self-update checker, release notes parser, and staged download pipeline.
//! Uses `curl` subprocess (available natively on Linux, macOS, and Windows 10/11)
//! so no heavy TLS crate is needed in `icy-core`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
    pub poster_url: Option<String>,
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
        title: format!("icy v{version}"),
        published_at: "Latest build".into(),
        summary: String::new(),
        highlights: Vec::new(),
        media: Vec::new(),
        release_url: RELEASES_WEB_URL.into(),
    }
}

fn media_url(url: &str) -> Option<&str> {
    let url = url.trim().trim_start_matches('<').trim_end_matches('>');
    let authority = url
        .strip_prefix("https://")?
        .split(['/', '?', '#'])
        .next()?;
    (!authority.is_empty()
        && !authority.contains('@')
        && !url.contains(['\\', '"', '\''])
        && !url.chars().any(char::is_whitespace))
    .then_some(url)
}

fn extension_kind(url: &str) -> Option<&'static str> {
    let path = url.split(['?', '#']).next()?;
    match path.rsplit_once('.')?.1.to_ascii_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" => Some("image"),
        "mp4" | "webm" | "mov" | "ogv" => Some("video"),
        _ => None,
    }
}

fn media(kind: &str, title: &str, url: &str, poster: Option<&str>) -> Option<ReleaseMedia> {
    Some(ReleaseMedia {
        kind: kind.into(),
        title: title.trim().into(),
        duration: String::new(),
        url: media_url(url)?.into(),
        caption: String::new(),
        poster_url: poster.and_then(media_url).map(str::to_owned),
    })
}

fn markdown_media_link(line: &str) -> Option<(&str, &str)> {
    let (label, target) = line.strip_prefix('[')?.split_once("](")?;
    let url = target
        .trim_end()
        .strip_suffix(')')?
        .split_whitespace()
        .next()?;
    Some((label, url))
}

fn html_attribute(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=");
    let value = tag.split_once(&key)?.1;
    let quote = value.chars().next().filter(|ch| matches!(ch, '"' | '\''))?;
    value[1..]
        .split_once(quote)
        .map(|(value, _)| value.replace("&amp;", "&"))
}

fn release_media_line(line: &str) -> Option<ReleaseMedia> {
    let line = line.trim();
    let line = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .unwrap_or(line);
    if let Some((poster, target)) = line
        .strip_prefix("[!")
        .and_then(|line| line.rsplit_once("]("))
        && let Some(target) = target.strip_suffix(')').and_then(media_url)
        && (extension_kind(target) == Some("video") || github_attachment(target))
        && let Some((title, poster)) = markdown_media_link(poster)
    {
        return media("video", title, target, Some(poster));
    }
    if let Some(image) = line.strip_prefix('!') {
        let (title, url) = markdown_media_link(image)?;
        return media("image", title, url, None);
    }
    if line.starts_with("<img ") || line.starts_with("<video ") || line.starts_with("<video>") {
        let kind = if line.starts_with("<img ") {
            "image"
        } else {
            "video"
        };
        let url = html_attribute(line, "src")?;
        let title = html_attribute(line, "alt").unwrap_or_default();
        let poster = html_attribute(line, "poster");
        return media(kind, &title, &url, poster.as_deref());
    }
    let (title, url) = markdown_media_link(line).unwrap_or(("", line));
    let url = media_url(url)?;
    // GitHub places uploaded videos on their own line. Uploaded images use image markup.
    let kind = extension_kind(url).or_else(|| github_attachment(url).then_some("video"))?;
    media(kind, title, url, None)
}

fn github_attachment(url: &str) -> bool {
    url.starts_with("https://github.com/user-attachments/assets/")
}

fn release_content(
    body: &str,
    assets: &[serde_json::Value],
) -> (String, Vec<String>, Vec<ReleaseMedia>) {
    let mut summary = String::new();
    let mut highlights = Vec::new();
    let mut attachments = Vec::new();
    for line in body.lines().map(str::trim) {
        if let Some(item) = release_media_line(line) {
            attachments.push(item);
        } else if line.starts_with("![") || line.starts_with("<img ") || line.starts_with("<video")
        {
            continue;
        } else if let Some(bullet) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            if highlights.len() < 8 {
                highlights.push(bullet.trim().to_owned());
            }
        } else if summary.is_empty() && !line.is_empty() && !line.starts_with('#') {
            summary = line.to_owned();
        }
    }
    attachments.extend(assets.iter().filter_map(|asset| {
        let url = asset.get("browser_download_url")?.as_str()?;
        let name = asset.get("name")?.as_str()?;
        let kind = match asset.get("content_type").and_then(|v| v.as_str()) {
            Some(kind) if kind.starts_with("image/") => "image",
            Some(kind) if kind.starts_with("video/") => "video",
            _ => extension_kind(name)?,
        };
        media(kind, name, url, None)
    }));
    let mut seen = std::collections::HashSet::new();
    attachments.retain(|item| seen.insert(item.url.clone()));
    (summary, highlights, attachments)
}

/// Fetch a bounded raster cover off the UI thread. Redirects stay on HTTPS.
pub fn fetch_release_image(url: &str) -> Option<tempfile::TempPath> {
    const MAX_BYTES: usize = 8 * 1024 * 1024;
    const MAX_PIXELS: u64 = 4096 * 4096;
    let url = media_url(url)?;
    // Close the handle before invoking `curl -o` so Windows does not reject writes to the temp file.
    let path = tempfile::NamedTempFile::new().ok()?.into_temp_path();
    let status = Command::new("curl")
        .args([
            "-fsL",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "4",
            "--max-time",
            "10",
            "--max-filesize",
            &MAX_BYTES.to_string(),
            "-o",
        ])
        .arg(&*path)
        .args(["--", url])
        .stdin(Stdio::null())
        .status()
        .ok()?;
    if !status.success() || fs::metadata(&path).ok()?.len() > MAX_BYTES as u64 {
        return None;
    }
    let (w, h) = crate::graphics::image_dimensions(&fs::read(&path).ok()?)?;
    (w > 0 && h > 0 && u64::from(w) * u64::from(h) <= MAX_PIXELS).then_some(path)
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

        let assets = val
            .get("assets")
            .and_then(|v| v.as_array())
            .map_or(&[][..], Vec::as_slice);
        let (summary, highlights, media) = release_content(&body, assets);

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
                highlights,
                media,
                release_url: html_url,
            },
        };
    }

    // Offline or no published release yet: never invent an update.
    let mut fallback = UpdateState::initial();
    fallback.status_text = format!("Up to date (v{CURRENT_VERSION})");
    fallback
}

/// Downloads the published release asset for this platform, invoking
/// `on_progress(stage, percent, status)` as it advances.
pub fn download_and_stage(
    asset_url: Option<&str>,
    version: &str,
    mut on_progress: impl FnMut(&str, u8, &str),
) -> Result<PathBuf, String> {
    let Some(url) = asset_url else {
        return Err("No release binary published for this platform".into());
    };
    let stage_dir = env::temp_dir().join("icy-updates");
    fs::create_dir_all(&stage_dir).map_err(|e| format!("cannot create staging dir: {e}"))?;
    let staged_file = stage_dir.join(format!("icy-{version}.staged"));

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
    fn extracts_release_cover_media_without_turning_attachments_into_notes() {
        let body = r#"
# icy v2
![Workspace](https://example.com/cover.png)
[![Preview](https://example.com/poster.jpg)](https://example.com/demo.webm)
<video controls poster="https://example.com/poster.jpg"><source src='https://example.com/tour.mp4'></video>
https://github.com/user-attachments/assets/example
![Unsafe](file:///private/image.png)

A new workspace.
- A faster overview.
"#;
        let assets = [
            serde_json::json!({"name": "preview", "content_type": "image/png",
                "browser_download_url": "https://example.com/cover.png"}),
            serde_json::json!({"name": "demo.mp4", "content_type": "application/octet-stream",
                "browser_download_url": "https://example.com/asset.mp4"}),
        ];
        let (summary, notes, media) = release_content(body, &assets);
        assert_eq!(summary, "A new workspace.");
        assert_eq!(notes, ["A faster overview."]);
        assert_eq!(media.len(), 5);
        assert_eq!(media[0].kind, "image");
        assert_eq!(
            media[1].poster_url.as_deref(),
            Some("https://example.com/poster.jpg")
        );
        assert_eq!(media[2].url, "https://example.com/tour.mp4");
        assert_eq!(media[3].kind, "video");
        assert_eq!(media[4].url, "https://example.com/asset.mp4");
    }

    #[test]
    fn compares_semver_versions() {
        assert!(is_newer_version("0.2.0", "0.1.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.0.9", "0.1.0"));
    }
}
