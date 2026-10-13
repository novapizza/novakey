//! main.rs
//! NovaKey Windows entry point: a hidden message-only window that installs the
//! low-level keyboard/mouse hooks, a foreground-change WinEvent hook, a tray
//! icon, and a toggle hotkey, then runs a GetMessage pump.
//!
//! Known limitation: a non-elevated low-level hook is inert while an elevated
//! application has focus. Run NovaKey elevated to type into elevated apps.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod apps;
mod badge;
mod hook;
mod hotkey;
mod sender;
mod settings;
mod tray;
mod updater;
mod ui;
mod vk;

use std::cell::{Cell, RefCell};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcess, ProcessPowerThrottling, SetPriorityClass,
    SetProcessInformation, ABOVE_NORMAL_PRIORITY_CLASS, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_STATE,
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
    GetMessageW, KillTimer, MessageBoxW, PostMessageW,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetProcessDPIAware, SetTimer,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, EVENT_SYSTEM_FOREGROUND, HHOOK,
    HMENU, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MSG, WH_KEYBOARD_LL, WH_MOUSE_LL,
    WINEVENT_OUTOFCONTEXT, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_DPICHANGED, WM_HOTKEY,
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

    /// Whether the toggle hotkey is currently registered. `RegisterHotKey` is
    /// first-come-first-served per session, so a combination another app already
    /// owns leaves us unbound — surfaced in the tray tooltip and Settings rather
    /// than failing silently.
    static HOTKEY_BOUND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

    /// The message window, so the keyboard hook can ask for a toggle.
    static MAIN_HWND: std::cell::Cell<isize> = const { std::cell::Cell::new(0) };

    /// The app the user is typing in (normalized exe name), for the per-app
    /// rules. Only updated for real apps — not for NovaKey's own windows or the
    /// taskbar — so opening the tray menu still knows which app it is about.
    static CURRENT_APP: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The app the user is typing in, if it could be identified.
pub(crate) fn current_app() -> Option<String> {
    CURRENT_APP.with(|a| a.borrow().clone())
}

/// Whether the toggle hotkey is live. Same thread as the message pump.
pub(crate) fn hotkey_bound() -> bool {
    HOTKEY_BOUND.with(|b| b.get())
}

/// Ask the pump to toggle the language. Called from the keyboard hook when a
/// modifier-only shortcut completes; posted rather than handled inline so the
/// hook returns immediately (a slow hook gets dropped by Windows).
pub(crate) fn post_toggle() {
    let hwnd = MAIN_HWND.with(|h| h.get());
    if hwnd == 0 {
        return;
    }
    unsafe {
        let _ = PostMessageW(
            HWND(hwnd as *mut _),
            WM_COMMAND,
            WPARAM(tray::CMD_TOGGLE),
            LPARAM(0),
        );
    }
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

    // `--finish-update` means we are the freshly-swapped-in exe relaunched by
    // the updater; the exiting parent may still hold the mutex briefly, so
    // retry for a few seconds instead of bailing immediately.
    let finishing = std::env::args().any(|a| a == "--finish-update");

    let _instance_mutex = {
        let mut handle = None;
        let attempts = if finishing { 25 } else { 1 };
        for _ in 0..attempts {
            match CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr())) {
                Ok(h) => {
                    if GetLastError() == ERROR_ALREADY_EXISTS {
                        let _ = CloseHandle(h);
                        if !finishing {
                            // Normal launch, another instance is genuinely
                            // running — exit instantly, no retry latency.
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        continue;
                    }
                    handle = Some(h);
                    break;
                }
                Err(_) => return,
            }
        }
        match handle {
            Some(h) => h,
            None => return, // another instance genuinely running
        }
    };

    if finishing {
        updater::finish_pending_update();
    }

    // Load persisted settings and apply them.
    let loaded = settings::Settings::load();
    hook::set_enabled(loaded.enabled);
    hook::set_step_by_step(loaded.step_by_step);
    hook::set_fix_autocomplete(loaded.fix_browser_autocomplete);
    hook::set_quick_vietnamese(loaded.quick_vietnamese);
    hook::set_deferred_diacritics(loaded.deferred_diacritics);
    SETTINGS.with(|s| *s.borrow_mut() = loaded);
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

    MAIN_HWND.with(|h| h.set(hwnd.0 as isize));

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

    // Toggle hotkey: from settings (defaults to Ctrl+Space). If the saved
    // combination is already owned by another app, fall back to the default
    // pair; if that is taken too we start unbound, which the tray tooltip and
    // the Settings window both spell out.
    let (hk_mods, hk_vk) =
        SETTINGS.with(|s| (s.borrow().hotkey_mods, s.borrow().hotkey_vk));
    if !register_toggle_hotkey(hwnd, hk_mods, hk_vk)
        && (hk_mods, hk_vk) != (hotkey::DEFAULT_MODS, hotkey::DEFAULT_VK)
        && register_toggle_hotkey(hwnd, hotkey::DEFAULT_MODS, hotkey::DEFAULT_VK)
    {
        SETTINGS.with(|s| {
            let mut s = s.borrow_mut();
            s.hotkey_mods = hotkey::DEFAULT_MODS;
            s.hotkey_vk = hotkey::DEFAULT_VK;
            s.save();
        });
    }

    // Seed the browser flag and per-app rules from the currently-focused
    // window, before the tray icon is drawn so it shows the right state.
    on_foreground(GetForegroundWindow());

    // Tray icon. At sign-in the notification area may not exist yet; keep
    // retrying until it does (TaskbarCreated also covers this when it fires).
    if !tray_add(hwnd) {
        SetTimer(hwnd, TIMER_TRAY_RETRY, TRAY_RETRY_MS, None);
    }

    // Background update check: at most once every 24h, silent unless it applies.
    {
        let last = SETTINGS.with(|s| s.borrow().last_update_check);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if now.saturating_sub(last) > 86_400 {
            SETTINGS.with(|s| {
                let mut s = s.borrow_mut();
                s.last_update_check = now;
                s.save();
            });
            spawn_update_check(hwnd, /*background=*/ true);
        }
    }

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
    on_foreground(hwnd);
}

/// Cache what the hot keydown path needs to know about the new foreground
/// window — is it a browser, do the per-app rules allow composing in it — so
/// it only reads atomics instead of querying the process on each keystroke.
unsafe fn on_foreground(hwnd: HWND) {
    let app = apps::window_app(hwnd);
    hook::set_is_browser(app.as_deref().is_some_and(apps::is_browser));
    // NovaKey's own windows and the taskbar (clicked on the way to the tray
    // menu) keep the previous app's rules in force.
    if apps::is_transient_window(hwnd) {
        return;
    }
    CURRENT_APP.with(|a| *a.borrow_mut() = app);
    apply_app_rules();
}

/// Re-evaluate the per-app rules for the current app and reflect a change in
/// the tray. Called on app switch and whenever the rules are edited.
unsafe fn apply_app_rules() {
    let app = current_app();
    let allowed = SETTINGS.with(|s| s.borrow().allows_app(app.as_deref()));
    let was_allowed = hook::is_app_allowed();
    hook::set_app_allowed(allowed);
    // The tooltip names the paused app, so a switch between two paused apps
    // also needs a refresh; between two allowed apps nothing changes.
    if allowed != was_allowed || !allowed {
        let hwnd = MAIN_HWND.with(|h| h.get());
        if hwnd != 0 {
            tray_update(HWND(hwnd as *mut _));
        }
    }
}

/// Publish the current state to the tray icon (`add` on first publish or
/// after Explorer restarts). Returns whether the shell accepted it.
unsafe fn publish_tray(hwnd: HWND, add: bool) -> bool {
    let enabled = hook::is_enabled();
    let allowed = hook::is_app_allowed();
    let app = current_app();
    let hotkey = current_hotkey_desc();
    let paused_in = (enabled && !allowed).then(|| app.as_deref().unwrap_or("this app"));
    let state = tray::TrayState {
        vietnamese: enabled && allowed,
        hotkey: &hotkey,
        paused_in,
    };
    if add {
        tray::add(hwnd, &state)
    } else {
        tray::update(hwnd, &state);
        true
    }
}

unsafe fn tray_add(hwnd: HWND) -> bool {
    publish_tray(hwnd, true)
}

unsafe fn tray_update(hwnd: HWND) {
    publish_tray(hwnd, false);
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
                let app = current_app();
                let app_item = app.as_deref().map(|a| (a, hook::is_app_allowed()));
                tray::show_menu(
                    hwnd,
                    hook::is_enabled(),
                    app_item,
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
                    if tray_add(hwnd) {
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
            tray_update(hwnd);
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
                if tray_add(hwnd) {
                    let _ = KillTimer(hwnd, TIMER_TRAY_RETRY);
                }
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

/// Modal notification with NULL owner. Safe to call from a worker thread:
/// MessageBoxW runs its own modal loop on the calling thread. Owning it to
/// another thread's window can deadlock, so we intentionally pass no owner.
fn notify(msg: &str, warn: bool) {
    let text = wide(msg);
    let cap = wide("NovaKey");
    let icon = if warn { MB_ICONWARNING } else { MB_ICONINFORMATION };
    unsafe {
        MessageBoxW(HWND::default(), PCWSTR(text.as_ptr()), PCWSTR(cap.as_ptr()), MB_OK | icon);
    }
}

/// Run the update check on a worker thread. `background` suppresses the
/// "already up to date" and failure notifications.
fn spawn_update_check(hwnd: HWND, background: bool) {
    let hwnd_val = hwnd.0 as isize;
    std::thread::spawn(move || {
        let outcome = updater::check_now();
        match outcome {
            updater::UpdateOutcome::Applied => {
                // New instance is launching; tear this one down cleanly. Win32
                // forbids destroying a window from a thread other than the one
                // that created it (DestroyWindow would silently no-op here), so
                // post WM_CLOSE instead — PostMessageW is documented safe across
                // threads. The owning thread's wnd_proc has no WM_CLOSE arm, so
                // it falls through to DefWindowProcW, which calls DestroyWindow
                // on the owning (pump) thread, triggering the existing
                // WM_DESTROY handler (tray::remove + PostQuitMessage).
                unsafe {
                    let _ = PostMessageW(
                        HWND(hwnd_val as *mut _),
                        WM_CLOSE,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            }
            updater::UpdateOutcome::UpToDate if !background => {
                notify("NovaKey is up to date.", false);
            }
            updater::UpdateOutcome::Failed(reason) if !background => {
                notify(&format!("Update check failed: {reason}."), true);
            }
            // Background checks stay silent; manual up-to-date/failure handled above.
            _ => {}
        }
    });
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
    tray_update(hwnd);
    if play {
        let _ = MessageBeep(MB_OK);
    }
    // Keep the Settings window's V/E indicator in sync if it is open.
    ui::refresh();
}

/// Description of the currently-configured toggle hotkey (e.g. "Ctrl+Space"),
/// flagged when it could not be registered so the tooltip never advertises a
/// shortcut that does nothing.
fn current_hotkey_desc() -> String {
    let desc = SETTINGS.with(|s| {
        let s = s.borrow();
        hotkey::describe(s.hotkey_mods, s.hotkey_vk)
    });
    if hotkey_bound() {
        desc
    } else {
        format!("{desc} — unavailable")
    }
}

/// Register (replacing any existing) the global toggle hotkey. Returns whether
/// registration succeeded. MOD_NOREPEAT keeps a held combo from auto-firing.
unsafe fn register_toggle_hotkey(hwnd: HWND, mods: u32, vk: u32) -> bool {
    let _ = UnregisterHotKey(hwnd, HOTKEY_ID);

    // Modifier-only shortcuts (Ctrl+Shift, …) can't go through RegisterHotKey;
    // the keyboard hook watches for them instead, and can't be refused.
    if hotkey::is_modifier_only(mods, vk) {
        hook::set_mod_combo(mods);
        HOTKEY_BOUND.with(|b| b.set(true));
        return true;
    }
    hook::set_mod_combo(0);

    let ok = RegisterHotKey(
        hwnd,
        HOTKEY_ID,
        HOT_KEY_MODIFIERS(mods | MOD_NOREPEAT.0),
        vk,
    )
    .is_ok();
    HOTKEY_BOUND.with(|b| b.set(ok));
    ok
}

/// Try to switch to a new toggle hotkey. On success it is persisted and the tray
/// tooltip refreshed; on failure the previous hotkey is restored so the toggle
/// never ends up unbound.
unsafe fn set_toggle_hotkey(hwnd: HWND, mods: u32, vk: u32) -> bool {
    // Second line of defence: the recorder validates too, but nothing else may
    // reach RegisterHotKey with a combination that would swallow a bare key.
    if !hotkey::is_valid(mods, vk) {
        return false;
    }

    let (old_mods, old_vk) =
        SETTINGS.with(|s| (s.borrow().hotkey_mods, s.borrow().hotkey_vk));

    if register_toggle_hotkey(hwnd, mods, vk) {
        SETTINGS.with(|s| {
            let mut s = s.borrow_mut();
            s.hotkey_mods = mods;
            s.hotkey_vk = vk;
            s.save();
        });
        tray_update(hwnd);
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
        tray::CMD_APP_TOGGLE => {
            if let Some(app) = current_app() {
                SETTINGS.with(|s| {
                    let mut s = s.borrow_mut();
                    let on = !s.allows_app(Some(&app));
                    s.set_app_enabled(&app, on);
                    s.save();
                });
                apply_app_rules();
                ui::refresh();
            }
        }
        tray::CMD_APPS_CHANGED => apply_app_rules(),
        tray::CMD_UPDATE => spawn_update_check(hwnd, /*background=*/ false),
        tray::CMD_QUIT => {
            let _ = DestroyWindow(hwnd);
        }
        _ => {}
    }
}
