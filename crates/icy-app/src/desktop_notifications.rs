use std::{io, sync::mpsc};

use icy_core::{
    agent_notifications::AgentNotice,
    config::CliConfig,
    platform::{DesktopNotification, DesktopNotifier},
    session::SessionInfo,
};

use crate::BridgeEvent;

fn single_line(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control())
        .take(100)
        .collect()
}

pub(crate) fn agent_notice(
    session: &SessionInfo,
    event: AgentNotice,
    clis: &[CliConfig],
) -> DesktopNotification {
    let name = clis
        .iter()
        .find(|cli| cli.id == session.cli)
        .map_or(session.cli.as_str(), |cli| cli.name.as_str());
    let project = session
        .cwd
        .file_name()
        .unwrap_or(session.cwd.as_os_str())
        .to_string_lossy();
    let status = match event {
        AgentNotice::Finished => "finished",
        AgentNotice::NeedsInput => "needs your input",
    };
    DesktopNotification {
        title: format!("{} {status}", single_line(name)),
        body: single_line(&project),
        needs_input: event == AgentNotice::NeedsInput,
    }
}

pub(crate) fn test_notice() -> DesktopNotification {
    DesktopNotification {
        title: "icy notifications".into(),
        body: "This is a test notification from icy.".into(),
        needs_input: false,
    }
}

struct Job {
    notice: DesktopNotification,
    reply: Option<mpsc::Sender<BridgeEvent>>,
}

#[derive(Default)]
pub(crate) struct Delivery {
    sender: Option<mpsc::SyncSender<Job>>,
}

impl Delivery {
    pub(crate) fn send(
        &mut self,
        notice: DesktopNotification,
        reply: Option<mpsc::Sender<BridgeEvent>>,
    ) -> io::Result<()> {
        if self.sender.is_none() {
            let (sender, jobs) = mpsc::sync_channel::<Job>(32);
            std::thread::Builder::new()
                .name("icy-desktop-notifications".into())
                .spawn(move || {
                    let mut desktop = DesktopNotifier::default();
                    for job in jobs {
                        let result = desktop.show(&job.notice);
                        if let Some(reply) = job.reply {
                            let error = result.err().map(|error| error.to_string());
                            let _ = reply.send(BridgeEvent::NotificationTested(error));
                        } else if let Err(error) = result {
                            eprintln!("icy: desktop notification failed: {error}");
                        }
                    }
                })?;
            self.sender = Some(sender);
        }
        self.sender
            .as_ref()
            .unwrap()
            .try_send(Job { notice, reply })
            .map_err(|error| io::Error::other(error.to_string()))
    }
}
