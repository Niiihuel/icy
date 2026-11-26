//! Local transport for terminals owned by a process independent of the window.
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use bincode::Options;
use interprocess::local_socket::{
    Stream,
    traits::{Listener as _, Stream as _},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::graphics::{ImageData, VisibleImage};
use crate::platform;
use crate::session::{Session, SessionInfo};
use crate::tiling::Workspaces;
use crate::vt::{CellPoint, CursorPos, Screen, SelectKind};

const PROTOCOL: &[u8; 8] = b"ICYPTY01";
const MAX_FRAME: usize = 64 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const IO_TIMEOUT: Duration = Duration::from_secs(6);
const WATCH_TIMEOUT: Duration = Duration::from_secs(2);
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

pub fn default_directory() -> PathBuf {
    platform::config_dir().join("session-server")
}

#[derive(Serialize, Deserialize)]
enum Role {
    Control,
    Watch,
    Stop,
}

#[derive(Serialize, Deserialize)]
enum Request {
    SaveLayout(Workspaces),
    Spawn {
        command: Spawn,
        environment: Vec<(OsString, OsString)>,
    },
    Terminal(u64, TerminalRequest),
}

#[derive(Serialize, Deserialize)]
pub struct Spawn {
    pub id: u64,
    pub title: String,
    pub cwd: PathBuf,
    pub program: String,
    pub args: Vec<String>,
    pub size: (u16, u16),
    pub cli_hint: Option<String>,
    pub initial_output: Option<String>,
}

impl Spawn {
    pub fn start(self, on_update: Arc<dyn Fn() + Send + Sync>) -> io::Result<Session> {
        self.start_with_environment(on_update, None)
    }

    fn start_with_environment(
        self,
        on_update: Arc<dyn Fn() + Send + Sync>,
        environment: Option<Vec<(OsString, OsString)>>,
    ) -> io::Result<Session> {
        let mut command = portable_pty::CommandBuilder::new(&self.program);
        command.args(&self.args);
        if let Some(environment) = environment {
            command.env_clear();
            for (key, value) in environment {
                command.env(key, value);
            }
        }
        let mut session = Session::spawn_configured(
            self.id, self.title, self.cwd, command, self.size, on_update,
        )?;
        if let Some(cli) = self.cli_hint {
            session = session.with_cli_hint(cli);
        }
        if let Some(output) = self.initial_output {
            session.feed_local(output.as_bytes());
        }
        Ok(session)
    }
}

#[derive(Serialize, Deserialize)]
enum TerminalRequest {
    Info,
    Frame,
    Image(u64),
    Write(Vec<u8>),
    Input(Vec<u8>),
    PasteContext(String),
    PasteClipboard(String),
    KittyKeys,
    StartSelection(SelectKind, CellPoint),
    ExtendSelection(CellPoint),
    ClearSelection,
    SelectionText,
    Wheel(i32, u16, u16),
    ScrollToBottom,
    Resize(u16, u16),
    Close,
}

#[derive(Serialize, Deserialize)]
pub struct Attachment {
    pub terminals: Vec<SessionInfo>,
    pub layout: Option<Workspaces>,
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new().with_limit(MAX_FRAME as u64)
}

fn write_frame<T: Serialize>(stream: &mut Stream, value: &T) -> io::Result<()> {
    let bytes = codec().serialize(value).map_err(io::Error::other)?;
    let size = u32::try_from(bytes.len())
        .ok()
        .filter(|size| *size as usize <= MAX_FRAME)
        .ok_or_else(|| io::Error::other("session message too large"))?;
    stream.write_all(&size.to_le_bytes())?;
    stream.write_all(&bytes)
}

fn read_frame<T: DeserializeOwned>(stream: &mut Stream) -> io::Result<T> {
    let mut size = [0; 4];
    stream.read_exact(&mut size)?;
    let size = u32::from_le_bytes(size) as usize;
    if size > MAX_FRAME {
        return Err(io::Error::other("session message too large"));
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes)?;
    codec().deserialize(&bytes).map_err(io::Error::other)
}

fn reply<T: Serialize>(stream: &mut Stream, result: io::Result<T>) -> io::Result<()> {
    write_frame(stream, &result.map_err(|error| error.to_string()))
}

fn answer<T: DeserializeOwned>(stream: &mut Stream) -> io::Result<io::Result<T>> {
    read_frame::<Result<T, String>>(stream).map(|result| result.map_err(io::Error::other))
}

fn connect(dir: &Path, role: Role) -> io::Result<Stream> {
    let mut stream = platform::connect_session_socket(&dir.join("pty.sock"))?;
    let _ = stream.set_recv_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_send_timeout(Some(IO_TIMEOUT));
    stream.write_all(PROTOCOL)?;
    let mut protocol = [0; PROTOCOL.len()];
    stream.read_exact(&mut protocol)?;
    if &protocol != PROTOCOL {
        return Err(io::Error::other("incompatible icy session server protocol"));
    }
    write_frame(&mut stream, &role)?;
    Ok(stream)
}

fn lock_file(dir: &Path, name: &str) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(name))?;
    file.try_lock().map_err(io::Error::from)?;
    Ok(file)
}

fn server_absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}

/// One interactive window owns the layout while attached. Dropping the client detaches.
pub struct Client {
    stream: Mutex<Option<Stream>>,
    _window_lock: File,
    directory: PathBuf,
}

impl Client {
    pub fn attach(
        dir: &Path,
        executable: &Path,
        changed: impl Fn() + Send + 'static,
        disconnected: impl FnOnce() + Send + 'static,
    ) -> io::Result<(Arc<Self>, Attachment)> {
        platform::prepare_session_dir(dir)?;
        let dir = &dir.canonicalize()?;
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        let window_lock = loop {
            match lock_file(dir, "window.lock") {
                Ok(lock) => break lock,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            error.kind(),
                            "another icy window is already attached",
                        ));
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                Err(error) => return Err(error),
            }
        };
        let mut stream = match connect(dir, Role::Control) {
            Ok(stream) => stream,
            Err(error) if server_absent(&error) => {
                let mut child = platform::session_server_command(executable, dir)?
                    .arg("--session-server")
                    .arg(dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()?;
                let deadline = Instant::now() + CONNECT_TIMEOUT;
                loop {
                    match connect(dir, Role::Control) {
                        Ok(stream) => break stream,
                        Err(error) if server_absent(&error) && Instant::now() < deadline => {
                            if let Some(status) = child.try_wait()? {
                                return Err(io::Error::other(format!(
                                    "session server exited: {status}"
                                )));
                            }
                            thread::sleep(Duration::from_millis(25));
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
            Err(error) => return Err(error),
        };
        let attachment = answer(&mut stream)??;
        let mut watcher = connect(dir, Role::Watch)?;
        let client = Arc::new(Self {
            stream: Mutex::new(Some(stream)),
            _window_lock: window_lock,
            directory: dir.clone(),
        });
        let owner = Arc::downgrade(&client);
        thread::Builder::new()
            .name("icy-session-watch".into())
            .spawn(move || {
                let mut revision = 0;
                while owner.strong_count() > 0 {
                    let next = write_frame(&mut watcher, &revision)
                        .and_then(|()| read_frame::<u64>(&mut watcher));
                    match next {
                        Ok(next) if next != revision => {
                            revision = next;
                            changed();
                            thread::sleep(FRAME_INTERVAL);
                        }
                        Ok(_) => {}
                        Err(_) => {
                            if owner.strong_count() > 0 {
                                disconnected();
                            }
                            break;
                        }
                    }
                }
            })?;
        Ok((client, attachment))
    }

    pub fn stop(dir: &Path) -> io::Result<()> {
        match connect(dir, Role::Stop) {
            Ok(mut stream) => answer(&mut stream)?,
            Err(error) if server_absent(&error) => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn request<T: DeserializeOwned>(&self, request: Request) -> io::Result<T> {
        let mut slot = self.stream.lock().unwrap();
        let stream = slot
            .as_mut()
            .ok_or_else(|| io::Error::other("session server disconnected"))?;
        match write_frame(stream, &request).and_then(|()| answer(stream)) {
            Ok(result) => result,
            Err(error) => {
                *slot = None;
                Err(error)
            }
        }
    }

    pub fn save_layout(&self, layout: &Workspaces) -> io::Result<()> {
        self.request(Request::SaveLayout(layout.clone()))
    }

    /// Session IDs and archive timestamps, stored separately from the layout protocol.
    pub fn load_archive(&self) -> io::Result<BTreeMap<u64, u64>> {
        match File::open(self.directory.join("archive.json")) {
            Ok(file) => serde_json::from_reader(file).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(error) => Err(error),
        }
    }

    pub fn save_archive(&self, archive: &BTreeMap<u64, u64>) -> io::Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)?;
        serde_json::to_writer(&mut file, archive).map_err(io::Error::other)?;
        file.as_file().sync_all()?;
        file.persist(self.directory.join("archive.json"))?;
        Ok(())
    }

    pub fn spawn(self: &Arc<Self>, spawn: Spawn) -> io::Result<Terminal> {
        let info = self.request(Request::Spawn {
            command: spawn,
            environment: std::env::vars_os().collect(),
        })?;
        Ok(self.terminal(info))
    }

    pub fn terminal(self: &Arc<Self>, info: SessionInfo) -> Terminal {
        Terminal {
            id: info.id,
            cwd: info.cwd.clone(),
            runtime: Runtime::Attached {
                client: Arc::clone(self),
                info: Mutex::new(info),
                images: Mutex::new(AttachedImages::default()),
            },
        }
    }
}

#[derive(Default)]
struct AttachedImages {
    seen: HashSet<u64>,
    pending: Vec<(u64, ImageData)>,
}

enum Runtime {
    InProcess(Session),
    Attached {
        client: Arc<Client>,
        info: Mutex<SessionInfo>,
        images: Mutex<AttachedImages>,
    },
}

/// A terminal for the compositor. The in-process variant is used for offscreen runs and tests.
pub struct Terminal {
    pub id: u64,
    cwd: PathBuf,
    runtime: Runtime,
}

impl From<Session> for Terminal {
    fn from(session: Session) -> Self {
        Self {
            id: session.id,
            cwd: session.cwd().to_path_buf(),
            runtime: Runtime::InProcess(session),
        }
    }
}

macro_rules! terminal_call {
    ($terminal:expr, $operation:expr, $call:expr) => {
        match &$terminal.runtime {
            Runtime::InProcess(session) => $call(session),
            Runtime::Attached { client, .. } => {
                client.request(Request::Terminal($terminal.id, $operation))
            }
        }
    };
}

impl Terminal {
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn info(&self) -> SessionInfo {
        match &self.runtime {
            Runtime::InProcess(session) => session.info(),
            Runtime::Attached { client, info, .. } => {
                let mut cached = info.lock().unwrap();
                match client.request(Request::Terminal(self.id, TerminalRequest::Info)) {
                    Ok(next) => *cached = next,
                    Err(_) => cached.alive = false,
                }
                cached.clone()
            }
        }
    }

    pub fn is_alive(&self) -> bool {
        match &self.runtime {
            Runtime::InProcess(session) => session.is_alive(),
            Runtime::Attached { .. } => self.info().alive,
        }
    }

    pub fn frame(&self) -> (Screen, Vec<VisibleImage>) {
        let (mut screen, placements) =
            terminal_call!(self, TerminalRequest::Frame, |session: &Session| {
                let screen = session.snapshot();
                let images = session.visible_images(screen.rows_count);
                Ok((screen, images))
            })
            .unwrap_or_else(|_| {
                (
                    Screen {
                        cols: 0,
                        rows_count: 0,
                        rows: Vec::new(),
                        display_offset: 0,
                        cursor: CursorPos {
                            row: 0,
                            col: 0,
                            visible: false,
                        },
                    },
                    Vec::new(),
                )
            });
        if let Runtime::Attached { client, images, .. } = &self.runtime {
            // A session server kept alive across a UI update can still send one run per glyph.
            for row in &mut screen.rows {
                row.coalesce_monospace_runs();
            }
            let mut images = images.lock().unwrap();
            for placement in &placements {
                if !images.seen.contains(&placement.seq)
                    && let Ok(Some(data)) = client.request(Request::Terminal(
                        self.id,
                        TerminalRequest::Image(placement.seq),
                    ))
                {
                    images.seen.insert(placement.seq);
                    images.pending.push((placement.seq, data));
                }
            }
        }
        (screen, placements)
    }

    pub fn take_image_uploads(&self) -> Vec<(u64, ImageData)> {
        match &self.runtime {
            Runtime::InProcess(session) => session.take_image_uploads(),
            Runtime::Attached { images, .. } => std::mem::take(&mut images.lock().unwrap().pending),
        }
    }

    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        terminal_call!(
            self,
            TerminalRequest::Write(bytes.to_vec()),
            |session: &Session| session.write(bytes)
        )
    }

    pub fn write_input(&self, bytes: &[u8]) -> io::Result<bool> {
        terminal_call!(
            self,
            TerminalRequest::Input(bytes.to_vec()),
            |session: &Session| session.write_input(bytes)
        )
    }

    pub fn paste_context(&self, text: &str) -> io::Result<()> {
        terminal_call!(
            self,
            TerminalRequest::PasteContext(text.into()),
            |session: &Session| session.paste_context(text)
        )
    }

    pub fn paste_clipboard(&self, text: &str) -> io::Result<bool> {
        terminal_call!(
            self,
            TerminalRequest::PasteClipboard(text.into()),
            |session: &Session| session.paste_clipboard(text)
        )
    }

    pub fn kitty_keys(&self) -> bool {
        terminal_call!(self, TerminalRequest::KittyKeys, |session: &Session| Ok(
            session.kitty_keys()
        ))
        .unwrap_or(false)
    }

    pub fn start_selection(&self, kind: SelectKind, at: CellPoint) {
        let _ = terminal_call!(
            self,
            TerminalRequest::StartSelection(kind, at),
            |session: &Session| {
                session.start_selection(kind, at);
                Ok(())
            }
        );
    }

    pub fn extend_selection(&self, to: CellPoint) {
        let _ = terminal_call!(
            self,
            TerminalRequest::ExtendSelection(to),
            |session: &Session| {
                session.extend_selection(to);
                Ok(())
            }
        );
    }

    pub fn clear_selection(&self) {
        let _ = terminal_call!(
            self,
            TerminalRequest::ClearSelection,
            |session: &Session| {
                session.clear_selection();
                Ok(())
            }
        );
    }

    pub fn selection_text(&self) -> Option<String> {
        terminal_call!(
            self,
            TerminalRequest::SelectionText,
            |session: &Session| Ok(session.selection_text())
        )
        .unwrap_or(None)
    }

    pub fn wheel(&self, lines: i32, col: u16, row: u16) -> io::Result<bool> {
        terminal_call!(
            self,
            TerminalRequest::Wheel(lines, col, row),
            |session: &Session| session.wheel(lines, col, row)
        )
    }

    pub fn scroll_to_bottom(&self) -> bool {
        terminal_call!(
            self,
            TerminalRequest::ScrollToBottom,
            |session: &Session| Ok(session.scroll_to_bottom())
        )
        .unwrap_or(false)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<bool> {
        terminal_call!(
            self,
            TerminalRequest::Resize(cols, rows),
            |session: &Session| session.resize(cols, rows)
        )
    }

    pub fn close(&self) -> io::Result<()> {
        terminal_call!(self, TerminalRequest::Close, |session: &Session| {
            session.kill();
            Ok(())
        })
    }

    pub fn update_sync_timeout(&self, now: Instant) -> (bool, Option<Instant>) {
        match &self.runtime {
            Runtime::InProcess(session) => session.update_sync_timeout(now),
            Runtime::Attached { .. } => (false, None),
        }
    }
}

#[derive(Default)]
struct State {
    terminals: BTreeMap<u64, Session>,
    layout: Option<Workspaces>,
}

#[derive(Default)]
struct Changes {
    revision: Mutex<u64>,
    ready: Condvar,
}

impl Changes {
    fn notify(&self) {
        let mut revision = self.revision.lock().unwrap();
        *revision = revision.wrapping_add(1);
        self.ready.notify_all();
    }

    fn wait(&self, since: u64, timeout: Duration) -> u64 {
        let revision = self.revision.lock().unwrap();
        let (revision, _) = self
            .ready
            .wait_timeout_while(revision, timeout, |revision| *revision == since)
            .unwrap();
        *revision
    }
}

fn serve_terminal(
    state: &mut State,
    stream: &mut Stream,
    id: u64,
    request: TerminalRequest,
) -> io::Result<()> {
    if matches!(request, TerminalRequest::Close) {
        if let Some(layout) = &mut state.layout {
            layout.remove_session(id);
        }
        // Drop closes the PTY and kills the child; socket EOF alone never enters this path.
        state.terminals.remove(&id);
        return reply(stream, Ok(()));
    }
    let Some(session) = state.terminals.get(&id) else {
        return reply::<()>(stream, Err(io::Error::other("unknown terminal")));
    };
    match request {
        TerminalRequest::Info => reply(stream, Ok(session.info())),
        TerminalRequest::Frame => {
            let screen = session.snapshot();
            let images = session.visible_images(screen.rows_count);
            reply(stream, Ok((screen, images)))
        }
        TerminalRequest::Image(seq) => reply(stream, Ok(session.image(seq))),
        TerminalRequest::Write(bytes) => reply(stream, session.write(&bytes)),
        TerminalRequest::Input(bytes) => reply(stream, session.write_input(&bytes)),
        TerminalRequest::PasteContext(text) => reply(stream, session.paste_context(&text)),
        TerminalRequest::PasteClipboard(text) => reply(stream, session.paste_clipboard(&text)),
        TerminalRequest::KittyKeys => reply(stream, Ok(session.kitty_keys())),
        TerminalRequest::StartSelection(kind, point) => {
            session.start_selection(kind, point);
            reply(stream, Ok(()))
        }
        TerminalRequest::ExtendSelection(point) => {
            session.extend_selection(point);
            reply(stream, Ok(()))
        }
        TerminalRequest::ClearSelection => {
            session.clear_selection();
            reply(stream, Ok(()))
        }
        TerminalRequest::SelectionText => reply(stream, Ok(session.selection_text())),
        TerminalRequest::Wheel(lines, col, row) => reply(stream, session.wheel(lines, col, row)),
        TerminalRequest::ScrollToBottom => reply(stream, Ok(session.scroll_to_bottom())),
        TerminalRequest::Resize(cols, rows) => reply(stream, session.resize(cols, rows)),
        TerminalRequest::Close => unreachable!(),
    }
}

fn serve_controller(
    mut stream: Stream,
    state: &Mutex<State>,
    changes: &Arc<Changes>,
    running: &AtomicBool,
) -> io::Result<()> {
    let attachment = {
        let state = state.lock().unwrap();
        Attachment {
            terminals: state.terminals.values().map(Session::info).collect(),
            layout: state.layout.clone(),
        }
    };
    reply(&mut stream, Ok(attachment))?;
    while let Ok(request) = read_frame::<Request>(&mut stream) {
        let mut state = state.lock().unwrap();
        if !running.load(Ordering::Acquire) {
            break;
        }
        match request {
            Request::SaveLayout(layout) => {
                state.layout = Some(layout);
                reply(&mut stream, Ok(()))?;
            }
            Request::Spawn {
                command,
                environment,
            } => {
                let result = match state.terminals.entry(command.id) {
                    std::collections::btree_map::Entry::Occupied(_) => {
                        Err(io::Error::other("terminal id already exists"))
                    }
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        let changes = Arc::clone(changes);
                        command
                            .start_with_environment(
                                Arc::new(move || changes.notify()),
                                Some(environment),
                            )
                            .map(|session| {
                                session.serve_images_on_demand();
                                let info = session.info();
                                entry.insert(session);
                                info
                            })
                    }
                };
                changes.notify();
                reply(&mut stream, result)?;
            }
            Request::Terminal(id, operation) => {
                serve_terminal(&mut state, &mut stream, id, operation)?
            }
        }
    }
    Ok(())
}

fn maintain(state: &Mutex<State>, changes: &Changes, running: &AtomicBool) {
    let mut revision = 0;
    let mut interval = WATCH_TIMEOUT;
    let mut last_scan = Instant::now() - FRAME_INTERVAL;
    while running.load(Ordering::Acquire) {
        revision = changes.wait(revision, interval);
        if !running.load(Ordering::Acquire) {
            break;
        }
        thread::sleep(FRAME_INTERVAL.saturating_sub(last_scan.elapsed()));
        let now = Instant::now();
        last_scan = now;
        let mut flushed = false;
        interval = WATCH_TIMEOUT;
        for session in state.lock().unwrap().terminals.values() {
            session.reap_if_exited();
            let (changed, deadline) = session.update_sync_timeout(now);
            flushed |= changed;
            if let Some(deadline) = deadline {
                interval = interval.min(deadline.saturating_duration_since(now));
            }
        }
        if flushed {
            changes.notify();
        }
    }
}

fn accept_role(stream: &mut Stream) -> io::Result<Role> {
    let _ = stream.set_recv_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_send_timeout(Some(IO_TIMEOUT));
    let mut protocol = [0; PROTOCOL.len()];
    stream.read_exact(&mut protocol)?;
    stream.write_all(PROTOCOL)?;
    if &protocol != PROTOCOL {
        return Err(io::Error::other("incompatible icy session client protocol"));
    }
    let role = read_frame(stream)?;
    let _ = stream.set_recv_timeout(None);
    Ok(role)
}

/// Keeps PTYs and the last terminal layout alive until explicitly stopped.
pub fn serve(dir: &Path) -> io::Result<()> {
    platform::prepare_session_dir(dir)?;
    let _server_lock = lock_file(dir, "server.lock")?;
    let listener = platform::listen_session_socket(&dir.join("pty.sock"))?;
    let state = Arc::new(Mutex::new(State::default()));
    let changes = Arc::new(Changes::default());
    let running = Arc::new(AtomicBool::new(true));
    let (worker_state, worker_changes, worker_running) = (
        Arc::clone(&state),
        Arc::clone(&changes),
        Arc::clone(&running),
    );
    let worker = thread::Builder::new()
        .name("icy-session-maintenance".into())
        .spawn(move || maintain(&worker_state, &worker_changes, &worker_running))?;
    let result = loop {
        let mut stream = match listener.accept() {
            Ok(stream) => stream,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => break Err(error),
        };
        let Ok(role) = accept_role(&mut stream) else {
            continue;
        };
        match role {
            Role::Stop => {
                running.store(false, Ordering::Release);
                state.lock().unwrap().terminals.clear();
                break reply(&mut stream, Ok(()));
            }
            Role::Watch => {
                let changes = Arc::clone(&changes);
                let running = Arc::clone(&running);
                thread::spawn(move || {
                    while running.load(Ordering::Acquire)
                        && let Ok(since) = read_frame(&mut stream)
                    {
                        let revision = changes.wait(since, WATCH_TIMEOUT);
                        if !running.load(Ordering::Acquire)
                            || write_frame(&mut stream, &revision).is_err()
                        {
                            break;
                        }
                    }
                });
            }
            Role::Control => {
                let state = Arc::clone(&state);
                let changes = Arc::clone(&changes);
                let running = Arc::clone(&running);
                thread::spawn(move || {
                    let _ = serve_controller(stream, &state, &changes, &running);
                });
            }
        }
    };
    running.store(false, Ordering::Release);
    changes.notify();
    let _ = worker.join();
    result
}
