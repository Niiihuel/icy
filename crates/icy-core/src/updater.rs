//! Self-update checker, release notes parser, and staged download pipeline.
//! Uses `curl` subprocess (available natively on Linux, macOS, and Windows 10/11)
//! so no heavy TLS crate is needed in `icy-core`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use semver::Version;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::platform;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_API_URL: &str = "https://api.github.com/repos/Niiihuel/icy/releases/latest";
const RELEASES_WEB_URL: &str = "https://github.com/Niiihuel/icy/releases";
const CHECK_INTERVAL: Duration = Duration::from_hours(4);
const FIRST_RETRY: Duration = Duration::from_mins(5);
const MAX_RETRY: Duration = Duration::from_hours(1);

pub struct CheckSchedule {
    enabled: bool,
    next: Instant,
    retry: Duration,
    rate_limited_until: Instant,
}

impl CheckSchedule {
    pub fn new(enabled: bool, now: Instant) -> Self {
        Self {
            enabled,
            next: now,
            retry: FIRST_RETRY,
            rate_limited_until: now,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool, now: Instant) {
        if enabled && !self.enabled {
            self.next = now;
            self.retry = FIRST_RETRY;
        }
        self.enabled = enabled;
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.enabled
            .then_some(self.next.max(self.rate_limited_until))
    }

    pub fn can_check(&self, now: Instant) -> bool {
        now >= self.rate_limited_until
    }

    pub fn complete(&mut self, now: Instant, error: Option<&CheckError>) {
        if let Some(error) = error {
            self.next = now + self.retry;
            self.retry = (self.retry * 2).min(MAX_RETRY);
            if let Some(delay) = error.retry_after.and_then(|delay| now.checked_add(delay)) {
                self.rate_limited_until = delay;
            }
        } else {
            self.next = now + CHECK_INTERVAL;
            self.retry = FIRST_RETRY;
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    #[default]
    Idle,
    Checking,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReleaseAsset {
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Default)]
pub struct CheckError {
    retry_after: Option<Duration>,
}

pub struct CheckedRelease {
    pub release: UpdateState,
    pub etag: Option<String>,
}

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
    pub check_status: CheckStatus,
    pub stage: String,
    pub progress: u8,
    pub status_text: String,
    pub asset: Option<ReleaseAsset>,
    #[serde(skip)]
    pub staged_path: Option<PathBuf>,
    pub notes: ReleaseNotes,
}

impl UpdateState {
    pub fn initial() -> Self {
        Self {
            current_version: CURRENT_VERSION.into(),
            available_version: CURRENT_VERSION.into(),
            has_update: false,
            check_status: CheckStatus::Idle,
            stage: "idle".into(),
            progress: 0,
            status_text: "Ready to check for updates".into(),
            asset: None,
            staged_path: None,
            notes: default_release_notes(CURRENT_VERSION),
        }
    }

    pub fn can_check(&self) -> bool {
        self.check_status != CheckStatus::Checking
            && matches!(self.stage.as_str(), "idle" | "error")
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

fn curl() -> Command {
    let mut command = platform::background_command("curl");
    command
        .args(["-q", "-sSL", "--proto", "=https", "--proto-redir", "=https"])
        .stdin(Stdio::null());
    command
}

fn retry_after<'a>(header: &impl Fn(&str) -> Option<&'a str>, now: SystemTime) -> Option<Duration> {
    let seconds = |key| header(key)?.parse::<u64>().ok();
    if let Some(delay) = seconds("retry-after") {
        return Some(Duration::from_secs(delay));
    }
    if header("x-ratelimit-remaining") == Some("0") {
        let reset = UNIX_EPOCH.checked_add(Duration::from_secs(seconds("x-ratelimit-reset")?))?;
        return reset.duration_since(now).ok();
    }
    None
}

/// Checks the public stable release feed. A conditional response keeps the last known release.
pub fn check_latest_release(etag: Option<&str>) -> Result<Option<CheckedRelease>, CheckError> {
    let temp = tempfile::tempdir().map_err(|_| CheckError::default())?;
    let headers = temp.path().join("headers");
    let mut command = curl();
    command
        .args([
            "--connect-timeout",
            "4",
            "--max-time",
            "10",
            "--max-filesize",
            "2097152",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: icy-updater",
            "--dump-header",
        ])
        .arg(&headers);
    if let Some(etag) = etag {
        command.arg("-H").arg(format!("If-None-Match: {etag}"));
    }
    let output = command
        .arg(RELEASES_API_URL)
        .output()
        .map_err(|_| CheckError::default())?;
    if !output.status.success() {
        return Err(CheckError::default());
    }

    let headers = fs::read_to_string(headers).map_err(|_| CheckError::default())?;
    let response = headers
        .rsplit("\r\n\r\n")
        .find(|block| block.starts_with("HTTP/"))
        .ok_or_else(CheckError::default)?;
    let header = |key: &str| {
        response
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.trim())
    };
    match response.split_whitespace().nth(1) {
        Some("304") if etag.is_some() => Ok(None),
        Some("200") => {
            let value =
                serde_json::from_slice(&output.stdout).map_err(|_| CheckError::default())?;
            let release = parse_release(&value, CURRENT_VERSION, platform::release_asset_name())
                .ok_or_else(CheckError::default)?;
            let etag = header("etag")
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            Ok(Some(CheckedRelease { release, etag }))
        }
        Some("403" | "429" | "503") => Err(CheckError {
            retry_after: retry_after(&header, SystemTime::now()),
        }),
        _ => Err(CheckError::default()),
    }
}

fn stable_version(tag: &str) -> Option<Version> {
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()?;
    version.pre.is_empty().then_some(version)
}

fn release_asset(asset: &Value, tag: &str, asset_name: &str) -> Option<ReleaseAsset> {
    if asset["name"].as_str()? != asset_name || asset["state"].as_str()? != "uploaded" {
        return None;
    }
    let url = asset["browser_download_url"].as_str()?;
    let path = url.strip_prefix("https://github.com/Niiihuel/icy/releases/download/")?;
    let (release, filename) = path.split_once('/')?;
    if release.replace("%2B", "+") != tag || filename != asset_name {
        return None;
    }
    let size = asset["size"].as_u64().filter(|size| *size > 0)?;
    let sha256 = asset["digest"].as_str()?.strip_prefix("sha256:")?;
    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(ReleaseAsset {
        url: url.into(),
        size,
        sha256: sha256.to_ascii_lowercase(),
    })
}

fn parse_release(value: &Value, current: &str, asset_name: &str) -> Option<UpdateState> {
    if value["draft"].as_bool()? || value["prerelease"].as_bool()? {
        return None;
    }
    let tag = value["tag_name"].as_str()?;
    let candidate = stable_version(tag)?;
    let installed = Version::parse(current).ok()?;
    let has_update = candidate.cmp_precedence(&installed).is_gt();
    let version = candidate.to_string();
    let title = value["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(tag);
    let body = value["body"].as_str().unwrap_or("");
    let published_at = value["published_at"]
        .as_str()
        .unwrap_or("Recently")
        .chars()
        .take(10)
        .collect();
    let html_url = value["html_url"]
        .as_str()
        .filter(|url| url.starts_with("https://github.com/Niiihuel/icy/releases/tag/"))
        .unwrap_or(RELEASES_WEB_URL);
    let assets = value["assets"].as_array().map_or(&[][..], Vec::as_slice);
    let asset = if has_update {
        assets
            .iter()
            .find_map(|asset| release_asset(asset, tag, asset_name))
    } else {
        None
    };
    let (summary, highlights, media) = release_content(body, assets);

    Some(UpdateState {
        current_version: current.into(),
        available_version: version.clone(),
        has_update,
        check_status: CheckStatus::Succeeded,
        status_text: match (&asset, has_update) {
            (Some(_), _) => format!("Update v{version} available"),
            (None, true) => {
                "No verified in-app download for this platform. See the release on GitHub.".into()
            }
            (None, false) => format!("Up to date (v{current})"),
        },
        asset,
        notes: ReleaseNotes {
            version,
            title: title.into(),
            published_at,
            summary,
            highlights,
            media,
            release_url: html_url.into(),
        },
        ..UpdateState::initial()
    })
}

fn verify_download(path: &Path, asset: &ReleaseAsset) -> Result<(), String> {
    let mut file = fs::File::open(path).map_err(|e| format!("Cannot read the update: {e}"))?;
    let size = file
        .metadata()
        .map_err(|e| format!("Cannot read the update size: {e}"))?
        .len();
    if size != asset.size {
        return Err("Downloaded update has an unexpected size".into());
    }
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| format!("Cannot verify the update: {e}"))?;
    if format!("{:x}", hasher.finalize()) != asset.sha256 {
        return Err("Downloaded update failed its SHA-256 check".into());
    }
    Ok(())
}

/// Downloads and verifies the published executable before exposing it to the installer.
pub fn download_and_stage(
    asset: &ReleaseAsset,
    mut on_progress: impl FnMut(&str, u8, &str),
) -> Result<PathBuf, String> {
    let staged = tempfile::Builder::new()
        .prefix("icy-update-")
        .suffix(".staged")
        .tempfile()
        .map_err(|e| format!("Cannot create the staging file: {e}"))?
        .into_temp_path();
    on_progress("downloading", 35, "Downloading release binary...");
    let status = curl()
        .args([
            "-f",
            "--connect-timeout",
            "10",
            "--max-time",
            "120",
            "--max-filesize",
        ])
        .arg(asset.size.to_string())
        .arg("-o")
        .arg(&staged)
        .arg("--")
        .arg(&asset.url)
        .status()
        .map_err(|e| format!("Failed to run curl: {e}"))?;
    if !status.success() {
        return Err("Download failed from GitHub Releases".into());
    }
    on_progress("verifying", 82, "Verifying staged binary...");
    verify_download(&staged, asset)?;
    staged
        .keep()
        .map_err(|e| format!("Cannot stage the update: {e}"))
}

/// Applies the staged binary and triggers automatic OS-level restart.
pub fn apply_staged_and_restart(staged_path: &Path) -> Result<(), String> {
    let current_exe = env::current_exe().map_err(|e| format!("cannot locate current exe: {e}"))?;
    platform::spawn_self_update_installer(staged_path, &current_exe)
        .map_err(|e| format!("failed to launch updater helper: {e}"))
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
    fn stable_release_uses_the_exact_verified_executable() {
        let asset_name = "icy-windows-x86_64.exe";
        let url = format!("{RELEASES_WEB_URL}/download/v0.1.12/{asset_name}");
        let digest = "ab".repeat(32);
        let mut release = serde_json::json!({
            "tag_name": "v0.1.12", "draft": false, "prerelease": false,
            "assets": [
                {"name": "old-icy-windows-x86_64.exe", "size": 1, "state": "uploaded", "digest": format!("sha256:{digest}")},
                {"name": asset_name, "browser_download_url": url, "size": 123,
                    "state": "uploaded", "digest": format!("sha256:{digest}")},
            ]
        });
        let available = parse_release(&release, "0.1.2", asset_name).unwrap();
        assert!(available.has_update);
        assert_eq!(
            available.asset.unwrap(),
            ReleaseAsset {
                url,
                size: 123,
                sha256: digest
            }
        );
        assert!(
            !parse_release(&release, "0.1.12+local", asset_name)
                .unwrap()
                .has_update
        );
        assert!(
            !parse_release(&release, "1.0.0", asset_name)
                .unwrap()
                .has_update
        );
        release["assets"][1]["digest"] = Value::Null;
        let manual = parse_release(&release, "0.1.2", asset_name).unwrap();
        assert!(manual.has_update && manual.asset.is_none());
        for tag in ["v0.1.12-rc.1", "v0.x.12", "v01.2.3"] {
            release["tag_name"] = tag.into();
            assert!(parse_release(&release, "0.1.2", asset_name).is_none());
        }
        release["tag_name"] = "v0.1.12".into();
        release["draft"] = true.into();
        assert!(parse_release(&release, "0.1.2", asset_name).is_none());
        release["draft"] = false.into();
        release["prerelease"] = true.into();
        assert!(parse_release(&release, "0.1.2", asset_name).is_none());
    }

    #[test]
    fn automatic_checks_back_off_and_respect_rate_limits_and_the_preference() {
        let now = Instant::now();
        let mut schedule = CheckSchedule::new(false, now);
        assert_eq!(schedule.deadline(), None);
        assert!(schedule.can_check(now)); // Manual checks remain available.
        schedule.set_enabled(true, now);
        assert_eq!(schedule.deadline(), Some(now));
        schedule.complete(now, Some(&CheckError::default()));
        let next = now + FIRST_RETRY;
        assert_eq!(schedule.deadline(), Some(next));
        schedule.complete(next, Some(&CheckError::default()));
        assert_eq!(schedule.deadline(), Some(next + FIRST_RETRY * 2));
        let limited = CheckError {
            retry_after: Some(Duration::from_hours(2)),
        };
        schedule.complete(next, Some(&limited));
        let reset = next + limited.retry_after.unwrap();
        assert_eq!(schedule.deadline(), Some(reset));
        assert!(!schedule.can_check(next));
        schedule.set_enabled(false, next);
        assert_eq!(schedule.deadline(), None);
        schedule.set_enabled(true, next);
        assert_eq!(schedule.deadline(), Some(reset));
        schedule.complete(reset, None);
        assert_eq!(schedule.deadline(), Some(reset + CHECK_INTERVAL));
    }

    #[test]
    fn staged_download_rejects_truncation_and_same_size_tampering() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let asset = ReleaseAsset {
            url: String::new(),
            size: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
        };
        fs::write(file.path(), b"abc").unwrap();
        verify_download(file.path(), &asset).unwrap();
        fs::write(file.path(), b"ab").unwrap();
        assert!(
            verify_download(file.path(), &asset)
                .unwrap_err()
                .contains("size")
        );
        fs::write(file.path(), b"abd").unwrap();
        assert!(
            verify_download(file.path(), &asset)
                .unwrap_err()
                .contains("SHA-256")
        );
    }
}
