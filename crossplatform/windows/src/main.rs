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

use std::cell::{Cell, RefCell};

use windows::core::PWSTR;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcess, OpenProcess, ProcessPowerThrottling,
    QueryFullProcessImageNameW, SetPriorityClass, SetProcessInformation,
    ABOVE_NORMAL_PRIORITY_CLASS, PROCESS_NAME_WIN32, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_STATE, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetLastInputInfo, RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, LASTINPUTINFO,
    MOD_NOREPEAT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, GetWindowThreadProcessId, KillTimer, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, SetProcessDPIAware, SetTimer, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, EVENT_SYSTEM_FOREGROUND, HHOOK, HMENU, MB_OK, MSG, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT, WM_COMMAND, WM_DESTROY, WM_DPICHANGED, WM_HOTKEY,
    WM_LBUTTONUP, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_THEMECHANGED, WM_TIMER, WNDCLASSW,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

const HOTKEY_ID: i32 = 1;

/// Retries adding the tray icon until the shell accepts it (sign-in race).
const TIMER_TRAY_RETRY: usize = 1;
const TRAY_RETRY_MS: u32 = 2_000;

/// Periodically checks that the low-level hooks are still installed.
const TIMER_HOOK_WATCHDOG: usize = 2;
const HOOK_WATCHDOG_MS: u32 = 3_000;
/// Input the hooks haven't seen for this long means Windows dropped them.
const HOOK_STALE_MS: u32 = 1_500;

/// App-defined message the Settings window sends to (re)register the toggle
/// hotkey: `wparam` = MOD_* bitmask, `lparam` = virtual-key. Returns 1 on
/// success, 0 if the combination could not be registered.
pub const WM_APP_SETHOTKEY: u32 = 0x0400 + 2; // WM_APP + 2

thread_local! {
    /// Current settings; owned by the (single) message-pump thread.
    pub(crate) static SETTINGS: RefCell<settings::Settings> =
        RefCell::new(settings::Settings::default());

    /// Installed low-level hooks, kept so the watchdog can reinstall them.
    static HOOKS: Cell<(HHOOK, HHOOK)> = Cell::new((HHOOK::default(), HHOOK::default()));
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    unsafe { run() }
}

unsafe fn run() {
    // Per-monitor DPI awareness so the Settings window, menus and tray badge
    // render crisply at 125%/150%/200% instead of being bitmap-stretched (blurry)
    // by the system. Must happen before any window is created.
    if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_err() {
        let _ = SetProcessDPIAware();
    }

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
    sync_autostart();

    // Low-level hook callbacks must answer within LowLevelHooksTimeout or
    // Windows silently unhooks them. Sign-in is when the machine is busiest, so
    // keep the scheduler and EcoQoS (Efficiency mode) from starving us.
    let process = GetCurrentProcess();
    let _ = SetPriorityClass(process, ABOVE_NORMAL_PRIORITY_CLASS);
    let throttling = PROCESS_POWER_THROTTLING_STATE {
        Version: 1, // PROCESS_POWER_THROTTLING_CURRENT_VERSION
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: 0,
    };
    let _ = SetProcessInformation(
        process,
        ProcessPowerThrottling,
        &throttling as *const _ as *const std::ffi::c_void,
        std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
    );

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

    // Hidden top-level window: never shown, no taskbar button. Deliberately
    // NOT a message-only (HWND_MESSAGE) window: those receive no broadcasts, so
    // "TaskbarCreated" (re-add the tray icon after Explorer starts/restarts)
    // and WM_SETTINGCHANGE/WM_THEMECHANGED would never arrive.
    let window_name = wide("NovaKey");
    let hwnd = match CreateWindowExW(
        WS_EX_TOOLWINDOW,
        PCWSTR(class_name.as_ptr()),
        PCWSTR(window_name.as_ptr()),
        WS_POPUP,
        0,
        0,
        0,
        0,
        HWND::default(),
        HMENU::default(),
        hinst,
        None,
    ) {
        Ok(h) => h,
        Err(_) => return,
    };

    // Install the low-level keyboard and mouse hooks on this thread, and keep
    // checking they stay installed.
    install_hooks();
    SetTimer(hwnd, TIMER_HOOK_WATCHDOG, HOOK_WATCHDOG_MS, None);

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

    // Tray icon. At sign-in the notification area may not exist yet; keep
    // retrying until it does (TaskbarCreated also covers this when it fires).
    if !tray::add(hwnd, hook::is_enabled(), &current_hotkey_desc()) {
        SetTimer(hwnd, TIMER_TRAY_RETRY, TRAY_RETRY_MS, None);
    }

    // Seed the browser flag from the currently-focused window.
    hook::set_is_browser(window_is_browser(GetForegroundWindow()));

    // Standard message pump. GetMessageW returns 0 on WM_QUIT.
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

/// (Re)install the low-level keyboard and mouse hooks, replacing any previous
/// ones. A failure is left for the watchdog to retry rather than aborting.
unsafe fn install_hooks() {
    let (old_kbd, old_mouse) = HOOKS.with(|h| h.get());
    if !old_kbd.is_invalid() {
        let _ = UnhookWindowsHookEx(old_kbd);
    }
    if !old_mouse.is_invalid() {
        let _ = UnhookWindowsHookEx(old_mouse);
    }
    let hinst = GetModuleHandleW(None).map(|m| HINSTANCE(m.0)).unwrap_or_default();
    let kbd = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook::keyboard_proc), hinst, 0)
        .unwrap_or_default();
    let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(hook::mouse_proc), hinst, 0)
        .unwrap_or_default();
    HOOKS.with(|h| h.set((kbd, mouse)));
}

/// Watchdog tick. Windows removes a low-level hook without notice when a
/// callback overruns LowLevelHooksTimeout (common while the machine is busy
/// right after sign-in), leaving NovaKey running but typing plain letters.
/// There is no API to ask whether a hook is alive, so compare the system's
/// last-input time with the last event our hooks saw: input they missed means
/// they are gone, so install them again.
unsafe fn check_hooks() {
    let (kbd, mouse) = HOOKS.with(|h| h.get());
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if !GetLastInputInfo(&mut info).as_bool() {
        return;
    }
    // Wrapping tick arithmetic; a "negative" gap means the hook saw newer input.
    let missed = info.dwTime.wrapping_sub(hook::last_hook_time());
    let stale = missed > HOOK_STALE_MS && missed < u32::MAX / 2;
    // Let fresh input settle first: the hook may simply not have run yet.
    let settled = GetTickCount().wrapping_sub(info.dwTime) >= HOOK_STALE_MS;
    if kbd.is_invalid() || mouse.is_invalid() || (stale && settled) {
        install_hooks();
        // Count the missed input as seen so an app our hooks cannot observe
        // (an elevated window) does not trigger a reinstall on every tick.
        hook::mark_hook_alive(info.dwTime);
    }
}

/// Re-sync the HKCU Run entry with the setting at launch. If Windows disabled
/// the entry (Task Manager > Startup apps), reflect that in the setting.
/// Otherwise rewrite it so it points at this exe, which fixes a Run entry left
/// pointing at an old or moved copy after an update.
fn sync_autostart() {
    SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        if !s.start_with_windows {
            return;
        }
        if settings::autostart_disabled_by_windows() {
            s.start_with_windows = false;
            s.save();
        } else {
            settings::set_autostart(true);
        }
    });
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
        WM_TIMER => {
            match wparam.0 {
                TIMER_TRAY_RETRY => {
                    if tray::add(hwnd, hook::is_enabled(), &current_hotkey_desc()) {
                        let _ = KillTimer(hwnd, TIMER_TRAY_RETRY);
                    }
                }
                TIMER_HOOK_WATCHDOG => check_hooks(),
                _ => {}
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
                if tray::add(hwnd, hook::is_enabled(), &current_hotkey_desc()) {
                    let _ = KillTimer(hwnd, TIMER_TRAY_RETRY);
                }
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
            if new {
                // The user explicitly asked for it: undo a Task Manager disable.
                settings::clear_startup_disabled();
            }
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
