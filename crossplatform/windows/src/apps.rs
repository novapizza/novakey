//! apps.rs
//! Identifies applications by their exe name: the foreground app (for the
//! browser autocomplete guard and the per-app rules) and the apps currently
//! showing a window (for the Settings "Add app" picker).
//!
//! Everything here is cheap — no COM, no cross-process messaging — because the
//! foreground lookup runs on every app switch.

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetClassNameW, GetWindow, GetWindowLongW,
    GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE, GW_OWNER,
    WS_EX_TOOLWINDOW,
};

use crate::settings::normalize_exe;

/// Host process of Microsoft Store (UWP) app windows. The frame belongs to
/// it; the app itself owns a child window.
const UWP_HOST: &str = "applicationframehost.exe";

/// The app owning `hwnd`, as a normalized exe name (`zalo.exe`). Store apps are
/// resolved to the app inside the frame rather than its host. `None` when the
/// process cannot be queried (e.g. it runs elevated).
pub unsafe fn window_app(hwnd: HWND) -> Option<String> {
    if hwnd.is_invalid() {
        return None;
    }
    let pid = window_pid(hwnd)?;
    let exe = process_exe(pid)?;
    if exe != UWP_HOST {
        return Some(exe);
    }
    // Find the child window owned by a different process: the app itself.
    let mut found: (u32, u32) = (pid, 0);
    let _ = EnumChildWindows(
        hwnd,
        Some(find_foreign_child),
        LPARAM(&mut found as *mut (u32, u32) as isize),
    );
    match found.1 {
        0 => Some(exe),
        child_pid => process_exe(child_pid).or(Some(exe)),
    }
}

unsafe extern "system" fn find_foreign_child(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let found = &mut *(lparam.0 as *mut (u32, u32));
    match window_pid(hwnd) {
        Some(pid) if pid != found.0 => {
            found.1 = pid;
            false.into() // stop
        }
        _ => true.into(),
    }
}

/// Whether `hwnd` belongs to NovaKey itself (Settings, the abbreviation
/// editor, the tray menu owner).
pub unsafe fn is_own_window(hwnd: HWND) -> bool {
    !hwnd.is_invalid() && window_pid(hwnd) == Some(GetCurrentProcessId())
}

/// Whether `hwnd` is a window that should not count as "the app the user is
/// typing in": NovaKey's own windows (Settings, the tray menu owner) and the
/// taskbar / notification area the user clicks to reach the tray icon.
pub unsafe fn is_transient_window(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return true;
    }
    if window_pid(hwnd) == Some(GetCurrentProcessId()) {
        return true;
    }
    let mut buf = [0u16; 64];
    let len = GetClassNameW(hwnd, &mut buf).max(0) as usize;
    let class = String::from_utf16_lossy(&buf[..len]);
    matches!(
        class.as_str(),
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "TopLevelWindowForOverflowXamlIsland"
    )
}

/// Whether `exe` (normalized) is a known web browser. Scopes the
/// autocomplete-defeating U+202F trick to browsers (it can misbehave in
/// terminals).
pub fn is_browser(exe: &str) -> bool {
    matches!(
        exe,
        "chrome.exe"
            | "msedge.exe"
            | "firefox.exe"
            | "brave.exe"
            | "opera.exe"
            | "opera_gx.exe"
            | "vivaldi.exe"
            | "chromium.exe"
            | "arc.exe"
            | "iexplore.exe"
    )
}

/// Exe names of the apps currently showing a regular top-level window (what
/// the user would see in Alt+Tab), sorted and deduplicated, NovaKey excluded.
pub unsafe fn running_apps() -> Vec<String> {
    let mut apps: Vec<String> = Vec::new();
    let _ = EnumWindows(
        Some(collect_app_window),
        LPARAM(&mut apps as *mut Vec<String> as isize),
    );
    apps.sort();
    apps.dedup();
    apps
}

unsafe extern "system" fn collect_app_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let apps = &mut *(lparam.0 as *mut Vec<String>);
    if is_alt_tab_window(hwnd) && window_pid(hwnd) != Some(GetCurrentProcessId()) {
        if let Some(exe) = window_app(hwnd) {
            if exe != UWP_HOST {
                apps.push(exe);
            }
        }
    }
    true.into()
}

/// Roughly the Alt+Tab filter: visible, titled, unowned, not a tool window,
/// and not cloaked (suspended Store apps keep an invisible-but-"visible" frame).
unsafe fn is_alt_tab_window(hwnd: HWND) -> bool {
    if !IsWindowVisible(hwnd).as_bool() || GetWindowTextLengthW(hwnd) == 0 {
        return false;
    }
    if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.is_invalid()) {
        return false;
    }
    if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
        return false;
    }
    let mut cloaked: u32 = 0;
    let _ = DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        &mut cloaked as *mut u32 as *mut _,
        std::mem::size_of::<u32>() as u32,
    );
    cloaked == 0
}

unsafe fn window_pid(hwnd: HWND) -> Option<u32> {
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    (pid != 0).then_some(pid)
}

/// Normalized exe name of process `pid`.
unsafe fn process_exe(pid: u32) -> Option<String> {
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(
        handle,
        PROCESS_NAME_WIN32,
        PWSTR(buf.as_mut_ptr()),
        &mut len,
    )
    .is_ok();
    let _ = CloseHandle(handle);
    if !ok {
        return None;
    }
    normalize_exe(&String::from_utf16_lossy(&buf[..len as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_browsers() {
        assert!(is_browser("chrome.exe"));
        assert!(is_browser("msedge.exe"));
        assert!(!is_browser("zalo.exe"));
    }
}
