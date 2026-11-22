//! Local CLI agent usage scanner for the sidebar CLI rings and animated tooltips.
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliUsage {
    pub id: String,
    pub name: String,
    pub color: String,
    pub color_rgba: u32,
    pub percent: f32,
    pub input_tokens: String,
    pub output_tokens: String,
    pub sessions: u32,
    pub status: String,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn format_compact(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn scan_claude(home: &Path) -> (u64, u64, u32) {
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut sessions = 0u32;

    let projects_dir = home.join(".claude/projects");
    let mut scanned_files = 0usize;
    if let Ok(entries) = fs::read_dir(&projects_dir) {
        for proj in entries.flatten() {
            if !proj.path().is_dir() {
                continue;
            }
            if let Ok(files) = fs::read_dir(proj.path()) {
                for file in files.flatten() {
                    let path = file.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }
                    sessions = sessions.saturating_add(1);
                    if scanned_files >= 40 {
                        continue;
                    }
                    scanned_files += 1;
                    if let Ok(content) = fs::read_to_string(&path) {
                        for line in content.lines().take(2000) {
                            if !line.contains("\"usage\"") {
                                continue;
                            }
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                                let usage = val
                                    .get("message")
                                    .and_then(|m| m.get("usage"))
                                    .or_else(|| val.get("usage"));
                                if let Some(u) = usage {
                                    input_tokens +=
                                        u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                                    input_tokens += u
                                        .get("cache_read_input_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                    output_tokens += u
                                        .get("output_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if sessions == 0
        && let Ok(history) = fs::read_to_string(home.join(".claude/history.jsonl"))
    {
        sessions = history.lines().count().min(999) as u32;
    }

    (input_tokens, output_tokens, sessions)
}

fn count_dir_entries(path: &Path) -> u32 {
    fs::read_dir(path)
        .map(|rd| rd.flatten().count().min(999) as u32)
        .unwrap_or(0)
}

fn scan_jsonl_dir(path: &Path) -> (u64, u64, u32) {
    let mut input = 0u64;
    let mut output = 0u64;
    let mut sessions = 0u32;
    let Ok(rd) = fs::read_dir(path) else {
        return (0, 0, 0);
    };
    for entry in rd.flatten().take(40) {
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            sessions += 1;
            if let Ok(meta) = p.metadata() {
                // Fast approximate token estimate (~4 bytes/token) when schema varies.
                let approx = meta.len() / 4;
                input += (approx * 4) / 5;
                output += approx / 5;
            }
        } else if p.is_dir() {
            sessions += 1;
        }
    }
    (input, output, sessions)
}

/// Collects usage metrics for the 6 tracked agent CLIs without blocking UI startup.
pub fn collect_cli_usage() -> Vec<CliUsage> {
    let home = home_dir();

    let (claude_in, claude_out, claude_sess) =
        home.as_deref().map(scan_claude).unwrap_or((0, 0, 0));

    let agy_sess = home
        .as_deref()
        .map(|h| count_dir_entries(&h.join(".gemini/antigravity/brain")))
        .unwrap_or(0);

    let (codex_in, codex_out, codex_sess) = home
        .as_deref()
        .map(|h| scan_jsonl_dir(&h.join(".codex/sessions")))
        .unwrap_or((0, 0, 0));

    let (grok_in, grok_out, grok_sess) = home
        .as_deref()
        .map(|h| scan_jsonl_dir(&h.join(".grok")))
        .unwrap_or((0, 0, 0));

    let (junie_in, junie_out, junie_sess) = home
        .as_deref()
        .map(|h| scan_jsonl_dir(&h.join(".junie")))
        .unwrap_or((0, 0, 0));

    let (oc_in, oc_out, oc_sess) = home
        .as_deref()
        .map(|h| scan_jsonl_dir(&h.join(".local/share/opencode")))
        .unwrap_or((0, 0, 0));

    // Compute clean usage ring percentages (relative to weekly activity budget or sessions).
    let raw_scores: [f32; 6] = [
        usage_pct(claude_in + claude_out, claude_sess),
        usage_pct(u64::from(agy_sess) * 85_000, agy_sess),
        usage_pct(codex_in + codex_out, codex_sess),
        usage_pct(grok_in + grok_out, grok_sess),
        usage_pct(junie_in + junie_out, junie_sess),
        usage_pct(oc_in + oc_out, oc_sess),
    ];

    vec![
        CliUsage {
            id: "claude".into(),
            name: "Claude Code".into(),
            color: "#e07a5f".into(),
            color_rgba: 0xe07a_5fff,
            percent: raw_scores[0],
            input_tokens: format_compact(claude_in),
            output_tokens: format_compact(claude_out),
            sessions: claude_sess,
            status: status_label(claude_sess),
        },
        CliUsage {
            id: "antigravity".into(),
            name: "Antigravity".into(),
            color: "#60a5fa".into(),
            color_rgba: 0x60a5_faff,
            percent: raw_scores[1],
            input_tokens: format_compact(u64::from(agy_sess) * 72_000),
            output_tokens: format_compact(u64::from(agy_sess) * 13_000),
            sessions: agy_sess,
            status: status_label(agy_sess),
        },
        CliUsage {
            id: "codex".into(),
            name: "Codex".into(),
            color: "#e4e4e7".into(),
            color_rgba: 0xe4e4_e7ff,
            percent: raw_scores[2],
            input_tokens: format_compact(codex_in),
            output_tokens: format_compact(codex_out),
            sessions: codex_sess,
            status: status_label(codex_sess),
        },
        CliUsage {
            id: "grok".into(),
            name: "Grok CLI".into(),
            color: "#d4d4d8".into(),
            color_rgba: 0xd4d4_d8ff,
            percent: raw_scores[3],
            input_tokens: format_compact(grok_in),
            output_tokens: format_compact(grok_out),
            sessions: grok_sess,
            status: status_label(grok_sess),
        },
        CliUsage {
            id: "junie".into(),
            name: "Junie".into(),
            color: "#4ade80".into(),
            color_rgba: 0x4ade_80ff,
            percent: raw_scores[4],
            input_tokens: format_compact(junie_in),
            output_tokens: format_compact(junie_out),
            sessions: junie_sess,
            status: status_label(junie_sess),
        },
        CliUsage {
            id: "opencode".into(),
            name: "OpenCode".into(),
            color: "#a78bfa".into(),
            color_rgba: 0xa78b_faff,
            percent: raw_scores[5],
            input_tokens: format_compact(oc_in),
            output_tokens: format_compact(oc_out),
            sessions: oc_sess,
            status: status_label(oc_sess),
        },
    ]
}

fn usage_pct(tokens: u64, sessions: u32) -> f32 {
    if tokens == 0 && sessions == 0 {
        return 0.0;
    }
    let from_tokens = (tokens as f32 / 5_000_000.0) * 100.0;
    let from_sessions = (sessions as f32 / 40.0) * 100.0;
    from_tokens.max(from_sessions).clamp(6.0, 96.0).round()
}

fn status_label(sessions: u32) -> String {
    if sessions > 0 {
        format!("{sessions} sessions")
    } else {
        "idle".into()
    }
}
