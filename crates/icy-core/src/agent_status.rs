//! Per-PTY activity: prefer the program's OSC report, otherwise inspect only live TUI chrome.

use alacritty_terminal::vte::Perform;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    #[default]
    Idle,
    Working,
    Blocked,
    Done,
    Failed,
}

pub(crate) const STATUS_REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

#[derive(Default)]
pub(crate) struct StatusSignals {
    root: Option<(AgentStatus, Option<String>)>,
    pub(crate) progress: Option<AgentStatus>,
    pub(crate) queries: usize,
}

impl StatusSignals {
    pub(crate) fn app(&self) -> Option<&str> {
        self.root.as_ref()?.1.as_deref()
    }

    pub(crate) fn reported(&self, cli: &str) -> Option<AgentStatus> {
        let (status, app) = self.root.as_ref()?;
        if let Some(app) = app {
            let canonical = match app.as_str() {
                "agy" | "antigravity-cli" => "antigravity",
                "opencode2" | "open-code" => "opencode",
                "claude-code" => "claude",
                "grok-build" => "grok",
                other => other,
            };
            if canonical != cli {
                return None;
            }
        }
        Some(*status)
    }

    pub(crate) fn clear(&mut self) {
        self.root = None;
        self.progress = None;
    }
}

impl Perform for StatusSignals {
    fn osc_dispatch(&mut self, params: &[&[u8]], _: bool) {
        match params {
            [b"7501", b"?"] => self.queries += 1,
            [b"7501", payload] => {
                let Ok(text) = std::str::from_utf8(payload) else {
                    return;
                };
                let mut status = None;
                let mut app = None;
                for field in text.split(':') {
                    if let Some(value) = field.strip_prefix("state=") {
                        status = Some(value);
                    }
                    if let Some(value) = field.strip_prefix("app=")
                        && !value.is_empty()
                        && value.len() <= 64
                    {
                        app = Some(value.to_ascii_lowercase());
                    }
                    // App-specific child records must not overwrite the session summary.
                    if field.starts_with("id=") {
                        return;
                    }
                }
                self.root = match status {
                    Some("idle") => Some((AgentStatus::Idle, app)),
                    Some("working") => Some((AgentStatus::Working, app)),
                    Some("blocked") => Some((AgentStatus::Blocked, app)),
                    Some("done") => Some((AgentStatus::Done, app)),
                    Some("error") => Some((AgentStatus::Failed, app)),
                    Some("clear") => None,
                    _ => return,
                };
            }
            [b"9", b"4", state, ..] => {
                self.progress = match *state {
                    b"1" | b"3" => Some(AgentStatus::Working),
                    b"2" => Some(AgentStatus::Failed),
                    b"4" => Some(AgentStatus::Blocked),
                    _ => None,
                };
            }
            [b"133", b"A" | b"D", ..] => self.clear(),
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        if !ignore && intermediates.is_empty() && byte == b'c' {
            self.clear();
        }
    }
}

fn braille(ch: char) -> bool {
    ('\u{2801}'..='\u{28ff}').contains(&ch)
}

pub(crate) fn fallback(
    cli: &str,
    title: &str,
    tail: &[String],
    progress: Option<AgentStatus>,
) -> AgentStatus {
    use AgentStatus::*;
    let footer = || tail.iter().take(3).map(|line| line.trim());
    let any = |needle: &str| tail.iter().any(|line| line.contains(needle));
    let title_status = title
        .split_whitespace()
        .any(|word| word.starts_with(braille) && word.chars().count() == 1);
    match cli {
        "opencode" => {
            if any("△ permission required")
                || footer().any(|line| {
                    line.contains("esc dismiss")
                        && (line.contains("enter confirm")
                            || line.contains("enter submit")
                            || line.contains("enter toggle"))
                })
            {
                return Blocked;
            }
            if footer().any(|line| {
                line.contains("esc interrupt")
                    || line.contains("esc to interrupt")
                    || line.contains("esc stop")
                    || line.contains("ctrl+c to interrupt")
            }) {
                return Working;
            }
        }
        "antigravity" => {
            if footer().any(|line| {
                line.contains("navigate")
                    && (line.contains("tab amend") || line.contains("esc skip"))
            }) || (any("trust the contents") && any("enter confirm"))
            {
                return Blocked;
            }
            if footer().any(|line| line.contains("esc to cancel"))
                && !footer()
                    .any(|line| line.contains("? for shortcuts") && !line.contains("esc to cancel"))
            {
                return Working;
            }
        }
        "grok" => {
            if title.to_ascii_lowercase().contains("action required")
                || footer().any(|line| {
                    (line.contains(":select") && line.contains("ctrl+o:yolo"))
                        || (line.contains("tab:scrollback") && line.contains("shift+x:dismiss"))
                })
            {
                return Blocked;
            }
            if tail
                .iter()
                .any(|line| line.trim_start().starts_with(braille) && line.contains("[stop]"))
                || footer().any(|line| line.contains("esc:cancel") && line.contains("shortcuts"))
            {
                return Working;
            }
            if let Some(progress) = progress {
                return progress;
            }
            if title_status {
                return Working;
            }
        }
        "codex" => {
            if title.to_ascii_lowercase().contains("action required")
                || any("allow command?")
                || any("enter to submit answer")
                || (any("trust this folder?") && any("trust and continue"))
            {
                return Blocked;
            }
            if title_status
                || tail
                    .iter()
                    .take(7)
                    .any(|line| line.contains(" to interrupt)"))
            {
                return Working;
            }
        }
        "claude" => {
            if footer().any(|line| line.contains("esc to cancel"))
                && (any("do you want to proceed?") || any("do you want to allow this connection?"))
            {
                return Blocked;
            }
            if title
                .trim_start()
                .starts_with(|ch: char| braille(ch) || "·✢✶✻✽*◐◓◑◒".contains(ch))
                || footer().any(|line| line.contains("esc to interrupt"))
            {
                return Working;
            }
        }
        "junie" => {
            if footer().any(|line| line.split_whitespace().last() == Some("deny"))
                && any("allow once")
            {
                return Blocked;
            }
            if footer().any(|line| line.contains("working") && line.contains("esc to stop")) {
                return Working;
            }
            if let Some(progress) = progress {
                return progress;
            }
        }
        _ => {}
    }
    Idle
}
