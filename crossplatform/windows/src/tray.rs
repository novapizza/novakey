//! tray.rs
//! System-tray icon (`Shell_NotifyIcon`) and its right-click context menu.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW, SetForegroundWindow,
    TrackPopupMenu, HICON, HMENU, IDI_APPLICATION, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    MF_UNCHECKED, TPM_BOTTOMALIGN, TPM_RIGHTBUTTON,
};

/// Resource id of the embedded application icon (see build.rs).
pub const APP_ICON_ID: u16 = 1;

/// Callback message the tray icon posts to our window (WM_APP + n).
pub const WM_TRAY: u32 = 0x0400 + 1; // WM_APP + 1

/// Menu command IDs (arrive via WM_COMMAND).
pub const CMD_TOGGLE: usize = 1;
pub const CMD_AUTOSTART: usize = 2;
pub const CMD_STEP: usize = 3;
pub const CMD_AUTOCOMPLETE: usize = 4;
pub const CMD_QUIT: usize = 5;
pub const CMD_SETTINGS: usize = 6;
pub const CMD_PLAYSOUND: usize = 7;

const TRAY_UID: u32 = 1;

/// Load the embedded NovaKey icon (resource id 1). Falls back to the default
/// Windows application icon if it can't be found.
pub fn app_icon() -> HICON {
    unsafe {
        if let Ok(hmod) = GetModuleHandleW(None) {
            // MAKEINTRESOURCE: a numeric resource id passed as a PCWSTR.
            let name = PCWSTR(APP_ICON_ID as usize as *const u16);
            if let Ok(icon) = LoadIconW(HINSTANCE(hmod.0), name) {
                if !icon.is_invalid() {
                    return icon;
                }
            }
        }
        LoadIconW(HINSTANCE::default(), IDI_APPLICATION).unwrap_or_default()
    }
}

fn base_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        ..Default::default()
    };
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = app_icon();
    data
}

fn set_tip(data: &mut NOTIFYICONDATAW, tip: &str) {
    for (i, u) in tip.encode_utf16().take(data.szTip.len() - 1).enumerate() {
        data.szTip[i] = u;
    }
}

/// Add the tray icon.
pub fn add(hwnd: HWND, enabled: bool, hotkey: &str) {
    let mut data = base_data(hwnd);
    set_tip(&mut data, &tip_for(enabled, hotkey));
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &data);
    }
}

/// Update the tooltip to reflect the current on/off state and hotkey.
pub fn update(hwnd: HWND, enabled: bool, hotkey: &str) {
    let mut data = base_data(hwnd);
    set_tip(&mut data, &tip_for(enabled, hotkey));
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
    }
}

/// Remove the tray icon (on quit).
pub fn remove(hwnd: HWND) {
    let data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

fn tip_for(enabled: bool, hotkey: &str) -> String {
    let state = if enabled { "ON" } else { "OFF" };
    format!("NovaKey — Vietnamese {state} ({hotkey})")
}

/// Show the right-click context menu. Selecting an item posts WM_COMMAND to hwnd.
pub fn show_menu(hwnd: HWND, enabled: bool, autostart: bool, step: bool, fix_autocomplete: bool) {
    unsafe {
        let menu: HMENU = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return,
        };

        let check = |on: bool| if on { MF_CHECKED } else { MF_UNCHECKED };

        let _ = AppendMenuW(menu, MF_STRING | check(enabled), CMD_TOGGLE, w("Vietnamese input"));
        let _ = AppendMenuW(menu, MF_STRING, CMD_SETTINGS, w("Settings…"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING | check(autostart), CMD_AUTOSTART, w("Start with Windows"));
        let _ = AppendMenuW(menu, MF_STRING | check(step), CMD_STEP, w("Compatibility (step-by-step) mode"));
        let _ = AppendMenuW(menu, MF_STRING | check(fix_autocomplete), CMD_AUTOCOMPLETE, w("Fix browser URL autocomplete"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT, w("Quit NovaKey"));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Required so the menu closes when the user clicks elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
    }
}

/// Build a NUL-terminated wide string and hand out a PCWSTR into a leaked buffer.
/// Menu labels live for the whole process, so a small deliberate leak is fine.
fn w(s: &str) -> PCWSTR {
    let v: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    let boxed = v.into_boxed_slice();
    let ptr = boxed.as_ptr();
    std::mem::forget(boxed);
    PCWSTR(ptr)
}

// Silence unused-import warnings when only some WPARAM/LPARAM paths are used.
#[allow(dead_code)]
fn _touch(_: WPARAM, _: LPARAM) {}
