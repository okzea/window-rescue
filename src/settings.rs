//! The only state Window Rescue keeps: its shortcut and whether it starts with Windows,
//! both under HKEY_CURRENT_USER.

use std::ffi::c_void;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HWND};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

use crate::wide;

const APP_KEY: &str = r"Software\WindowRescue";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "WindowRescue";
pub const HOTKEY_ID: i32 = 1;

#[derive(Clone, Copy, PartialEq)]
pub struct Shortcut {
    pub mods: u32,
    pub vk: u32,
}

impl Shortcut {
    pub const DEFAULT: Shortcut = Shortcut { mods: MOD_CONTROL | MOD_ALT, vk: VK_HOME as u32 };
    /// At least one of these is required, so the shortcut never steals plain typing.
    pub const REQUIRED: u32 = MOD_CONTROL | MOD_ALT | MOD_WIN;

    pub fn load() -> Shortcut {
        match (read_dword("ShortcutModifiers"), read_dword("ShortcutKey")) {
            (Some(mods), Some(vk)) if mods & Self::REQUIRED != 0 && vk != 0 => Shortcut { mods, vk },
            _ => Self::DEFAULT,
        }
    }

    pub fn save(&self) {
        write_dword("ShortcutModifiers", self.mods);
        write_dword("ShortcutKey", self.vk);
    }

    pub fn register(&self, hwnd: HWND) -> bool {
        unsafe { RegisterHotKey(hwnd, HOTKEY_ID, self.mods | MOD_NOREPEAT, self.vk) != 0 }
    }

    pub fn unregister(hwnd: HWND) {
        unsafe { UnregisterHotKey(hwnd, HOTKEY_ID) };
    }

    pub fn name(&self) -> String {
        let mut name = String::new();
        for (flag, label) in [(MOD_CONTROL, "Ctrl+"), (MOD_ALT, "Alt+"), (MOD_SHIFT, "Shift+"), (MOD_WIN, "Win+")] {
            if self.mods & flag != 0 {
                name.push_str(label);
            }
        }
        name + &key_name(self.vk)
    }
}

fn key_name(vk: u32) -> String {
    unsafe {
        let scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) as i32;
        // Without the extended bit, Home reads as "Num 7", Delete as "Num Del", and so on.
        let extended = matches!(
            vk as u16,
            VK_PRIOR | VK_NEXT | VK_END | VK_HOME | VK_LEFT | VK_UP | VK_RIGHT | VK_DOWN | VK_INSERT | VK_DELETE | VK_DIVIDE
        );
        let lparam = (scan << 16) | if extended { 1 << 24 } else { 0 };
        let mut buf = [0u16; 64];
        let len = GetKeyNameTextW(lparam, buf.as_mut_ptr(), buf.len() as i32);
        if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            format!("Key {vk}")
        }
    }
}

pub fn autostart_enabled() -> bool {
    read_string(RUN_KEY, RUN_VALUE).is_some_and(|cmd| cmd.eq_ignore_ascii_case(&run_command()))
}

pub fn set_autostart(on: bool) {
    unsafe {
        if on {
            let data = wide(&run_command());
            let bytes = (data.len() * 2) as u32;
            RegSetKeyValueW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), wide(RUN_VALUE).as_ptr(), REG_SZ, data.as_ptr() as *const c_void, bytes);
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), wide(RUN_VALUE).as_ptr());
        }
    }
}

fn run_command() -> String {
    let mut buf = vec![0u16; 1024];
    let len = unsafe { GetModuleFileNameW(null_mut(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    format!("\"{}\"", String::from_utf16_lossy(&buf[..len]))
}

fn read_dword(name: &str) -> Option<u32> {
    let mut value = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, wide(APP_KEY).as_ptr(), wide(name).as_ptr(), RRF_RT_REG_DWORD, null_mut(), &mut value as *mut _ as *mut c_void, &mut size)
    };
    (status == ERROR_SUCCESS).then_some(value)
}

fn write_dword(name: &str, value: u32) {
    unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, wide(APP_KEY).as_ptr(), wide(name).as_ptr(), REG_DWORD, &value as *const _ as *const c_void, 4);
    }
}

fn read_string(key: &str, name: &str) -> Option<String> {
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, wide(key).as_ptr(), wide(name).as_ptr(), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr() as *mut c_void, &mut size)
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let chars = (size as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buf[..chars]))
}
