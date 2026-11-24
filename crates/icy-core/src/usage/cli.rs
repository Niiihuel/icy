//! Sidebar usage meters: every limit a CLI reports (5-hour, weekly, per model, credits) plus its
//! recent per-model token use, grouped into the sections the tooltip separates. A meter only has a
//! `percent` (the ring) when the CLI reports a real limit; nothing is estimated.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::logs::{entries, jsonl, jsonl_files};
use super::{Tokens, UsageSnapshot};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliUsage {
    pub id: String,
    /// Highest used share of any real limit, 0..=100; `None` when the CLI reports no limits.
    pub percent: Option<f32>,
    pub sections: Vec<UsageSection>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct UsageSection {
    pub title: String,
    pub rows: Vec<UsageRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRow {
    pub label: String,
    pub value: String,
    /// Used share of a limit (0..=100), drawn as a bar.
    pub percent: Option<f32>,
    /// Unix seconds when the limit window resets.
    pub resets_at: Option<i64>,
}

impl CliUsage {
    fn new(id: &str, sections: Vec<UsageSection>) -> Self {
        let percent = sections
            .iter()
            .flat_map(|section| &section.rows)
            .filter_map(|row| row.percent)
            .reduce(f32::max);
        Self {
            id: id.into(),
            percent,
            sections: sections
                .into_iter()
                .filter(|s| !s.rows.is_empty())
                .collect(),
        }
    }
}

const RECENT_DAYS: i64 = 7;
const MAX_MODELS: usize = 5;
/// Newest transcripts searched for a limit record before giving up.
const LIMIT_SCAN_FILES: usize = 24;

/// Meters for the tracked CLIs, from local transcripts (`snapshot`) and limit records; live
/// `remote` limits replace a CLI's local ones.
pub fn collect(
    home: Option<&Path>,
    snapshot: &UsageSnapshot,
    now: i64,
    remote: &super::RemoteLimits,
) -> Vec<CliUsage> {
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join(".codex")));
    let limits = |id: &str| -> Vec<UsageSection> {
        match id {
            "codex" => codex_home
                .as_deref()
                .map(|root| codex_limits(root, now))
                .unwrap_or_default(),
            "claude" => home
                .map(|home| claude_limits(&home.join(".claude/projects"), now))
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    };
    snapshot
        .agents
        .iter()
        .map(|agent| {
            let mut sections = match remote.iter().find(|(id, _)| id == agent.id) {
                Some((_, live)) => live.clone(),
                None => limits(agent.id),
            };
            sections.push(recent_models(snapshot, agent.id));
            CliUsage::new(agent.id, sections)
        })
        .collect()
}

/// Per-model tokens and sessions of the last `RECENT_DAYS`, busiest model first.
fn recent_models(snapshot: &UsageSnapshot, id: &str) -> UsageSection {
    let mut section = UsageSection {
        title: format!("Last {RECENT_DAYS} days"),
        rows: Vec::new(),
    };
    let Some(agent) = snapshot.agents.iter().find(|agent| agent.id == id) else {
        return section;
    };
    let mut models: Vec<(String, Tokens)> = Vec::new();
    for day in agent
        .days
        .iter()
        .filter(|d| d.day > snapshot.today - RECENT_DAYS)
    {
        for model in &day.models {
            match models.iter_mut().find(|(name, _)| *name == model.name) {
                Some((_, tokens)) => tokens.add(model.tokens),
                None => models.push((model.name.clone(), model.tokens)),
            }
        }
    }
    models.sort_by_key(|(_, t)| std::cmp::Reverse(t.input.saturating_add(t.output)));
    let hidden = models.len().saturating_sub(MAX_MODELS);
    section.rows = models
        .into_iter()
        .take(MAX_MODELS)
        .map(|(name, t)| UsageRow {
            label: name,
            value: format!("{} in · {} out", compact(t.input), compact(t.output)),
            ..UsageRow::default()
        })
        .collect();
    if hidden > 0 {
        section.rows.push(UsageRow {
            label: format!("+{hidden} more models"),
            ..UsageRow::default()
        });
    }
    let sessions = agent.periods[0].current;
    if sessions > 0 {
        section.rows.push(UsageRow {
            label: "Sessions".into(),
            value: sessions.to_string(),
            ..UsageRow::default()
        });
    }
    section
}

/// Codex writes `rate_limits` into every `token_count` event: one bucket per `limit_id`, each
/// with a primary (5-hour) and secondary (weekly) window, plus credits.
fn codex_limits(root: &Path, now: i64) -> Vec<UsageSection> {
    for path in newest_files(&root.join("sessions"), 3) {
        let mut buckets: Vec<(String, Value)> = Vec::new();
        jsonl(&path, &["\"rate_limits\""], |record| {
            let limits = &record["payload"]["rate_limits"];
            if !limits.is_object() {
                return;
            }
            let id = limits["limit_id"].as_str().unwrap_or("codex").to_owned();
            match buckets.iter_mut().find(|(key, _)| *key == id) {
                Some((_, last)) => *last = limits.clone(),
                None => buckets.push((id, limits.clone())),
            }
        });
        if buckets.is_empty() {
            continue;
        }
        let single = buckets.len() == 1;
        return buckets
            .into_iter()
            .map(|(id, limits)| {
                let mut rows: Vec<UsageRow> = ["primary", "secondary"]
                    .into_iter()
                    .filter_map(|key| codex_window(&limits[key], now))
                    .collect();
                let credits = &limits["credits"];
                if credits["unlimited"].as_bool() == Some(true) {
                    rows.push(text_row("Credits", "Unlimited"));
                } else if credits["has_credits"].as_bool() == Some(true) {
                    let balance = credits["balance"].as_str().unwrap_or("0");
                    rows.push(text_row("Credits", balance));
                }
                let name = limits["limit_name"].as_str().filter(|n| !n.is_empty());
                UsageSection {
                    title: match name {
                        Some(name) => format!("Limits · {name}"),
                        None if single => "Limits".into(),
                        None => format!("Limits · {id}"),
                    },
                    rows,
                }
            })
            .collect();
    }
    Vec::new()
}

fn codex_window(window: &Value, now: i64) -> Option<UsageRow> {
    let used = window["used_percent"].as_f64()? as f32;
    let minutes = window["window_minutes"].as_i64().unwrap_or_default();
    let resets_at = window["resets_at"].as_i64();
    // A window whose reset already passed starts over at zero.
    let (used, resets_at) = match resets_at {
        Some(at) if at <= now => (0.0, None),
        at => (used.clamp(0.0, 100.0), at),
    };
    Some(UsageRow {
        label: window_label(minutes),
        value: format!("{}%", used.round()),
        percent: Some(used),
        resets_at,
    })
}

/// Claude Code only records a limit when a request is rejected by it (`usage_limit_reached`):
/// while that window has not reset, the limit is full.
fn claude_limits(projects: &Path, now: i64) -> Vec<UsageSection> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries(projects, true)
        .flat_map(|project| jsonl_files(&project.path()).collect::<Vec<_>>())
        .filter_map(|path| Some((path.metadata().ok()?.modified().ok()?, path)))
        .collect();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    let mut latest: Option<(i64, String)> = None;
    for (_, path) in files.into_iter().take(LIMIT_SCAN_FILES) {
        jsonl(&path, &["\"rate_limit_info\""], |record| {
            let info = &record["apiErrorParams"]["rate_limit_info"];
            if let (Some(at), Some(kind)) =
                (info["resetsAt"].as_i64(), info["rateLimitType"].as_str())
                && latest.as_ref().is_none_or(|(seen, _)| at > *seen)
            {
                latest = Some((at, kind.to_owned()));
            }
        });
    }
    match latest {
        Some((at, kind)) if at > now => vec![UsageSection {
            title: "Limits".into(),
            rows: vec![UsageRow {
                label: claude_window_label(&kind),
                value: "Limit reached".into(),
                percent: Some(100.0),
                resets_at: Some(at),
            }],
        }],
        _ => Vec::new(),
    }
}

/// Newest `YYYY/MM/DD/*.jsonl` transcripts first (Codex session layout), at most `days` folders.
fn newest_files(root: &Path, days: usize) -> Vec<PathBuf> {
    let sorted = |path: &Path, dirs: bool| {
        let mut list: Vec<PathBuf> = if dirs {
            entries(path, true).map(|e| e.path()).collect()
        } else {
            jsonl_files(path).collect()
        };
        list.sort_by(|a, b| b.cmp(a));
        list
    };
    sorted(root, true)
        .iter()
        .flat_map(|year| sorted(year, true))
        .flat_map(|month| sorted(&month, true))
        .take(days)
        .flat_map(|date| sorted(&date, false))
        .take(LIMIT_SCAN_FILES)
        .collect()
}

pub(super) fn window_label(minutes: i64) -> String {
    match minutes {
        m if m == 7 * 24 * 60 => "Weekly".into(),
        m if m == 24 * 60 => "Daily".into(),
        m if m > 0 && m % 60 == 0 => format!("{}-hour", m / 60),
        m if m > 0 => format!("{m}-minute"),
        _ => "Limit".into(),
    }
}

pub(super) fn claude_window_label(kind: &str) -> String {
    match kind {
        "five_hour" => "5-hour".into(),
        "seven_day" => "Weekly".into(),
        "seven_day_opus" => "Weekly · Opus".into(),
        "seven_day_sonnet" => "Weekly · Sonnet".into(),
        other => other.replace('_', " "),
    }
}

fn text_row(label: &str, value: &str) -> UsageRow {
    UsageRow {
        label: label.into(),
        value: value.into(),
        ..UsageRow::default()
    }
}

pub(super) fn compact(n: u64) -> String {
    match n {
        n if n >= 1_000_000_000 => format!("{:.1}B", n as f64 / 1e9),
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.1}K", n as f64 / 1e3),
        n => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("icy-cli-usage-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn codex_limits_read_the_last_record_and_reset_expired_windows() {
        let root = temp("codex");
        let day = root.join("sessions/2026/11/23");
        fs::create_dir_all(&day).unwrap();
        let event = |used: f64, weekly_reset: i64| {
            format!(
                r#"{{"type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"limit_id":"codex","primary":{{"used_percent":{used},"window_minutes":300,"resets_at":2000}},"secondary":{{"used_percent":40.0,"window_minutes":10080,"resets_at":{weekly_reset}}},"credits":{{"has_credits":false,"unlimited":false,"balance":"0"}}}}}}}}"#
            )
        };
        fs::write(
            day.join("rollout-a.jsonl"),
            format!("{}\n{}\n", event(10.0, 500), event(73.0, 500)),
        )
        .unwrap();

        let sections = codex_limits(&root, 1000);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "Limits");
        let rows = &sections[0].rows;
        assert_eq!(
            (rows[0].label.as_str(), rows[0].percent),
            ("5-hour", Some(73.0))
        );
        assert_eq!(rows[0].resets_at, Some(2000));
        // The weekly window reset at 500 < now, so it starts over.
        assert_eq!(
            (rows[1].label.as_str(), rows[1].percent, rows[1].resets_at),
            ("Weekly", Some(0.0), None)
        );
        assert_eq!(CliUsage::new("codex", sections).percent, Some(73.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn claude_limit_is_full_only_until_it_resets() {
        let root = temp("claude");
        let project = root.join("p");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("s.jsonl"),
            r#"{"apiError":"usage_limit_reached","apiErrorParams":{"rate_limit_info":{"status":"rejected","resetsAt":5000,"rateLimitType":"five_hour"}}}"#,
        )
        .unwrap();
        let open = claude_limits(&root, 4000);
        assert_eq!(open[0].rows[0].label, "5-hour");
        assert_eq!(open[0].rows[0].percent, Some(100.0));
        assert!(claude_limits(&root, 6000).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn meters_without_limits_have_no_ring() {
        let usage = CliUsage::new(
            "grok",
            vec![UsageSection {
                title: "Last 7 days".into(),
                rows: vec![text_row("grok-4", "1K in · 2K out")],
            }],
        );
        assert_eq!(usage.percent, None);
        assert_eq!(window_label(300), "5-hour");
        assert_eq!(window_label(10080), "Weekly");
    }
}
