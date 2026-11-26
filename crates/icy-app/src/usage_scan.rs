use std::{
    sync::{Arc, Mutex, mpsc::Sender},
    time::{Duration, Instant},
};

use icy_core::usage::{self, UsageSnapshot};

use crate::{BridgeEvent, CLI_USAGE_REFRESH, REMOTE_LIMITS_REFRESH};

/// Shared by the sidebar and Metrics workers. Neither the window nor the bridge waits on this lock.
#[derive(Default)]
pub(crate) struct UsageScan {
    latest: Mutex<Option<(Instant, Arc<UsageSnapshot>)>>,
}

impl UsageScan {
    pub(crate) fn snapshot(&self, max_age: Duration) -> Arc<UsageSnapshot> {
        let requested = Instant::now();
        let mut latest = self.latest.lock().unwrap();
        // A scan that finished while we waited also satisfies an explicit refresh.
        if let Some((completed, snapshot)) = latest.as_ref()
            && requested.saturating_duration_since(*completed) <= max_age
        {
            return Arc::clone(snapshot);
        }
        let snapshot = Arc::new(usage::collect_local());
        *latest = Some((Instant::now(), Arc::clone(&snapshot)));
        snapshot
    }
}

pub(crate) fn spawn_sidebar(scan: Arc<UsageScan>, tx: Sender<BridgeEvent>) {
    let _ = std::thread::Builder::new()
        .name("icy-cli-usage".into())
        .spawn(move || {
            let publish = |snapshot: &UsageSnapshot, remote: &usage::RemoteLimits| {
                tx.send(BridgeEvent::CliUsage(usage::collect_cli(snapshot, remote)))
                    .is_ok()
            };
            let mut remote = Vec::new();
            let mut remote_at: Option<Instant> = None;
            loop {
                let snapshot = scan.snapshot(Duration::ZERO);
                if !publish(&snapshot, &remote) {
                    break;
                }
                if remote_at.is_none_or(|at| at.elapsed() >= REMOTE_LIMITS_REFRESH) {
                    remote = usage::fetch_remote_limits();
                    remote_at = Some(Instant::now());
                    if !publish(&scan.snapshot(CLI_USAGE_REFRESH), &remote) {
                        break;
                    }
                }
                std::thread::sleep(CLI_USAGE_REFRESH);
            }
        });
}
