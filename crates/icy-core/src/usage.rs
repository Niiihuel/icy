//! Local agent usage, independent of the terminal and window runtime.

mod cli;
mod logs;
mod remote;

pub use cli::{CliUsage, UsageRow, UsageSection};
pub use remote::RemoteLimits;

use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Timelike, Utc};
use serde::Serialize;

type Day = i64;
type SessionDays = BTreeSet<Day>;

const SECONDS_PER_DAY: i64 = 86_400;
const PERIODS: [Option<i64>; 4] = [Some(7), Some(30), Some(90), None];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    today: Day,
    agents: Vec<AgentUsage>,
}

#[derive(Serialize)]
struct AgentUsage {
    id: &'static str,
    periods: [SessionComparison; PERIODS.len()],
    days: Vec<UsageDay>,
}

#[derive(Default, Serialize)]
struct SessionComparison {
    current: usize,
    previous: usize,
}

#[derive(Clone, Copy, Default, Serialize)]
struct Tokens {
    input: u64,
    output: u64,
    cached: u64,
    calls: u64,
}

impl Tokens {
    fn add(&mut self, other: Self) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cached = self.cached.saturating_add(other.cached);
        self.calls = self.calls.saturating_add(other.calls);
    }
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageDay {
    day: Day,
    tokens: Tokens,
    sessions: usize,
    duration_ms: u64,
    timed_turns: u64,
    hours: [u64; 24],
    models: Vec<ModelUsage>,
}

#[derive(Serialize)]
struct ModelUsage {
    name: String,
    provider: &'static str,
    tokens: Tokens,
}

#[derive(Default)]
struct DailyUsage {
    usage: UsageDay,
    models: BTreeMap<String, Tokens>,
}

struct AgentHistory {
    today: Day,
    days: BTreeMap<Day, DailyUsage>,
    periods: [SessionComparison; PERIODS.len()],
}

impl AgentHistory {
    fn new(today: Day) -> Self {
        Self {
            today,
            days: BTreeMap::new(),
            periods: Default::default(),
        }
    }

    fn record_tokens(&mut self, at: DateTime<Utc>, model: Option<&str>, tokens: Tokens) {
        let bucket = self.days.entry(day(at)).or_default();
        bucket.usage.tokens.add(tokens);
        let hour = &mut bucket.usage.hours[at.hour() as usize];
        *hour = hour.saturating_add(tokens.calls);
        if let Some(model) = model.filter(|model| !model.is_empty()) {
            bucket.models.entry(model.into()).or_default().add(tokens);
        }
    }

    fn record_duration(&mut self, at: DateTime<Utc>, duration: u64) {
        let usage = &mut self.days.entry(day(at)).or_default().usage;
        usage.duration_ms = usage.duration_ms.saturating_add(duration);
        usage.timed_turns += 1;
    }

    fn record_session(&mut self, dates: SessionDays) {
        if dates.is_empty() {
            return;
        }
        for &date in &dates {
            self.days.entry(date).or_default().usage.sessions += 1;
        }
        for (length, counts) in PERIODS.into_iter().zip(&mut self.periods) {
            let Some(length) = length else {
                counts.current += 1;
                continue;
            };
            let start = self.today + 1 - length;
            counts.current += usize::from(dates.range(start..=self.today).next().is_some());
            counts.previous += usize::from(dates.range(start - length..start).next().is_some());
        }
    }

    fn finish(self, id: &'static str) -> AgentUsage {
        AgentUsage {
            id,
            periods: self.periods,
            days: self
                .days
                .into_iter()
                .map(|(day, mut bucket)| {
                    bucket.usage.day = day;
                    bucket.usage.models = bucket
                        .models
                        .into_iter()
                        .map(|(name, tokens)| ModelUsage {
                            provider: model_provider(&name, id),
                            name,
                            tokens,
                        })
                        .collect();
                    bucket.usage
                })
                .collect(),
        }
    }
}

fn day(at: DateTime<Utc>) -> Day {
    at.timestamp().div_euclid(SECONDS_PER_DAY)
}

fn model_provider(name: &str, agent: &str) -> &'static str {
    let model = name.rsplit('/').next().unwrap_or(name).to_ascii_lowercase();
    if model.starts_with("claude") {
        "anthropic"
    } else if model.starts_with("gemini") {
        "google"
    } else if model.starts_with("grok") {
        "xai"
    } else if model.starts_with("gpt-")
        || ["o1", "o3", "o4"].iter().any(|prefix| {
            model == *prefix
                || model
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('-'))
        })
    {
        "openai"
    } else if agent == "claude" {
        "anthropic"
    } else {
        "other"
    }
}

pub fn collect_local() -> UsageSnapshot {
    let today = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        / SECONDS_PER_DAY;
    let home = crate::platform::home_dir();
    logs::collect(home.as_deref(), today)
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Sidebar meters for every tracked CLI: a shared local snapshot plus the last live limits.
pub fn collect_cli(snapshot: &UsageSnapshot, remote: &RemoteLimits) -> Vec<CliUsage> {
    let now = now_seconds();
    let home = crate::platform::home_dir();
    cli::collect(home.as_deref(), snapshot, now, remote)
}

/// Live limits (network / CLI calls); poll sparingly, the vendors rate-limit these endpoints.
pub fn fetch_remote_limits() -> RemoteLimits {
    remote::fetch(crate::platform::home_dir().as_deref(), now_seconds())
}
