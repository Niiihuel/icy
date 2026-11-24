//! Live limits the CLIs do not write to disk, read the way their own clients do (same sources as
//! CodexBar, Orca and t3code). Every source is optional: a missing login, an expired token or a
//! changed response just leaves that CLI on its local data. Tokens are never refreshed (Claude Code
//! rotates refresh tokens, so redeeming one would log the CLI out) and never reach argv: curl
//! reads the headers from stdin.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use chrono::DateTime;
use serde_json::Value;

use super::cli::{UsageRow, UsageSection, claude_window_label};

/// Limits per CLI id; replaces the local limit sections of that CLI.
pub type RemoteLimits = Vec<(String, Vec<UsageSection>)>;

pub fn fetch(home: Option<&Path>, now: i64) -> RemoteLimits {
    let mut out = Vec::new();
    if let Some(sections) = claude(home, now) {
        out.push(("claude".into(), sections));
    }
    if let Some(sections) = antigravity() {
        out.push(("antigravity".into(), sections));
    }
    if let Some(sections) = grok(home, now) {
        out.push(("grok".into(), sections));
    }
    out
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// GET with headers passed on stdin (`-H @-`) so tokens stay out of the process list.
fn curl_json(url: &str, headers: &[String]) -> Option<Value> {
    let mut child = Command::new("curl")
        .args(["-sS", "--fail", "--max-time", "15", "-H", "@-", url])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child
        .stdin
        .take()?
        .write_all(headers.join("\n").as_bytes())
        .ok()?;
    let output = child.wait_with_output().ok()?;
    output.status.success().then_some(())?;
    serde_json::from_slice(&output.stdout).ok()
}

fn iso_seconds(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n
            .as_i64()
            .map(|t| if t > 10_000_000_000 { t / 1000 } else { t }),
        Value::String(s) => DateTime::parse_from_rfc3339(s).ok().map(|t| t.timestamp()),
        _ => None,
    }
    // Epoch 0 means "no window running" (full quota).
    .filter(|&t| t > 0)
}

fn limit_row(label: String, used: f64, resets_at: Option<i64>, now: i64) -> UsageRow {
    let used = used.clamp(0.0, 100.0) as f32;
    UsageRow {
        label,
        value: format!("{}%", used.round()),
        percent: Some(used),
        resets_at: resets_at.filter(|&at| at > now),
    }
}

/// Claude Code subscription limits: `GET api.anthropic.com/api/oauth/usage` with the CLI's OAuth
/// token. Prefers the generic `limits[]` list (it carries per-model scopes) over the legacy keys.
fn claude(home: Option<&Path>, now: i64) -> Option<Vec<UsageSection>> {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".claude")))?;
    let oauth = read_json(&dir.join(".credentials.json"))?["claudeAiOauth"].take();
    let token = oauth["accessToken"].as_str()?;
    if oauth["expiresAt"]
        .as_i64()
        .is_some_and(|ms| ms / 1000 <= now)
    {
        return None; // the CLI refreshes it on its next run
    }
    let usage = curl_json(
        "https://api.anthropic.com/api/oauth/usage",
        &[
            format!("Authorization: Bearer {token}"),
            "anthropic-beta: oauth-2025-04-20".into(),
            "Accept: application/json".into(),
            "User-Agent: claude-code/2.1.0".into(),
        ],
    )?;
    Some(vec![UsageSection {
        title: "Limits".into(),
        rows: claude_rows(&usage, now),
    }])
}

pub(super) fn claude_rows(usage: &Value, now: i64) -> Vec<UsageRow> {
    let mut rows: Vec<UsageRow> = usage["limits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|limit| {
            let used = limit["percent"].as_f64()?;
            let model = limit["scope"]["model"]["display_name"].as_str();
            let label = match (limit["kind"].as_str().unwrap_or_default(), model) {
                ("session", _) => "5-hour".to_owned(),
                ("weekly_all", _) => "Weekly".to_owned(),
                (_, Some(model)) => format!("Weekly · {model}"),
                (kind, None) => claude_window_label(kind),
            };
            Some(limit_row(
                label,
                used,
                iso_seconds(&limit["resets_at"]),
                now,
            ))
        })
        .collect();
    if rows.is_empty() {
        for key in [
            "five_hour",
            "seven_day",
            "seven_day_opus",
            "seven_day_sonnet",
        ] {
            let window = &usage[key];
            if let Some(used) = window["utilization"].as_f64() {
                rows.push(limit_row(
                    claude_window_label(key),
                    used,
                    iso_seconds(&window["resets_at"]),
                    now,
                ));
            }
        }
    }
    let extra = &usage["extra_usage"];
    if extra["is_enabled"].as_bool() == Some(true) {
        let used = extra["used_credits"].as_f64().unwrap_or_default();
        let currency = extra["currency"].as_str().unwrap_or("USD");
        rows.push(match extra["monthly_limit"].as_f64() {
            Some(limit) if limit > 0.0 => UsageRow {
                value: format!("{used:.2} / {limit:.2} {currency}"),
                ..limit_row("Extra usage".into(), used / limit * 100.0, None, now)
            },
            _ => UsageRow {
                label: "Extra usage".into(),
                value: format!("{used:.2} {currency}"),
                ..UsageRow::default()
            },
        });
    }
    rows
}

/// Antigravity keeps its Google login in the OS keyring, so only `agy` can ask: `/usage` in print
/// mode answers locally (agy ≥ 1.1.11) without spending a model turn. One section per model
/// group, each with its 5-hour and weekly buckets.
fn antigravity() -> Option<Vec<UsageSection>> {
    let output = Command::new("agy")
        .args([
            "-p",
            "/usage",
            "--output-format",
            "json",
            "--print-timeout",
            "20s",
        ])
        .env("AGY_CLI_DISABLE_AUTO_UPDATE", "true")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let response: Value = serde_json::from_slice(&output.stdout).ok()?;
    let sections = antigravity_sections(&response["command"]["data"], now_secs());
    Some(sections).filter(|s| !s.is_empty())
}

pub(super) fn antigravity_sections(data: &Value, now: i64) -> Vec<UsageSection> {
    data["groups"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|group| UsageSection {
            title: group["name"].as_str().unwrap_or("Limits").to_owned(),
            rows: group["buckets"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|bucket| bucket["disabled"].as_bool() != Some(true))
                .filter_map(|bucket| {
                    let remaining = bucket["remaining_fraction"].as_f64()?;
                    let label = match bucket["window"].as_str() {
                        Some("5h") => "5-hour".to_owned(),
                        Some("weekly") => "Weekly".to_owned(),
                        _ => bucket["name"].as_str().unwrap_or("Limit").to_owned(),
                    };
                    Some(limit_row(
                        label,
                        (1.0 - remaining) * 100.0,
                        iso_seconds(&bucket["reset_time"]),
                        now,
                    ))
                })
                .collect(),
        })
        .filter(|section| !section.rows.is_empty())
        .collect()
}

/// Grok CLI credits: `GET cli-chat-proxy.grok.com/v1/billing?format=credits` with the CLI key.
/// shortcut: shape taken from CodexBar/Orca, not verified against a live response; on any
/// mismatch it yields nothing and Grok stays on local data.
fn grok(home: Option<&Path>, now: i64) -> Option<Vec<UsageSection>> {
    let path = std::env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".grok")))?
        .join("auth.json");
    let auth = read_json(&path)?;
    let entries: Vec<&Value> = auth.as_object()?.values().collect();
    let entry = entries
        .iter()
        .find(|e| e["auth_mode"].as_str().is_some_and(|m| m.contains("oidc")))
        .or_else(|| entries.first())?;
    if iso_seconds(&entry["expires_at"]).is_some_and(|at| at <= now) {
        return None;
    }
    let billing = curl_json(
        "https://cli-chat-proxy.grok.com/v1/billing?format=credits",
        &[
            format!("Authorization: Bearer {}", entry["key"].as_str()?),
            "x-xai-token-auth: xai-grok-cli".into(),
            "Accept: application/json".into(),
        ],
    )?;
    let config = &billing["config"];
    let used = config["creditUsagePercent"].as_f64().or_else(|| {
        let used = billing["onDemandUsed"]["val"].as_f64()?;
        let cap = billing["onDemandCap"]["val"]
            .as_f64()
            .filter(|c| *c > 0.0)?;
        Some(used / cap * 100.0)
    })?;
    let resets = iso_seconds(&config["currentPeriod"]["end"])
        .or_else(|| iso_seconds(&billing["billingPeriodEnd"]));
    Some(vec![UsageSection {
        title: "Limits".into(),
        rows: vec![limit_row("Credits".into(), used, resets, now)],
    }])
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn claude_limits_prefer_the_generic_list_with_model_scopes() {
        let usage = json!({
            "five_hour": {"utilization": 99.0, "resets_at": "2026-11-24T05:10:00+00:00"},
            "limits": [
                {"kind": "session", "percent": 28, "resets_at": "2026-11-24T05:10:00+00:00"},
                {"kind": "weekly_all", "percent": 45, "resets_at": "2026-11-29T06:00:00+00:00"},
                {"kind": "weekly_model", "percent": 60, "resets_at": null, "scope": {"model": {"display_name": "Opus"}}}
            ],
            "extra_usage": {"is_enabled": false}
        });
        let rows = claude_rows(&usage, 0);
        let labels: Vec<_> = rows.iter().map(|r| (r.label.as_str(), r.percent)).collect();
        assert_eq!(
            labels,
            [
                ("5-hour", Some(28.0)),
                ("Weekly", Some(45.0)),
                ("Weekly · Opus", Some(60.0))
            ]
        );
        assert_eq!(rows[0].resets_at, Some(1_795_497_000));
    }

    #[test]
    fn claude_legacy_keys_are_the_fallback() {
        let usage =
            json!({"five_hour": {"utilization": 12.0, "resets_at": null}, "seven_day": null});
        let rows = claude_rows(&usage, 0);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].label.as_str(), rows[0].percent),
            ("5-hour", Some(12.0))
        );
    }

    #[test]
    fn antigravity_groups_become_sections_of_used_share() {
        let data = json!({"groups": [
            {"name": "Gemini Models", "buckets": [
                {"window": "weekly", "remaining_fraction": 0.87, "reset_time": "2026-11-30T20:04:58Z"},
                {"window": "5h", "remaining_fraction": 0.49, "reset_time": "2026-11-24T01:04:58Z"}
            ]},
            {"name": "Claude and GPT models", "buckets": [
                {"window": "weekly", "remaining_fraction": 1.0, "reset_time": "1970-01-01T00:00:00Z"}
            ]}
        ]});
        let sections = antigravity_sections(&data, 0);
        assert_eq!(sections[0].title, "Gemini Models");
        assert_eq!(sections[0].rows[0].label, "Weekly");
        assert_eq!(sections[0].rows[1].value, "51%");
        // A bucket that never started has no reset time and nothing used.
        assert_eq!(
            (sections[1].rows[0].percent, sections[1].rows[0].resets_at),
            (Some(0.0), None)
        );
    }
}
