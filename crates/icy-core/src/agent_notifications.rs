//! Turns observed PTY states into settled notification events.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::{agent_status::AgentStatus, session::SessionInfo};

const SETTLE: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NotificationPreferences {
    pub finished: bool,
    pub needs_input: bool,
    pub background_only: bool,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            finished: false,
            needs_input: false,
            background_only: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentNotice {
    Finished,
    NeedsInput,
}

impl NotificationPreferences {
    pub fn allows(self, notice: AgentNotice, pane_focused: bool) -> bool {
        (!self.background_only || !pane_focused)
            && match notice {
                AgentNotice::Finished => self.finished,
                AgentNotice::NeedsInput => self.needs_input,
            }
    }
}

struct Observed {
    cli: String,
    status: AgentStatus,
    pending: Option<(AgentNotice, Instant)>,
    completion_reported: bool,
    present: bool,
}

#[derive(Default)]
pub struct AgentNotifications {
    sessions: HashMap<u64, Observed>,
    initialized: bool,
}

impl AgentNotifications {
    pub fn deadline(&self) -> Option<Instant> {
        self.sessions
            .values()
            .filter_map(|state| state.pending.map(|(_, due)| due))
            .min()
    }

    /// The first snapshot is a baseline. Only changes observed in this window produce notices.
    pub fn observe<'a>(
        &mut self,
        sessions: &'a [SessionInfo],
        now: Instant,
    ) -> Vec<(&'a SessionInfo, AgentNotice)> {
        use AgentStatus::{Blocked, Done, Idle, Working};

        let mut notices = Vec::new();
        for state in self.sessions.values_mut() {
            state.present = false;
        }
        for session in sessions
            .iter()
            .filter(|item| item.alive && item.cli != "terminal")
        {
            let initialize = || Observed {
                cli: session.cli.clone(),
                status: session.status,
                present: true,
                completion_reported: session.status == Done,
                pending: (self.initialized && session.status == Blocked)
                    .then_some((AgentNotice::NeedsInput, now + SETTLE)),
            };
            let state = self.sessions.entry(session.id).or_insert_with(initialize);
            if state.cli != session.cli {
                *state = initialize();
            }
            state.present = true;
            if session.status != state.status {
                state.pending = match session.status {
                    Working => {
                        state.completion_reported = false;
                        None
                    }
                    Blocked => {
                        state.completion_reported = false;
                        Some((AgentNotice::NeedsInput, now + SETTLE))
                    }
                    Done if !state.completion_reported => state
                        .pending
                        .filter(|(kind, _)| *kind == AgentNotice::Finished)
                        .or(Some((AgentNotice::Finished, now + SETTLE))),
                    Idle if state.status == Working => Some((AgentNotice::Finished, now + SETTLE)),
                    Idle if state.status == Done => state.pending,
                    _ => None,
                };
                state.status = session.status;
            }
            if let Some((notice, due)) = state.pending
                && now >= due
            {
                state.pending = None;
                state.completion_reported |= notice == AgentNotice::Finished;
                notices.push((session, notice));
            }
        }
        self.sessions.retain(|_, state| state.present);
        self.initialized = true;
        notices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AgentNotice::{Finished, NeedsInput};
    use AgentStatus::{Blocked, Done, Failed, Idle, Working};

    fn agent(id: u64, status: AgentStatus) -> SessionInfo {
        SessionInfo {
            id,
            title: "Codex".into(),
            cli: "codex".into(),
            cwd: "project".into(),
            alive: true,
            status,
        }
    }

    fn events(
        observer: &mut AgentNotifications,
        now: Instant,
        sessions: &[SessionInfo],
    ) -> Vec<(u64, AgentNotice)> {
        observer
            .observe(sessions, now)
            .into_iter()
            .map(|(session, event)| (session.id, event))
            .collect()
    }

    #[test]
    fn reports_each_settled_transition_once_and_ignores_reattached_status() {
        let now = Instant::now();
        let mut observer = AgentNotifications::default();
        let before = [agent(1, Working), agent(2, Blocked), agent(3, Done)];
        assert!(events(&mut observer, now, &before).is_empty());
        assert_eq!(observer.deadline(), None);

        let after = [
            agent(1, Idle),
            agent(2, Blocked),
            agent(3, Done),
            agent(4, Blocked),
        ];
        assert!(events(&mut observer, now, &after).is_empty());
        assert_eq!(observer.deadline(), Some(now + SETTLE));
        assert_eq!(
            events(&mut observer, now + SETTLE, &after),
            [(1, Finished), (4, NeedsInput)]
        );
        assert!(events(&mut observer, now + SETTLE * 2, &after).is_empty());

        let resumed = [agent(4, Working)];
        assert!(events(&mut observer, now + SETTLE * 3, &resumed).is_empty());
        let asked_again = [agent(4, Blocked)];
        assert!(events(&mut observer, now + SETTLE * 4, &asked_again).is_empty());
        assert_eq!(
            events(&mut observer, now + SETTLE * 5, &asked_again),
            [(4, NeedsInput)]
        );
    }

    #[test]
    fn drops_notifications_for_transient_prompts_failures_and_removed_panes() {
        let now = Instant::now();
        let mut observer = AgentNotifications::default();
        let before = [agent(1, Working), agent(2, Working), agent(3, Working)];
        let pending = [agent(1, Blocked), agent(2, Idle), agent(3, Done)];
        let after = [agent(1, Working), agent(2, Failed)];
        events(&mut observer, now, &before);
        events(&mut observer, now, &pending);
        assert!(events(&mut observer, now + SETTLE, &after).is_empty());
        assert_eq!(observer.deadline(), None);
    }

    #[test]
    fn explicit_done_can_arrive_without_a_working_frame_and_does_not_repeat_on_idle() {
        let now = Instant::now();
        let mut observer = AgentNotifications::default();
        events(&mut observer, now, &[agent(1, Idle)]);
        events(&mut observer, now, &[agent(1, Done)]);
        assert_eq!(
            events(&mut observer, now + SETTLE, &[agent(1, Idle)]),
            [(1, Finished)]
        );
        assert!(events(&mut observer, now + SETTLE * 2, &[agent(1, Done)]).is_empty());
        assert_eq!(observer.deadline(), None);
    }

    #[test]
    fn preferences_mute_the_focused_pane_and_allow_each_event_separately() {
        let mut prefs = NotificationPreferences::default();
        assert!(!prefs.allows(NeedsInput, false));
        prefs.needs_input = true;
        assert!(prefs.allows(NeedsInput, false));
        assert!(!prefs.allows(NeedsInput, true));
        assert!(!prefs.allows(Finished, false));
        prefs.background_only = false;
        assert!(prefs.allows(NeedsInput, true));
    }
}
