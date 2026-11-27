//! Readers for the local transcripts. Only timestamped usage records reach the UI.

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, DirEntry, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{AgentHistory, Day, SessionDays, Tokens, UsageSnapshot, day};

type Reader = fn(&Path, &mut AgentHistory);

pub(super) fn collect(home: Option<&Path>, today: Day) -> UsageSnapshot {
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join(".codex")));

    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join(".local/share")));

    let pi_path = |variable| {
        let value = std::env::var_os(variable).filter(|value| !value.is_empty())?;
        let path = PathBuf::from(value);
        match path.strip_prefix("~") {
            Ok(rest) => Some(home?.join(rest)),
            Err(_) => Some(path),
        }
    };
    let pi_sessions = pi_path("PI_CODING_AGENT_SESSION_DIR").or_else(|| {
        pi_path("PI_CODING_AGENT_DIR")
            .or_else(|| home.map(|home| home.join(".pi/agent")))
            .map(|agent| agent.join("sessions"))
    });

    let readers: [(&str, Option<PathBuf>, Reader); 6] = [
        ("claude", home.map(|p| p.join(".claude/projects")), claude),
        (
            "antigravity",
            home.map(|p| p.join(".gemini/antigravity/brain")),
            antigravity,
        ),
        ("codex", codex_home, codex),
        ("grok", home.map(|p| p.join(".grok/sessions")), grok),
        (
            "opencode",
            data_home.map(|p| p.join("opencode/opencode.db")),
            opencode,
        ),
        ("pi", pi_sessions, pi),
    ];

    UsageSnapshot {
        today,
        agents: readers
            .into_iter()
            .map(|(id, path, read)| {
                let mut history = AgentHistory::new(today);
                if let Some(path) = path {
                    read(&path, &mut history);
                }
                history.finish(id)
            })
            .collect(),
    }
}

pub(super) fn entries(path: &Path, dirs: bool) -> impl Iterator<Item = DirEntry> + use<> {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter(move |entry| {
            entry
                .file_type()
                .is_ok_and(|kind| if dirs { kind.is_dir() } else { kind.is_file() })
        })
}

pub(super) fn jsonl_files(path: &Path) -> impl Iterator<Item = PathBuf> + use<> {
    entries(path, false)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
}

pub(super) fn jsonl(path: &Path, relevant: &[&str], mut consume: impl FnMut(&Value)) {
    let Ok(file) = File::open(path) else { return };
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    while reader.read_until(b'\n', &mut buffer).is_ok_and(|n| n > 0) {
        if let Ok(line) = std::str::from_utf8(&buffer)
            && relevant.iter().any(|needle| line.contains(needle))
            && let Ok(record) = serde_json::from_str::<Value>(line)
        {
            consume(&record);
        }
        buffer.clear();
    }
}

fn timestamp(value: &Value, today: Day) -> Option<DateTime<Utc>> {
    let time = if let Some(seconds) = value.as_i64() {
        DateTime::from_timestamp_secs(seconds)?
    } else {
        DateTime::parse_from_rfc3339(value.as_str()?).ok()?.to_utc()
    };
    (0..=today).contains(&day(time)).then_some(time)
}

fn fields(value: &Value, input: &str, output: &str, cached: &str) -> Option<Tokens> {
    if value.get(input).and_then(Value::as_u64).is_none()
        && value.get(output).and_then(Value::as_u64).is_none()
    {
        return None;
    }
    let input = value[input].as_u64().unwrap_or_default();
    Some(Tokens {
        input,
        output: value[output].as_u64().unwrap_or_default(),
        cached: value[cached].as_u64().unwrap_or_default().min(input),
        calls: 1,
    })
}

fn pi(root: &Path, history: &mut AgentHistory) {
    let paths =
        jsonl_files(root).chain(entries(root, true).flat_map(|dir| jsonl_files(&dir.path())));
    let mut seen = HashSet::new();
    for path in paths {
        let mut started = None;
        let mut dates = SessionDays::new();
        jsonl(&path, &["\"session\"", "\"usage\""], |record| {
            let Some(at) = timestamp(&record["timestamp"], history.today) else {
                return;
            };
            let message = match record["type"].as_str() {
                Some("session") => {
                    started = Some(at);
                    return;
                }
                Some("message") if record["message"]["role"] == "assistant" => &record["message"],
                Some("usage" | "compaction" | "branch_summary") => record,
                _ => return,
            };
            let Some(started) = started else { return };
            let usage = &message["usage"];
            let cached = usage["cacheRead"].as_u64().unwrap_or_default();
            let input = usage["input"]
                .as_u64()
                .unwrap_or_default()
                .saturating_add(cached)
                .saturating_add(usage["cacheWrite"].as_u64().unwrap_or_default());
            let output = usage["output"].as_u64().unwrap_or_default();
            if input == 0 && output == 0 {
                return;
            }

            // A fork can copy its parent's entries, preserving IDs and timestamps.
            if at >= started {
                dates.insert(day(at));
            }
            if let Some(id) = record["id"].as_str()
                && !seen.insert((at, id.to_owned()))
            {
                return;
            }
            history.record_tokens(
                at,
                message["model"].as_str(),
                Tokens {
                    input,
                    output,
                    cached,
                    calls: 1,
                },
            );
        });
        history.record_session(dates);
    }
}

fn claude(root: &Path, history: &mut AgentHistory) {
    for project in entries(root, true) {
        for path in jsonl_files(&project.path()) {
            let mut dates = SessionDays::new();
            let mut responses = BTreeMap::new();
            let mut durations = HashSet::new();
            jsonl(&path, &["\"usage\"", "\"turn_duration\""], |record| {
                let Some(at) = timestamp(&record["timestamp"], history.today) else {
                    return;
                };
                match record["type"].as_str() {
                    Some("assistant") => {
                        let message = &record["message"];
                        let usage = message.get("usage").unwrap_or(&record["usage"]);
                        let mut tokens = usage["input_tokens"].as_u64().unwrap_or_default();
                        let cached = usage["cache_read_input_tokens"]
                            .as_u64()
                            .unwrap_or_default();
                        let written = usage["cache_creation_input_tokens"]
                            .as_u64()
                            .unwrap_or_default();
                        tokens = tokens.saturating_add(cached).saturating_add(written);
                        let output = usage["output_tokens"].as_u64().unwrap_or_default();
                        if tokens == 0 && output == 0 {
                            return;
                        }
                        let model = message["model"].as_str();
                        dates.insert(day(at));
                        let totals = Tokens {
                            input: tokens,
                            output,
                            cached,
                            calls: 1,
                        };
                        // One Claude response can be persisted once for every content block.
                        let key = record["requestId"]
                            .as_str()
                            .or_else(|| message["id"].as_str());
                        if let Some(key) = key {
                            let entry = responses.entry(key.to_owned()).or_insert((
                                at,
                                model.map(str::to_owned),
                                totals,
                            ));
                            entry.2.input = entry.2.input.max(totals.input);
                            entry.2.cached = entry.2.cached.max(totals.cached);
                            entry.2.output = entry.2.output.max(totals.output);
                        } else {
                            history.record_tokens(at, model, totals);
                        }
                    }
                    Some("system") if record["subtype"] == "turn_duration" => {
                        if let Some(ms) = record["durationMs"].as_u64()
                            && record["uuid"]
                                .as_str()
                                .is_none_or(|id| durations.insert(id.to_owned()))
                        {
                            dates.insert(day(at));
                            history.record_duration(at, ms);
                        }
                    }
                    _ => {}
                }
            });
            for (_, (at, model, tokens)) in responses {
                history.record_tokens(at, model.as_deref(), tokens);
            }
            history.record_session(dates);
        }
    }
}

fn codex(root: &Path, history: &mut AgentHistory) {
    let mut seen = HashSet::new();
    for year in entries(&root.join("sessions"), true) {
        for month in entries(&year.path(), true) {
            for date in entries(&month.path(), true) {
                for path in jsonl_files(&date.path()) {
                    codex_session(&path, history, &mut seen);
                }
            }
        }
    }
    for path in jsonl_files(&root.join("archived_sessions")) {
        codex_session(&path, history, &mut seen);
    }
}

fn codex_session(path: &Path, history: &mut AgentHistory, seen: &mut HashSet<String>) {
    let mut dates = SessionDays::new();
    let mut model = None;
    let mut last_totals = None;
    let mut active_turns = BTreeMap::new();
    let mut completed = HashSet::new();
    let mut duplicate = false;
    jsonl(
        path,
        &[
            "\"session_meta\"",
            "\"turn_context\"",
            "\"token_count\"",
            "\"task_started\"",
            "\"task_complete\"",
        ],
        |record| {
            if duplicate {
                return;
            }
            let payload = &record["payload"];
            if record["type"] == "session_meta" {
                duplicate = payload["id"]
                    .as_str()
                    .is_some_and(|id| !seen.insert(id.to_owned()));
                return;
            }
            let Some(at) = timestamp(&record["timestamp"], history.today) else {
                return;
            };
            if record["type"] == "turn_context" {
                model = payload["model"].as_str().map(str::to_owned);
            }
            if record["type"] != "event_msg" {
                return;
            }
            let turn = payload["turn_id"].as_str();
            match payload["type"].as_str() {
                Some("task_started") => {
                    dates.insert(day(at));
                    if let Some(turn) = turn {
                        active_turns.insert(turn.to_owned(), at);
                    }
                }
                Some("task_complete") if turn.is_none_or(|id| completed.insert(id.to_owned())) => {
                    dates.insert(day(at));
                    let start = turn.and_then(|id| active_turns.remove(id));
                    let duration = payload["duration_ms"].as_u64().or_else(|| {
                        let span = at.signed_duration_since(start?);
                        u64::try_from(span.num_milliseconds()).ok()
                    });
                    if let Some(ms) = duration {
                        history.record_duration(at, ms);
                    }
                }
                Some("token_count") => {
                    let info = &payload["info"];
                    let read = |usage: &Value| {
                        fields(
                            usage,
                            "input_tokens",
                            "output_tokens",
                            "cached_input_tokens",
                        )
                    };
                    let total = read(&info["total_token_usage"]);
                    if let Some(total) = total
                        && last_totals == Some((total.input, total.output, total.cached))
                    {
                        return;
                    }
                    if let Some(tokens) = read(&info["last_token_usage"])
                        && (tokens.input > 0 || tokens.output > 0)
                    {
                        dates.insert(day(at));
                        history.record_tokens(at, model.as_deref(), tokens);
                    }
                    if let Some(total) = total {
                        last_totals = Some((total.input, total.output, total.cached));
                    }
                }
                _ => {}
            }
        },
    );
    history.record_session(dates);
}

fn antigravity(root: &Path, history: &mut AgentHistory) {
    for session in entries(root, true) {
        let logs = session.path().join(".system_generated/logs");
        let mut paths: Vec<_> = jsonl_files(&logs.join("chunks/transcript_full")).collect();
        paths.sort();
        paths.push(logs.join("transcript_full.jsonl"));
        paths.push(logs.join("transcript.jsonl"));
        let mut dates = SessionDays::new();
        let mut steps = HashSet::new();
        for path in paths {
            jsonl(
                &path,
                &["\"PLANNER_RESPONSE\"", "\"USER_INPUT\""],
                |record| {
                    let Some(at) = timestamp(&record["created_at"], history.today) else {
                        return;
                    };
                    match record["type"].as_str() {
                        Some("PLANNER_RESPONSE") => {
                            let Some(mut tokens) = fields(
                                record,
                                "input_tokens",
                                "output_tokens",
                                "cache_read_tokens",
                            ) else {
                                return;
                            };
                            // Antigravity reports cache reads separately from uncached input.
                            tokens.cached =
                                record["cache_read_tokens"].as_u64().unwrap_or_default();
                            tokens.input = tokens.input.saturating_add(tokens.cached);
                            let key = (record["step_index"].as_u64(), at);
                            if steps.insert(key) {
                                dates.insert(day(at));
                                history.record_tokens(at, record["model"].as_str(), tokens);
                            }
                        }
                        Some("USER_INPUT") => {
                            dates.insert(day(at));
                        }
                        _ => {}
                    }
                },
            );
        }
        history.record_session(dates);
    }
}

fn grok(root: &Path, history: &mut AgentHistory) {
    for project in entries(root, true) {
        for session in entries(&project.path(), true) {
            let mut dates = SessionDays::new();
            let mut prompts = HashSet::new();
            jsonl(
                &session.path().join("updates.jsonl"),
                &["\"turn_completed\"", "\"user_message_chunk\""],
                |record| {
                    let Some(at) = timestamp(&record["timestamp"], history.today) else {
                        return;
                    };
                    let update = &record["params"]["update"];
                    match update["sessionUpdate"].as_str() {
                        Some("user_message_chunk") => {
                            dates.insert(day(at));
                        }
                        Some("turn_completed")
                            if update["prompt_id"]
                                .as_str()
                                .is_none_or(|id| prompts.insert(id.to_owned())) =>
                        {
                            dates.insert(day(at));
                            if let Some(ms) = update["elapsed_ms"].as_u64() {
                                history.record_duration(at, ms);
                            }
                            let usage = &update["usage"];
                            let read = |value: &Value| {
                                let mut tokens = fields(
                                    value,
                                    "inputTokens",
                                    "outputTokens",
                                    "cachedReadTokens",
                                )?;
                                tokens.calls = value["modelCalls"].as_u64().unwrap_or(1);
                                Some(tokens)
                            };
                            // Record the turn's aggregate only once. Model splits carry their own tallies.
                            if let Some(tokens) = read(usage) {
                                history.record_tokens(at, None, tokens);
                            }
                            if let Some(models) = usage["modelUsage"].as_object() {
                                let bucket = history.days.entry(day(at)).or_default();
                                for (model, value) in models {
                                    if let Some(tokens) = read(value) {
                                        bucket.models.entry(model.clone()).or_default().add(tokens);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                },
            );
            history.record_session(dates);
        }
    }
}

/// OpenCode keeps every session in one SQLite database. A turn runs from a user message to the
/// last assistant step completed before the next user message.
fn opencode(db: &Path, history: &mut AgentHistory) {
    use rusqlite::{Connection, OpenFlags};
    if !db.is_file() {
        return;
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let Ok(conn) = Connection::open_with_flags(db, flags) else {
        return;
    };
    // OpenCode may be writing; wait briefly for its lock instead of failing the scan.
    let _ = conn.busy_timeout(Duration::from_millis(500));
    // Extract in SQL so message bodies (the bulk of the file) never reach Rust.
    let Ok(mut query) = conn.prepare(
        "SELECT session_id, type,
                coalesce(json_extract(data, '$.time.created'), time_created),
                json_extract(data, '$.time.completed'),
                json_extract(data, '$.tokens'),
                coalesce(json_extract(data, '$.model.id'), json_extract(data, '$.modelID'))
         FROM session_message
         WHERE type IN ('user', 'assistant')
         ORDER BY session_id, seq",
    ) else {
        return;
    };
    let Ok(rows) = query.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    }) else {
        return;
    };

    let mut session: Option<String> = None;
    let mut dates = SessionDays::new();
    // (started, last completed) in Unix milliseconds.
    let mut turn: Option<(i64, i64)> = None;
    for (id, kind, created, completed, tokens, model) in rows.flatten() {
        if session.as_deref() != Some(id.as_str()) {
            opencode_turn(history, turn.take());
            history.record_session(std::mem::take(&mut dates));
            session = Some(id);
        }
        let Some(at) = millis(created, history.today) else {
            continue;
        };
        dates.insert(day(at));
        if kind == "user" {
            opencode_turn(history, turn.replace((created, created)));
            continue;
        }
        if let Some((_, end)) = &mut turn {
            *end = (*end).max(completed.unwrap_or(created));
        }
        let Some(usage) = tokens.and_then(|text| serde_json::from_str::<Value>(&text).ok()) else {
            continue;
        };
        let count = |value: &Value| value.as_u64().unwrap_or_default();
        let cached = count(&usage["cache"]["read"]);
        let input = count(&usage["input"])
            .saturating_add(cached)
            .saturating_add(count(&usage["cache"]["write"]));
        let output = count(&usage["output"]).saturating_add(count(&usage["reasoning"]));
        if input == 0 && output == 0 {
            continue;
        }
        let totals = Tokens {
            input,
            output,
            cached,
            calls: 1,
        };
        history.record_tokens(at, model.as_deref(), totals);
    }
    opencode_turn(history, turn);
    history.record_session(dates);
}

fn opencode_turn(history: &mut AgentHistory, turn: Option<(i64, i64)>) {
    if let Some((start, end)) = turn
        && end > start
        && let Some(at) = millis(end, history.today)
    {
        history.record_duration(at, end.abs_diff(start));
    }
}

fn millis(ms: i64, today: Day) -> Option<DateTime<Utc>> {
    let time = DateTime::from_timestamp_millis(ms)?;
    (0..=today).contains(&day(time)).then_some(time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pi_counts_usage_without_recounting_forked_messages() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("--project--");
        fs::create_dir(&project).unwrap();
        let response = r#"{"type":"message","id":"reply","timestamp":"2026-11-20T03:00:01.000Z","message":{"role":"assistant","model":"claude-sonnet-4-5","usage":{"input":11,"output":12,"cacheRead":13,"cacheWrite":14}}}"#;
        fs::write(
            project.join("original.jsonl"),
            format!(
                "{}\n{response}\n",
                r#"{"type":"session","id":"original","timestamp":"2026-11-20T03:00:00.000Z"}"#,
            ),
        )
        .unwrap();
        fs::write(
            root.path().join("fork.jsonl"),
            format!(
                "{}\n{response}\n{}\n{}\n{{\"type\":",
                r#"{"type":"session","id":"fork","parentSession":"original.jsonl","timestamp":"2026-11-21T10:00:00.000Z"}"#,
                r#"{"type":"compaction","id":"summary","timestamp":"2026-11-21T10:00:01.000Z","model":"claude-sonnet-4-5","usage":{"input":20,"output":5,"cacheRead":30}}"#,
                r#"{"type":"usage","id":"warm","timestamp":"2026-11-21T10:00:02.000Z","model":"claude-sonnet-4-5","usage":{"cacheRead":40}}"#,
            ),
        )
        .unwrap();

        let today = day(DateTime::parse_from_rfc3339("2026-11-21T00:00:00Z")
            .unwrap()
            .to_utc());
        let mut history = AgentHistory::new(today);
        pi(root.path(), &mut history);
        let data = history.finish("pi");
        assert_eq!(data.periods[0].current, 2);
        assert_eq!(data.days.len(), 2);
        let original = &data.days[0];
        assert_eq!(original.sessions, 1);
        assert_eq!(original.tokens.input, 38);
        assert_eq!(original.tokens.cached, 13);
        assert_eq!(original.tokens.output, 12);
        assert_eq!(original.tokens.calls, 1);
        let fork = &data.days[1];
        assert_eq!(fork.sessions, 1);
        assert_eq!(fork.tokens.input, 90);
        assert_eq!(fork.tokens.cached, 70);
        assert_eq!(fork.tokens.output, 5);
        assert_eq!(fork.models[0].provider, "anthropic");
    }

    #[test]
    fn codex_counts_unique_sessions_and_ignores_repeated_token_totals() {
        let dir = std::env::temp_dir().join(format!("icy-usage-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("codex.jsonl");
        fs::write(&path, concat!(
            r#"{"type":"session_meta","payload":{"id":"session-a"}}"#, "\n",
            r#"{"type":"event_msg","timestamp":"2026-11-10T03:00:00Z","payload":{"type":"task_started","turn_id":"old"}}"#, "\n",
            r#"{"type":"event_msg","timestamp":"2026-11-20T03:00:00Z","payload":{"type":"task_started","turn_id":"new"}}"#, "\n",
            r#"{"type":"turn_context","timestamp":"2026-11-20T03:00:01Z","payload":{"model":"gpt-example"}}"#, "\n",
            r#"{"type":"event_msg","timestamp":"2026-11-20T03:00:02Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":120,"output_tokens":30,"cached_input_tokens":20},"last_token_usage":{"input_tokens":80,"output_tokens":20,"cached_input_tokens":20}}}}"#, "\n",
            r#"{"type":"event_msg","timestamp":"2026-11-20T03:00:03Z","payload":{"type":"token_count","info":null}}"#, "\n",
            r#"{"type":"event_msg","timestamp":"2026-11-20T03:00:04Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":120,"output_tokens":30,"cached_input_tokens":20},"last_token_usage":{"input_tokens":80,"output_tokens":20,"cached_input_tokens":20}}}}"#, "\n",
            r#"{"type":"event_msg","timestamp":"2026-11-20T03:00:08Z","payload":{"type":"task_complete","turn_id":"new"}}"#, "\n",
        )).unwrap();

        let today = day(DateTime::parse_from_rfc3339("2026-11-21T00:00:00Z")
            .unwrap()
            .to_utc());
        let mut history = AgentHistory::new(today);
        let mut seen = HashSet::new();
        codex_session(&path, &mut history, &mut seen);
        codex_session(&path, &mut history, &mut seen);
        let data = history.finish("codex");
        fs::remove_file(&path).unwrap();

        assert_eq!(data.periods[0].current, 1);
        assert_eq!(data.periods[0].previous, 1);
        assert_eq!(data.periods[1].current, 1);
        let latest = data.days.last().unwrap();
        assert_eq!(latest.tokens.input, 80);
        assert_eq!(latest.tokens.cached, 20);
        assert_eq!(latest.tokens.output, 20);
        assert_eq!(latest.hours[3], 1);
        assert_eq!(latest.duration_ms, 8000);
        assert_eq!(latest.models[0].provider, "openai");
    }

    #[test]
    fn opencode_reads_steps_turns_and_sessions_from_sqlite() {
        let dir = std::env::temp_dir().join(format!("icy-opencode-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("opencode.db");
        let _ = fs::remove_file(&path);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
               type TEXT NOT NULL, seq INTEGER NOT NULL, time_created INTEGER NOT NULL, data TEXT NOT NULL);",
        )
        .unwrap();
        // 2026-11-20T03:00:00Z in milliseconds.
        let t = 1_795_143_600_000_i64;
        let rows = [
            (
                "a",
                "user",
                0,
                t,
                format!(r#"{{"time":{{"created":{t}}}}}"#),
            ),
            (
                "a",
                "assistant",
                1,
                t,
                format!(
                    r#"{{"time":{{"created":{},"completed":{}}},"model":{{"id":"claude-haiku-5-5"}},"tokens":{{"input":10,"output":5,"reasoning":2,"cache":{{"read":100,"write":4}}}}}}"#,
                    t + 1_000,
                    t + 3_000
                ),
            ),
            (
                "a",
                "assistant",
                2,
                t,
                format!(
                    r#"{{"time":{{"created":{},"completed":{}}},"model":{{"id":"gpt-6.2-sol"}},"tokens":{{"input":0,"output":0,"reasoning":0,"cache":{{"read":0,"write":0}}}}}}"#,
                    t + 4_000,
                    t + 6_000
                ),
            ),
            ("a", "synthetic", 3, t, String::from("{}")),
            ("b", "user", 0, t, String::from("{}")),
        ];
        for (index, (session, kind, seq, created, data)) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO session_message VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![index.to_string(), session, kind, seq, created, data],
            )
            .unwrap();
        }
        drop(conn);

        let today = (t / 1000).div_euclid(86_400) + 1;
        let mut history = AgentHistory::new(today);
        opencode(&path, &mut history);
        let data = history.finish("opencode");
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(data.periods[0].current, 2);
        let latest = data.days.last().unwrap();
        assert_eq!(latest.tokens.input, 114);
        assert_eq!(latest.tokens.cached, 100);
        assert_eq!(latest.tokens.output, 7);
        assert_eq!(latest.tokens.calls, 1);
        assert_eq!(latest.timed_turns, 1);
        assert_eq!(latest.duration_ms, 6_000);
        assert_eq!(latest.models[0].provider, "anthropic");
    }
}
