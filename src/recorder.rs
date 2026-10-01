//! The small "press the new shortcut" window.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow, SystemParametersInfoForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::settings::Shortcut;
use crate::{app_icon, shortcut, shortcut_changed, shortcut_unavailable, wide};

const CLASS: &str = "WindowRescueShortcut";
const PROMPT: &str = "Press the new shortcut.";
const STYLE: u32 = WS_POPUP | WS_CAPTION | WS_SYSMENU;
const EX_STYLE: u32 = WS_EX_TOPMOST;

thread_local! {
    static WINDOW: Cell<HWND> = const { Cell::new(null_mut()) };
    static OWNER: Cell<HWND> = const { Cell::new(null_mut()) };
    static SAVED: Cell<bool> = const { Cell::new(false) };
    static MESSAGE: RefCell<String> = const { RefCell::new(String::new()) };
}

pub fn open(owner: HWND) {
    unsafe {
        let existing = WINDOW.get();
        if !existing.is_null() {
            SetForegroundWindow(existing);
            return;
        }

        let instance = GetModuleHandleW(null());
        let class = wide(CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as usize as HBRUSH,
            hIcon: app_icon(GetSystemMetrics(SM_CXSMICON)),
            ..zeroed()
        };
        RegisterClassW(&wc); // Fails harmlessly from the second time on.

        // Released while recording, so pressing the current shortcut is a valid answer.
        Shortcut::unregister(owner);
        OWNER.set(owner);
        SAVED.set(false);
        MESSAGE.with_borrow_mut(|m| *m = PROMPT.into());

        // Created on the monitor under the pointer first, so its DPI is the right one to size by.
        let work = crate::rescue::target_work_area();
        let title = wide("Change shortcut — Window Rescue");
        let hwnd = CreateWindowExW(
            EX_STYLE, class.as_ptr(), title.as_ptr(), STYLE, work.left, work.top, 1, 1, null_mut(), null_mut(), instance, null(),
        );
        WINDOW.set(hwnd);

        let dpi = GetDpiForWindow(hwnd);
        let mut rect = RECT { left: 0, top: 0, right: scale(400, dpi), bottom: scale(130, dpi) };
        AdjustWindowRectExForDpi(&mut rect, STYLE, 0, EX_STYLE, dpi);
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        let x = work.left + (work.right - work.left - w) / 2;
        let y = work.top + (work.bottom - work.top - h) / 2;
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_SHOWWINDOW);
        SetForegroundWindow(hwnd);
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            on_key(hwnd, wparam as u16);
            0
        }
        // Swallowed so Alt combinations neither beep nor open the window menu.
        WM_SYSKEYUP | WM_SYSCHAR | WM_CHAR => 0,
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_DPICHANGED => {
            let r = &*(lparam as *const RECT);
            SetWindowPos(hwnd, null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
            InvalidateRect(hwnd, null(), 1);
            0
        }
        WM_DESTROY => {
            if !SAVED.get() {
                let current = shortcut();
                if !current.register(OWNER.get()) {
                    shortcut_unavailable(OWNER.get(), current);
                }
            }
            WINDOW.set(null_mut());
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn on_key(hwnd: HWND, vk: u16) {
    unsafe {
        if vk == VK_ESCAPE {
            DestroyWindow(hwnd);
            return;
        }
        if matches!(vk, VK_SHIFT | VK_CONTROL | VK_MENU | VK_LWIN | VK_RWIN) {
            return;
        }
        let down = |key: u16| GetKeyState(key as i32) < 0;
        let mut mods = 0;
        if down(VK_CONTROL) {
            mods |= MOD_CONTROL;
        }
        if down(VK_MENU) {
            mods |= MOD_ALT;
        }
        if down(VK_SHIFT) {
            mods |= MOD_SHIFT;
        }
        if down(VK_LWIN) || down(VK_RWIN) {
            mods |= MOD_WIN;
        }
        if mods & Shortcut::REQUIRED == 0 {
            return set_message(hwnd, "Hold Ctrl, Alt or Win, then press a key.");
        }

        let candidate = Shortcut { mods, vk: vk as u32 };
        if candidate.register(OWNER.get()) {
            candidate.save();
            shortcut_changed(OWNER.get(), candidate);
            SAVED.set(true);
            DestroyWindow(hwnd);
        } else {
            set_message(hwnd, &format!("{} is already used by another app. Try another.", candidate.name()));
        }
    }
}

fn set_message(hwnd: HWND, text: &str) {
    MESSAGE.with_borrow_mut(|m| *m = text.into());
    unsafe { InvalidateRect(hwnd, null(), 1) };
}

fn paint(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        let dpi = GetDpiForWindow(hwnd);

        let mut metrics: NONCLIENTMETRICSW = zeroed();
        metrics.cbSize = size_of::<NONCLIENTMETRICSW>() as u32;
        SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS, metrics.cbSize, &mut metrics as *mut _ as *mut c_void, 0, dpi);
        let font = CreateFontIndirectW(&metrics.lfMessageFont);
        let previous = SelectObject(hdc, font as HGDIOBJ);
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, GetSysColor(COLOR_WINDOWTEXT));

        let mut rect: RECT = zeroed();
        GetClientRect(hwnd, &mut rect);
        let pad = scale(20, dpi);
        rect.left += pad;
        rect.top += pad;
        rect.right -= pad;
        rect.bottom -= pad;
        let text = MESSAGE.with_borrow(|m| format!("{m}\n\nCurrent: {}    ·    Esc to cancel", shortcut().name()));
        let mut text = wide(&text);
        DrawTextW(hdc, text.as_mut_ptr(), -1, &mut rect, DT_CENTER | DT_WORDBREAK | DT_NOPREFIX);

        SelectObject(hdc, previous);
        DeleteObject(font as HGDIOBJ);
        EndPaint(hwnd, &ps);
    }
}

fn scale(value: i32, dpi: u32) -> i32 {
    value * dpi as i32 / 96
}
