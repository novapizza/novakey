//! main.rs
//! NovaKey Windows entry point: a hidden message-only window that installs the
//! low-level keyboard/mouse hooks, a foreground-change WinEvent hook, a tray
//! icon, and a toggle hotkey, then runs a GetMessage pump.
//!
//! Known limitation: a non-elevated low-level hook is inert while an elevated
//! application has focus. Run NovaKey elevated to type into elevated apps.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod badge;
mod hook;
mod hotkey;
mod sender;
mod settings;
mod tray;
mod ui;
mod vk;

use std::cell::RefCell;

use windows::core::PWSTR;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    CreateMutexW, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_NOREPEAT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, GetWindowThreadProcessId, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
    SetWindowsHookExW, TranslateMessage, EVENT_SYSTEM_FOREGROUND, HMENU, HWND_MESSAGE, MB_OK, MSG,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT, WM_COMMAND, WM_DESTROY, WM_DPICHANGED,
    WM_HOTKEY, WM_LBUTTONUP, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_THEMECHANGED, WNDCLASSW,
};

const HOTKEY_ID: i32 = 1;

/// App-defined message the Settings window sends to (re)register the toggle
/// hotkey: `wparam` = MOD_* bitmask, `lparam` = virtual-key. Returns 1 on
/// success, 0 if the combination could not be registered.
pub const WM_APP_SETHOTKEY: u32 = 0x0400 + 2; // WM_APP + 2

thread_local! {
    /// Current settings; owned by the (single) message-pump thread.
    pub(crate) static SETTINGS: RefCell<settings::Settings> =
        RefCell::new(settings::Settings::default());
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    unsafe { run() }
}

unsafe fn run() {
    // Single-instance guard: a named mutex shared across all sessions of the
    // current user. If it already exists, another NovaKey is running, so bail
    // out before installing hooks or a second tray icon. The handle is held for
    // the process lifetime (Windows frees it on exit); we never signal it.
    let mutex_name = wide("Local\\NovaKeySingleInstanceMutex");
    let _instance_mutex = match CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr())) {
        Ok(h) => h,
        Err(_) => return,
    };
    if GetLastError() == ERROR_ALREADY_EXISTS {
        // Another instance owns the mutex — exit quietly.
        return;
    }

    // Load persisted settings and apply them.
    let loaded = settings::Settings::load();
    SETTINGS.with(|s| *s.borrow_mut() = loaded);
    hook::set_enabled(loaded.enabled);
    hook::set_step_by_step(loaded.step_by_step);
    hook::set_fix_autocomplete(loaded.fix_browser_autocomplete);
    hook::set_quick_vietnamese(loaded.quick_vietnamese);
    hook::set_deferred_diacritics(loaded.deferred_diacritics);

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

    // Toggle hotkey: from settings (defaults to Ctrl+Space).
    let (hk_mods, hk_vk) =
        SETTINGS.with(|s| (s.borrow().hotkey_mods, s.borrow().hotkey_vk));
    let _ = register_toggle_hotkey(hwnd, hk_mods, hk_vk);

    // Tray icon.
    tray::add(hwnd, hook::is_enabled(), &current_hotkey_desc());

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
                ui::open(hwnd);
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
        WM_APP_SETHOTKEY => {
            let mods = wparam.0 as u32;
            let vk = lparam.0 as u32;
            let ok = set_toggle_hotkey(hwnd, mods, vk);
            LRESULT(if ok { 1 } else { 0 })
        }
        WM_COMMAND => {
            let cmd = (wparam.0 & 0xFFFF) as usize;
            handle_command(hwnd, cmd);
            LRESULT(0)
        }
        // Light/dark switch or a DPI change alters how the badge should be
        // drawn (letter colour, icon size), so redraw it.
        WM_SETTINGCHANGE | WM_THEMECHANGED | WM_DPICHANGED => {
            tray::update(hwnd, hook::is_enabled(), &current_hotkey_desc());
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_DESTROY => {
            tray::remove(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => {
            // Explorer restarted and dropped every tray icon: publish ours again.
            if msg != 0 && msg == taskbar_created_msg() {
                tray::add(hwnd, hook::is_enabled(), &current_hotkey_desc());
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

/// The shell's "TaskbarCreated" broadcast message id (0 if registration fails).
fn taskbar_created_msg() -> u32 {
    thread_local! {
        static ID: std::cell::Cell<u32> = const { std::cell::Cell::new(u32::MAX) };
    }
    ID.with(|id| {
        if id.get() == u32::MAX {
            let name = wide("TaskbarCreated");
            id.set(unsafe { RegisterWindowMessageW(PCWSTR(name.as_ptr())) });
        }
        id.get()
    })
}

unsafe fn do_toggle(hwnd: HWND) {
    let new = hook::toggle_enabled();
    let play = SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        s.enabled = new;
        s.save();
        s.play_sound
    });
    tray::update(hwnd, new, &current_hotkey_desc());
    if play {
        let _ = MessageBeep(MB_OK);
    }
    // Keep the Settings window's V/E indicator in sync if it is open.
    ui::refresh();
}

/// Description of the currently-configured toggle hotkey (e.g. "Ctrl+Space").
fn current_hotkey_desc() -> String {
    SETTINGS.with(|s| {
        let s = s.borrow();
        hotkey::describe(s.hotkey_mods, s.hotkey_vk)
    })
}

/// Register (replacing any existing) the global toggle hotkey. Returns whether
/// registration succeeded. MOD_NOREPEAT keeps a held combo from auto-firing.
unsafe fn register_toggle_hotkey(hwnd: HWND, mods: u32, vk: u32) -> bool {
    let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
    RegisterHotKey(
        hwnd,
        HOTKEY_ID,
        HOT_KEY_MODIFIERS(mods | MOD_NOREPEAT.0),
        vk,
    )
    .is_ok()
}

/// Try to switch to a new toggle hotkey. On success it is persisted and the tray
/// tooltip refreshed; on failure the previous hotkey is restored so the toggle
/// never ends up unbound.
unsafe fn set_toggle_hotkey(hwnd: HWND, mods: u32, vk: u32) -> bool {
    let (old_mods, old_vk) =
        SETTINGS.with(|s| (s.borrow().hotkey_mods, s.borrow().hotkey_vk));

    if register_toggle_hotkey(hwnd, mods, vk) {
        SETTINGS.with(|s| {
            let mut s = s.borrow_mut();
            s.hotkey_mods = mods;
            s.hotkey_vk = vk;
            s.save();
        });
        tray::update(hwnd, hook::is_enabled(), &hotkey::describe(mods, vk));
        true
    } else {
        let _ = register_toggle_hotkey(hwnd, old_mods, old_vk);
        false
    }
}

unsafe fn handle_command(hwnd: HWND, cmd: usize) {
    match cmd {
        tray::CMD_TOGGLE => do_toggle(hwnd),
        tray::CMD_SETTINGS => ui::open(hwnd),
        tray::CMD_PLAYSOUND => {
            SETTINGS.with(|s| {
                let mut s = s.borrow_mut();
                s.play_sound = !s.play_sound;
                s.save();
            });
        }
        tray::CMD_QUICKVN => {
            let new = SETTINGS.with(|s| {
                let mut s = s.borrow_mut();
                s.quick_vietnamese = !s.quick_vietnamese;
                s.save();
                s.quick_vietnamese
            });
            hook::set_quick_vietnamese(new);
        }
        tray::CMD_DEFERRED => {
            let new = SETTINGS.with(|s| {
                let mut s = s.borrow_mut();
                s.deferred_diacritics = !s.deferred_diacritics;
                s.save();
                s.deferred_diacritics
            });
            hook::set_deferred_diacritics(new);
        }
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
