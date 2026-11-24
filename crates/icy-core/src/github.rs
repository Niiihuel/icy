//! GitHub + SSH detection for workspace identity, repository, branch, and key status.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::platform;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubStatus {
    pub connected: bool,
    pub method: String,
    pub username: Option<String>,
    pub ssh_keys: Vec<String>,
    pub ssh_github_ready: bool,
    pub repo_slug: Option<String>,
    pub branch: Option<String>,
    pub dirty_count: usize,
    pub remote_url: Option<String>,
    pub detail: String,
}

/// Inspects local SSH keys, tests `ssh -T git@github.com` non-interactively (falling back
/// to `gh api user` when `gh` is authenticated), and reads git metadata from `cwd`.
pub fn detect_github(cwd: &Path) -> GitHubStatus {
    let ssh_keys = list_ssh_keys();
    let ssh_user = probe_ssh_github();

    let (connected, method, username, ssh_github_ready, detail) = if let Some(user) = ssh_user {
        (
            true,
            "ssh".to_string(),
            Some(user.clone()),
            true,
            format!("Authenticated via SSH as @{user}"),
        )
    } else if let Some(user) = probe_gh_cli_user() {
        (
            true,
            "gh-cli".to_string(),
            Some(user.clone()),
            false,
            format!("Signed in via gh CLI as @{user} (SSH key not bound to github.com)"),
        )
    } else {
        (
            false,
            "none".to_string(),
            None,
            false,
            if ssh_keys.is_empty() {
                "No SSH keys found in ~/.ssh".to_string()
            } else {
                "SSH keys found, but git@github.com did not authenticate".to_string()
            },
        )
    };

    let remote_url = run_git(cwd, &["remote", "get-url", "origin"]);
    let repo_slug = remote_url.as_deref().and_then(parse_github_slug);
    let branch = run_git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let dirty_count = run_git(cwd, &["status", "--short"])
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);

    GitHubStatus {
        connected,
        method,
        username,
        ssh_keys,
        ssh_github_ready,
        repo_slug,
        branch,
        dirty_count,
        remote_url,
        detail,
    }
}

fn list_ssh_keys() -> Vec<String> {
    let Some(home) = platform::home_dir() else {
        return Vec::new();
    };
    let ssh_dir = home.join(".ssh");
    let Ok(entries) = fs::read_dir(ssh_dir) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.ends_with(".pub") {
            keys.push(name.trim_end_matches(".pub").to_string());
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

fn probe_ssh_github() -> Option<String> {
    let output = Command::new("ssh")
        .args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=3",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "git@github.com",
        ])
        .stdin(Stdio::null())
        .output()
        .ok()?;

    // GitHub returns exit code 1 with `Hi <username>! You've successfully authenticated...` on stderr.
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_ssh_greeting(&combined)
}

fn probe_gh_cli_user() -> Option<String> {
    let output = Command::new("gh")
        .args(["api", "user", "--jq", ".login"])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let login = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!login.is_empty()).then_some(login)
}

/// GitHub logins are 1-39 ASCII alphanumerics or hyphens; anything else never reaches a URL.
pub fn is_valid_login(login: &str) -> bool {
    (1..=39).contains(&login.len()) && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Downloads the public avatar of `login` (`https://github.com/<login>.png`) to `dest`.
pub fn fetch_avatar(login: &str, dest: &Path) -> bool {
    if !is_valid_login(login) {
        return false;
    }
    Command::new("curl")
        .args(["-fsSL", "--connect-timeout", "4", "--max-time", "10", "-o"])
        .arg(dest)
        .arg(format!("https://github.com/{login}.png?size=96"))
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
        && fs::metadata(dest).is_ok_and(|m| m.len() > 0)
}

fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

pub fn parse_ssh_greeting(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("Hi ")
            && let Some((user, tail)) = rest.split_once('!')
            && tail.contains("successfully authenticated")
            && !user.is_empty()
        {
            return Some(user.to_string());
        }
    }
    None
}

pub fn parse_github_slug(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches(".git");
    if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        return Some(rest.to_string());
    }
    if let Some(rest) = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("ssh://git@github.com/"))
    {
        return Some(rest.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_github_logins() {
        assert!(is_valid_login("Niiihuel"));
        assert!(is_valid_login("a-b-1"));
        assert!(!is_valid_login(""));
        assert!(!is_valid_login("x/../../etc"));
        assert!(!is_valid_login("evil?size=1&"));
        assert!(!is_valid_login(&"a".repeat(40)));
    }

    #[test]
    fn parses_ssh_greeting_and_github_remotes() {
        let msg = "Hi Niiihuel! You've successfully authenticated, but GitHub does not provide shell access.";
        assert_eq!(parse_ssh_greeting(msg).as_deref(), Some("Niiihuel"));
        assert_eq!(
            parse_github_slug("git@github.com:Niiihuel/icy.git").as_deref(),
            Some("Niiihuel/icy")
        );
        assert_eq!(
            parse_github_slug("https://github.com/Niiihuel/icy.git").as_deref(),
            Some("Niiihuel/icy")
        );
    }
}
