use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zbus::{
    blocking::{Connection, Proxy, connection::Builder, fdo::DBusProxy},
    interface,
    object_server::SignalEmitter,
    zvariant::{OwnedObjectPath, OwnedValue, Structure, Value},
};

use super::{TrayCallbacks, TrayIcon, TrayState};

const WATCHER_SERVICE: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const ITEM_PATH: &str = "/StatusNotifierItem";
const MENU_PATH: &str = "/MenuBar";
const MENU_STATUS: i32 = 1;
const MENU_SEP_TOP: i32 = 2;
const MENU_OPEN: i32 = 3;
const MENU_SEP_BOTTOM: i32 = 4;
const MENU_QUIT: i32 = 5;
const WATCHER_POLL: Duration = Duration::from_secs(5);

type PixmapEntry = (i32, i32, Vec<u8>);
type ToolTipTuple = (String, Vec<PixmapEntry>, String, String);
type MenuLayoutNode = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

struct SharedState {
    tray: TrayState,
    revision: u32,
}

enum Command {
    Update(TrayState),
    Shutdown,
}

pub(super) struct LinuxTray {
    tx: mpsc::Sender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl LinuxTray {
    pub(super) fn start(
        icon: Option<TrayIcon>,
        initial: TrayState,
        callbacks: TrayCallbacks,
    ) -> Option<Self> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("icy-tray".into())
            .spawn(move || run_tray(icon, initial, callbacks, rx, ready_tx))
            .ok()?;
        match ready_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(true) => Some(Self {
                tx,
                thread: Some(thread),
            }),
            _ => {
                let _ = tx.send(Command::Shutdown);
                None
            }
        }
    }

    pub(super) fn update(&self, state: TrayState) {
        let _ = self.tx.send(Command::Update(state));
    }
}

impl Drop for LinuxTray {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_tray(
    icon: Option<TrayIcon>,
    initial: TrayState,
    callbacks: TrayCallbacks,
    rx: mpsc::Receiver<Command>,
    ready: mpsc::Sender<bool>,
) {
    let shared = Arc::new(Mutex::new(SharedState {
        tray: initial,
        revision: 1,
    }));
    let callbacks = Arc::new(callbacks);
    let pixmap = icon
        .map(|icon| vec![(icon.width as i32, icon.height as i32, icon.to_argb_be())])
        .unwrap_or_default();

    let bus_name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    let item = StatusNotifierItem {
        shared: Arc::clone(&shared),
        callbacks: Arc::clone(&callbacks),
        pixmap,
    };
    let menu = DbusMenu {
        shared: Arc::clone(&shared),
        callbacks: Arc::clone(&callbacks),
    };

    let Ok(connection) = Builder::session()
        .and_then(|builder| builder.name(bus_name.as_str()))
        .and_then(|builder| builder.serve_at(ITEM_PATH, item))
        .and_then(|builder| builder.serve_at(MENU_PATH, menu))
        .map(|builder| builder.method_timeout(Duration::from_secs(3)))
        .and_then(Builder::build)
    else {
        let _ = ready.send(false);
        return;
    };

    let mut watcher_owner = watcher_bus_owner(&connection);
    if watcher_owner.is_some() {
        register_with_watcher(&connection, &bus_name);
    }
    let _ = ready.send(true);

    loop {
        match rx.recv_timeout(WATCHER_POLL) {
            Ok(Command::Update(next)) => {
                let revision = {
                    let mut state = shared.lock().unwrap();
                    if state.tray == next {
                        continue;
                    }
                    state.tray = next;
                    state.revision = state.revision.wrapping_add(1).max(1);
                    state.revision
                };
                emit_updates(&connection, revision);
            }
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let current_owner = watcher_bus_owner(&connection);
                if current_owner.is_some() && current_owner != watcher_owner {
                    register_with_watcher(&connection, &bus_name);
                }
                watcher_owner = current_owner;
            }
        }
    }
}

fn watcher_bus_owner(connection: &Connection) -> Option<String> {
    let proxy = DBusProxy::new(connection).ok()?;
    proxy
        .get_name_owner(WATCHER_SERVICE.try_into().ok()?)
        .ok()
        .map(|owner| owner.to_string())
}

fn register_with_watcher(connection: &Connection, bus_name: &str) {
    if let Ok(proxy) = Proxy::new(connection, WATCHER_SERVICE, WATCHER_PATH, WATCHER_SERVICE) {
        let _: zbus::Result<()> = proxy.call("RegisterStatusNotifierItem", &(bus_name,));
    }
}

fn emit_updates(connection: &Connection, revision: u32) {
    if let Ok(iface) = connection
        .object_server()
        .interface::<_, StatusNotifierItem>(ITEM_PATH)
    {
        let _ = zbus::block_on(StatusNotifierItem::new_tool_tip(iface.signal_emitter()));
    }
    if let Ok(iface) = connection
        .object_server()
        .interface::<_, DbusMenu>(MENU_PATH)
    {
        let _ = zbus::block_on(DbusMenu::layout_updated(
            iface.signal_emitter(),
            revision,
            0,
        ));
    }
}

struct StatusNotifierItem {
    shared: Arc<Mutex<SharedState>>,
    callbacks: Arc<TrayCallbacks>,
    pixmap: Vec<PixmapEntry>,
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    #[zbus(property)]
    fn category(&self) -> String {
        "ApplicationStatus".into()
    }

    #[zbus(property)]
    fn id(&self) -> String {
        "icy".into()
    }

    #[zbus(property)]
    fn title(&self) -> String {
        "icy".into()
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "Active".into()
    }

    #[zbus(property)]
    fn window_id(&self) -> u32 {
        0
    }

    #[zbus(property)]
    fn icon_name(&self) -> String {
        if self.pixmap.is_empty() {
            "icy".into()
        } else {
            String::new()
        }
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<PixmapEntry> {
        self.pixmap.clone()
    }

    #[zbus(property)]
    fn overlay_icon_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    fn overlay_icon_pixmap(&self) -> Vec<PixmapEntry> {
        Vec::new()
    }

    #[zbus(property)]
    fn attention_icon_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    fn attention_icon_pixmap(&self) -> Vec<PixmapEntry> {
        Vec::new()
    }

    #[zbus(property)]
    fn attention_movie_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    fn tool_tip(&self) -> ToolTipTuple {
        let status = self.shared.lock().unwrap().tray.status_label();
        (String::new(), Vec::new(), "icy".into(), status)
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn menu(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(MENU_PATH).expect("valid dbusmenu path")
    }

    fn activate(&self, _x: i32, _y: i32) {
        let on_open = Arc::clone(&self.callbacks.on_open);
        let _ = thread::Builder::new()
            .name("icy-tray-open".into())
            .spawn(move || on_open());
    }

    fn secondary_activate(&self, _x: i32, _y: i32) {}

    fn context_menu(&self, _x: i32, _y: i32) {}

    fn scroll(&self, _delta: i32, _orientation: &str) {}

    #[zbus(signal)]
    async fn new_title(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn new_icon(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn new_tool_tip(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn new_status(emitter: &SignalEmitter<'_>, status: &str) -> zbus::Result<()>;
}

fn owned_str(value: &str) -> OwnedValue {
    OwnedValue::try_from(Value::from(value)).expect("string variant")
}

fn owned_bool(value: bool) -> OwnedValue {
    OwnedValue::from(value)
}

fn item_properties(id: i32, status_label: &str) -> HashMap<String, OwnedValue> {
    let mut props = HashMap::new();
    match id {
        0 => {
            props.insert("children-display".into(), owned_str("submenu"));
        }
        MENU_STATUS => {
            props.insert("type".into(), owned_str("standard"));
            props.insert("label".into(), owned_str(status_label));
            props.insert("enabled".into(), owned_bool(false));
            props.insert("visible".into(), owned_bool(true));
        }
        MENU_SEP_TOP | MENU_SEP_BOTTOM => {
            props.insert("type".into(), owned_str("separator"));
            props.insert("visible".into(), owned_bool(true));
        }
        MENU_OPEN => {
            props.insert("type".into(), owned_str("standard"));
            props.insert("label".into(), owned_str("Open icy"));
            props.insert("enabled".into(), owned_bool(true));
            props.insert("visible".into(), owned_bool(true));
        }
        MENU_QUIT => {
            props.insert("type".into(), owned_str("standard"));
            props.insert("label".into(), owned_str("Quit icy"));
            props.insert("enabled".into(), owned_bool(true));
            props.insert("visible".into(), owned_bool(true));
        }
        _ => {}
    }
    props
}

fn filter_properties(
    mut props: HashMap<String, OwnedValue>,
    names: &[String],
) -> HashMap<String, OwnedValue> {
    if !names.is_empty() {
        props.retain(|key, _| names.iter().any(|name| name == key));
    }
    props
}

fn menu_child_value(id: i32, status_label: &str, names: &[String]) -> OwnedValue {
    let props = filter_properties(item_properties(id, status_label), names);
    let children: Vec<OwnedValue> = Vec::new();
    let structure = Structure::from((id, props, children));
    OwnedValue::try_from(Value::from(structure)).expect("dbusmenu child variant")
}

struct DbusMenu {
    shared: Arc<Mutex<SharedState>>,
    callbacks: Arc<TrayCallbacks>,
}

#[interface(name = "com.canonical.dbusmenu")]
impl DbusMenu {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn text_direction(&self) -> String {
        "ltr".into()
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "normal".into()
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }

    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<String>,
    ) -> (u32, MenuLayoutNode) {
        let state = self.shared.lock().unwrap();
        let revision = state.revision;
        let status = state.tray.status_label();
        drop(state);

        let props = filter_properties(item_properties(parent_id, &status), &property_names);
        let children = if parent_id == 0 && recursion_depth != 0 {
            [
                MENU_STATUS,
                MENU_SEP_TOP,
                MENU_OPEN,
                MENU_SEP_BOTTOM,
                MENU_QUIT,
            ]
            .into_iter()
            .map(|id| menu_child_value(id, &status, &property_names))
            .collect()
        } else {
            Vec::new()
        };
        (revision, (parent_id, props, children))
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        property_names: Vec<String>,
    ) -> Vec<(i32, HashMap<String, OwnedValue>)> {
        let status = self.shared.lock().unwrap().tray.status_label();
        ids.into_iter()
            .map(|id| {
                (
                    id,
                    filter_properties(item_properties(id, &status), &property_names),
                )
            })
            .collect()
    }

    fn get_property(&self, id: i32, name: String) -> zbus::fdo::Result<OwnedValue> {
        let status = self.shared.lock().unwrap().tray.status_label();
        item_properties(id, &status)
            .remove(&name)
            .ok_or_else(|| zbus::fdo::Error::Failed(format!("unknown property {name}")))
    }

    fn event(&self, id: i32, event_id: String, _data: OwnedValue, _timestamp: u32) {
        if event_id != "clicked" {
            return;
        }
        match id {
            MENU_OPEN => {
                let on_open = Arc::clone(&self.callbacks.on_open);
                let _ = thread::Builder::new()
                    .name("icy-tray-open".into())
                    .spawn(move || on_open());
            }
            MENU_QUIT => {
                let on_quit = Arc::clone(&self.callbacks.on_quit);
                let _ = thread::Builder::new()
                    .name("icy-tray-quit".into())
                    .spawn(move || on_quit());
            }
            _ => {}
        }
    }

    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        for (id, event_id, data, timestamp) in events {
            self.event(id, event_id, data, timestamp);
        }
        Vec::new()
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (Vec::new(), Vec::new())
    }

    #[zbus(signal)]
    async fn layout_updated(
        emitter: &SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn items_properties_updated(
        emitter: &SignalEmitter<'_>,
        updated_props: Vec<(i32, HashMap<String, OwnedValue>)>,
        removed_props: Vec<(i32, Vec<String>)>,
    ) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_valid_dbusmenu_layout_variants() {
        let status = "Running in background · 2 active sessions";
        let open = menu_child_value(MENU_OPEN, status, &[]);
        assert_eq!(open.value_signature().to_string(), "(ia{sv}av)");
        let props = item_properties(MENU_STATUS, status);
        assert!(props.contains_key("label"));
        assert!(props.contains_key("enabled"));
    }
}
