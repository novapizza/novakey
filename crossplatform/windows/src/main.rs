//! main.rs
//! NovaKey Windows entry point: a hidden message-only window that installs the
//! low-level keyboard/mouse hooks, a foreground-change WinEvent hook, a tray
//! icon, and a toggle hotkey, then runs a GetMessage pump.
//!
//! Known limitation: a non-elevated low-level hook is inert while an elevated
//! application has focus. Run NovaKey elevated to type into elevated apps.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hook;
mod sender;
mod settings;
mod tray;
mod vk;

use std::cell::RefCell;

use windows::core::PWSTR;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, MOD_CONTROL, MOD_SHIFT, VK_Z,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, GetWindowThreadProcessId, PostQuitMessage, RegisterClassW, SetWindowsHookExW,
    TranslateMessage, EVENT_SYSTEM_FOREGROUND, HMENU, HWND_MESSAGE, MSG, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT, WM_COMMAND, WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP,
    WM_RBUTTONUP, WNDCLASSW,
};

const HOTKEY_ID: i32 = 1;

thread_local! {
    /// Current settings; owned by the (single) message-pump thread.
    static SETTINGS: RefCell<settings::Settings> = RefCell::new(settings::Settings::default());
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    unsafe { run() }
}

unsafe fn run() {
    // Load persisted settings and apply them.
    let loaded = settings::Settings::load();
    SETTINGS.with(|s| *s.borrow_mut() = loaded);
    hook::set_enabled(loaded.enabled);
    hook::set_step_by_step(loaded.step_by_step);
    hook::set_fix_autocomplete(loaded.fix_browser_autocomplete);

    let hmodule = GetModuleHandleW(None).expect("GetModuleHandleW failed");
    let hinst = HINSTANCE(hmodule.0);

    // Register the (hidden) window class.
    let class_name = wide("NovaKeyMessageWindow");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinst,
        hIcon: tray::app_icon(),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    RegisterClassW(&wc);

    // Message-only window (HWND_MESSAGE parent): no UI, just a message sink.
    let window_name = wide("NovaKey");
    let hwnd = CreateWindowExW(
        Default::default(),
        PCWSTR(class_name.as_ptr()),
        PCWSTR(window_name.as_ptr()),
        Default::default(),
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        HMENU::default(),
        hinst,
        None,
    )
    .expect("CreateWindowExW failed");

    // Install the low-level keyboard and mouse hooks on this thread.
    let _kbd_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook::keyboard_proc), hinst, 0)
        .expect("SetWindowsHookExW(WH_KEYBOARD_LL) failed");

    let _mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(hook::mouse_proc), hinst, 0)
        .expect("SetWindowsHookExW(WH_MOUSE_LL) failed");

    // Reset the composition session whenever the foreground window changes.
    let _winevent = SetWinEventHook(
        EVENT_SYSTEM_FOREGROUND,
        EVENT_SYSTEM_FOREGROUND,
        None,
        Some(winevent_proc),
        0,
        0,
        WINEVENT_OUTOFCONTEXT,
    );

    // Toggle hotkey: Ctrl+Shift+Z.
    let _ = RegisterHotKey(hwnd, HOTKEY_ID, MOD_CONTROL | MOD_SHIFT, VK_Z.0 as u32);

    // Tray icon.
    tray::add(hwnd, hook::is_enabled());

    // Seed the browser flag from the currently-focused window.
    hook::set_is_browser(window_is_browser(GetForegroundWindow()));

    // Standard message pump. GetMessageW returns 0 on WM_QUIT.
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

/// Foreground-change callback — reset the session so we never compose across
/// an app switch.
unsafe extern "system" fn winevent_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    hook::reset_engine();
    // Cache whether the new foreground app is a browser, so the hot keydown
    // path only reads an atomic instead of querying the process each keystroke.
    hook::set_is_browser(window_is_browser(hwnd));
}

/// Whether the process owning `hwnd` is a known web browser. Used to scope the
/// autocomplete-defeating U+202F trick to browsers (it can misbehave in
/// terminals). Cheap: no COM, no cross-process messaging.
unsafe fn window_is_browser(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return false;
    }
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return false;
    }
    let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
        Ok(h) => h,
        Err(_) => return false,
    };
    let mut buf = [0u16; 260];
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
        return false;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
    let base = path.rsplit(['\\', '/']).next().unwrap_or(&path);
    matches!(
        base,
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

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        tray::WM_TRAY => {
            // lParam carries the originating mouse message.
            let mouse = lparam.0 as u32;
            if mouse == WM_LBUTTONUP {
                do_toggle(hwnd);
            } else if mouse == WM_RBUTTONUP {
                let (autostart, step) =
                    SETTINGS.with(|s| (s.borrow().start_with_windows, s.borrow().step_by_step));
                tray::show_menu(
                    hwnd,
                    hook::is_enabled(),
                    autostart,
                    step,
                    hook::is_fix_autocomplete(),
                );
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            if wparam.0 as i32 == HOTKEY_ID {
                do_toggle(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd = (wparam.0 & 0xFFFF) as usize;
            handle_command(hwnd, cmd);
            LRESULT(0)
        }
        WM_DESTROY => {
            tray::remove(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn do_toggle(hwnd: HWND) {
    let new = hook::toggle_enabled();
    SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        s.enabled = new;
        s.save();
    });
    tray::update(hwnd, new);
}

unsafe fn handle_command(hwnd: HWND, cmd: usize) {
    match cmd {
        tray::CMD_TOGGLE => do_toggle(hwnd),
        tray::CMD_AUTOSTART => {
            let new = SETTINGS.with(|s| {
                let mut s = s.borrow_mut();
                s.start_with_windows = !s.start_with_windows;
                s.save();
                s.start_with_windows
            });
            settings::set_autostart(new);
        }
        tray::CMD_STEP => {
            let new = SETTINGS.with(|s| {
                let mut s = s.borrow_mut();
                s.step_by_step = !s.step_by_step;
                s.save();
                s.step_by_step
            });
            hook::set_step_by_step(new);
        }
        tray::CMD_AUTOCOMPLETE => {
            let new = SETTINGS.with(|s| {
                let mut s = s.borrow_mut();
                s.fix_browser_autocomplete = !s.fix_browser_autocomplete;
                s.save();
                s.fix_browser_autocomplete
            });
            hook::set_fix_autocomplete(new);
        }
        tray::CMD_QUIT => {
            let _ = DestroyWindow(hwnd);
        }
        _ => {}
    }
}
