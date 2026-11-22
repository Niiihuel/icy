//! Native JSNIR window adapted from `jsnir-gpu`'s MIT-licensed host so raw keyboard
//! input routes directly to the active PTY whenever no TSX text field has focus.
use crate::view::View;
use jsnir::{
    editor::EditorEvent,
    host::{Cursor, Host},
};
use jsnir_gpu::{
    Result,
    input::{Clicks, Clipboard, ImeArea, KeyInput, editor_key, ime_event},
    surface::{self, Surface},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{CursorIcon, Window, WindowId},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowShortcut {
    NewSession,
    CloseSession,
    NextTab,
    PrevTab,
}

pub type PtyInputCallback = Box<dyn Fn(&[u8]) + Send>;
pub type ShortcutCallback = Box<dyn Fn(WindowShortcut) + Send>;
pub type ViewportCallback = Box<dyn Fn(f32, f32) + Send>;

pub struct WindowHooks {
    pub on_pty_input: PtyInputCallback,
    pub on_shortcut: ShortcutCallback,
    pub on_viewport: ViewportCallback,
}

enum Message {
    Push(String),
    Image {
        source: String,
        rgba: Vec<u8>,
        size: [u32; 2],
    },
    #[allow(dead_code)]
    Font(Vec<u8>),
    #[allow(dead_code)]
    DefaultFamily(String),
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

    pub fn image(&self, source: String, rgba: Vec<u8>, size: [u32; 2]) -> bool {
        self.send(Message::Image { source, rgba, size })
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
    ime_area: ImeArea,
    window_focused: bool,
    ime_enabled: bool,
    wake: Option<Instant>,
    inbox: Receiver<Message>,
    shared: Arc<Shared>,
    hooks: WindowHooks,
    error: Option<Box<dyn std::error::Error>>,
}

impl AppWindow {
    fn new(
        host: Host,
        title: &str,
        size: [f32; 2],
        inbox: Receiver<Message>,
        shared: Arc<Shared>,
        hooks: WindowHooks,
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
            ime_area: ImeArea::default(),
            window_focused: true,
            ime_enabled: false,
            wake: None,
            inbox,
            shared,
            hooks,
            error: None,
        }
    }

    fn finish(self) -> Result<()> {
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
        self.wake = None;
        self.host.poll();
        let Some(surface) = &mut self.surface else {
            return;
        };
        let size = surface.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
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
        let scale = surface.window.scale_factor() as f32;
        let pixels = [frame.texture.width(), frame.texture.height()];
        let [width, height] = pixels.map(|side| side as f32 / scale);
        let max_side = surface.renderer.device.limits().max_texture_dimension_2d as usize;
        let scene = self.host.frame(width, height, scale, max_side);
        let draws = self
            .view
            .draws(&mut self.host, &surface.renderer, scene.items, scale);
        if let (true, Some(area)) = (self.ime_enabled, scene.ime_area) {
            self.ime_area.update(&surface.window, area);
        }
        surface.renderer.draw(&frame.texture, pixels, scale, &draws);
        surface.window.pre_present_notify();
        surface.renderer.queue.present(frame);
        if reconfigure {
            surface.resize();
        }
        if scene.animating || reconfigure {
            surface.window.request_redraw();
        }
    }

    fn cursor(&mut self, position: Option<PhysicalPosition<f64>>) {
        let Some(surface) = &self.surface else { return };
        let point = position.map(|p| {
            let p = p.to_logical::<f32>(surface.window.scale_factor());
            [p.x, p.y]
        });
        let icon = match self.host.pointer_move(point) {
            Cursor::Default => CursorIcon::Default,
            Cursor::Pointer => CursorIcon::Pointer,
            Cursor::Text => CursorIcon::Text,
            Cursor::NotAllowed => CursorIcon::NotAllowed,
        };
        surface.window.set_cursor(icon);
        self.request_redraw();
    }

    fn wheel(&mut self, delta: MouseScrollDelta) {
        let Some(surface) = &self.surface else { return };
        let changed = match delta {
            MouseScrollDelta::LineDelta(_, y) => self.host.wheel_smooth(-y * 40.),
            MouseScrollDelta::PixelDelta(p) => self
                .host
                .wheel(-(p.y / surface.window.scale_factor()) as f32),
        };
        if changed {
            self.request_redraw();
        }
    }

    fn mouse(&mut self, state: ElementState, point: Option<[f32; 2]>) {
        match state {
            ElementState::Pressed => {
                let select = self
                    .clicks
                    .select(point.unwrap_or_default(), self.modifiers.shift_key());
                self.host.pointer_down(select);
            }
            ElementState::Released => self.host.pointer_up(),
        }
        self.sync_ime();
        self.request_redraw();
    }

    fn keyboard(&mut self, key: KeyEvent) {
        if key.state == ElementState::Released {
            return;
        }

        // Global workspace shortcuts (Ctrl+Shift+T / W, Ctrl+Tab).
        if self.modifiers.control_key() && !self.modifiers.alt_key() {
            if matches!(key.logical_key, Key::Named(NamedKey::Tab)) {
                let shortcut = if self.modifiers.shift_key() {
                    WindowShortcut::PrevTab
                } else {
                    WindowShortcut::NextTab
                };
                (self.hooks.on_shortcut)(shortcut);
                return;
            }
            if self.modifiers.shift_key()
                && let Key::Character(c) = &key.logical_key
            {
                match c.to_ascii_lowercase().as_str() {
                    "t" => {
                        (self.hooks.on_shortcut)(WindowShortcut::NewSession);
                        return;
                    }
                    "w" => {
                        (self.hooks.on_shortcut)(WindowShortcut::CloseSession);
                        return;
                    }
                    "v" if !self.host.focused() => {
                        if let Some(text) = self.clipboard.text() {
                            (self.hooks.on_pty_input)(text.as_bytes());
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }

        // If a TSX TextInput (e.g. Sidebar Search) has focus, route keys to JSNIR's editor.
        if self.host.focused() {
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

        // Escape dismisses open popovers/tooltips AND forwards ESC to the active CLI.
        if matches!(key.logical_key, Key::Named(NamedKey::Escape)) {
            self.host.escape();
            self.request_redraw();
        }

        if let Some(bytes) = key_to_vt_bytes(&key, self.modifiers) {
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
                let phys = surface.window.inner_size();
                let logical_w = phys.width as f32 / scale;
                let logical_h = phys.height as f32 / scale;
                self.instance = Some(instance);
                self.surface = Some(surface);
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
            WindowEvent::MouseWheel { delta, .. } => self.wheel(delta),
            WindowEvent::Focused(focused) => {
                self.window_focused = focused;
                if !focused {
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
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
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

    fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
        for message in self.inbox.try_iter() {
            match message {
                Message::Push(text) => self.host.push(text),
                Message::Image { source, rgba, size } => self.host.set_image(source, rgba, size),
                Message::Font(bytes) => self.host.add_font(bytes),
                Message::DefaultFamily(family) => self.host.set_default_family(family),
            }
        }
        self.request_redraw();
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
        events.set_control_flow(self.wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
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
    start(Pusher {
        tx,
        proxy: event_loop.create_proxy(),
        shared: shared.clone(),
    });
    host.poll();
    let mut window = AppWindow::new(
        host,
        title,
        size.map(|side| side as f32),
        inbox,
        shared,
        hooks,
    );
    event_loop.run_app(&mut window)?;
    window.finish()
}
