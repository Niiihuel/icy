//! Native browser tiles: CEF in windowless (off-screen) mode. Each page is painted into an RGBA
//! buffer that is pushed as the image `browser:{id}`, so a browser tile is an `<Image>` in TSX.
//! Everything here runs on the main thread, where winit and CEF's UI thread are the same.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use cef::*;
use icy_core::browser::address_url;
use jsnir::host::Host;
use serde::Serialize;
use serde_json::Value;
use winit::event::{ElementState, KeyEvent as WinitKeyEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;

use crate::window::Pusher;

/// Opaque dark surface matching `appBg` (`#111214`) in CEF's ARGB format (`0xAARRGGBB`).
/// Keeping alpha = `0xFF` lets Chromium enable LCD subpixel font rendering in OSR mode
/// instead of falling back to grayscale alpha compositing, and avoids white flashes on load.
const OPAQUE_BG: u32 = 0xFF11_1214;
const NO_PUMP_WORK: i64 = i64::MAX;
/// Fallback pump ceiling while browsers are open (CEF docs recommend keeping a safety timer).
const MAX_FALLBACK_PUMP_MS: u64 = 33;
/// Bound how long one `about_to_wait` burst pumps immediate CEF tasks so winit stays responsive.
const MAX_PUMP_BURST: Duration = Duration::from_millis(4);
const MAX_PUMP_STEPS: usize = 6;
/// Minimum interval between resize notifications during a live tiling spring.
const LIVE_RESIZE_INTERVAL: Duration = Duration::from_millis(48);

/// Cursor the page asked for on its last change: 0 default, 1 pointer, 2 text.
pub static BROWSER_CURSOR: AtomicU8 = AtomicU8::new(0);

/// Work the TSX side asks the browsers for, parsed from `handle_request`'s `"browser"` payloads.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Open {
        id: u64,
        url: String,
        size: [i32; 2],
        /// Optional JSNIR layout node for the browser body so the host can keep exact physical
        /// dimensions synced with `one_to_one` rendering.
        node: Option<u16>,
    },
    Navigate {
        id: u64,
        text: String,
    },
    Back(u64),
    Forward(u64),
    Reload(u64),
    Resize {
        id: u64,
        size: [i32; 2],
    },
    Hidden {
        id: u64,
        hidden: bool,
    },
    /// `0` clears the focused browser.
    Focus(u64),
    Close(u64),
}

impl Command {
    pub fn from_request(payload: &Value) -> Option<Self> {
        let op = payload.get("op")?.as_str()?;
        let id = payload.get("id").and_then(Value::as_u64);
        if op == "focus" {
            return Some(Self::Focus(id.unwrap_or(0)));
        }
        let id = id?;
        let text = |key: &str| {
            payload
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let size = || {
            ["width", "height"].map(|key| {
                payload
                    .get(key)
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
                    .clamp(0.0, 16384.0)
                    .round() as i32
            })
        };
        let node = payload
            .get("node")
            .and_then(Value::as_u64)
            .and_then(|n| u16::try_from(n).ok());
        Some(match op {
            "open" => Self::Open {
                id,
                url: match text("url") {
                    url if url.is_empty() => "about:blank".into(),
                    url => url,
                },
                size: size(),
                node,
            },
            "navigate" => Self::Navigate {
                id,
                text: text("text"),
            },
            "back" => Self::Back(id),
            "forward" => Self::Forward(id),
            "reload" => Self::Reload(id),
            "resize" => Self::Resize { id, size: size() },
            "hidden" => Self::Hidden {
                id,
                hidden: payload
                    .get("hidden")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
            "close" => Self::Close(id),
            _ => return None,
        })
    }
}

/// What the TSX bar shows for one browser; pushed as `{"view":"browserState","data":…}`.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserState {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub error: Option<String>,
}

/// Lock-free coalesced deadline shared between `BrowserProcessHandler::on_schedule_message_pump_work`
/// (which CEF invokes from arbitrary threads) and the main winit event loop.
struct PumpSchedule {
    start: Instant,
    due_ms: AtomicI64,
    notified: AtomicBool,
    wake: Box<dyn Fn() + Send + Sync>,
}

impl PumpSchedule {
    fn new(wake: impl Fn() + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            start: Instant::now(),
            due_ms: AtomicI64::new(NO_PUMP_WORK),
            notified: AtomicBool::new(false),
            wake: Box::new(wake),
        })
    }

    fn elapsed_ms(&self, now: Instant) -> i64 {
        now.saturating_duration_since(self.start)
            .as_millis()
            .min((i64::MAX - 10_000) as u128) as i64
    }

    /// Called by CEF from any thread when work needs to be pumped after `delay_ms`.
    fn schedule(&self, delay_ms: i64) {
        let now_ms = self.elapsed_ms(Instant::now());
        let target = now_ms.saturating_add(delay_ms.clamp(0, MAX_FALLBACK_PUMP_MS as i64));
        let prev = self.due_ms.fetch_min(target, Ordering::AcqRel);
        let earlier = target < prev;
        let first = !self.notified.swap(true, Ordering::AcqRel);
        if earlier || first {
            (self.wake)();
        }
    }

    /// After a pump the next one is the safety pump: CEF does not schedule all of its work (a
    /// navigation stalls without it), so cefclient also pumps at least every 33 ms.
    fn clear(&self) {
        let fallback = self.elapsed_ms(Instant::now()) + MAX_FALLBACK_PUMP_MS as i64;
        self.due_ms.store(fallback, Ordering::Release);
        self.notified.store(false, Ordering::Release);
    }

    fn next_deadline(&self, now: Instant) -> Instant {
        let now_ms = self.elapsed_ms(now);
        let due = self.due_ms.load(Ordering::Acquire);
        let wait_ms = if due == NO_PUMP_WORK {
            MAX_FALLBACK_PUMP_MS
        } else {
            due.saturating_sub(now_ms)
                .clamp(0, MAX_FALLBACK_PUMP_MS as i64) as u64
        };
        now + Duration::from_millis(wait_ms)
    }

    fn is_due(&self, now: Instant) -> bool {
        self.due_ms.load(Ordering::Acquire) <= self.elapsed_ms(now)
    }
}

/// The CEF subprocess check. CEF re-executes icy for its helper processes, so this must run
/// before anything else in `main`. `Some(code)` means the process was a CEF helper and must exit.
pub fn run_subprocess() -> Option<i32> {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = cef::args::Args::new();
    let mut app = AppBuilder::new(None);
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    (code >= 0).then_some(code)
}

enum Engine {
    Idle,
    Ready,
    Failed(String),
}

/// Last painted frame of one browser, shared with its CEF handlers.
#[derive(Default)]
struct Pixels {
    view: Vec<u8>,
    view_size: [u32; 2],
    /// Popup (a `<select>` list, say) rectangle in logical pixels, as CEF reports it.
    popup_rect: Option<[i32; 4]>,
    popup: Option<(Vec<u8>, [u32; 2])>,
}

/// State a browser's CEF handlers write to. Handlers hold it through `Rc`; it never holds CEF
/// objects, so dropping a view does not leak the browser.
struct ViewInner {
    /// Size in device pixels reported to CEF via `view_rect` / `screen_info` (see
    /// `view_and_phys_for_rect`).
    size: Cell<[i32; 2]>,
    /// Exact target physical pixels of the body node so `jsnir-gpu`'s `one_to_one` draws 1:1.
    target_phys: Cell<Option<[u32; 2]>>,
    scale: Rc<Cell<f32>>,
    state: RefCell<BrowserState>,
    pixels: RefCell<Pixels>,
    frame_dirty: Cell<bool>,
    any_frame_dirty: Rc<Cell<bool>>,
    pusher: Pusher,
    open_count: Rc<Cell<usize>>,
    /// The pane has the app's focus. CEF drops `set_focus` before its OSR view exists, so the
    /// first paint applies it again.
    wants_focus: Cell<bool>,
    painted: Cell<bool>,
}

impl ViewInner {
    fn push_state(&self) {
        let json = serde_json::json!({ "view": "browserState", "data": &*self.state.borrow() });
        self.pusher.push(json.to_string());
    }

    /// Marks this view as having fresh pixels ready to be uploaded once on the next redraw.
    fn mark_dirty(&self) {
        self.frame_dirty.set(true);
        self.any_frame_dirty.set(true);
    }

    /// Produces the final coalesced RGBA buffer (cropped to `target_phys` for 1:1 display snap,
    /// with any active popup blended on top).
    fn compose_frame(&self) -> Option<(Vec<u8>, [u32; 2])> {
        let pixels = self.pixels.borrow();
        let [vw, vh] = pixels.view_size;
        if vw == 0 || vh == 0 {
            return None;
        }
        // When fractional DPI (`ceil`) makes CEF's buffer 1-2px larger than the target physical
        // rect, crop to exact `target_phys` so `jsnir-gpu` renders 1:1 without bilinear blur.
        let [ow, oh] = match self.target_phys.get() {
            Some([tw, th])
                if tw > 0 && th > 0 && tw <= vw && th <= vh && vw - tw <= 2 && vh - th <= 2 =>
            {
                [tw, th]
            }
            _ => [vw, vh],
        };
        let mut rgba = if [ow, oh] == [vw, vh] {
            pixels.view.clone()
        } else {
            crop_rgba(&pixels.view, [vw, vh], [ow, oh])
        };
        if let (Some(rect), Some((popup, popup_size))) = (pixels.popup_rect, &pixels.popup) {
            blend_popup(&mut rgba, [ow, oh], rect, popup, *popup_size);
        }
        Some((rgba, [ow, oh]))
    }
}

/// The view size CEF lays out in and the exact target physical pixels for a logical layout rect.
/// Both are device pixels: CEF's Linux OSR ignores `ScreenInfo::device_scale_factor` and paints
/// at 1x, so the page gets a 1x screen of device pixels and `page_zoom` restores CSS sizes.
fn view_and_phys_for_rect(logical_w: f32, logical_h: f32, scale: f32) -> ([i32; 2], [u32; 2]) {
    let scale = scale.max(0.5);
    let phys_w = (logical_w * scale).round().max(1.0) as u32;
    let phys_h = (logical_h * scale).round().max(1.0) as u32;
    ([phys_w as i32, phys_h as i32], [phys_w, phys_h])
}

/// CEF's zoom level (`1.2^level` is the page zoom) that makes one CSS pixel `scale` device pixels,
/// as Chromium's zoom-for-DSF does, so `devicePixelRatio`, media queries and `srcset` match.
fn page_zoom(scale: f32) -> f64 {
    f64::from(scale.max(0.5)).ln() / 1.2f64.ln()
}

/// Zoom is per host in Chromium, so a page on a new site starts at 100% until this runs again.
fn apply_page_zoom(host: &BrowserHost, scale: f32) {
    let level = page_zoom(scale);
    if (host.zoom_level() - level).abs() > 1e-6 {
        host.set_zoom_level(level);
    }
}

fn crop_rgba(src: &[u8], src_size: [u32; 2], dst_size: [u32; 2]) -> Vec<u8> {
    let [sw, _] = src_size.map(|s| s as usize);
    let [dw, dh] = dst_size.map(|s| s as usize);
    let mut out = vec![0u8; dw * dh * 4];
    for row in 0..dh {
        let src_start = row * sw * 4;
        let dst_start = row * dw * 4;
        out[dst_start..dst_start + dw * 4].copy_from_slice(&src[src_start..src_start + dw * 4]);
    }
    out
}

/// Copies a popup over the view at its rectangle (view pixels), clipped to the view.
fn blend_popup(
    view: &mut [u8],
    view_size: [u32; 2],
    rect: [i32; 4],
    popup: &[u8],
    popup_size: [u32; 2],
) {
    let [vw, vh] = view_size.map(i64::from);
    let [pw, ph] = popup_size.map(i64::from);
    let [ox, oy] = [rect[0], rect[1]].map(i64::from);
    let cols = (0.max(-ox))..pw.min(vw - ox).max(0);
    if cols.is_empty() {
        return;
    }
    for row in 0..ph {
        let y = oy + row;
        if !(0..vh).contains(&y) {
            continue;
        }
        let src = ((row * pw + cols.start) * 4) as usize;
        let dst = ((y * vw + ox + cols.start) * 4) as usize;
        let len = (cols.end - cols.start) as usize * 4;
        view[dst..dst + len].copy_from_slice(&popup[src..src + len]);
    }
}

/// Converts BGRA8 pixels to opaque RGBA8 in one 32-bit word operation per pixel (auto-vectorizable).
#[inline]
fn convert_bgra_slice_to_rgba(dst: &mut [u8], src: &[u8]) {
    for (d, s) in dst
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(src.as_chunks::<4>().0)
    {
        let px = u32::from_le_bytes(*s);
        let rgba = (px & 0x0000_FF00)
            | ((px & 0x00FF_0000) >> 16)
            | ((px & 0x0000_00FF) << 16)
            | 0xFF00_0000;
        *d = rgba.to_le_bytes();
    }
}

/// Updates `dst` (size `width x height`) from CEF's BGRA `src`, touching only `dirty_rects` when
/// the buffer dimensions match and dirty rects are provided.
fn copy_bgra_dirty_to_rgba(
    dst: &mut Vec<u8>,
    dst_size: &mut [u32; 2],
    src: &[u8],
    size: [u32; 2],
    dirty_rects: &[Rect],
) {
    let [w, h] = size;
    let total = w as usize * h as usize * 4;
    if *dst_size != size || dst.len() != total || dirty_rects.is_empty() {
        dst.resize(total, 0);
        convert_bgra_slice_to_rgba(dst, &src[..total]);
        *dst_size = size;
        return;
    }
    let stride = w as usize * 4;
    let w_i = w as i32;
    let h_i = h as i32;
    for rect in dirty_rects {
        let x0 = rect.x.clamp(0, w_i) as usize;
        let y0 = rect.y.clamp(0, h_i) as usize;
        let x1 = rect.x.saturating_add(rect.width).clamp(0, w_i) as usize;
        let y1 = rect.y.saturating_add(rect.height).clamp(0, h_i) as usize;
        if x0 >= x1 || y0 >= y1 {
            continue;
        }
        let row_bytes = (x1 - x0) * 4;
        if x0 == 0 && x1 == w as usize {
            let start = y0 * stride;
            let end = y1 * stride;
            convert_bgra_slice_to_rgba(&mut dst[start..end], &src[start..end]);
        } else {
            for y in y0..y1 {
                let start = y * stride + x0 * 4;
                let end = start + row_bytes;
                convert_bgra_slice_to_rgba(&mut dst[start..end], &src[start..end]);
            }
        }
    }
}

fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut rgba = vec![0u8; bgra.len()];
    convert_bgra_slice_to_rgba(&mut rgba, bgra);
    rgba
}

pub struct Browsers {
    engine: Engine,
    views: HashMap<u64, View>,
    scale: Rc<Cell<f32>>,
    refresh_rate: i32,
    pusher: Pusher,
    pump: Arc<PumpSchedule>,
    open_count: Rc<Cell<usize>>,
    any_frame_dirty: Rc<Cell<bool>>,
}

struct View {
    browser: cef::Browser,
    inner: Rc<ViewInner>,
    /// JSNIR body node for live 1:1 physical-pixel tracking (absent for short-lived overlays).
    node: Option<u16>,
    /// Last physical size seen during an animation, so settling commits on the very next frame.
    pending_phys: Option<[u32; 2]>,
    last_resize: Instant,
}

impl Browsers {
    pub fn new(pusher: Pusher, scale: f32) -> Self {
        let wake_pusher = pusher.clone();
        let pump = PumpSchedule::new(move || wake_pusher.wake());
        Self {
            engine: Engine::Idle,
            views: HashMap::new(),
            scale: Rc::new(Cell::new(scale)),
            refresh_rate: 60,
            pusher,
            pump,
            open_count: Rc::new(Cell::new(0)),
            any_frame_dirty: Rc::new(Cell::new(false)),
        }
    }

    /// Whether CEF runs, so the window must pump it.
    pub fn active(&self) -> bool {
        // Browsers still closing count too: CEF needs the pump to finish them.
        matches!(self.engine, Engine::Ready) && self.open_count.get() > 0
    }

    /// Whether any browser painted new pixels since the last `flush_frames`.
    pub fn has_dirty_frames(&self) -> bool {
        self.any_frame_dirty.get()
    }

    /// Runs scheduled CEF work in a bounded burst and returns the next deadline for the event
    /// loop's `WaitUntil` without forcing unnecessary window redraws.
    pub fn pump_due(&self, now: Instant) -> Option<Instant> {
        if !self.active() {
            return None;
        }
        if self.pump.is_due(now) {
            let started = Instant::now();
            for _ in 0..MAX_PUMP_STEPS {
                self.pump.clear();
                do_message_loop_work();
                if !self.pump.is_due(Instant::now()) || started.elapsed() >= MAX_PUMP_BURST {
                    break;
                }
            }
        }
        Some(self.pump.next_deadline(Instant::now()))
    }

    /// Uploads each dirty browser's latest coalesced frame directly into JSNIR once per redraw.
    pub fn flush_frames(&mut self, host: &mut Host) {
        if !self.any_frame_dirty.replace(false) {
            return;
        }
        for (&id, view) in &self.views {
            if !view.inner.frame_dirty.replace(false) {
                continue;
            }
            if let Some((rgba, size)) = view.inner.compose_frame() {
                host.set_image(format!("browser:{id}"), rgba, size);
            }
        }
    }

    /// Keeps each tracked browser view matched to its JSNIR body node's exact physical pixels,
    /// resizing responsively during springs and snapping to 1:1 the frame the layout settles.
    pub fn sync_sizes(&mut self, host: &Host) {
        if self.views.is_empty() {
            return;
        }
        let scale = self.scale.get().max(0.5);
        let now = Instant::now();
        for view in self.views.values_mut() {
            let Some(node) = view.node else { continue };
            let Some([_, _, w, h]) = host.screen_rect(node) else {
                continue;
            };
            if w <= 0.0 || h <= 0.0 {
                continue;
            }
            let (view_size, target_phys) = view_and_phys_for_rect(w, h, scale);
            if view.inner.target_phys.get() == Some(target_phys) {
                view.pending_phys = None;
                continue;
            }
            let settled = view.pending_phys == Some(target_phys);
            if settled || now.duration_since(view.last_resize) >= LIVE_RESIZE_INTERVAL {
                view.pending_phys = None;
                view.last_resize = now;
                view.inner.size.set(view_size);
                view.inner.target_phys.set(Some(target_phys));
                if let Some(browser_host) = view.browser.host() {
                    browser_host.was_resized();
                }
            } else {
                view.pending_phys = Some(target_phys);
            }
        }
    }

    pub fn apply(&mut self, command: Command) {
        match command {
            Command::Open {
                id,
                url,
                size,
                node,
            } => self.open(id, &url, size, node),
            Command::Navigate { id, text } => {
                if let (Some(view), Some(url)) = (self.views.get(&id), address_url(&text))
                    && let Some(frame) = view.browser.main_frame()
                {
                    frame.load_url(Some(&url.as_str().into()));
                }
            }
            Command::Back(id) => {
                if let Some(view) = self.views.get(&id) {
                    view.browser.go_back();
                }
            }
            Command::Forward(id) => {
                if let Some(view) = self.views.get(&id) {
                    view.browser.go_forward();
                }
            }
            Command::Reload(id) => {
                if let Some(view) = self.views.get(&id) {
                    view.browser.reload();
                }
            }
            Command::Resize { id, size } => {
                if let Some(view) = self.views.get(&id) {
                    let scale = self.scale.get().max(0.5);
                    let (view_size, phys) =
                        view_and_phys_for_rect(size[0] as f32, size[1] as f32, scale);
                    view.inner.size.set(view_size);
                    view.inner.target_phys.set(Some(phys));
                    if let Some(host) = view.browser.host() {
                        host.was_resized();
                    }
                }
            }
            Command::Hidden { id, hidden } => {
                if let Some(host) = self.host(id) {
                    host.was_hidden(hidden as _);
                }
            }
            Command::Focus(id) => {
                for (&view_id, view) in &self.views {
                    view.inner.wants_focus.set(view_id == id);
                    if let Some(host) = view.browser.host() {
                        host.set_focus((view_id == id) as _);
                    }
                }
            }
            Command::Close(id) => {
                if let Some(view) = self.views.remove(&id)
                    && let Some(host) = view.browser.host()
                {
                    host.close_browser(1);
                }
            }
        }
    }

    fn open(&mut self, id: u64, url: &str, size: [i32; 2], node: Option<u16>) {
        let scale = self.scale.get().max(0.5);
        let (view_size, target_phys) = if size[0] > 0 && size[1] > 0 {
            let (view_size, phys) = view_and_phys_for_rect(size[0] as f32, size[1] as f32, scale);
            (view_size, Some(phys))
        } else {
            (size, None)
        };
        if let Some(view) = self.views.get_mut(&id) {
            if node.is_some() {
                view.node = node;
            }
            view.inner.size.set(view_size);
            view.inner.target_phys.set(target_phys);
            if let Some(host) = view.browser.host() {
                host.was_hidden(0);
                host.was_resized();
                host.invalidate(PaintElementType::VIEW);
            }
            return;
        }
        if let Err(error) = self.ensure_started() {
            self.fail(id, url, error);
            return;
        }
        let inner = Rc::new(ViewInner {
            size: Cell::new(view_size),
            target_phys: Cell::new(target_phys),
            scale: self.scale.clone(),
            state: RefCell::new(BrowserState {
                id,
                url: url.to_string(),
                loading: true,
                ..Default::default()
            }),
            pixels: RefCell::new(Pixels::default()),
            frame_dirty: Cell::new(false),
            any_frame_dirty: self.any_frame_dirty.clone(),
            pusher: self.pusher.clone(),
            open_count: self.open_count.clone(),
            wants_focus: Cell::new(false),
            painted: Cell::new(false),
        });
        let window_info = WindowInfo {
            windowless_rendering_enabled: 1,
            ..Default::default()
        };
        let settings = BrowserSettings {
            windowless_frame_rate: self.refresh_rate,
            background_color: OPAQUE_BG,
            ..Default::default()
        };
        let mut client = ClientBuilder::build(inner.clone());
        let browser = browser_host_create_browser_sync(
            Some(&window_info),
            Some(&mut client),
            Some(&url.into()),
            Some(&settings),
            None,
            None,
        );
        match browser {
            Some(browser) => {
                self.open_count.set(self.open_count.get() + 1);
                self.pump.schedule(0);
                inner.push_state();
                self.views.insert(
                    id,
                    View {
                        browser,
                        inner,
                        node,
                        pending_phys: None,
                        last_resize: Instant::now(),
                    },
                );
            }
            None => self.fail(id, url, "Browser could not be created".into()),
        }
    }

    fn fail(&self, id: u64, url: &str, error: String) {
        let state = BrowserState {
            id,
            url: url.to_string(),
            error: Some(error),
            ..Default::default()
        };
        let json = serde_json::json!({ "view": "browserState", "data": state });
        self.pusher.push(json.to_string());
    }

    /// Starts CEF on the first Open. Missing runtime files are reported instead of letting CEF abort.
    fn ensure_started(&mut self) -> Result<(), String> {
        let started = match &self.engine {
            Engine::Ready => return Ok(()),
            Engine::Failed(error) => return Err(error.clone()),
            Engine::Idle => start_engine(self.pump.clone()),
        };
        self.engine = match &started {
            Ok(()) => Engine::Ready,
            Err(error) => Engine::Failed(error.clone()),
        };
        started
    }

    fn host(&self, id: u64) -> Option<BrowserHost> {
        self.views.get(&id)?.browser.host()
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale.set(scale);
        for view in self.views.values_mut() {
            view.inner.target_phys.set(None);
            view.pending_phys = None;
            if let Some(host) = view.browser.host() {
                apply_page_zoom(&host, scale);
                host.was_resized();
            }
        }
    }

    /// Matches CEF's OSR cadence to the active monitor's refresh rate (clamped 60..=144 Hz).
    pub fn set_refresh_rate(&mut self, fps: i32) {
        let fps = fps.clamp(60, 144);
        if self.refresh_rate == fps {
            return;
        }
        self.refresh_rate = fps;
        for view in self.views.values() {
            if let Some(host) = view.browser.host() {
                host.set_windowless_frame_rate(fps);
            }
        }
    }

    /// `point` is in logical pixels of the browser's view; `mods` comes from `cef_modifiers`.
    pub fn mouse_move(&self, id: u64, point: [f32; 2], mods: u32, leave: bool) {
        if let Some(host) = self.host(id) {
            let event = self.mouse_event(point, mods);
            host.send_mouse_move_event(Some(&event), leave as _);
        }
    }

    pub fn mouse_click(&self, id: u64, point: [f32; 2], mods: u32, up: bool, clicks: i32) {
        if let Some(host) = self.host(id) {
            let event = self.mouse_event(point, mods);
            if !up {
                // A press on the page is a focus-in, as in cefclient's OSR windows.
                host.set_focus(1);
            }
            host.send_mouse_click_event(Some(&event), MouseButtonType::LEFT, up as _, clicks);
        }
    }

    /// `dx`/`dy` are logical pixels, like `point`.
    pub fn wheel(&self, id: u64, point: [f32; 2], mods: u32, dx: i32, dy: i32) {
        if let Some(host) = self.host(id) {
            let event = self.mouse_event(point, mods);
            let [dx, dy] = [dx, dy].map(|d| (d as f32 * self.scale.get().max(0.5)).round() as i32);
            host.send_mouse_wheel_event(Some(&event), dx, dy);
        }
    }

    /// A pointer event in the view's device pixels.
    fn mouse_event(&self, [x, y]: [f32; 2], modifiers: u32) -> cef::MouseEvent {
        let scale = self.scale.get().max(0.5);
        cef::MouseEvent {
            x: (x * scale).round() as i32,
            y: (y * scale).round() as i32,
            modifiers,
        }
    }

    /// Forwards one key event: a raw key down/up, plus a char event per typed UTF-16 unit on press.
    pub fn key(&self, id: u64, event: &WinitKeyEvent, mods: ModifiersState) {
        let Some(host) = self.host(id) else { return };
        let windows_key_code = vk_code(&event.key_without_modifiers());
        let mut key = cef::KeyEvent {
            modifiers: cef_modifiers(mods, false),
            windows_key_code,
            ..Default::default()
        };
        if event.state == ElementState::Released {
            key.type_ = KeyEventType::KEYUP;
            host.send_key_event(Some(&key));
            return;
        }
        key.type_ = KeyEventType::RAWKEYDOWN;
        host.send_key_event(Some(&key));
        if mods.control_key() && !mods.alt_key() {
            return;
        }
        for unit in event.text.as_deref().unwrap_or_default().encode_utf16() {
            if unit < 0x20 && unit != 0x0D {
                continue;
            }
            key.type_ = KeyEventType::CHAR;
            key.windows_key_code = unit as i32;
            key.character = unit;
            key.unmodified_character = unit;
            host.send_key_event(Some(&key));
        }
    }

    /// Commits typed text (paste) into the page's focused field.
    pub fn insert_text(&self, id: u64, text: &str) {
        if let Some(host) = self.host(id) {
            host.ime_commit_text(Some(&text.into()), None, 1);
        }
    }

    /// Closes every browser and CEF. Pages get up to 2s to close before CEF stops.
    pub fn shutdown(&mut self) {
        if !self.active() {
            return;
        }
        for view in self.views.values() {
            if let Some(host) = view.browser.host() {
                host.close_browser(1);
            }
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.open_count.get() > 0 && Instant::now() < deadline {
            do_message_loop_work();
            std::thread::sleep(Duration::from_millis(10));
        }
        self.views.clear();
        shutdown();
        self.engine = Engine::Failed("Browser engine stopped".into());
    }
}

/// The directory with libcef.so's resources: `CEF_PATH` when set, else next to the executable.
fn runtime_dir() -> Option<PathBuf> {
    std::env::var_os("CEF_PATH").map(PathBuf::from).or_else(|| {
        let exe = std::env::current_exe().ok()?;
        exe.parent().map(Path::to_path_buf)
    })
}

fn start_engine(pump: Arc<PumpSchedule>) -> Result<(), String> {
    let dir = runtime_dir().ok_or("Browser runtime not found")?;
    if !dir.join("icudtl.dat").is_file() {
        return Err("Browser runtime not found".into());
    }
    // CEF spawns helper subprocesses from the running executable's path; if the binary was
    // replaced on disk (e.g. by a rebuild or self-update), Chromium aborts with SIGTRAP.
    let exe = std::env::current_exe().map_err(|_| "Browser executable not found")?;
    if !exe.is_file() {
        return Err("icy was updated on disk; restart to use the browser".into());
    }
    let args = cef::args::Args::new();
    let settings = Settings {
        no_sandbox: 1,
        windowless_rendering_enabled: 1,
        external_message_pump: 1,
        background_color: OPAQUE_BG,
        root_cache_path: icy_core::platform::config_dir()
            .join("browser")
            .to_string_lossy()
            .as_ref()
            .into(),
        resources_dir_path: dir.to_string_lossy().as_ref().into(),
        locales_dir_path: dir.join("locales").to_string_lossy().as_ref().into(),
        log_severity: LogSeverity::WARNING,
        ..Default::default()
    };
    let mut app = AppBuilder::new(Some(BrowserProcessHandlerBuilder::new(pump)));
    let ready = initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    if ready == 1 {
        Ok(())
    } else {
        Err("Browser engine failed to start".into())
    }
}

wrap_browser_process_handler! {
    struct BrowserProcessHandlerBuilder {
        pump: Arc<PumpSchedule>,
    }

    impl BrowserProcessHandler {
        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            self.pump.schedule(delay_ms);
        }
    }
}

wrap_app! {
    struct AppBuilder {
        browser_process: Option<BrowserProcessHandler>,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            if process_type.is_none_or(|p| p.to_string().is_empty())
                && let Some(cmd) = command_line
            {
                // OSR is already paced by `windowless_frame_rate` and winit's FIFO VSync;
                // Chromium's internal GPU VSync double-waits and halves FPS to 30fps stutter.
                cmd.append_switch(Some(&"disable-gpu-vsync".into()));
                // Windowless views have no native OS window; prevent Chromium from throttling
                // timers/compositing as if the tab were hidden/backgrounded.
                cmd.append_switch(Some(&"disable-renderer-backgrounding".into()));
                cmd.append_switch(Some(&"disable-background-timer-throttling".into()));
                cmd.append_switch(Some(&"enable-smooth-scrolling".into()));
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            self.browser_process.clone()
        }
    }
}

/// CEF modifier flags for a winit modifier state. `left_down` marks a held left button.
pub fn cef_modifiers(mods: ModifiersState, left_down: bool) -> u32 {
    use cef::sys::cef_event_flags_t as flags;
    let mut out = 0;
    if mods.shift_key() {
        out |= flags::EVENTFLAG_SHIFT_DOWN.0;
    }
    if mods.control_key() {
        out |= flags::EVENTFLAG_CONTROL_DOWN.0;
    }
    if mods.alt_key() {
        out |= flags::EVENTFLAG_ALT_DOWN.0;
    }
    if mods.super_key() {
        out |= flags::EVENTFLAG_COMMAND_DOWN.0;
    }
    if left_down {
        out |= flags::EVENTFLAG_LEFT_MOUSE_BUTTON.0;
    }
    #[allow(clippy::unnecessary_cast)]
    {
        out as u32
    }
}

/// Windows virtual-key code of a key, as CEF's raw key events expect; 0 when unmapped.
fn vk_code(key: &Key) -> i32 {
    match key {
        Key::Named(named) => match named {
            NamedKey::Backspace => 0x08,
            NamedKey::Tab => 0x09,
            NamedKey::Enter => 0x0D,
            NamedKey::Shift => 0x10,
            NamedKey::Control => 0x11,
            NamedKey::Alt => 0x12,
            NamedKey::Escape => 0x1B,
            NamedKey::Space => 0x20,
            NamedKey::PageUp => 0x21,
            NamedKey::PageDown => 0x22,
            NamedKey::End => 0x23,
            NamedKey::Home => 0x24,
            NamedKey::ArrowLeft => 0x25,
            NamedKey::ArrowUp => 0x26,
            NamedKey::ArrowRight => 0x27,
            NamedKey::ArrowDown => 0x28,
            NamedKey::Insert => 0x2D,
            NamedKey::Delete => 0x2E,
            other => match format!("{other:?}")
                .strip_prefix('F')
                .and_then(|n| n.parse::<i32>().ok())
                .filter(|n| (1..=24).contains(n))
            {
                Some(n) => 0x70 + n - 1,
                None => 0,
            },
        },
        Key::Character(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => c.to_ascii_uppercase() as i32,
                (Some(c), None) => match c {
                    ';' => 0xBA,
                    '=' => 0xBB,
                    ',' => 0xBC,
                    '-' => 0xBD,
                    '.' => 0xBE,
                    '/' => 0xBF,
                    '`' => 0xC0,
                    '[' => 0xDB,
                    '\\' => 0xDC,
                    ']' => 0xDD,
                    '\'' => 0xDE,
                    _ => 0,
                },
                _ => 0,
            }
        }
        _ => 0,
    }
}

wrap_render_handler! {
    struct RenderBuilder {
        inner: Rc<ViewInner>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                let [width, height] = view_size(&self.inner);
                rect.width = width;
                rect.height = height;
            }
        }

        fn screen_point(
            &self,
            _browser: Option<&mut Browser>,
            view_x: ::std::os::raw::c_int,
            view_y: ::std::os::raw::c_int,
            screen_x: Option<&mut ::std::os::raw::c_int>,
            screen_y: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            // The view is in device pixels on a 1x screen, so view and screen pixels match.
            if let Some(sx) = screen_x {
                *sx = view_x;
            }
            if let Some(sy) = screen_y {
                *sy = view_y;
            }
            1
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> ::std::os::raw::c_int {
            let Some(info) = screen_info else { return 0 };
            let [width, height] = view_size(&self.inner);
            // 1x: the HiDPI scale is the page zoom (see `view_and_phys_for_rect`).
            info.device_scale_factor = 1.0;
            info.depth = 24;
            info.depth_per_component = 8;
            info.is_monochrome = 0;
            info.rect = Rect { x: 0, y: 0, width, height };
            info.available_rect = info.rect.clone();
            1
        }

        fn on_popup_show(&self, _browser: Option<&mut Browser>, show: ::std::os::raw::c_int) {
            if show == 0 {
                let mut pixels = self.inner.pixels.borrow_mut();
                pixels.popup = None;
                pixels.popup_rect = None;
                drop(pixels);
                self.inner.mark_dirty();
            }
        }

        fn on_popup_size(&self, _browser: Option<&mut Browser>, rect: Option<&Rect>) {
            if let Some(rect) = rect {
                self.inner.pixels.borrow_mut().popup_rect =
                    Some([rect.x, rect.y, rect.width, rect.height]);
            }
        }

        fn on_paint(
            &self,
            browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            if !self.inner.painted.replace(true)
                && self.inner.wants_focus.get()
                && let Some(host) = browser.and_then(|b| b.host())
            {
                host.set_focus(1);
            }
            let len = width as usize * height as usize * 4;
            // SAFETY: CEF passes a width * height * 4 BGRA buffer, valid only for this call.
            let bgra = unsafe { std::slice::from_raw_parts(buffer, len) };
            let size = [width as u32, height as u32];
            if type_ == PaintElementType::POPUP {
                self.inner.pixels.borrow_mut().popup = Some((bgra_to_rgba(bgra), size));
                self.inner.mark_dirty();
            } else {
                let mut pixels = self.inner.pixels.borrow_mut();
                let Pixels {
                    view, view_size, ..
                } = &mut *pixels;
                copy_bgra_dirty_to_rgba(view, view_size, bgra, size, dirty_rects.unwrap_or(&[]));
                drop(pixels);
                self.inner.mark_dirty();
            }
        }
    }
}

fn view_size(inner: &ViewInner) -> [i32; 2] {
    let [width, height] = inner.size.get();
    if width > 0 && height > 0 {
        [width, height]
    } else {
        [800, 600]
    }
}

wrap_display_handler! {
    struct DisplayBuilder {
        inner: Rc<ViewInner>,
    }

    impl DisplayHandler {
        fn on_address_change(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            if frame.is_some_and(|frame| frame.is_main() == 1) {
                if let Some(host) = browser.and_then(|b| b.host()) {
                    apply_page_zoom(&host, self.inner.scale.get());
                }
                let mut state = self.inner.state.borrow_mut();
                state.url = url.map(ToString::to_string).unwrap_or_default();
                state.error = None;
                drop(state);
                self.inner.push_state();
            }
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            self.inner.state.borrow_mut().title = title.map(ToString::to_string).unwrap_or_default();
            self.inner.push_state();
        }

        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: cef::sys::cef_cursor_handle_t,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            let cursor = if type_ == CursorType::HAND {
                1
            } else if type_ == CursorType::IBEAM {
                2
            } else {
                0
            };
            BROWSER_CURSOR.store(cursor, Ordering::Relaxed);
            1
        }
    }
}

wrap_load_handler! {
    struct LoadBuilder {
        inner: Rc<ViewInner>,
    }

    impl LoadHandler {
        fn on_loading_state_change(
            &self,
            _browser: Option<&mut Browser>,
            is_loading: ::std::os::raw::c_int,
            can_go_back: ::std::os::raw::c_int,
            can_go_forward: ::std::os::raw::c_int,
        ) {
            {
                let mut state = self.inner.state.borrow_mut();
                state.loading = is_loading == 1;
                state.can_back = can_go_back == 1;
                state.can_forward = can_go_forward == 1;
            }
            self.inner.push_state();
        }

        fn on_load_error(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            // A navigation replaced by a newer one is not an error the user should see.
            if error_code == Errorcode::ABORTED || !frame.is_some_and(|f| f.is_main() == 1) {
                return;
            }
            let text = error_text.map(ToString::to_string).unwrap_or_default();
            let url = failed_url.map(ToString::to_string).unwrap_or_default();
            self.inner.state.borrow_mut().error = Some(format!("{text} {url}").trim().to_string());
            self.inner.push_state();
        }
    }
}

wrap_life_span_handler! {
    struct LifeSpanBuilder {
        inner: Rc<ViewInner>,
    }

    impl LifeSpanHandler {
        // Links that open a new window load in this browser instead.
        fn on_before_popup(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            if let (Some(browser), Some(url)) = (browser, target_url)
                && let Some(frame) = browser.main_frame()
            {
                frame.load_url(Some(url));
            }
            1
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            self.inner.open_count.set(self.inner.open_count.get().saturating_sub(1));
        }
    }
}

wrap_context_menu_handler! {
    struct ContextMenuBuilder {
        inner: Rc<ViewInner>,
    }

    impl ContextMenuHandler {
        // shortcut: no native context menu in tiles yet, so an emptied model shows nothing
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            if let Some(model) = model {
                model.clear();
            }
        }
    }
}

wrap_client! {
    struct ClientBuilder {
        render: RenderHandler,
        display: DisplayHandler,
        load: LoadHandler,
        life_span: LifeSpanHandler,
        context_menu: ContextMenuHandler,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render.clone())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display.clone())
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.load.clone())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life_span.clone())
        }

        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(self.context_menu.clone())
        }
    }
}

impl ClientBuilder {
    fn build(inner: Rc<ViewInner>) -> Client {
        Self::new(
            RenderBuilder::build(inner.clone()),
            DisplayBuilder::build(inner.clone()),
            LoadBuilder::build(inner.clone()),
            LifeSpanBuilder::build(inner.clone()),
            ContextMenuBuilder::build(inner),
        )
    }
}

impl RenderBuilder {
    fn build(inner: Rc<ViewInner>) -> RenderHandler {
        Self::new(inner)
    }
}

impl DisplayBuilder {
    fn build(inner: Rc<ViewInner>) -> DisplayHandler {
        Self::new(inner)
    }
}

impl LoadBuilder {
    fn build(inner: Rc<ViewInner>) -> LoadHandler {
        Self::new(inner)
    }
}

impl LifeSpanBuilder {
    fn build(inner: Rc<ViewInner>) -> LifeSpanHandler {
        Self::new(inner)
    }
}

impl ContextMenuBuilder {
    fn build(inner: Rc<ViewInner>) -> ContextMenuHandler {
        Self::new(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn requests_parse_into_commands() {
        assert_eq!(
            Command::from_request(
                &json!({"op": "open", "id": 7, "url": "http://localhost:3000", "width": 640, "height": 480.4, "node": 42})
            ),
            Some(Command::Open {
                id: 7,
                url: "http://localhost:3000".into(),
                size: [640, 480],
                node: Some(42),
            })
        );
        assert_eq!(
            Command::from_request(&json!({"op": "open", "id": 7})),
            Some(Command::Open {
                id: 7,
                url: "about:blank".into(),
                size: [0, 0],
                node: None,
            })
        );
        assert_eq!(
            Command::from_request(&json!({"op": "navigate", "id": 2, "text": "rust lang"})),
            Some(Command::Navigate {
                id: 2,
                text: "rust lang".into()
            })
        );
        assert_eq!(
            Command::from_request(&json!({"op": "back", "id": 3})),
            Some(Command::Back(3))
        );
        assert_eq!(
            Command::from_request(&json!({"op": "forward", "id": 3})),
            Some(Command::Forward(3))
        );
        assert_eq!(
            Command::from_request(&json!({"op": "reload", "id": 3})),
            Some(Command::Reload(3))
        );
        assert_eq!(
            Command::from_request(&json!({"op": "resize", "id": 4, "width": 100, "height": 50})),
            Some(Command::Resize {
                id: 4,
                size: [100, 50]
            })
        );
        assert_eq!(
            Command::from_request(&json!({"op": "hidden", "id": 5, "hidden": true})),
            Some(Command::Hidden {
                id: 5,
                hidden: true
            })
        );
        assert_eq!(
            Command::from_request(&json!({"op": "focus", "id": 6})),
            Some(Command::Focus(6))
        );
        assert_eq!(
            Command::from_request(&json!({"op": "focus"})),
            Some(Command::Focus(0))
        );
        assert_eq!(
            Command::from_request(&json!({"op": "close", "id": 8})),
            Some(Command::Close(8))
        );
    }

    #[test]
    fn requests_without_an_id_or_known_op_are_rejected() {
        assert_eq!(Command::from_request(&json!({"op": "back"})), None);
        assert_eq!(
            Command::from_request(&json!({"op": "explode", "id": 1})),
            None
        );
        assert_eq!(Command::from_request(&json!({"id": 1})), None);
    }

    #[test]
    fn virtual_key_codes_follow_windows_conventions() {
        let named = |key| vk_code(&Key::Named(key));
        assert_eq!(named(NamedKey::Enter), 0x0D);
        assert_eq!(named(NamedKey::Backspace), 0x08);
        assert_eq!(named(NamedKey::ArrowLeft), 0x25);
        assert_eq!(named(NamedKey::F5), 0x74);
        assert_eq!(vk_code(&Key::Character("a".into())), 0x41);
        assert_eq!(vk_code(&Key::Character("/".into())), 0xBF);
        assert_eq!(vk_code(&Key::Character("é".into())), 0);
    }

    #[test]
    fn popups_are_blended_at_their_clipped_rectangle() {
        // 2x2 black view; a 2x1 white popup at (1, 1) only fits its first column.
        let mut view = vec![0u8; 2 * 2 * 4];
        let popup = vec![255u8; 2 * 4];
        blend_popup(&mut view, [2, 2], [1, 1, 2, 1], &popup, [2, 1]);
        assert_eq!(&view[..12], &[0; 12]);
        assert_eq!(&view[12..16], &[255; 4]);
    }

    #[test]
    fn bgra_frames_become_opaque_rgba() {
        assert_eq!(bgra_to_rgba(&[1, 2, 3, 0]), vec![3, 2, 1, 255]);
    }

    #[test]
    fn dirty_rect_paint_updates_only_changed_pixels() {
        let mut dst = [10, 20, 30, 255].repeat(4);
        let mut dst_size = [2, 2];
        // BGRA source with distinct pixels everywhere; only (1, 1) is dirty.
        let src = [
            9, 8, 7, 0, 6, 5, 4, 0, // row 0
            3, 2, 1, 0, 100, 150, 200, 0, // row 1
        ];
        let dirty = [Rect {
            x: 1,
            y: 1,
            width: 1,
            height: 1,
        }];
        copy_bgra_dirty_to_rgba(&mut dst, &mut dst_size, &src, [2, 2], &dirty);
        assert_eq!(&dst[0..12], &[10, 20, 30, 255].repeat(3));
        assert_eq!(&dst[12..16], &[200, 150, 100, 255]);
    }

    #[test]
    fn fractional_scale_computes_exact_one_to_one_phys_and_crops_ceil_overhang() {
        let (view, phys) = view_and_phys_for_rect(1019.2, 673.4, 1.25);
        assert_eq!(phys, [1274, 842]);
        assert_eq!(view, [1274, 842]);
        // 1.25x page zoom is 1.2^level.
        assert!((1.2f64.powf(page_zoom(1.25)) - 1.25).abs() < 1e-9);
        assert_eq!(page_zoom(1.0), 0.0);

        // 3x2 image cropped to 2x2 keeps the top-left 2x2 pixels intact.
        let src = [
            1, 2, 3, 255, 4, 5, 6, 255, 9, 9, 9, 255, 7, 8, 9, 255, 10, 11, 12, 255, 9, 9, 9, 255,
        ];
        let cropped = crop_rgba(&src, [3, 2], [2, 2]);
        assert_eq!(
            cropped,
            vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
        );
    }

    #[test]
    fn pump_schedule_coalesces_repeated_immediate_requests() {
        let wakes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let wakes_clone = wakes.clone();
        let schedule = PumpSchedule::new(move || {
            wakes_clone.fetch_add(1, Ordering::Relaxed);
        });
        schedule.schedule(0);
        schedule.schedule(0);
        schedule.schedule(10);
        assert_eq!(wakes.load(Ordering::Relaxed), 1);
        assert!(schedule.is_due(Instant::now()));
        schedule.clear();
        assert!(!schedule.is_due(Instant::now()));
        // Without new requests the safety pump still comes due.
        let later = Instant::now() + Duration::from_millis(MAX_FALLBACK_PUMP_MS + 1);
        assert!(schedule.is_due(later));
    }
}
