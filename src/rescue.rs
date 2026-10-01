//! Finding windows that are out of reach and moving them back on screen.

use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::null_mut;

use windows_sys::core::BOOL;
use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub enum Outcome {
    Moved,
    AlreadyVisible,
    /// Windows refused the move — almost always because the target runs elevated.
    Denied(String),
    Skipped,
}

pub struct Report {
    pub moved: usize,
    pub denied: Vec<String>,
}

/// Centers the active window on the monitor under the mouse pointer, shrinking it to fit.
pub fn rescue_foreground() -> Outcome {
    unsafe {
        let hwnd = GetAncestor(GetForegroundWindow(), GA_ROOT);
        if hwnd.is_null() || is_own(hwnd) || is_shell(hwnd) {
            return Outcome::Skipped;
        }
        move_into(hwnd, &target_work_area(), 0, true)
    }
}

/// Brings back every window whose title bar is mostly off every screen.
pub fn rescue_offscreen() -> Report {
    let monitors = monitor_rects();
    let work = target_work_area();
    let mut report = Report { moved: 0, denied: Vec::new() };
    for hwnd in top_level_windows() {
        if !is_app_window(hwnd) || !is_offscreen(hwnd, &monitors) {
            continue;
        }
        // Cascade so several rescued windows do not land exactly on top of each other.
        match move_into(hwnd, &work, report.moved as i32 * 32, false) {
            Outcome::Moved => report.moved += 1,
            Outcome::Denied(title) => report.denied.push(title),
            _ => {}
        }
    }
    report
}

/// With `always`, a window already fully visible in `work` is still centered there, so the
/// shortcut visibly does something; maximized windows already there are left alone either way.
fn move_into(hwnd: HWND, work: &RECT, offset: i32, always: bool) -> Outcome {
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        let maximized = IsZoomed(hwnd) != 0;
        if (maximized || !always) && contains(work, &frame_rect(hwnd)) {
            return Outcome::AlreadyVisible;
        }
        // A maximized window is moved in its restored state, then maximized again on the new monitor.
        if maximized {
            ShowWindow(hwnd, SW_RESTORE);
        }
        if !place(hwnd, work, offset) {
            if maximized {
                ShowWindow(hwnd, SW_MAXIMIZE);
            }
            return Outcome::Denied(window_title(hwnd));
        }
        // Crossing to a monitor with another scale makes the app resize itself: fit it once more.
        if !contains(work, &frame_rect(hwnd)) {
            place(hwnd, work, offset);
        }
        if maximized {
            ShowWindow(hwnd, SW_MAXIMIZE);
        }
        SetForegroundWindow(hwnd);
        Outcome::Moved
    }
}

/// Centers the window's visible frame in `work`, shrinking it if it does not fit.
fn place(hwnd: HWND, work: &RECT, offset: i32) -> bool {
    unsafe {
        let mut outer: RECT = zeroed();
        if GetWindowRect(hwnd, &mut outer) == 0 {
            return false;
        }
        let frame = frame_rect(hwnd);
        // Windows 10/11 windows carry invisible resize borders outside their visible frame.
        let (bl, bt) = (frame.left - outer.left, frame.top - outer.top);
        let (br, bb) = (outer.right - frame.right, outer.bottom - frame.bottom);

        let w = width(&frame).min(width(work));
        let h = height(&frame).min(height(work));
        let x = (work.left + (width(work) - w) / 2 + offset).min(work.right - w);
        let y = (work.top + (height(work) - h) / 2 + offset).min(work.bottom - h);

        let mut flags = SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE;
        if w == width(&frame) && h == height(&frame) {
            if x == frame.left && y == frame.top {
                return true; // Already exactly there.
            }
            flags |= SWP_NOSIZE;
        }
        // An elevated window either fails the call or silently stays put.
        let ok = SetWindowPos(hwnd, null_mut(), x - bl, y - bt, w + bl + br, h + bt + bb, flags) != 0;
        let now = frame_rect(hwnd);
        ok && (now.left, now.top, now.right, now.bottom) != (frame.left, frame.top, frame.right, frame.bottom)
    }
}

/// The visible frame, without the invisible resize borders.
fn frame_rect(hwnd: HWND) -> RECT {
    unsafe {
        let mut rect: RECT = zeroed();
        let size = size_of::<RECT>() as u32;
        let attr = DWMWA_EXTENDED_FRAME_BOUNDS as u32;
        if DwmGetWindowAttribute(hwnd, attr, &mut rect as *mut _ as *mut c_void, size) != 0 {
            GetWindowRect(hwnd, &mut rect);
        }
        rect
    }
}

/// Off screen when less than a quarter of the title-bar strip is on any monitor.
fn is_offscreen(hwnd: HWND, monitors: &[RECT]) -> bool {
    let frame = frame_rect(hwnd);
    let strip = RECT { left: frame.left, top: frame.top, right: frame.right, bottom: frame.top + 32.min(height(&frame)) };
    let area = width(&strip) as i64 * height(&strip) as i64;
    let visible: i64 = monitors.iter().map(|m| overlap(&strip, m)).sum();
    area > 0 && visible * 4 < area
}

/// The work area (screen minus taskbar) of the monitor under the mouse pointer.
pub fn target_work_area() -> RECT {
    unsafe {
        let mut cursor = POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor);
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(MonitorFromPoint(cursor, MONITOR_DEFAULTTOPRIMARY), &mut info);
        info.rcWork
    }
}

fn monitor_rects() -> Vec<RECT> {
    unsafe extern "system" fn collect(_: HMONITOR, _: HDC, rect: *mut RECT, list: LPARAM) -> BOOL {
        (*(list as *mut Vec<RECT>)).push(*rect);
        1
    }
    let mut list = Vec::new();
    unsafe { EnumDisplayMonitors(null_mut(), null_mut(), Some(collect), &mut list as *mut _ as LPARAM) };
    list
}

fn top_level_windows() -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, list: LPARAM) -> BOOL {
        (*(list as *mut Vec<HWND>)).push(hwnd);
        1
    }
    let mut list = Vec::new();
    unsafe { EnumWindows(Some(collect), &mut list as *mut _ as LPARAM) };
    list
}

/// A regular application window a user could have lost: visible, titled, on this virtual desktop.
fn is_app_window(hwnd: HWND) -> bool {
    unsafe {
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 || GetWindowTextLengthW(hwnd) == 0 {
            return false;
        }
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if ex & WS_EX_TOOLWINDOW != 0 && ex & WS_EX_APPWINDOW == 0 {
            return false;
        }
        // Cloaked: suspended UWP apps and windows on other virtual desktops.
        let mut cloaked: u32 = 0;
        let attr = DWMWA_CLOAKED as u32;
        DwmGetWindowAttribute(hwnd, attr, &mut cloaked as *mut _ as *mut c_void, 4);
        // Tiny windows are helpers some apps park off screen on purpose, not something to rescue.
        let frame = frame_rect(hwnd);
        let big_enough = width(&frame) >= 100 && height(&frame) >= 40;
        cloaked == 0 && big_enough && !is_own(hwnd) && !is_shell(hwnd)
    }
}

fn is_own(hwnd: HWND) -> bool {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid == unsafe { GetCurrentProcessId() }
}

fn is_shell(hwnd: HWND) -> bool {
    let mut buf = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    let class = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
    matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
}

fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

fn width(r: &RECT) -> i32 {
    r.right - r.left
}

fn height(r: &RECT) -> i32 {
    r.bottom - r.top
}

/// True when `inner` fits in `outer`, give or take a few pixels of rounding.
fn contains(outer: &RECT, inner: &RECT) -> bool {
    const SLACK: i32 = 8;
    inner.left >= outer.left - SLACK
        && inner.top >= outer.top - SLACK
        && inner.right <= outer.right + SLACK
        && inner.bottom <= outer.bottom + SLACK
}

fn overlap(a: &RECT, b: &RECT) -> i64 {
    let w = (a.right.min(b.right) - a.left.max(b.left)).max(0) as i64;
    let h = (a.bottom.min(b.bottom) - a.top.max(b.top)).max(0) as i64;
    w * h
}
