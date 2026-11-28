use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{copy_nonoverlapping, null, null_mut};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BI_BITFIELDS, BITMAPV5HEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject,
    GetDC, ReleaseDC,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMessageW,
    GetWindowLongPtrW, HICON, ICONINFO, IDI_APPLICATION, LoadIconW, MF_DISABLED, MF_GRAYED,
    MF_SEPARATOR, MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, SetForegroundWindow, SetWindowLongPtrW, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP, WM_CLOSE, WM_CONTEXTMENU,
    WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
};

use super::{TrayCallbacks, TrayIcon, TrayState};

const TRAY_ICON_ID: u32 = 1;
const WM_TRAY_CALLBACK: u32 = WM_APP + 1;
const WM_TRAY_UPDATE: u32 = WM_APP + 2;
const MENU_OPEN: usize = 1001;
const MENU_QUIT: usize = 1002;

pub(super) struct WindowsTray {
    hwnd: usize,
    state: Arc<Mutex<TrayState>>,
    thread: Option<JoinHandle<()>>,
}

impl WindowsTray {
    pub(super) fn start(
        icon: Option<TrayIcon>,
        initial: TrayState,
        callbacks: TrayCallbacks,
    ) -> Option<Self> {
        let state = Arc::new(Mutex::new(initial));
        let worker_state = Arc::clone(&state);
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("icy-tray".into())
            .spawn(move || run_tray(icon, worker_state, callbacks, ready_tx))
            .ok()?;
        let hwnd = ready_rx.recv_timeout(Duration::from_secs(3)).ok()??;
        Some(Self {
            hwnd,
            state,
            thread: Some(thread),
        })
    }

    pub(super) fn update(&self, next: TrayState) {
        {
            let mut guard = self.state.lock().unwrap();
            if *guard == next {
                return;
            }
            *guard = next;
        }
        // SAFETY: `self.hwnd` was created on the tray thread and stays valid until Drop posts WM_CLOSE.
        unsafe {
            PostMessageW(self.hwnd as HWND, WM_TRAY_UPDATE, 0, 0);
        }
    }
}

impl Drop for WindowsTray {
    fn drop(&mut self) {
        // SAFETY: Posting WM_CLOSE to our own hidden window wakes GetMessageW to clean up and exit.
        unsafe {
            PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct WindowData {
    state: Arc<Mutex<TrayState>>,
    callbacks: TrayCallbacks,
    hicon: HICON,
    owns_icon: bool,
    taskbar_created: u32,
}

fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn fill_tip(buf: &mut [u16; 128], text: &str) {
    buf.fill(0);
    for (dst, ch) in buf.iter_mut().take(127).zip(text.encode_utf16()) {
        *dst = ch;
    }
}

fn notify_data(hwnd: HWND, hicon: HICON, tooltip: &str) -> NOTIFYICONDATAW {
    // SAFETY: NOTIFYICONDATAW is a plain Win32 C struct.
    let mut data: NOTIFYICONDATAW = unsafe { zeroed() };
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ICON_ID;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_TRAY_CALLBACK;
    data.hIcon = hicon;
    fill_tip(&mut data.szTip, tooltip);
    data
}

fn create_hicon(icon: Option<&TrayIcon>) -> (HICON, bool) {
    if let Some(icon) = icon
        && let Some(hicon) = unsafe { create_hicon_from_rgba(icon) }
    {
        return (hicon, true);
    }
    // SAFETY: Standard system fallback icon does not need to be destroyed.
    let fallback = unsafe { LoadIconW(null_mut(), IDI_APPLICATION) };
    (fallback, false)
}

unsafe fn create_hicon_from_rgba(icon: &TrayIcon) -> Option<HICON> {
    if !icon.is_valid() {
        return None;
    }
    let width = icon.width as i32;
    let height = icon.height as i32;
    let bgra = icon.to_bgra();
    // SAFETY: BITMAPV5HEADER is a plain Win32 C struct.
    let mut header: BITMAPV5HEADER = unsafe { zeroed() };
    header.bV5Size = size_of::<BITMAPV5HEADER>() as u32;
    header.bV5Width = width;
    header.bV5Height = -height;
    header.bV5Planes = 1;
    header.bV5BitCount = 32;
    header.bV5Compression = BI_BITFIELDS;
    header.bV5RedMask = 0x00ff_0000;
    header.bV5GreenMask = 0x0000_ff00;
    header.bV5BlueMask = 0x0000_00ff;
    header.bV5AlphaMask = 0xff00_0000;

    let mut bits: *mut c_void = null_mut();
    // SAFETY: All GDI handles created below are deleted before returning.
    unsafe {
        let hdc = GetDC(null_mut());
        let color_bitmap = CreateDIBSection(
            hdc,
            (&header as *const BITMAPV5HEADER).cast(),
            DIB_RGB_COLORS,
            &mut bits,
            null_mut(),
            0,
        );
        ReleaseDC(null_mut(), hdc);
        if color_bitmap.is_null() || bits.is_null() {
            if !color_bitmap.is_null() {
                DeleteObject(color_bitmap);
            }
            return None;
        }
        copy_nonoverlapping(bgra.as_ptr(), bits.cast::<u8>(), bgra.len());
        let stride = (icon.width as usize).div_ceil(16) * 2;
        let mask_bytes = vec![0u8; stride * (icon.height as usize)];
        let mask_bitmap = CreateBitmap(width, height, 1, 1, mask_bytes.as_ptr().cast());
        if mask_bitmap.is_null() {
            DeleteObject(color_bitmap);
            return None;
        }
        let info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask_bitmap,
            hbmColor: color_bitmap,
        };
        let hicon = CreateIconIndirect(&info);
        DeleteObject(mask_bitmap);
        DeleteObject(color_bitmap);
        (!hicon.is_null()).then_some(hicon)
    }
}

fn run_tray(
    icon: Option<TrayIcon>,
    state: Arc<Mutex<TrayState>>,
    callbacks: TrayCallbacks,
    ready: mpsc::Sender<Option<usize>>,
) {
    let class_name = wide_null(&format!("icy-tray-window-{}", std::process::id()));
    let taskbar_msg = wide_null("TaskbarCreated");
    // SAFETY: Single-threaded Win32 message loop owns this hidden window and its WindowData box.
    unsafe {
        let hinstance: HINSTANCE = GetModuleHandleW(null());
        let mut wc: WNDCLASSW = zeroed();
        wc.lpfnWndProc = Some(wnd_proc);
        wc.hInstance = hinstance;
        wc.lpszClassName = class_name.as_ptr();
        if RegisterClassW(&wc) == 0 {
            let _ = ready.send(None);
            return;
        }

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinstance,
            null(),
        );
        if hwnd.is_null() {
            let _ = ready.send(None);
            return;
        }

        let (hicon, owns_icon) = create_hicon(icon.as_ref());
        let taskbar_created = RegisterWindowMessageW(taskbar_msg.as_ptr());
        let tooltip = state.lock().unwrap().tooltip();
        let data = Box::new(WindowData {
            state,
            callbacks,
            hicon,
            owns_icon,
            taskbar_created,
        });
        let data_ptr = Box::into_raw(data);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, data_ptr as isize);

        let nid = notify_data(hwnd, hicon, &tooltip);
        if Shell_NotifyIconW(NIM_ADD, &nid) == 0 {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            let boxed = Box::from_raw(data_ptr);
            if boxed.owns_icon && !boxed.hicon.is_null() {
                DestroyIcon(boxed.hicon);
            }
            DestroyWindow(hwnd);
            let _ = ready.send(None);
            return;
        }

        let _ = ready.send(Some(hwnd as usize));

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // DestroyWindow has already finished before PostQuitMessage ends the loop.
        let boxed = Box::from_raw(data_ptr);
        if boxed.owns_icon && !boxed.hicon.is_null() {
            DestroyIcon(boxed.hicon);
        }
    }
}

unsafe fn show_context_menu(hwnd: HWND, data: &WindowData) {
    // SAFETY: Popup menu is created, tracked synchronously on the tray thread, and destroyed.
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let status = wide_null(&data.state.lock().unwrap().status_label());
        let open = wide_null("Open icy");
        let quit = wide_null("Quit icy");
        AppendMenuW(
            menu,
            MF_STRING | MF_DISABLED | MF_GRAYED,
            0,
            status.as_ptr(),
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        AppendMenuW(menu, MF_STRING, MENU_OPEN, open.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        AppendMenuW(menu, MF_STRING, MENU_QUIT, quit.as_ptr());

        let mut cursor: POINT = zeroed();
        GetCursorPos(&mut cursor);
        SetForegroundWindow(hwnd);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
            cursor.x,
            cursor.y,
            0,
            hwnd,
            null(),
        ) as usize;
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);

        match selected {
            MENU_OPEN => {
                let on_open = Arc::clone(&data.callbacks.on_open);
                let _ = thread::Builder::new()
                    .name("icy-tray-open".into())
                    .spawn(move || on_open());
            }
            MENU_QUIT => {
                let on_quit = Arc::clone(&data.callbacks.on_quit);
                let _ = thread::Builder::new()
                    .name("icy-tray-quit".into())
                    .spawn(move || on_quit());
            }
            _ => {}
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: GWLP_USERDATA holds the WindowData pointer for the lifetime of the message loop.
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowData;
        if !ptr.is_null() {
            let data = &*ptr;
            if msg == WM_TRAY_CALLBACK {
                let event = (lparam as u32) & 0xffff;
                if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
                    let on_open = Arc::clone(&data.callbacks.on_open);
                    let _ = thread::Builder::new()
                        .name("icy-tray-open".into())
                        .spawn(move || on_open());
                    return 0;
                }
                if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                    show_context_menu(hwnd, data);
                    return 0;
                }
            } else if msg == WM_TRAY_UPDATE {
                let tooltip = data.state.lock().unwrap().tooltip();
                let nid = notify_data(hwnd, data.hicon, &tooltip);
                Shell_NotifyIconW(NIM_MODIFY, &nid);
                return 0;
            } else if data.taskbar_created != 0 && msg == data.taskbar_created {
                let tooltip = data.state.lock().unwrap().tooltip();
                let nid = notify_data(hwnd, data.hicon, &tooltip);
                Shell_NotifyIconW(NIM_ADD, &nid);
                return 0;
            } else if msg == WM_CLOSE {
                let nid = notify_data(hwnd, data.hicon, "");
                Shell_NotifyIconW(NIM_DELETE, &nid);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                DestroyWindow(hwnd);
                return 0;
            }
        }
        if msg == WM_DESTROY {
            PostQuitMessage(0);
            return 0;
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
