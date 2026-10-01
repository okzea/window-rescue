//! Window Rescue — a tray icon and a shortcut that bring off-screen windows back.
//!
//! - The shortcut (Ctrl+Alt+Home by default) brings the active window fully onto the
//!   monitor under the mouse pointer.
//! - Clicking the tray icon brings back every window that is off screen.
//! - `WindowRescue.exe --rescue-all` and `--rescue-active` do the same once, without staying
//!   in the tray — handy to bind from another launcher.

#![windows_subsystem = "windows"]

mod recorder;
mod rescue;
mod settings;

use std::cell::Cell;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use rescue::Outcome;
use settings::Shortcut;

const CLASS: &str = "WindowRescueTray";
const WM_TRAY: u32 = WM_APP + 1;
const WM_ALREADY_RUNNING: u32 = WM_APP + 2;
const TRAY_ID: u32 = 1;
const REPO: &str = "github.com/okzea/window-rescue";

const IDM_RESCUE: usize = 1;
const IDM_SHORTCUT: usize = 2;
const IDM_AUTOSTART: usize = 3;
const IDM_ABOUT: usize = 4;
const IDM_EXIT: usize = 5;

thread_local! {
    static SHORTCUT: Cell<Shortcut> = const { Cell::new(Shortcut::DEFAULT) };
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn shortcut() -> Shortcut {
    SHORTCUT.get()
}

/// Called once a new shortcut is registered and saved.
pub fn shortcut_changed(hwnd: HWND, shortcut: Shortcut) {
    SHORTCUT.set(shortcut);
    set_tip(hwnd);
}

fn main() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        match std::env::args().nth(1).as_deref() {
            Some("--rescue-all") => std::process::exit(rescue::rescue_offscreen().moved as i32),
            Some("--rescue-active") => std::process::exit(matches!(rescue::rescue_foreground(), Outcome::Moved) as i32),
            _ => {}
        }

        // One instance only; a second launch just tells the first one to say it is there.
        CreateMutexW(null(), 0, wide(r"Local\WindowRescue").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let running = FindWindowW(wide(CLASS).as_ptr(), null());
            if !running.is_null() {
                PostMessageW(running, WM_ALREADY_RUNNING, 0, 0);
            }
            return;
        }

        let instance = GetModuleHandleW(null());
        let class = wide(CLASS);
        let wc = WNDCLASSW { lpfnWndProc: Some(wnd_proc), hInstance: instance, lpszClassName: class.as_ptr(), ..zeroed() };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW, class.as_ptr(), class.as_ptr(), WS_POPUP, 0, 0, 0, 0, null_mut(), null_mut(), instance, null(),
        );

        TASKBAR_CREATED.set(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()));
        let shortcut = Shortcut::load();
        SHORTCUT.set(shortcut);
        add_tray_icon(hwnd);
        if !shortcut.register(hwnd) {
            shortcut_unavailable(hwnd, shortcut);
        }

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            if let Outcome::Denied(title) = rescue::rescue_foreground() {
                denied(hwnd, &title);
            }
            0
        }
        WM_TRAY => {
            match (lparam & 0xffff) as u32 {
                WM_LBUTTONUP => rescue_all(hwnd),
                WM_RBUTTONUP => show_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_ALREADY_RUNNING => {
            let text = format!("Press {} or click this icon to bring windows back.", shortcut().name());
            notify(hwnd, "Window Rescue is already running", &text);
            0
        }
        WM_DESTROY => {
            Shell_NotifyIconW(NIM_DELETE, &tray_data(hwnd));
            PostQuitMessage(0);
            0
        }
        // Explorer restarted and lost every tray icon.
        _ if msg == TASKBAR_CREATED.get() => {
            add_tray_icon(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn rescue_all(hwnd: HWND) {
    let report = rescue::rescue_offscreen();
    if let Some(title) = report.denied.first() {
        denied(hwnd, title);
    } else if report.moved == 0 {
        notify(hwnd, "Nothing to rescue", "Every window is already on screen.");
    }
}

fn show_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        let item = |flags: u32, id: usize, text: &str| AppendMenuW(menu, flags, id, wide(text).as_ptr());
        item(MF_STRING, IDM_RESCUE, "Rescue off-screen windows");
        SetMenuDefaultItem(menu, IDM_RESCUE as u32, 0);
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        item(MF_STRING, IDM_SHORTCUT, &format!("Change shortcut…\t{}", shortcut().name()));
        item(if settings::autostart_enabled() { MF_CHECKED } else { MF_UNCHECKED }, IDM_AUTOSTART, "Start with Windows");
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        item(MF_STRING, IDM_ABOUT, "About Window Rescue");
        item(MF_STRING, IDM_EXIT, "Exit");

        // Without this dance the menu does not close when the user clicks elsewhere.
        let mut cursor = POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor);
        SetForegroundWindow(hwnd);
        let flags = TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY;
        let command = TrackPopupMenu(menu, flags, cursor.x, cursor.y, 0, hwnd, null()) as usize;
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);

        match command {
            IDM_RESCUE => rescue_all(hwnd),
            IDM_SHORTCUT => recorder::open(hwnd),
            IDM_AUTOSTART => settings::set_autostart(!settings::autostart_enabled()),
            IDM_ABOUT => about(hwnd),
            IDM_EXIT => {
                DestroyWindow(hwnd);
            }
            _ => {}
        }
    }
}

fn about(hwnd: HWND) {
    let text = format!(
        "Window Rescue {}\n\n\
         {} — brings the active window fully onto the screen under the mouse pointer.\n\n\
         Click the tray icon — brings back every window that is off screen.\n\n\
         {}",
        env!("CARGO_PKG_VERSION"),
        shortcut().name(),
        REPO,
    );
    unsafe {
        MessageBoxW(hwnd, wide(&text).as_ptr(), wide("About Window Rescue").as_ptr(), MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND);
    }
}

fn denied(hwnd: HWND, title: &str) {
    let text = format!("\"{title}\" runs as administrator. To move it, start Window Rescue as administrator too.");
    notify(hwnd, "Windows blocked the move", &text);
}

pub fn shortcut_unavailable(hwnd: HWND, shortcut: Shortcut) {
    let text = format!("{} is already used by another app. Right-click the Window Rescue icon to pick another shortcut.", shortcut.name());
    notify(hwnd, "Shortcut unavailable", &text);
}

/// The app icon from the executable's resources, at the given size.
pub fn app_icon(size: i32) -> HICON {
    unsafe {
        let icon = LoadImageW(GetModuleHandleW(null()), 1 as _, IMAGE_ICON, size, size, LR_DEFAULTCOLOR);
        if icon.is_null() {
            LoadIconW(null_mut(), IDI_APPLICATION)
        } else {
            icon as HICON
        }
    }
}

fn tray_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data: NOTIFYICONDATAW = unsafe { zeroed() };
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ID;
    data
}

fn add_tray_icon(hwnd: HWND) {
    let mut data = tray_data(hwnd);
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = app_icon(unsafe { GetSystemMetrics(SM_CXSMICON) });
    copy_into(&mut data.szTip, &tip());
    unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
}

fn set_tip(hwnd: HWND) {
    let mut data = tray_data(hwnd);
    data.uFlags = NIF_TIP;
    copy_into(&mut data.szTip, &tip());
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
}

fn tip() -> String {
    format!("Window Rescue — {}", shortcut().name())
}

pub fn notify(hwnd: HWND, title: &str, text: &str) {
    let mut data = tray_data(hwnd);
    data.uFlags = NIF_INFO;
    data.dwInfoFlags = NIIF_INFO | NIIF_NOSOUND;
    copy_into(&mut data.szInfoTitle, title);
    copy_into(&mut data.szInfo, text);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
}

fn copy_into(dst: &mut [u16], text: &str) {
    let src: Vec<u16> = text.encode_utf16().take(dst.len() - 1).collect();
    dst[..src.len()].copy_from_slice(&src);
    dst[src.len()] = 0;
}
