//! Native JSNIR window adapted from `jsnir-gpu`'s MIT-licensed host so raw keyboard
//! input routes directly to the active PTY whenever no TSX text field has focus.
use crate::browser;
use icy_core::keybinds::{Chord, Keymap, canonical_key};
use icy_core::platform::editor_command_modifier;
use icy_core::vt::{CellPoint, SelectKind, modified_enter};
use jsnir::{
    editor::{EditorEvent, PointSelection},
    host::{Cursor, Host},
};
use jsnir_gpu::{
    Result,
    input::{Clicks, Clipboard, ImeArea, KeyInput, editor_key, ime_event},
    surface::{self, Surface},
    view::View,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::{Duration, Instant},
};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{CursorIcon, Window, WindowId},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowShortcut {
    /// A keymap action id (`icy_core::keybinds::ACTIONS`).
    Action(&'static str),
    /// Input routed to a TSX modal (image viewer, overview) while `MODAL_CAPTURE` is on.
    Modal(&'static str),
    /// The chord recorded for Settings while `KEY_CAPTURE` is on (empty = cancelled).
    Captured(String),
}

/// The active keymap, rebuilt by the host whenever the config's overrides change.
pub static KEYMAP: std::sync::LazyLock<std::sync::RwLock<Keymap>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(Keymap::default()));

/// Settings is recording a shortcut: the next chord is reported instead of being handled.
pub static KEY_CAPTURE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The chord a key press makes, or `None` for modifier keys alone. Keys are read without
/// modifiers (Shift+, is ","); with Ctrl+Alt a key whose layout produces another character
/// (AltGr on Windows) is left for typing.
fn chord_of(key: &KeyEvent, mods: ModifiersState) -> Option<Chord> {
    let base = key.key_without_modifiers();
    let name = match &base {
        Key::Named(named) => match named {
            NamedKey::ArrowLeft => "Left".to_string(),
            NamedKey::ArrowRight => "Right".to_string(),
            NamedKey::ArrowUp => "Up".to_string(),
            NamedKey::ArrowDown => "Down".to_string(),
            NamedKey::Tab => "Tab".to_string(),
            NamedKey::Enter => "Enter".to_string(),
            NamedKey::Escape => "Escape".to_string(),
            NamedKey::Space => "Space".to_string(),
            NamedKey::Home => "Home".to_string(),
            NamedKey::End => "End".to_string(),
            NamedKey::PageUp => "PageUp".to_string(),
            NamedKey::PageDown => "PageDown".to_string(),
            NamedKey::Backspace => "Backspace".to_string(),
            NamedKey::Delete => "Delete".to_string(),
            NamedKey::Insert => "Insert".to_string(),
            other => {
                let text = format!("{other:?}");
                // F1..F24 only; other named keys (modifiers, media keys) are not chords.
                if text.starts_with('F') && text[1..].parse::<u8>().is_ok() {
                    text
                } else {
                    return None;
                }
            }
        },
        Key::Character(c) => {
            if mods.control_key()
                && mods.alt_key()
                && let Key::Character(typed) = &key.logical_key
                && !typed.eq_ignore_ascii_case(c)
            {
                return None;
            }
            c.to_string()
        }
        _ => return None,
    };
    Some(Chord {
        ctrl: mods.control_key(),
        alt: mods.alt_key(),
        shift: mods.shift_key(),
        key: canonical_key(&name)?,
    })
}

/// While a TSX modal that owns the keyboard is open (the image viewer), keys, wheel and touchpad
/// pinch become `WindowShortcut::Modal` actions instead of reaching the PTY or scroll boxes.
pub static MODAL_CAPTURE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static PALETTE_CAPTURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub static OVERVIEW_CAPTURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub static PROJECT_DIALOG: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub static HORIZONTAL_SCROLL_CAPTURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// The hovered Metrics chart's node, or zero when no chart needs pointer positions.
pub static METRICS_POINTER_TARGET: AtomicU16 = AtomicU16::new(0);
pub static AGENT_SHORTCUT_COUNT: AtomicU16 = AtomicU16::new(0);

fn only_alt(modifiers: ModifiersState) -> bool {
    modifiers.alt_key()
        && !modifiers.control_key()
        && !modifiers.super_key()
        && !modifiers.shift_key()
}

/// The terminal grid under the pointer, set by the TSX pane on hover: the wheel over it scrolls that
/// session (whether or not it is focused) instead of the tiling or any scroll box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalWheelTarget {
    pub session: u64,
    /// Top-left of the cell grid, in logical window pixels.
    pub origin: [f32; 2],
    /// Cell width and height in logical pixels.
    pub cell: [f32; 2],
    /// The pointed row, known by the pane: panes stack through paint offsets the layout's `y`
    /// does not include, so the row is not derived from `origin`.
    pub row: u16,
    /// The grid's node: its drawn rect (with those offsets) places a selection drag.
    pub grid: u16,
    /// First cell's offset inside the grid node, in logical pixels.
    pub pad: [f32; 2],
}

/// A mouse selection in a terminal (see `on_terminal_select`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TerminalSelect {
    Start(SelectKind, CellPoint),
    Extend(CellPoint),
    /// Returns the selected text, if any.
    Copy,
    Clear,
}

/// The terminal holding the mouse selection; `held` while the button is down.
#[derive(Clone, Copy)]
struct TermSelection {
    target: TerminalWheelTarget,
    held: bool,
    last: CellPoint,
    dragged: bool,
}

pub static TERMINAL_WHEEL: std::sync::Mutex<Option<TerminalWheelTarget>> =
    std::sync::Mutex::new(None);

/// The browser tile under the pointer, set by its pane on hover. `node` is the pane's body, whose
/// drawn rectangle places the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserPointer {
    pub id: u64,
    pub node: u16,
}

pub static BROWSER_POINTER: Mutex<Option<BrowserPointer>> = Mutex::new(None);
/// The focused browser tile (0 = none): keys reach its page unless the TSX has text focus.
pub static BROWSER_FOCUS: AtomicU64 = AtomicU64::new(0);

/// Lines a wheel notch scrolls in a terminal (xterm, alacritty and most terminals use 3).
const TERMINAL_LINES_PER_NOTCH: f32 = 3.0;

fn modal_capture() -> bool {
    MODAL_CAPTURE.load(Ordering::Relaxed) && !PALETTE_CAPTURE.load(Ordering::Relaxed)
}

pub type PtyInputCallback = Box<dyn Fn(&[u8]) + Send>;
/// `false` leaves a keymap shortcut to the focused pane (e.g. Ctrl+L in a terminal).
pub type ShortcutCallback = Box<dyn Fn(WindowShortcut) -> bool + Send>;
pub type ViewportCallback = Box<dyn Fn(f32, f32) + Send>;
/// Wheel over a terminal: session id, lines (> 0 up) and the pointed cell (column, row).
pub type TerminalWheelCallback = Box<dyn Fn(u64, i32, u16, u16) + Send>;
/// Selection in a terminal session; `Copy` answers with the selected text.
pub type TerminalSelectCallback = Box<dyn Fn(u64, TerminalSelect) -> Option<String> + Send>;

pub struct WindowHooks {
    pub on_pty_input: PtyInputCallback,
    pub on_pty_paste: Box<dyn Fn(&str) + Send>,
    pub on_shortcut: ShortcutCallback,
    pub on_viewport: ViewportCallback,
    pub on_terminal_wheel: TerminalWheelCallback,
    pub on_terminal_select: TerminalSelectCallback,
    /// Whether the focused terminal's program turned on the kitty keyboard protocol.
    pub kitty_keys: Box<dyn Fn() -> bool + Send>,
}

enum Message {
    Push(String),
    /// A whole-state push: JSNIR replaces a queued one with the same key (see `push_keyed`).
    PushKeyed(&'static str, String),
    Image {
        source: String,
        rgba: Vec<u8>,
        size: [u32; 2],
    },
    #[allow(dead_code)]
    Font(Vec<u8>),
    #[allow(dead_code)]
    DefaultFamily(String),
    Browser(browser::Command),
    Quit,
}

type ScaleCallback = Box<dyn Fn(f32) + Send>;

struct Shared {
    scale: AtomicU32,
    on_scale: Mutex<Option<ScaleCallback>>,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            scale: AtomicU32::new(1f32.to_bits()),
            on_scale: Mutex::new(None),
        }
    }
}

impl Shared {
    fn scale(&self) -> f32 {
        f32::from_bits(self.scale.load(Ordering::Relaxed))
    }

    fn set_scale(&self, scale: f32) {
        self.scale.store(scale.to_bits(), Ordering::Relaxed);
        if let Some(on_scale) = self.on_scale.lock().unwrap().as_ref() {
            on_scale(scale);
        }
    }
}

#[derive(Clone)]
pub struct Pusher {
    tx: Sender<Message>,
    proxy: EventLoopProxy<()>,
    shared: Arc<Shared>,
}

impl Pusher {
    #[allow(dead_code)]
    pub fn scale(&self) -> f32 {
        self.shared.scale()
    }

    pub fn on_scale(&self, f: impl Fn(f32) + Send + 'static) {
        *self.shared.on_scale.lock().unwrap() = Some(Box::new(f));
    }

    pub fn push(&self, json: String) -> bool {
        self.send(Message::Push(json))
    }

    /// Like `push`, but a newer push with the same `key` replaces one not yet delivered. Frames
    /// stall while the window is hidden; queuing every terminal frame meanwhile made the app
    /// replay them all on return instead of showing the current state.
    pub fn push_keyed(&self, key: &'static str, json: String) -> bool {
        self.send(Message::PushKeyed(key, json))
    }

    pub fn image(&self, source: String, rgba: Vec<u8>, size: [u32; 2]) -> bool {
        self.send(Message::Image { source, rgba, size })
    }

    pub fn quit(&self) -> bool {
        self.send(Message::Quit)
    }

    pub fn browser(&self, command: browser::Command) -> bool {
        self.send(Message::Browser(command))
    }

    /// Wakes the event loop without queueing a redraw (used by CEF's message pump scheduler).
    pub fn wake(&self) {
        let _ = self.proxy.send_event(());
    }

    fn send(&self, message: Message) -> bool {
        self.tx.send(message).is_ok() && self.proxy.send_event(()).is_ok()
    }
}

struct AppWindow {
    instance: Option<wgpu::Instance>,
    surface: Option<Surface>,
    host: Host,
    view: View,
    title: String,
    size: [f32; 2],
    last_logical_size: [f32; 2],
    modifiers: ModifiersState,
    clicks: Clicks,
    clipboard: Clipboard,
    /// Wheel / pinch accumulator for modal zoom steps.
    modal_scroll: f32,
    /// Pointer in logical window pixels, for terminal wheel input and chart hover.
    pointer: Option<[f32; 2]>,
    metrics_pointer: Option<(u16, [f32; 4])>,
    release_glow: crate::release_glow::Sampler,
    /// Touchpad pixels not yet worth a whole terminal line.
    terminal_scroll: f32,
    ime_area: ImeArea,
    window_focused: bool,
    ime_enabled: bool,
    wake: Option<Instant>,
    /// `ICY_FRAME_STATS=1`: per-second render timings on stderr.
    stats: Option<FrameStats>,
    inbox: Receiver<Message>,
    term_selection: Option<TermSelection>,
    shared: Arc<Shared>,
    hooks: WindowHooks,
    error: Option<Box<dyn std::error::Error>>,
    browsers: browser::Browsers,
    /// The browser whose page holds the left button (a press that began on it).
    browser_held: Option<BrowserPointer>,
    browser_hover: Option<u64>,
    /// Click count of the held press, repeated on its release.
    browser_clicks: i32,
}

impl AppWindow {
    fn new(
        host: Host,
        title: &str,
        size: [f32; 2],
        inbox: Receiver<Message>,
        shared: Arc<Shared>,
        hooks: WindowHooks,
        browsers: browser::Browsers,
    ) -> Self {
        Self {
            instance: None,
            surface: None,
            host,
            view: View::default(),
            title: title.into(),
            size,
            last_logical_size: [0., 0.],
            modifiers: ModifiersState::empty(),
            clicks: Clicks::default(),
            clipboard: Clipboard::default(),
            modal_scroll: 0.0,
            pointer: None,
            metrics_pointer: None,
            release_glow: crate::release_glow::Sampler::default(),
            terminal_scroll: 0.0,
            ime_area: ImeArea::default(),
            window_focused: true,
            ime_enabled: false,
            wake: None,
            stats: std::env::var_os("ICY_FRAME_STATS").map(|_| FrameStats::new()),
            inbox,
            term_selection: None,
            shared,
            hooks,
            error: None,
            browsers,
            browser_held: None,
            browser_hover: None,
            browser_clicks: 1,
        }
    }

    fn finish(mut self) -> Result<()> {
        self.browsers.shutdown();
        self.error.map_or(Ok(()), Err)
    }

    fn request_redraw(&self) {
        if let Some(surface) = &self.surface {
            surface.window.request_redraw();
        }
    }

    fn notify_viewport(&mut self, width: f32, height: f32) {
        if width <= 0. || height <= 0. {
            return;
        }
        if (self.last_logical_size[0] - width).abs() < 0.5
            && (self.last_logical_size[1] - height).abs() < 0.5
        {
            return;
        }
        self.last_logical_size = [width, height];
        (self.hooks.on_viewport)(width, height);
        self.host.push(format!(
            r#"{{"view":"viewport","data":[{width:.1},{height:.1}]}}"#
        ));
    }

    fn resize(&mut self) {
        if let Some(surface) = &mut self.surface {
            surface.resize();
            let scale = surface.window.scale_factor() as f32;
            let phys = surface.window.inner_size();
            let logical_w = phys.width as f32 / scale;
            let logical_h = phys.height as f32 / scale;
            self.notify_viewport(logical_w, logical_h);
        }
        self.request_redraw();
    }

    fn set_scale(&mut self, scale: f32) {
        self.shared.set_scale(scale);
        self.browsers.set_scale(scale);
        self.host.push(format!(r#"{{"jsnirScale":{scale}}}"#));
    }

    fn sync_ime(&mut self) {
        let enabled = self.window_focused && self.host.focused();
        if enabled != self.ime_enabled
            && let Some(surface) = &self.surface
        {
            surface.window.set_ime_allowed(enabled);
            self.ime_enabled = enabled;
            self.ime_area.forget();
        }
    }

    fn edit(&mut self, events: &[EditorEvent]) {
        let output = self.host.edit(events);
        if let Some(text) = output.clipboard {
            self.clipboard.set_text(text);
        }
        if output.interrupt_ime
            && self.ime_enabled
            && let Some(surface) = &self.surface
        {
            self.ime_area.forget();
            surface.window.set_ime_allowed(false);
            surface.window.set_ime_allowed(true);
        }
        self.request_redraw();
    }

    fn redraw(&mut self, events: &ActiveEventLoop) {
        let started = Instant::now();
        self.wake = None;
        self.browsers.flush_frames(&mut self.host);
        self.host.poll();
        let Some(surface) = &mut self.surface else {
            return;
        };
        let size = surface.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let acquire = Instant::now();
        let (frame, reconfigure) = match surface.target.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.wake = Some(Instant::now() + Duration::from_millis(16));
                return;
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                self.wake = Some(Instant::now() + Duration::from_millis(250));
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                surface.resize();
                surface.window.request_redraw();
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                match Surface::new(
                    self.instance.as_ref().expect("instance"),
                    surface.window.clone(),
                ) {
                    Ok(next) => {
                        *surface = next;
                        self.view.reset();
                        self.host.reset_images();
                        surface.window.request_redraw();
                    }
                    Err(error) => {
                        self.error = Some(error);
                        events.exit();
                    }
                }
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                self.error = Some("wgpu rejected the window surface".into());
                events.exit();
                return;
            }
        };
        let acquired = Instant::now();
        let scale = surface.window.scale_factor() as f32;
        let pixels = [frame.texture.width(), frame.texture.height()];
        let [width, height] = pixels.map(|side| side as f32 / scale);
        let max_side = surface.renderer.device.limits().max_texture_dimension_2d as usize;
        let scene = self.host.frame(width, height, scale, max_side);
        let laid_out = Instant::now();
        let draws = self
            .view
            .draws(&mut self.host, &mut surface.renderer, scene.items, scale);
        if let (true, Some(area)) = (self.ime_enabled, scene.ime_area) {
            self.ime_area.update(&surface.window, area);
        }
        let prepared = Instant::now();
        surface.renderer.draw(&frame.texture, pixels, scale, &draws);
        surface.window.pre_present_notify();
        surface.renderer.queue.present(frame);
        if let Some(stats) = &mut self.stats {
            let done = Instant::now();
            stats.record([
                acquire - started,
                acquired - acquire,
                laid_out - acquired,
                prepared - laid_out,
                done - prepared,
            ]);
        }
        if reconfigure {
            surface.resize();
        }
        if scene.animating || reconfigure {
            surface.window.request_redraw();
        }
        self.sync_metrics_pointer();
        self.sync_release_glow_pointer();
        self.browsers.sync_sizes(&self.host);
        self.sync_ime();
    }

    fn sync_metrics_pointer(&mut self) {
        let id = METRICS_POINTER_TARGET.load(Ordering::Relaxed);
        let next = self.pointer.filter(|_| id != 0).and_then(|[x, y]| {
            // The drawn rectangle includes the pane's transforms and the dashboard's scroll.
            let [left, top, width, height] = self.host.screen_rect(id)?;
            Some((id, [x - left, y - top, width, height]))
        });
        if next != self.metrics_pointer {
            self.metrics_pointer = next;
            if let Some((id, [x, y, width, height])) = next {
                self.host.push(
                    serde_json::json!({ "view": "metricsPointer", "data": [id, x, y, width, height] })
                        .to_string(),
                );
            }
        }
    }

    /// The Release notes row's hover frames (see release_glow.rs); keeps redrawing while hovered.
    fn sync_release_glow_pointer(&mut self) {
        if self.release_glow.sample(&mut self.host, self.pointer) {
            self.request_redraw();
        }
    }

    fn cursor(&mut self, position: Option<PhysicalPosition<f64>>) {
        let Some(surface) = &self.surface else { return };
        let point = position.map(|p| {
            let p = p.to_logical::<f32>(surface.window.scale_factor());
            [p.x, p.y]
        });
        self.pointer = point;
        if let (Some(mut selection), Some(point)) = (self.term_selection, point)
            && selection.held
        {
            let cell = self.term_cell(&selection.target, point);
            if cell != selection.last {
                (self.hooks.on_terminal_select)(
                    selection.target.session,
                    TerminalSelect::Extend(cell),
                );
                selection.last = cell;
                selection.dragged = true;
                self.term_selection = Some(selection);
            }
        }
        let icon = match self.host.pointer_move(point) {
            Cursor::Default => CursorIcon::Default,
            Cursor::Pointer => CursorIcon::Pointer,
            Cursor::Text => CursorIcon::Text,
            Cursor::NotAllowed => CursorIcon::NotAllowed,
            // Drag / resize shapes added to JSNIR's contract with pointer drag events.
            Cursor::Grab => CursorIcon::Grab,
            Cursor::Grabbing => CursorIcon::Grabbing,
            Cursor::Move => CursorIcon::Move,
            Cursor::ColResize => CursorIcon::ColResize,
            Cursor::RowResize => CursorIcon::RowResize,
            Cursor::EwResize => CursorIcon::EwResize,
            Cursor::NsResize => CursorIcon::NsResize,
            Cursor::NeswResize => CursorIcon::NeswResize,
            Cursor::NwseResize => CursorIcon::NwseResize,
        };
        let browser = point.and_then(|p| self.browser_hit(p));
        let hover = browser.map(|(pointer, _)| pointer.id);
        if hover != self.browser_hover {
            if let Some(old) = self.browser_hover {
                self.browsers.mouse_move(
                    old,
                    [0., 0.],
                    browser::cef_modifiers(self.modifiers, false),
                    true,
                );
            }
            self.browser_hover = hover;
        }
        let icon = match browser {
            Some((pointer, local)) => {
                let mods = browser::cef_modifiers(self.modifiers, self.browser_held.is_some());
                self.browsers.mouse_move(pointer.id, local, mods, false);
                match browser::BROWSER_CURSOR.load(Ordering::Relaxed) {
                    1 => CursorIcon::Pointer,
                    2 => CursorIcon::Text,
                    _ => CursorIcon::Default,
                }
            }
            None => icon,
        };
        surface.window.set_cursor(icon);
        self.request_redraw();
    }

    fn wheel(&mut self, delta: MouseScrollDelta) {
        let Some(surface) = &self.surface else { return };
        let (x, y, smooth) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x * 40., y * 40., true),
            MouseScrollDelta::PixelDelta(p) => {
                let p = p.to_logical::<f32>(surface.window.scale_factor());
                (p.x, p.y, false)
            }
        };
        // Winit deltas describe the movement of the content. The viewport offset moves the
        // other way, just as it does for JSNIR's vertical wheel handling.
        let palette = PALETTE_CAPTURE.load(Ordering::Relaxed);
        let overview = !palette && OVERVIEW_CAPTURE.load(Ordering::Relaxed);
        let overlay = palette || overview;
        let shifted_wheel = !palette
            && self.modifiers.shift_key()
            && x == 0.
            && (overview || HORIZONTAL_SCROLL_CAPTURE.load(Ordering::Relaxed));
        let horizontal = x.abs() > y.abs() || shifted_wheel;
        if !overlay && let Some((pointer, local)) = self.pointer.and_then(|p| self.browser_hit(p)) {
            // Chromium expects standard 120 units per wheel notch for smooth native scrolling.
            let (bx, by) = match delta {
                MouseScrollDelta::LineDelta(lx, ly) => (lx * 120., ly * 120.),
                MouseScrollDelta::PixelDelta(p) => {
                    let p = p.to_logical::<f32>(surface.window.scale_factor());
                    (p.x, p.y)
                }
            };
            let mods = browser::cef_modifiers(self.modifiers, false);
            self.browsers.wheel(
                pointer.id,
                local,
                mods,
                bx.round() as i32,
                by.round() as i32,
            );
            return;
        }
        if !overlay && !horizontal && self.terminal_wheel(delta) {
            return;
        }
        if horizontal {
            let delta = if x == 0. { -y } else { -x };
            if delta.is_finite() && delta != 0. && !overlay {
                self.host.push(format!(
                    r#"{{"view":"horizontalScroll","data":{{"delta":{delta},"smooth":{smooth}}}}}"#
                ));
                self.request_redraw();
            }
            return;
        }
        let changed = if smooth {
            self.host.wheel_smooth(-y)
        } else {
            self.host.wheel(-y)
        };
        if changed {
            self.request_redraw();
        }
    }

    /// A vertical wheel over a terminal grid: whole lines to that session. Returns whether it took it.
    fn terminal_wheel(&mut self, delta: MouseScrollDelta) -> bool {
        let Some(target) = *TERMINAL_WHEEL.lock().unwrap() else {
            self.terminal_scroll = 0.0;
            return false;
        };
        let Some(point) = self.pointer else {
            return false;
        };
        let cell_h = target.cell[1].max(1.0);
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => (y * TERMINAL_LINES_PER_NOTCH).round(),
            MouseScrollDelta::PixelDelta(p) => {
                let scale = self
                    .surface
                    .as_ref()
                    .map_or(1.0, |s| s.window.scale_factor());
                self.terminal_scroll += p.to_logical::<f32>(scale).y / cell_h;
                let whole = self.terminal_scroll.trunc();
                self.terminal_scroll -= whole;
                whole
            }
        };
        if lines != 0.0 {
            // The drawn rect, not `origin`: tiled columns are placed by a paint offset.
            let col = self.term_cell(&target, point).col;
            (self.hooks.on_terminal_wheel)(target.session, lines as i32, col, target.row);
        }
        true
    }

    /// The cell under `point` in a terminal grid; outside it, the nearest edge cell.
    fn term_cell(&self, target: &TerminalWheelTarget, [x, y]: [f32; 2]) -> CellPoint {
        let [left, top] = self
            .host
            .screen_rect(target.grid)
            .map_or(target.origin, |r| {
                [r[0] + target.pad[0], r[1] + target.pad[1]]
            });
        let [cell_w, cell_h] = target.cell.map(|side| side.max(1.0));
        let col = ((x - left) / cell_w).max(0.0);
        CellPoint {
            col: col as u16,
            row: ((y - top) / cell_h).max(0.0) as u16,
            right: col.fract() >= 0.5,
        }
    }

    fn select_in_terminal(&mut self, select: PointSelection, point: Option<[f32; 2]>) {
        let previous = self.term_selection.take();
        let target = *TERMINAL_WHEEL.lock().unwrap();
        let (Some(target), Some(point)) = (target, point) else {
            if let Some(old) = previous {
                (self.hooks.on_terminal_select)(old.target.session, TerminalSelect::Clear);
            }
            return;
        };
        let cell = self.term_cell(&target, point);
        let same = previous.is_some_and(|old| old.target.session == target.session);
        if let Some(old) = previous.filter(|_| !same) {
            (self.hooks.on_terminal_select)(old.target.session, TerminalSelect::Clear);
        }
        let action = match select {
            PointSelection::Extend if same => TerminalSelect::Extend(cell),
            PointSelection::Word => TerminalSelect::Start(SelectKind::Word, cell),
            PointSelection::All => TerminalSelect::Start(SelectKind::Line, cell),
            _ => TerminalSelect::Start(SelectKind::Cell, cell),
        };
        (self.hooks.on_terminal_select)(target.session, action);
        self.term_selection = Some(TermSelection {
            target,
            held: true,
            last: cell,
            dragged: false,
        });
    }

    /// The browser under `point` (logical window pixels) and the point in its page's pixels. A
    /// press that began on a browser keeps it, so a drag can leave the rectangle.
    fn browser_hit(&self, point: [f32; 2]) -> Option<(BrowserPointer, [f32; 2])> {
        if PROJECT_DIALOG.load(Ordering::Relaxed)
            || PALETTE_CAPTURE.load(Ordering::Relaxed)
            || OVERVIEW_CAPTURE.load(Ordering::Relaxed)
            || modal_capture()
        {
            return None;
        }
        let pointer = match self.browser_held {
            Some(held) => held,
            None => (*BROWSER_POINTER.lock().unwrap())?,
        };
        let [left, top, width, height] = self.host.screen_rect(pointer.node)?;
        let inside =
            (left..left + width).contains(&point[0]) && (top..top + height).contains(&point[1]);
        (self.browser_held.is_some() || inside)
            .then(|| (pointer, [point[0] - left, point[1] - top]))
    }

    /// The focused browser tile when keys belong to its page: no TSX text field has focus and no
    /// palette, dialog, modal or overview is open.
    fn browser_focus(&self) -> Option<u64> {
        let id = BROWSER_FOCUS.load(Ordering::Relaxed);
        let blocked = self.host.focused()
            || KEY_CAPTURE.load(Ordering::Relaxed)
            || PROJECT_DIALOG.load(Ordering::Relaxed)
            || PALETTE_CAPTURE.load(Ordering::Relaxed)
            || modal_capture()
            || OVERVIEW_CAPTURE.load(Ordering::Relaxed);
        (id != 0 && !blocked).then_some(id)
    }

    fn mouse(&mut self, state: ElementState, point: Option<[f32; 2]>) {
        match state {
            ElementState::Pressed => {
                let select = self
                    .clicks
                    .select(point.unwrap_or_default(), self.modifiers.shift_key());
                let browser_press = point.and_then(|p| self.browser_hit(p));
                if let Some((pointer, local)) = browser_press {
                    self.browser_held = Some(pointer);
                    self.browser_clicks = match select {
                        PointSelection::Word => 2,
                        PointSelection::All => 3,
                        _ => 1,
                    };
                    let mods = browser::cef_modifiers(self.modifiers, true);
                    self.browsers
                        .mouse_click(pointer.id, local, mods, false, self.browser_clicks);
                }
                if !PALETTE_CAPTURE.load(Ordering::Relaxed)
                    && !OVERVIEW_CAPTURE.load(Ordering::Relaxed)
                    && browser_press.is_none()
                {
                    self.select_in_terminal(select, point);
                }
                self.host.pointer_down(select);
            }
            ElementState::Released => {
                if let Some(held) = self.browser_held {
                    let local = point
                        .and_then(|p| self.browser_hit(p))
                        .map_or([0., 0.], |(_, local)| local);
                    self.browser_held = None;
                    let mods = browser::cef_modifiers(self.modifiers, false);
                    self.browsers
                        .mouse_click(held.id, local, mods, true, self.browser_clicks);
                }
                let dragged = self.term_selection.is_some_and(|s| s.held && s.dragged);
                if let Some(selection) = &mut self.term_selection {
                    selection.held = false;
                }
                // A selection drag is not a click on the row it ends on (that opens its link).
                if dragged {
                    self.host.pointer_cancel();
                } else {
                    self.host.pointer_up();
                }
            }
        }
        self.sync_ime();
        self.request_redraw();
    }

    fn keyboard(&mut self, key: KeyEvent) {
        let browser_keys = self.browser_focus();
        if key.state == ElementState::Released {
            if let Some(id) = browser_keys {
                self.browsers.key(id, &key, self.modifiers);
            }
            return;
        }
        // Alt used for a chord (Alt+Left, Alt+H…) is not a request for the agent digits.
        if only_alt(self.modifiers)
            && !matches!(
                key.key_without_modifiers(),
                Key::Named(NamedKey::Alt | NamedKey::AltGraph)
            )
            && !matches!(key.key_without_modifiers(), Key::Character(c) if c.len() == 1 && c.as_bytes()[0].is_ascii_digit())
        {
            self.host.push_keyed(
                "agentDigitsHeld",
                r#"{"view":"agentDigitsHeld","data":false}"#.into(),
            );
        }
        // Settings is recording a new shortcut: the next chord goes there (Esc cancels).
        if KEY_CAPTURE.load(Ordering::Relaxed) {
            if let Some(chord) = chord_of(&key, self.modifiers) {
                let plain_escape =
                    chord.key == "Escape" && !chord.ctrl && !chord.alt && !chord.shift;
                (self.hooks.on_shortcut)(WindowShortcut::Captured(if plain_escape {
                    String::new()
                } else {
                    chord.to_string()
                }));
            }
            return;
        }
        let project_dialog = PROJECT_DIALOG.load(Ordering::Relaxed);
        let palette_capture = !project_dialog && PALETTE_CAPTURE.load(Ordering::Relaxed);
        let overview_capture =
            !project_dialog && !palette_capture && OVERVIEW_CAPTURE.load(Ordering::Relaxed);
        if project_dialog {
            // The session launcher's own keys; the rest edit its search field.
            let action = match key.key_without_modifiers() {
                Key::Named(NamedKey::Escape) => Some("escape"),
                Key::Named(NamedKey::ArrowUp) if self.modifiers.is_empty() => Some("up"),
                Key::Named(NamedKey::ArrowDown) if self.modifiers.is_empty() => Some("down"),
                Key::Named(NamedKey::Tab) if self.modifiers.is_empty() => Some("tab"),
                Key::Named(NamedKey::Enter) if !self.host.focused() && !key.repeat => Some("enter"),
                Key::Character(c) if only_alt(self.modifiers) => match c.as_str() {
                    "1" => Some("agent1"),
                    "2" => Some("agent2"),
                    "3" => Some("agent3"),
                    "4" => Some("agent4"),
                    "5" => Some("agent5"),
                    "6" => Some("agent6"),
                    "7" => Some("agent7"),
                    "8" => Some("agent8"),
                    "9" => Some("agent9"),
                    _ => None,
                },
                _ => None,
            };
            if let Some(action) = action {
                (self.hooks.on_shortcut)(WindowShortcut::Modal(action));
                return;
            }
        }
        if overview_capture
            && let Some(id @ ("view.overview" | "panel.side" | "session.search")) =
                chord_of(&key, self.modifiers).and_then(|chord| KEYMAP.read().ok()?.resolve(&chord))
        {
            (self.hooks.on_shortcut)(WindowShortcut::Modal(id));
            return;
        }
        if !project_dialog && (modal_capture() || overview_capture) {
            let action = match &key.logical_key {
                Key::Named(NamedKey::Escape) => Some("escape"),
                Key::Named(NamedKey::Enter) => Some("enter"),
                Key::Named(NamedKey::ArrowLeft) => Some("left"),
                Key::Named(NamedKey::ArrowRight) => Some("right"),
                Key::Named(NamedKey::ArrowUp) => Some("up"),
                Key::Named(NamedKey::ArrowDown) => Some("down"),
                Key::Named(NamedKey::Home) => Some("first"),
                Key::Named(NamedKey::End) => Some("last"),
                Key::Character(c) => match c.as_str() {
                    "+" | "=" => Some("zoomIn"),
                    "-" | "_" => Some("zoomOut"),
                    "0" => Some("reset"),
                    _ => None,
                },
                _ => None,
            };
            if let Some(action) = action {
                (self.hooks.on_shortcut)(WindowShortcut::Modal(action));
            }
            return;
        }

        if palette_capture {
            let action = if let Some(id @ ("panel.side" | "session.search")) =
                chord_of(&key, self.modifiers).and_then(|chord| KEYMAP.read().ok()?.resolve(&chord))
            {
                Some(id)
            } else {
                match key.logical_key {
                    Key::Named(NamedKey::Escape) => Some("escape"),
                    Key::Named(NamedKey::ArrowUp) if self.modifiers.is_empty() => Some("up"),
                    Key::Named(NamedKey::ArrowDown) if self.modifiers.is_empty() => Some("down"),
                    Key::Named(NamedKey::Enter)
                        if !self.host.focused() && !key.repeat && self.modifiers.is_empty() =>
                    {
                        Some("enter")
                    }
                    _ => None,
                }
            };
            if let Some(action) = action {
                self.host
                    .push(format!(r#"{{"view":"modalKey","data":"{action}"}}"#));
                self.request_redraw();
                return;
            }
        }

        // Ctrl+Shift+V pastes into the terminal: a terminal convention, not a remappable action.
        if !project_dialog
            && !palette_capture
            && !self.host.focused()
            && self.modifiers.control_key()
            && self.modifiers.shift_key()
            && !self.modifiers.alt_key()
            && browser_keys.is_none()
            && matches!(&key.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("v"))
        {
            if let Some(text) = self.clipboard.text() {
                self.term_selection = None;
                (self.hooks.on_pty_paste)(&text);
            }
            return;
        }

        // Ctrl+Shift+C copies the terminal selection; Ctrl+C does too while there is one (and
        // clears it), otherwise it stays the interrupt.
        if !project_dialog
            && !palette_capture
            && !self.host.focused()
            && self.modifiers.control_key()
            && !self.modifiers.alt_key()
            && browser_keys.is_none()
            && matches!(&key.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("c"))
        {
            let shift = self.modifiers.shift_key();
            let text = self.term_selection.and_then(|s| {
                (self.hooks.on_terminal_select)(s.target.session, TerminalSelect::Copy)
            });
            if let Some(text) = &text {
                self.clipboard.set_text(text.clone());
            }
            if let (false, Some(_), Some(s)) = (shift, &text, self.term_selection.take()) {
                (self.hooks.on_terminal_select)(s.target.session, TerminalSelect::Clear);
            }
            if shift || text.is_some() {
                return;
            }
        }

        // User-editable shortcuts (Settings › Layout & Shortcuts). Ctrl+Shift chords also work
        // while a text field has focus; the rest only when the keyboard belongs to the terminal.
        if !project_dialog
            && !palette_capture
            && let Some(chord) = chord_of(&key, self.modifiers)
            && (!self.host.focused() || chord.fires_in_text_fields())
            && let Some(id) = KEYMAP.read().ok().and_then(|map| map.resolve(&chord))
            && (self.hooks.on_shortcut)(WindowShortcut::Action(id))
        {
            return;
        }

        // A focused browser tile takes the keys its page needs; the keymap above keeps priority.
        if let Some(id) = browser_keys {
            let command = editor_command_modifier(
                self.modifiers.control_key(),
                self.modifiers.alt_key(),
                self.modifiers.super_key(),
            );
            if command
                && !self.modifiers.shift_key()
                && matches!(&key.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("v"))
            {
                if let Some(text) = self.clipboard.text() {
                    self.browsers.insert_text(id, &text);
                }
            } else {
                self.browsers.key(id, &key, self.modifiers);
            }
            self.request_redraw();
            return;
        }

        if !project_dialog
            && !palette_capture
            && !self.host.focused()
            && only_alt(self.modifiers)
            && let Key::Character(key_name) = key.key_without_modifiers()
            && let [digit @ b'0'..=b'9'] = key_name.as_bytes()
        {
            let digit = digit - b'0';
            let count = AGENT_SHORTCUT_COUNT.load(Ordering::Relaxed);
            if count != 0 && u16::from(digit) <= count {
                if !key.repeat {
                    self.host
                        .push(format!(r#"{{"view":"agentDigit","data":{digit}}}"#));
                    self.request_redraw();
                }
                return;
            }
        }

        // If a TSX TextInput (e.g. Sidebar Search) has focus, route keys to JSNIR's editor.
        if self.host.focused() {
            let command = editor_command_modifier(
                self.modifiers.control_key(),
                self.modifiers.alt_key(),
                self.modifiers.super_key(),
            );
            if !project_dialog
                && !palette_capture
                && !key.repeat
                && chord_of(&key, self.modifiers).and_then(|c| KEYMAP.read().ok()?.resolve(&c))
                    == Some("editor.addToChat")
            {
                self.host
                    .push(r#"{"view":"modalKey","data":"editor.addToChat"}"#.into());
                self.request_redraw();
                return;
            }
            if command
                && !project_dialog
                && !palette_capture
                && !self.modifiers.shift_key()
                && !key.repeat
                && matches!(&key.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("s"))
            {
                self.host.push(r#"{"view":"editorSaveKey"}"#.into());
                self.request_redraw();
                return;
            }
            if matches!(key.logical_key, Key::Named(NamedKey::Escape)) {
                self.host.escape();
                self.sync_ime();
                self.request_redraw();
                return;
            }
            match editor_key(&key, self.modifiers) {
                Some(KeyInput::Edit(event)) => self.edit(&[event]),
                Some(KeyInput::Paste) => {
                    if let Some(text) = self.clipboard.text() {
                        self.edit(&[EditorEvent::Insert(text)]);
                    }
                }
                None => {}
            }
            return;
        }

        if project_dialog || palette_capture {
            return;
        }

        // Escape dismisses open popovers/tooltips AND forwards ESC to the active CLI.
        if matches!(key.logical_key, Key::Named(NamedKey::Escape)) {
            self.host.escape();
            self.request_redraw();
        }

        let (shift, alt, ctrl) = (
            self.modifiers.shift_key(),
            self.modifiers.alt_key(),
            self.modifiers.control_key(),
        );
        let bytes =
            if matches!(key.logical_key, Key::Named(NamedKey::Enter)) && (shift || alt || ctrl) {
                // Shift+Enter: a newline in the CLI's prompt instead of a submit.
                Some(modified_enter((self.hooks.kitty_keys)(), shift, alt, ctrl))
            } else {
                key_to_vt_bytes(&key, self.modifiers)
            };
        if let Some(bytes) = bytes {
            self.term_selection = None;
            (self.hooks.on_pty_input)(&bytes);
        }
    }
}

impl ApplicationHandler for AppWindow {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.surface.is_some() {
            return;
        }
        let [width, height] = self.size;
        let window = events
            .create_window(
                Window::default_attributes()
                    .with_title(&self.title)
                    .with_inner_size(LogicalSize::new(width, height)),
            )
            .map_err(Box::from)
            .and_then(|window| surface::open(&events.owned_display_handle(), Arc::new(window)));
        match window {
            Ok((instance, surface)) => {
                let scale = surface.window.scale_factor() as f32;
                let refresh_hz = surface
                    .window
                    .current_monitor()
                    .and_then(|m| m.refresh_rate_millihertz())
                    .map(|mhz| ((mhz + 500) / 1000) as i32)
                    .unwrap_or(60);
                let phys = surface.window.inner_size();
                let logical_w = phys.width as f32 / scale;
                let logical_h = phys.height as f32 / scale;
                self.instance = Some(instance);
                self.surface = Some(surface);
                self.browsers.set_refresh_rate(refresh_hz);
                self.set_scale(scale);
                self.notify_viewport(logical_w, logical_h);
                self.sync_ime();
                self.request_redraw();
            }
            Err(error) => {
                self.error = Some(error);
                events.exit();
            }
        }
    }

    fn window_event(&mut self, events: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if !self
            .surface
            .as_ref()
            .is_some_and(|surface| surface.window.id() == id)
        {
            return;
        }
        match event {
            WindowEvent::CloseRequested => events.exit(),
            WindowEvent::Resized(_) => self.resize(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.set_scale(scale_factor as f32);
                self.resize();
            }
            WindowEvent::RedrawRequested => self.redraw(events),
            WindowEvent::CursorMoved { position, .. } => self.cursor(Some(position)),
            WindowEvent::CursorLeft { .. } => self.cursor(None),
            WindowEvent::MouseWheel { delta, .. } if modal_capture() => {
                // Line wheels zoom one step per notch; touchpads accumulate ~40px per step.
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                self.modal_scroll += dy;
                while self.modal_scroll.abs() >= 40.0 {
                    let up = self.modal_scroll > 0.0;
                    self.modal_scroll -= 40.0_f32.copysign(self.modal_scroll);
                    (self.hooks.on_shortcut)(WindowShortcut::Modal(if up {
                        "zoomIn"
                    } else {
                        "zoomOut"
                    }));
                }
            }
            WindowEvent::PinchGesture { delta, .. } if modal_capture() => {
                self.modal_scroll += delta as f32 * 400.0;
                while self.modal_scroll.abs() >= 40.0 {
                    let grow = self.modal_scroll > 0.0;
                    self.modal_scroll -= 40.0_f32.copysign(self.modal_scroll);
                    (self.hooks.on_shortcut)(WindowShortcut::Modal(if grow {
                        "zoomIn"
                    } else {
                        "zoomOut"
                    }));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => self.wheel(delta),
            WindowEvent::Focused(focused) => {
                self.window_focused = focused;
                if !focused {
                    self.host.push_keyed(
                        "agentDigitsHeld",
                        r#"{"view":"agentDigitsHeld","data":false}"#.into(),
                    );
                    self.edit(&[EditorEvent::Preedit {
                        text: String::new(),
                        active: None,
                    }]);
                }
                self.sync_ime();
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let point = self.host.pointer();
                self.mouse(state, point);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                self.host.context_press();
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let was_alt = only_alt(self.modifiers);
                self.modifiers = modifiers.state();
                let alt = only_alt(self.modifiers);
                if was_alt != alt {
                    self.host.push_keyed(
                        "agentDigitsHeld",
                        format!(r#"{{"view":"agentDigitsHeld","data":{alt}}}"#),
                    );
                    self.request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => self.keyboard(event),
            WindowEvent::Ime(event) => {
                if self.host.focused()
                    && let Some(event) = ime_event(event)
                {
                    self.edit(&[event]);
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, events: &ActiveEventLoop, _: ()) {
        let mut redraw = false;
        for message in self.inbox.try_iter() {
            redraw = true;
            match message {
                Message::Push(text) => self.host.push(text),
                Message::PushKeyed(key, text) => self.host.push_keyed(key, text),
                Message::Image { source, rgba, size } => self.host.set_image(source, rgba, size),
                Message::Font(bytes) => self.host.add_font(bytes),
                Message::DefaultFamily(family) => self.host.set_default_family(family),
                Message::Browser(command) => {
                    if let browser::Command::Close(id) = command {
                        self.host.remove_image(&format!("browser:{id}"));
                    }
                    self.browsers.apply(command);
                }
                Message::Quit => {
                    events.exit();
                    return;
                }
            }
        }
        if redraw {
            self.request_redraw();
        }
    }

    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        let now = Instant::now();
        if self.wake.is_some_and(|wake| now >= wake) {
            self.wake = None;
            self.request_redraw();
        }
        if self.wake.is_none() && self.host.busy() {
            self.wake = Some(now + Duration::from_millis(4));
        }
        // Event-driven CEF pump: wakes only when Chromium schedules work (never forcing redraws
        // unless CEF actually painted a new frame).
        let cef_deadline = self.browsers.pump_due(now);
        if self.browsers.has_dirty_frames() {
            self.request_redraw();
        }
        let next = match (self.wake, cef_deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        events.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}

/// Translates a winit key press into VT/ANSI terminal bytes.
pub fn key_to_vt_bytes(key: &KeyEvent, mods: ModifiersState) -> Option<Vec<u8>> {
    let ctrl = mods.control_key();
    let alt = mods.alt_key();
    let shift = mods.shift_key();

    match &key.logical_key {
        Key::Named(named) => {
            let seq: &[u8] = match named {
                NamedKey::Enter => b"\r",
                NamedKey::Backspace => {
                    if ctrl {
                        b"\x08"
                    } else if alt {
                        b"\x1b\x7f"
                    } else {
                        b"\x7f"
                    }
                }
                NamedKey::Tab => {
                    if shift {
                        b"\x1b[Z"
                    } else {
                        b"\t"
                    }
                }
                NamedKey::Escape => b"\x1b",
                NamedKey::Space => {
                    if ctrl {
                        b"\x00"
                    } else if alt {
                        b"\x1b "
                    } else {
                        b" "
                    }
                }
                NamedKey::ArrowUp => arrow_seq(b'A', ctrl, alt, shift),
                NamedKey::ArrowDown => arrow_seq(b'B', ctrl, alt, shift),
                NamedKey::ArrowRight => arrow_seq(b'C', ctrl, alt, shift),
                NamedKey::ArrowLeft => arrow_seq(b'D', ctrl, alt, shift),
                NamedKey::Home => b"\x1b[H",
                NamedKey::End => b"\x1b[F",
                NamedKey::Insert => b"\x1b[2~",
                NamedKey::Delete => b"\x1b[3~",
                NamedKey::PageUp => b"\x1b[5~",
                NamedKey::PageDown => b"\x1b[6~",
                NamedKey::F1 => b"\x1bOP",
                NamedKey::F2 => b"\x1bOQ",
                NamedKey::F3 => b"\x1bOR",
                NamedKey::F4 => b"\x1bOS",
                NamedKey::F5 => b"\x1b[15~",
                NamedKey::F6 => b"\x1b[17~",
                NamedKey::F7 => b"\x1b[18~",
                NamedKey::F8 => b"\x1b[19~",
                NamedKey::F9 => b"\x1b[20~",
                NamedKey::F10 => b"\x1b[21~",
                NamedKey::F11 => b"\x1b[23~",
                NamedKey::F12 => b"\x1b[24~",
                _ => return None,
            };
            Some(seq.to_vec())
        }
        Key::Character(ch) => {
            if ctrl {
                let b = ch.as_bytes().first().copied()?;
                let lower = b.to_ascii_lowercase();
                let ctrl_byte = match lower {
                    b'a'..=b'z' => Some(lower - b'a' + 1),
                    b'[' => Some(0x1b),
                    b'\\' => Some(0x1c),
                    b']' => Some(0x1d),
                    b'^' | b'6' => Some(0x1e),
                    b'_' | b'-' => Some(0x1f),
                    b' ' | b'@' => Some(0x00),
                    _ => None,
                }?;
                return Some(if alt {
                    vec![0x1b, ctrl_byte]
                } else {
                    vec![ctrl_byte]
                });
            }
            let text = key.text.as_deref().unwrap_or(ch.as_str());
            if text.is_empty() {
                return None;
            }
            if alt {
                let mut out = Vec::with_capacity(1 + text.len());
                out.push(0x1b);
                out.extend_from_slice(text.as_bytes());
                Some(out)
            } else {
                Some(text.as_bytes().to_vec())
            }
        }
        _ => {
            let text = key.text.as_deref()?;
            if text.is_empty() {
                None
            } else {
                Some(text.as_bytes().to_vec())
            }
        }
    }
}

fn arrow_seq(dir: u8, ctrl: bool, alt: bool, shift: bool) -> &'static [u8] {
    match (dir, ctrl, alt, shift) {
        (b'A', true, false, false) => b"\x1b[1;5A",
        (b'B', true, false, false) => b"\x1b[1;5B",
        (b'C', true, false, false) => b"\x1b[1;5C",
        (b'D', true, false, false) => b"\x1b[1;5D",
        (b'A', false, true, false) => b"\x1b[1;3A",
        (b'B', false, true, false) => b"\x1b[1;3B",
        (b'C', false, true, false) => b"\x1b[1;3C",
        (b'D', false, true, false) => b"\x1b[1;3D",
        (b'A', false, false, true) => b"\x1b[1;2A",
        (b'B', false, false, true) => b"\x1b[1;2B",
        (b'C', false, false, true) => b"\x1b[1;2C",
        (b'D', false, false, true) => b"\x1b[1;2D",
        (b'A', ..) => b"\x1b[A",
        (b'B', ..) => b"\x1b[B",
        (b'C', ..) => b"\x1b[C",
        _ => b"\x1b[D",
    }
}

pub fn open_with(
    mut host: Host,
    title: &str,
    size: [u32; 2],
    hooks: WindowHooks,
    start: impl FnOnce(Pusher),
) -> Result<()> {
    let event_loop = EventLoop::<()>::with_user_event().build()?;
    let (tx, inbox) = channel();
    let shared = Arc::<Shared>::default();
    let pusher = Pusher {
        tx,
        proxy: event_loop.create_proxy(),
        shared: shared.clone(),
    };
    let browsers = browser::Browsers::new(pusher.clone(), shared.scale());
    start(pusher);
    host.poll();
    let mut window = AppWindow::new(
        host,
        title,
        size.map(|side| side as f32),
        inbox,
        shared,
        hooks,
        browsers,
    );
    event_loop.run_app(&mut window)?;
    window.finish()
}

/// Render timings split by stage, printed once per second (`ICY_FRAME_STATS=1`).
struct FrameStats {
    since: Instant,
    last: Option<Instant>,
    frames: u32,
    /// poll, acquire (vsync wait), layout, draw lists, GPU submit+present: sum and max.
    sum: [Duration; 5],
    max: [Duration; 5],
    worst_gap: Duration,
}

impl FrameStats {
    fn new() -> Self {
        Self {
            since: Instant::now(),
            last: None,
            frames: 0,
            sum: [Duration::ZERO; 5],
            max: [Duration::ZERO; 5],
            worst_gap: Duration::ZERO,
        }
    }

    fn record(&mut self, stages: [Duration; 5]) {
        let now = Instant::now();
        if let Some(last) = self.last.replace(now) {
            self.worst_gap = self.worst_gap.max(now - last);
        }
        self.frames += 1;
        for (i, stage) in stages.into_iter().enumerate() {
            self.sum[i] += stage;
            self.max[i] = self.max[i].max(stage);
        }
        if now - self.since < Duration::from_secs(1) {
            return;
        }
        let ms = |d: Duration| d.as_secs_f64() * 1e3;
        let n = f64::from(self.frames);
        let stage = |i: usize| format!("{:.2}/{:.2}", ms(self.sum[i]) / n, ms(self.max[i]));
        eprintln!(
            "icy frames: {} fps, worst gap {:.1}ms | avg/max ms poll {} acquire {} layout {} draws {} gpu {}",
            self.frames,
            ms(self.worst_gap),
            stage(0),
            stage(1),
            stage(2),
            stage(3),
            stage(4),
        );
        *self = Self {
            last: self.last,
            ..Self::new()
        };
    }
}
