use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use icy_core::platform;
use icy_core::session_server::{Client, Spawn, Terminal};
use icy_core::tiling::{TilingMode, Workspaces};

struct StopServer(PathBuf);

impl Drop for StopServer {
    fn drop(&mut self) {
        let _ = Client::stop(&self.0);
    }
}

fn wait_for(mut ready: impl FnMut() -> bool, message: &str) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !ready() {
        assert!(Instant::now() < deadline, "{message}");
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn screen_contains(terminal: &Terminal, text: &str) -> bool {
    terminal
        .frame()
        .0
        .rows
        .iter()
        .any(|row| row.plain_text().contains(text))
}

#[test]
fn terminal_server_keeps_working_without_a_window_and_reattaches() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("server");
    let _server = StopServer(dir.clone());
    let app = Path::new(env!("CARGO_BIN_EXE_icy"));
    let (notified, changes) = mpsc::channel();
    let (client, first) = Client::attach(
        &dir,
        app,
        move || {
            let _ = notified.send(());
        },
        || {},
    )
    .unwrap();
    assert!(first.terminals.is_empty());

    let terminal = client
        .spawn(Spawn {
            id: 1,
            title: "Long-running shell".into(),
            cwd: temp.path().to_path_buf(),
            program: std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            args: ["--exact", "interactive_fixture", "--ignored", "--nocapture"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            size: (80, 24),
            cli_hint: None,
            initial_output: None,
        })
        .unwrap();
    let (program, args) = platform::echo_command("second workspace");
    let second = client
        .spawn(Spawn {
            id: 2,
            title: "Second terminal".into(),
            cwd: temp.path().to_path_buf(),
            program,
            args,
            size: (80, 24),
            cli_hint: None,
            initial_output: None,
        })
        .unwrap();
    wait_for(
        || screen_contains(&terminal, "PROCESS_READY"),
        "terminal did not start",
    );
    changes
        .recv_timeout(Duration::from_secs(2))
        .expect("PTY updates wake the window");

    let mut layout = Workspaces::new(1, TilingMode::Niri);
    assert!(layout.focus_workspace_relative(1));
    layout.open_pane(2, false);
    assert!(layout.focus_workspace(0));
    layout.toggle_maximize();
    let archive = BTreeMap::from([(1, 123)]);
    client.save_archive(&archive).unwrap();
    layout.remove_session(1);
    client.save_layout(&layout).unwrap();

    drop(terminal);
    drop(second);
    drop(client);
    fs::write(temp.path().join("release"), "").unwrap();
    wait_for(
        || temp.path().join("finished-while-detached").exists(),
        "terminal stopped working after the window detached",
    );

    let (client, reopened) = Client::attach(&dir, app, || {}, || {}).unwrap();
    assert_eq!(reopened.layout.unwrap().snapshot(), layout.snapshot());
    assert_eq!(client.load_archive().unwrap(), archive);
    let mut terminals = reopened.terminals.into_iter();
    let running = client.terminal(terminals.next().unwrap());
    let previous = client.terminal(terminals.next().unwrap());
    assert_eq!((running.id, previous.id), (1, 2));
    assert!(running.is_alive());
    assert!(
        screen_contains(&running, "PROCESS_READY"),
        "screen history was lost"
    );

    client.save_archive(&BTreeMap::new()).unwrap();
    running.write_input(b"reply\r").unwrap();
    wait_for(
        || screen_contains(&running, "CONTINUED: reply"),
        "the reattached UI could not talk to the original process",
    );
    running.close().unwrap();
    previous.close().unwrap();
    drop(running);
    drop(previous);
    drop(client);

    let (client, reopened) = Client::attach(&dir, app, || {}, || {}).unwrap();
    assert!(
        reopened.terminals.is_empty(),
        "an explicitly closed terminal reappeared"
    );
    assert!(reopened.layout.unwrap().all_panes().is_empty());
    assert!(client.load_archive().unwrap().is_empty());
    drop(client);
}

// Executed as a child inside a PTY by the integration test.
#[test]
#[ignore]
fn interactive_fixture() {
    println!("PROCESS_READY");
    io::stdout().flush().unwrap();
    wait_for(
        || Path::new("release").exists(),
        "parent never released fixture",
    );
    fs::write("finished-while-detached", std::process::id().to_string()).unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    println!("CONTINUED: {}", input.trim());
}
