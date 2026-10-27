//! ui.rs
//! An owner-drawn Settings window (INPUT METHOD / ABBREVIATIONS / APP SCOPE /
//! GENERAL / APPEARANCE / COMPATIBILITY, mirroring the macOS preferences pane) in one of
//! four themes, chosen under Appearance: classic Windows 9x (the default — a
//! beveled gray frame with a gradient title bar, etched group boxes, 3D push
//! buttons and a sunken status bar), Windows 3.1, Windows 10/11 flat, and
//! Mac OS 9 Platinum. Every theme shares one layout; only the painting differs.
//!
//! Everything — including the title bar and frame — is drawn by hand with GDI
//! into off-screen bitmaps (flicker-free double buffering); interactive
//! elements are hit-tested manually against rectangles recorded during paint.
//! Buttons press in on mouse-down and act on release, as classic controls do.
//! No GUI dependency is added — this keeps the binary tiny.
//!
//! Layout is authored in 96-DPI units; `rect`, `font` and `px` scale it to
//! the window's current DPI so it stays crisp at any display zoom (the process
//! is per-monitor DPI aware, see main.rs).
//!
//! The window fits itself to the monitor's work area; when the content is
//! taller (high display zoom, a long app list) the area between the title bar
//! and the status bar scrolls with the mouse wheel or the scroll bar. Content
//! is painted at full height into its own bitmap and the visible slice is
//! blitted into the frame.
//!
//! The window is created on, and its messages pump on, the same thread that
//! owns `crate::SETTINGS` and the hooks, so it reads/writes shared state without
//! locking and talks to the main message window via `SendMessageW`.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW,
    CreatePen, CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect,
    GetMonitorInfoW, GetTextExtentPoint32W, GradientFill, InvalidateRect, MonitorFromWindow,
    Polyline, RoundRect, ScreenToClient, SelectObject, SetBkMode, SetTextColor, UpdateWindow,
    DRAW_TEXT_FORMAT, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
    GRADIENT_FILL_RECT_H, GRADIENT_FILL_RECT_V, GRADIENT_RECT, HBRUSH, HDC, HFONT, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, PAINTSTRUCT, PS_INSIDEFRAME, PS_SOLID, SRCCOPY, TRANSPARENT,
    TRIVERTEX,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RETURN,
    VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DrawIconEx, GetClientRect, GetSystemMetrics, GetWindowRect, IsWindow,
    LoadCursorW, LoadImageW, RegisterClassW, SetForegroundWindow, SetWindowPos, ShowWindow,
    TrackPopupMenu, DI_NORMAL, HICON, HMENU, HTCAPTION, IDC_ARROW, IMAGE_ICON, LR_DEFAULTCOLOR,
    MF_CHECKED, MF_GRAYED, MF_STRING, SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE, SWP_NOZORDER, SW_MINIMIZE,
    SW_SHOW, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_TOPALIGN, WINDOW_STYLE, WM_ACTIVATE,
    WM_CAPTURECHANGED, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_KEYDOWN, WM_KEYUP,
    WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCHITTEST,
    WM_PAINT, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU,
};

use crate::hotkey;
use crate::settings::{AppMode, Settings, Theme};
use crate::tray::{
    CMD_ABBREVIATIONS, CMD_APPS_CHANGED, CMD_AUTOCOMPLETE, CMD_AUTOSTART, CMD_DEFERRED,
    CMD_PLAYSOUND, CMD_QUICKVN, CMD_STEP, CMD_TOGGLE,
};

// MARK: - Layout constants (96-DPI layout units)

const CLIENT_W: i32 = 460;
/// No native caption or border: the frame and title bar are painted.
const SETTINGS_STYLE: WINDOW_STYLE = WINDOW_STYLE(WS_POPUP.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0);
/// Outer frame: a 2-unit raised bevel plus 2 units of face color.
const FRAME: i32 = 4;
const TITLE_H: i32 = 24;
/// Status bar height, including the gap above it.
const STATUS_H: i32 = 28;
/// Top edge of the scrolling content viewport.
const VIEW_TOP: i32 = FRAME + TITLE_H + 2;
/// Fixed chrome above and below the viewport.
const CHROME_H: i32 = VIEW_TOP + STATUS_H + FRAME;
const SCROLLBAR_W: i32 = 16;
/// Mouse-wheel scroll distance per notch (layout units).
const WHEEL_STEP: i32 = 48;
/// Scroll distance of one scroll-bar arrow click (layout units).
const ARROW_STEP: i32 = 24;

/// Content side margin, and the gap between group boxes.
const MARGIN: i32 = 10;
const GROUP_GAP: i32 = 12;
/// Group label row down to the first control inside the group.
const GROUP_HEAD: i32 = 24;
/// Inner side and bottom padding of a group box.
const GROUP_PAD: i32 = 12;
/// Height of one row in the app list box.
const APP_ROW_H: i32 = 30;
/// Row height of a checkbox without a caption line.
const CHECK_ROW_H: i32 = 24;
/// Row height of a checkbox with a caption line.
const CAPTIONED_ROW_H: i32 = 38;

/// The INPUT METHOD group: language buttons (52), shortcut panel (48), status
/// line (24), input mode (36), separator (12), two captioned options (46 + 38).
const INPUT_GROUP_H: i32 = GROUP_HEAD + 52 + 48 + 24 + 36 + 12 + 46 + CAPTIONED_ROW_H + GROUP_PAD;
/// ABBREVIATIONS: a captioned checkbox, then the "Edit List…" button row.
const ABBREV_GROUP_H: i32 = GROUP_HEAD + CAPTIONED_ROW_H + 8 + 28 + GROUP_PAD;
/// GENERAL and COMPATIBILITY: two plain checkbox rows each.
const TWO_ROW_GROUP_H: i32 = GROUP_HEAD + CHECK_ROW_H + 8 + CHECK_ROW_H + GROUP_PAD;
/// APPEARANCE: the theme picker row.
const APPEARANCE_GROUP_H: i32 = GROUP_HEAD + 26 + GROUP_PAD;
/// Etched separator plus the OK button row.
const FOOTER_H: i32 = 12 + 30 + 10;

// MARK: - Interactive regions

#[derive(Clone, Copy, PartialEq)]
enum Region {
    PillVi,
    PillEn,
    HotkeyChange,
    HotkeyReset,
    ToggleQuickVn,
    ToggleDeferred,
    ToggleAbbrev,
    AbbrevEdit,
    ToggleAutostart,
    ToggleSound,
    ToggleFix,
    ToggleStep,
    AppModeAll,
    AppModeOnly,
    AppModeExcept,
    /// "Remove" on row `n` of the current app list.
    AppRemove(usize),
    AppAdd,
    ThemePick,
    Ok,
    Minimize,
    Close,
    /// The Windows 3.1 system-menu box.
    SysMenu,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    ScrollThumb,
}

/// How a shortcut-row status line should read.
#[derive(Clone, Copy, PartialEq)]
enum NoticeKind {
    /// Neutral guidance while recording.
    Hint,
    /// Bound, but the combination may be unreliable (Win chords).
    Warn,
    /// Nothing changed — the combination was refused.
    Error,
}

/// Scroll-bar geometry from the last paint (device pixels).
#[derive(Clone, Copy, Default)]
struct ScrollGeom {
    /// Distance the thumb can travel along the track.
    travel: i32,
    max_scroll: i32,
    /// One page of scrolling (a track click).
    page: i32,
}

struct UiState {
    hwnd: HWND,
    main_hwnd: HWND,
    /// True while waiting for the user to press a new toggle shortcut.
    capturing: bool,
    /// Status line drawn under the toggle-shortcut row.
    notice: Option<(String, NoticeKind)>,
    /// Clickable rectangles recorded during the last paint, in client
    /// coordinates (content regions already offset by the scroll position).
    hits: Vec<(Region, RECT)>,
    /// How far the content is scrolled up, in device pixels.
    scroll: i32,
    /// Whether the window is the active one (title bar color).
    active: bool,
    /// The control held down by the mouse, and whether the cursor is still
    /// over it (it is drawn pressed only then, and acts on release only then).
    down: Option<Region>,
    down_over: bool,
    /// Scroll-thumb drag in progress: (cursor y, scroll) when it started.
    drag: Option<(i32, i32)>,
    scroll_geom: ScrollGeom,
}

/// Replace the shortcut-row status line.
fn set_notice(msg: Option<(String, NoticeKind)>) {
    UI.with(|u| {
        if let Some(s) = u.borrow_mut().as_mut() {
            s.notice = msg;
        }
    });
}

/// Enter or leave shortcut-capture mode.
fn set_capturing(on: bool) {
    UI.with(|u| {
        if let Some(s) = u.borrow_mut().as_mut() {
            s.capturing = on;
        }
    });
}

thread_local! {
    static UI: RefCell<Option<UiState>> = const { RefCell::new(None) };
    /// DPI of the Settings window's current monitor (96 = 100%).
    static DPI: Cell<u32> = const { Cell::new(96) };
    /// The theme being painted (a copy of `Settings::theme`, see `sync_theme`).
    static THEME: Cell<Theme> = const { Cell::new(Theme::Classic) };
}

/// The current theme.
fn theme() -> Theme {
    THEME.with(|t| t.get())
}

/// Adopt the saved theme.
fn sync_theme() {
    let t = crate::SETTINGS.with(|s| s.borrow().theme);
    THEME.with(|c| c.set(t));
}

/// Scale a 96-DPI layout length to the window's current DPI.
fn px(v: i32) -> i32 {
    let dpi = DPI.with(|d| d.get()) as f64;
    (v as f64 * dpi / 96.0).round() as i32
}

/// Inverse of `px`: device pixels back to 96-DPI layout units.
fn unpx(v: i32) -> i32 {
    let dpi = DPI.with(|d| d.get()).max(1) as f64;
    (v as f64 * 96.0 / dpi).round() as i32
}

/// Width of one bevel line in device pixels.
fn line() -> i32 {
    px(1).max(1)
}

// MARK: - Public entry points (called from main.rs)

/// Open the Settings window, or bring the existing one to the front.
pub unsafe fn open(main_hwnd: HWND) {
    if let Some(hwnd) = UI.with(|u| u.borrow().as_ref().map(|s| s.hwnd)) {
        if IsWindow(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            return;
        }
    }

    let hmodule = match GetModuleHandleW(None) {
        Ok(h) => h,
        Err(_) => return,
    };
    let hinst = HINSTANCE(hmodule.0);

    let class_name = wide("NovaKeySettingsWindow");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinst,
        hIcon: crate::tray::app_icon(),
        hCursor: LoadCursorW(HINSTANCE::default(), IDC_ARROW).unwrap_or_default(),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    // Ignore ERROR_CLASS_ALREADY_EXISTS on subsequent opens.
    RegisterClassW(&wc);

    sync_theme();
    let title = wide("NovaKey Settings");
    let hwnd = match CreateWindowExW(
        Default::default(),
        PCWSTR(class_name.as_ptr()),
        PCWSTR(title.as_ptr()),
        SETTINGS_STYLE,
        0,
        0,
        CLIENT_W,
        CHROME_H,
        HWND::default(),
        HMENU::default(),
        hinst,
        None,
    ) {
        Ok(h) => h,
        Err(_) => return,
    };

    UI.with(|u| {
        *u.borrow_mut() = Some(UiState {
            hwnd,
            main_hwnd,
            capturing: false,
            notice: None,
            hits: Vec::new(),
            scroll: 0,
            active: true,
            down: None,
            down_over: false,
            drag: None,
            scroll_geom: ScrollGeom::default(),
        });
    });

    apply_corners(hwnd);

    // Size for the monitor's DPI, then center on that monitor.
    DPI.with(|d| d.set(GetDpiForWindow(hwnd).max(96)));
    fit_window(hwnd, true);

    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = UpdateWindow(hwnd);
    let _ = SetForegroundWindow(hwnd);
}

/// Square corners for the retro themes: Windows 11 would otherwise round the
/// top-level window. The flat theme keeps the rounding. Fails harmlessly on
/// Windows 10, which never rounds.
unsafe fn apply_corners(hwnd: HWND) {
    let pref = if theme() == Theme::Flat { DWMWCP_ROUND } else { DWMWCP_DONOTROUND };
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &pref as *const _ as *const c_void,
        std::mem::size_of_val(&pref) as u32,
    );
}

/// Repaint the Settings window if it is open (called after a state change made
/// elsewhere — e.g. the tray/hotkey toggling the language).
pub unsafe fn refresh() {
    let Some(hwnd) = UI.with(|u| u.borrow().as_ref().map(|s| s.hwnd)) else { return };
    // The tray may have edited the app list, changing the content height.
    fit_window(hwnd, false);
    let _ = InvalidateRect(hwnd, None, false);
}

/// Total scrolling content height in layout units (the window shows as much
/// of it as fits on the monitor and scrolls the rest). Must mirror `paint`.
fn content_height() -> i32 {
    let (mode, n) = crate::SETTINGS.with(|s| {
        let s = s.borrow();
        (s.app_mode, s.active_app_list().len())
    });
    MARGIN
        + INPUT_GROUP_H
        + GROUP_GAP
        + ABBREV_GROUP_H
        + GROUP_GAP
        + scope_group_height(mode, n)
        + GROUP_GAP
        + TWO_ROW_GROUP_H
        + GROUP_GAP
        + APPEARANCE_GROUP_H
        + GROUP_GAP
        + TWO_ROW_GROUP_H
        + GROUP_GAP
        + FOOTER_H
}

/// Height of the app list box holding `n` apps (an empty list keeps one row
/// for its hint).
fn app_list_height(n: usize) -> i32 {
    4 + n.max(1) as i32 * APP_ROW_H
}

/// Height of the APP SCOPE group for `mode` with `n` listed apps. Must mirror
/// the layout in `paint_content`.
fn scope_group_height(mode: AppMode, n: usize) -> i32 {
    // Mode buttons, caption field.
    let mut h = GROUP_HEAD + 32 + 10 + 26;
    if mode != AppMode::All {
        h += 10 + app_list_height(n) + 8 + 28; // gap, list, gap, "Add App…" button
    }
    h + GROUP_PAD
}

/// Size the window to its content, capped to the monitor's work area. With
/// `center`, also center it on that monitor (first open); otherwise keep its
/// position, nudged up if it would now run off the bottom.
unsafe fn fit_window(hwnd: HWND, center: bool) {
    // No non-client area: the window rect is the client rect.
    let win_w = px(CLIENT_W);

    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let work = if GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut mi)
        .as_bool()
    {
        mi.rcWork
    } else {
        rect_px(0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
    };
    let work_h = work.bottom - work.top;
    let win_h = px(CHROME_H + content_height()).min(work_h);

    let mut cur = RECT::default();
    let _ = GetWindowRect(hwnd, &mut cur);
    let (x, mut y) = if center {
        (
            work.left + (work.right - work.left - win_w) / 2,
            work.top + ((work_h - win_h) / 3).max(0),
        )
    } else {
        (cur.left, cur.top)
    };
    y = y.min(work.bottom - win_h).max(work.top);

    if (x, y, win_w, win_h) == (cur.left, cur.top, cur.right - cur.left, cur.bottom - cur.top) {
        return;
    }
    let _ = SetWindowPos(hwnd, HWND::default(), x, y, win_w, win_h, SWP_NOZORDER | SWP_NOACTIVATE);
}

// MARK: - Window procedure

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1), // fully repainted in WM_PAINT
        WM_NCHITTEST => {
            // The painted title bar drags the window like a real caption,
            // except over its buttons.
            let mut pt = POINT {
                x: (lparam.0 & 0xFFFF) as i16 as i32,
                y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
            };
            let _ = ScreenToClient(hwnd, &mut pt);
            let mut client = RECT::default();
            let _ = GetClientRect(hwnd, &mut client);
            if contains(&title_bar(client.right), pt)
                && !title_buttons(client.right).iter().any(|(_, r)| contains(r, pt))
            {
                return LRESULT(HTCAPTION as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_ACTIVATE => {
            let active = (wparam.0 & 0xFFFF) != 0; // not WA_INACTIVE
            UI.with(|u| {
                if let Some(s) = u.borrow_mut().as_mut() {
                    s.active = active;
                }
            });
            let _ = InvalidateRect(hwnd, None, false);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_DPICHANGED => {
            // Moved to a monitor with a different zoom (or zoom changed):
            // adopt the new DPI and the frame Windows suggests for it.
            DPI.with(|d| d.set(((wparam.0 >> 16) & 0xFFFF) as u32));
            let r = &*(lparam.0 as *const RECT);
            let _ = SetWindowPos(
                hwnd,
                HWND::default(),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            // The suggested frame scales the old client height; re-cap it to
            // the new monitor's work area.
            fit_window(hwnd, false);
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
            scroll_by(hwnd, -delta * px(WHEEL_STEP) / 120);
            LRESULT(0)
        }
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = mouse_pos(lparam);
            on_mouse_down(hwnd, x, y);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = mouse_pos(lparam);
            on_mouse_move(hwnd, x, y);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let (x, y) = mouse_pos(lparam);
            on_mouse_up(hwnd, x, y);
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            // Mouse capture lost (released, or taken by another window):
            // whatever was held is no longer pressed.
            UI.with(|u| {
                if let Some(s) = u.borrow_mut().as_mut() {
                    s.down = None;
                    s.drag = None;
                }
            });
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if on_key(hwnd, wparam.0 as u32) {
                return LRESULT(0); // consumed during capture
            }
            // Dialog conventions: Enter is OK, Esc closes.
            let vk = wparam.0 as u16;
            if msg == WM_KEYDOWN && (vk == VK_RETURN.0 || vk == VK_ESCAPE.0) {
                let _ = DestroyWindow(hwnd);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        // Releasing the modifiers with no other key pressed records a
        // modifier-only shortcut such as Ctrl+Shift.
        WM_KEYUP | WM_SYSKEYUP => {
            if on_key_up(hwnd, wparam.0 as u32) {
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        // Losing focus mid-capture would otherwise leave the recorder armed, so
        // a keypress after switching back would be read as the new shortcut.
        WM_KILLFOCUS => {
            let capturing = UI.with(|u| u.borrow().as_ref().map(|s| s.capturing).unwrap_or(false));
            if capturing {
                set_capturing(false);
                set_notice(None);
                let _ = InvalidateRect(hwnd, None, false);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            UI.with(|u| *u.borrow_mut() = None);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn mouse_pos(lparam: LPARAM) -> (i32, i32) {
    (
        (lparam.0 & 0xFFFF) as i16 as i32,
        ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    )
}

fn contains(r: &RECT, p: POINT) -> bool {
    p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
}

// MARK: - Interaction

/// The region under client point (x, y), from the last paint.
fn hit_at(x: i32, y: i32) -> Option<(Region, RECT)> {
    UI.with(|u| {
        let b = u.borrow();
        let s = b.as_ref()?;
        s.hits.iter().find(|(_, r)| contains(r, POINT { x, y })).copied()
    })
}

/// Scroll the content by `delta` device pixels (clamped in `paint`).
unsafe fn scroll_by(hwnd: HWND, delta: i32) {
    UI.with(|u| {
        if let Some(s) = u.borrow_mut().as_mut() {
            s.scroll = (s.scroll + delta).clamp(0, s.scroll_geom.max_scroll.max(0));
        }
    });
    let _ = InvalidateRect(hwnd, None, false);
}

unsafe fn on_mouse_down(hwnd: HWND, x: i32, y: i32) {
    let Some((region, _)) = hit_at(x, y) else { return };
    let geom = UI.with(|u| u.borrow().as_ref().map(|s| s.scroll_geom).unwrap_or_default());
    match region {
        // Scroll-bar parts act at once, like the real control.
        Region::ScrollUp => scroll_by(hwnd, -px(ARROW_STEP)),
        Region::ScrollDown => scroll_by(hwnd, px(ARROW_STEP)),
        Region::PageUp => scroll_by(hwnd, -geom.page),
        Region::PageDown => scroll_by(hwnd, geom.page),
        Region::ScrollThumb => {
            UI.with(|u| {
                if let Some(s) = u.borrow_mut().as_mut() {
                    s.drag = Some((y, s.scroll));
                }
            });
            SetCapture(hwnd);
        }
        // Everything else presses in now and acts on release.
        _ => {
            UI.with(|u| {
                if let Some(s) = u.borrow_mut().as_mut() {
                    s.down = Some(region);
                    s.down_over = true;
                }
            });
            SetCapture(hwnd);
            let _ = InvalidateRect(hwnd, None, false);
        }
    }
}

unsafe fn on_mouse_move(hwnd: HWND, x: i32, y: i32) {
    let over = hit_at(x, y).map(|(r, _)| r);
    let changed = UI.with(|u| {
        let mut b = u.borrow_mut();
        let Some(s) = b.as_mut() else { return false };
        if let Some((start_y, start_scroll)) = s.drag {
            let g = s.scroll_geom;
            s.scroll = start_scroll + (y - start_y) * g.max_scroll / g.travel.max(1);
            s.scroll = s.scroll.clamp(0, g.max_scroll.max(0));
            return true;
        }
        let Some(down) = s.down else { return false };
        let now_over = over == Some(down);
        let changed = now_over != s.down_over;
        s.down_over = now_over;
        changed
    });
    if changed {
        let _ = InvalidateRect(hwnd, None, false);
    }
}

unsafe fn on_mouse_up(hwnd: HWND, x: i32, y: i32) {
    let down = UI.with(|u| {
        let mut b = u.borrow_mut();
        let s = b.as_mut()?;
        s.drag = None;
        s.down.take()
    });
    let _ = ReleaseCapture();
    let _ = InvalidateRect(hwnd, None, false);

    // Act only if released over the control that was pressed.
    let Some(down) = down else { return };
    let Some((region, hit_rect)) = hit_at(x, y) else { return };
    if region == down {
        activate(hwnd, region, hit_rect);
    }
}

unsafe fn activate(hwnd: HWND, region: Region, hit_rect: RECT) {
    let main_hwnd = UI.with(|u| u.borrow().as_ref().map(|s| s.main_hwnd).unwrap_or_default());

    match region {
        Region::Ok | Region::Close => {
            let _ = DestroyWindow(hwnd);
            return;
        }
        Region::Minimize => {
            let _ = ShowWindow(hwnd, SW_MINIMIZE);
            return;
        }
        Region::SysMenu => {
            let anchor = POINT {
                x: hit_rect.left,
                y: hit_rect.bottom,
            };
            match system_menu(hwnd, anchor) {
                Some(Region::Minimize) => {
                    let _ = ShowWindow(hwnd, SW_MINIMIZE);
                }
                Some(Region::Close) => {
                    let _ = DestroyWindow(hwnd);
                }
                _ => {}
            }
            return;
        }
        Region::ThemePick => {
            let anchor = POINT {
                x: hit_rect.left,
                y: hit_rect.bottom,
            };
            if let Some(theme) = pick_theme(hwnd, anchor) {
                crate::SETTINGS.with(|s| {
                    let mut s = s.borrow_mut();
                    s.theme = theme;
                    s.save();
                });
                sync_theme();
                apply_corners(hwnd);
            }
        }
        Region::HotkeyChange => {
            set_capturing(true);
            set_notice(None);
        }
        Region::HotkeyReset => {
            set_capturing(false);
            let ok = apply_hotkey(main_hwnd, hotkey::DEFAULT_MODS, hotkey::DEFAULT_VK);
            set_notice(if ok {
                None
            } else {
                Some((
                    "Another app is holding the default shortcut.".to_string(),
                    NoticeKind::Error,
                ))
            });
        }
        Region::PillVi => {
            if !crate::hook::is_enabled() {
                send_cmd(main_hwnd, CMD_TOGGLE);
            }
        }
        Region::PillEn => {
            if crate::hook::is_enabled() {
                send_cmd(main_hwnd, CMD_TOGGLE);
            }
        }
        Region::ToggleQuickVn => send_cmd(main_hwnd, CMD_QUICKVN),
        Region::ToggleDeferred => {
            // Sub-option of Quick Vietnamese — ignore clicks while it's off.
            if crate::SETTINGS.with(|s| s.borrow().quick_vietnamese) {
                send_cmd(main_hwnd, CMD_DEFERRED);
            }
        }
        Region::ToggleAbbrev => send_cmd(main_hwnd, CMD_ABBREVIATIONS),
        Region::AbbrevEdit => crate::abbrev_dialog::open(hwnd),
        Region::ToggleAutostart => send_cmd(main_hwnd, CMD_AUTOSTART),
        Region::ToggleSound => send_cmd(main_hwnd, CMD_PLAYSOUND),
        Region::ToggleFix => send_cmd(main_hwnd, CMD_AUTOCOMPLETE),
        Region::ToggleStep => send_cmd(main_hwnd, CMD_STEP),
        Region::AppModeAll => edit_apps(hwnd, main_hwnd, |s| s.app_mode = AppMode::All),
        Region::AppModeOnly => edit_apps(hwnd, main_hwnd, |s| s.app_mode = AppMode::Only),
        Region::AppModeExcept => edit_apps(hwnd, main_hwnd, |s| s.app_mode = AppMode::Except),
        Region::AppRemove(i) => edit_apps(hwnd, main_hwnd, |s| s.remove_app_at(i)),
        Region::AppAdd => {
            let anchor = POINT {
                x: hit_rect.left,
                y: hit_rect.bottom,
            };
            if let Some(exe) = pick_app(hwnd, anchor) {
                edit_apps(hwnd, main_hwnd, |s| {
                    s.add_app(&exe);
                });
            }
        }
        // Handled on mouse-down.
        Region::ScrollUp
        | Region::ScrollDown
        | Region::PageUp
        | Region::PageDown
        | Region::ScrollThumb => {}
    }

    let _ = InvalidateRect(hwnd, None, false);
}

/// Apply an edit to the per-app rules: persist it, have the main window
/// re-evaluate the current app, and resize for the new list length.
unsafe fn edit_apps(hwnd: HWND, main_hwnd: HWND, edit: impl FnOnce(&mut Settings)) {
    crate::SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        edit(&mut s);
        s.save();
    });
    send_cmd(main_hwnd, CMD_APPS_CHANGED);
    fit_window(hwnd, false);
}

/// Pop up a menu of the apps with an open window (minus those already listed)
/// below `anchor` (client coordinates). Returns the chosen exe name.
unsafe fn pick_app(hwnd: HWND, anchor: POINT) -> Option<String> {
    const FIRST_APP: usize = 100;
    let listed = crate::SETTINGS.with(|s| s.borrow().active_app_list().to_vec());
    let apps: Vec<String> = crate::apps::running_apps()
        .into_iter()
        .filter(|a| !listed.contains(a))
        .collect();

    let menu = CreatePopupMenu().ok()?;
    if apps.is_empty() {
        let label = wide("No other open apps — open the app first");
        let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, PCWSTR(label.as_ptr()));
    }
    for (i, app) in apps.iter().enumerate() {
        let label = wide(app);
        let _ = AppendMenuW(menu, MF_STRING, FIRST_APP + i, PCWSTR(label.as_ptr()));
    }

    let cmd = track_menu(hwnd, menu, anchor);
    apps.get(cmd.checked_sub(FIRST_APP)?).cloned()
}

/// Pop up the theme list below `anchor` (client coordinates), the current
/// theme checked. Returns the chosen theme.
unsafe fn pick_theme(hwnd: HWND, anchor: POINT) -> Option<Theme> {
    const FIRST_THEME: usize = 100;
    let menu = CreatePopupMenu().ok()?;
    for (i, t) in Theme::ALL.iter().enumerate() {
        let label = wide(t.name());
        let flags = if *t == theme() { MF_STRING | MF_CHECKED } else { MF_STRING };
        let _ = AppendMenuW(menu, flags, FIRST_THEME + i, PCWSTR(label.as_ptr()));
    }
    let cmd = track_menu(hwnd, menu, anchor);
    Theme::ALL.get(cmd.checked_sub(FIRST_THEME)?).copied()
}

/// The Windows 3.1 system menu, below `anchor` (client coordinates). Returns
/// `Region::Minimize` or `Region::Close` for the chosen command.
unsafe fn system_menu(hwnd: HWND, anchor: POINT) -> Option<Region> {
    const MINIMIZE: usize = 1;
    const CLOSE: usize = 2;
    let menu = CreatePopupMenu().ok()?;
    let minimize = wide("Mi&nimize");
    let close = wide("&Close\tAlt+F4");
    let _ = AppendMenuW(menu, MF_STRING, MINIMIZE, PCWSTR(minimize.as_ptr()));
    let _ = AppendMenuW(menu, MF_STRING, CLOSE, PCWSTR(close.as_ptr()));
    match track_menu(hwnd, menu, anchor) {
        MINIMIZE => Some(Region::Minimize),
        CLOSE => Some(Region::Close),
        _ => None,
    }
}

/// Show `menu` below `anchor` (client coordinates), then destroy it. Returns
/// the chosen command id, or 0 if the menu was dismissed.
unsafe fn track_menu(hwnd: HWND, menu: HMENU, anchor: POINT) -> usize {
    let mut pt = anchor;
    let _ = ClientToScreen(hwnd, &mut pt);
    let cmd = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_NONOTIFY | TPM_LEFTALIGN | TPM_TOPALIGN,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    )
    .0 as usize;
    let _ = DestroyMenu(menu);
    cmd
}

/// Reuse the main window's existing WM_COMMAND handlers so all state mutation
/// (persist, apply to hook, update tray) stays in one place.
unsafe fn send_cmd(main_hwnd: HWND, cmd: usize) {
    use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_COMMAND};
    SendMessageW(main_hwnd, WM_COMMAND, WPARAM(cmd), LPARAM(0));
}

/// Handle a keypress. Returns true if it was consumed (during hotkey capture).
unsafe fn on_key(hwnd: HWND, vk: u32) -> bool {
    let capturing = UI.with(|u| u.borrow().as_ref().map(|s| s.capturing).unwrap_or(false));
    if !capturing {
        return false;
    }

    // Wait for a non-modifier key; ignore modifiers pressed on their own.
    if hotkey::is_modifier_vk(vk) {
        return true;
    }

    // Escape cancels capture without changing anything.
    if vk == 0x1B {
        set_capturing(false);
        set_notice(None);
        let _ = InvalidateRect(hwnd, None, false);
        return true;
    }

    let mods = current_mods();

    // Refuse combinations that would misbehave once they are global — above all
    // a modifier-less key, which RegisterHotKey would accept and then swallow in
    // every application. Stay in capture mode so the user can simply try again.
    let warning = match hotkey::validate(mods, vk) {
        hotkey::Validity::Reject(why) => {
            set_notice(Some((why.to_string(), NoticeKind::Error)));
            let _ = InvalidateRect(hwnd, None, false);
            return true;
        }
        hotkey::Validity::Warn(why) => Some(why),
        hotkey::Validity::Ok => None,
    };

    let main_hwnd = UI.with(|u| u.borrow().as_ref().map(|s| s.main_hwnd).unwrap_or_default());
    set_capturing(false);

    if apply_hotkey(main_hwnd, mods, vk) {
        set_notice(warning.map(|w| (w.to_string(), NoticeKind::Warn)));
    } else {
        set_notice(Some((
            "That shortcut is already in use by another app. Try a different one.".to_string(),
            NoticeKind::Error,
        )));
    }
    let _ = InvalidateRect(hwnd, None, false);
    true
}

/// Handle a key release. Returns true if it was consumed (during capture).
///
/// Letting go of two or more modifiers without having pressed anything else
/// records a modifier-only shortcut — the Ctrl+Shift style most Vietnamese IMEs
/// use. A single modifier is ignored so the user can reach for the second one.
unsafe fn on_key_up(hwnd: HWND, vk: u32) -> bool {
    let capturing = UI.with(|u| u.borrow().as_ref().map(|s| s.capturing).unwrap_or(false));
    if !capturing {
        return false;
    }

    let Some(bit) = hotkey::mod_bit(vk) else { return false };

    // The key being released is already up as far as GetKeyState is concerned.
    let mods = current_mods() | bit;
    if mods.count_ones() < 2 {
        return true;
    }

    let warning = match hotkey::validate(mods, hotkey::VK_NONE) {
        hotkey::Validity::Reject(why) => {
            set_notice(Some((why.to_string(), NoticeKind::Error)));
            let _ = InvalidateRect(hwnd, None, false);
            return true;
        }
        hotkey::Validity::Warn(why) => Some(why),
        hotkey::Validity::Ok => None,
    };

    let main_hwnd = UI.with(|u| u.borrow().as_ref().map(|s| s.main_hwnd).unwrap_or_default());
    set_capturing(false);

    if apply_hotkey(main_hwnd, mods, hotkey::VK_NONE) {
        set_notice(warning.map(|w| (w.to_string(), NoticeKind::Warn)));
    } else {
        set_notice(Some((
            "That shortcut could not be set. Try a different one.".to_string(),
            NoticeKind::Error,
        )));
    }
    let _ = InvalidateRect(hwnd, None, false);
    true
}

/// Ask the main window to (re)register the global hotkey. Returns false when the
/// combination could not be bound (the previous one stays live).
unsafe fn apply_hotkey(main_hwnd: HWND, mods: u32, vk: u32) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::SendMessageW;
    SendMessageW(
        main_hwnd,
        crate::WM_APP_SETHOTKEY,
        WPARAM(mods as usize),
        LPARAM(vk as isize),
    )
    .0 != 0
}

/// Read the currently-held modifiers into a `RegisterHotKey` bitmask.
unsafe fn current_mods() -> u32 {
    let mut m = 0u32;
    if key_down(VK_CONTROL.0) {
        m |= hotkey::MOD_CONTROL;
    }
    if key_down(VK_MENU.0) {
        m |= hotkey::MOD_ALT;
    }
    if key_down(VK_SHIFT.0) {
        m |= hotkey::MOD_SHIFT;
    }
    if key_down(VK_LWIN.0) || key_down(VK_RWIN.0) {
        m |= hotkey::MOD_WIN;
    }
    m
}

unsafe fn key_down(vk: u16) -> bool {
    (GetKeyState(vk as i32) as u16 & 0x8000) != 0
}

// MARK: - Painting

/// Everything `paint` renders, snapshotted up front.
struct View {
    enabled: bool,
    fix: bool,
    autostart: bool,
    step: bool,
    sound: bool,
    quick_vn: bool,
    deferred: bool,
    abbrev: bool,
    abbrev_count: usize,
    hotkey_text: String,
    hotkey_bound: bool,
    app_mode: AppMode,
    app_list: Vec<String>,
    capturing: bool,
    notice: Option<(String, NoticeKind)>,
    active: bool,
    /// The control currently drawn pressed.
    down: Option<Region>,
}

impl View {
    fn is_down(&self, r: Region) -> bool {
        self.down == Some(r)
    }
}

struct Fonts {
    title: HFONT,
    group: HFONT,
    body: HFONT,
    bold: HFONT,
    small: HFONT,
    italic: HFONT,
    lang: HFONT,
    badge: HFONT,
    mono: HFONT,
    status: HFONT,
    status_bold: HFONT,
    /// Caption-button and arrow glyphs (see `glyph`): Marlett, the system font
    /// that holds the classic ones, or Segoe MDL2 Assets in the flat theme.
    glyph: HFONT,
}

impl Fonts {
    /// The current theme's fonts.
    unsafe fn new() -> Self {
        match theme() {
            Theme::Classic => Fonts {
                title: font("Tahoma", -14, 700, false),
                group: font("Tahoma", -14, 700, false),
                body: font("Tahoma", -14, 400, false),
                bold: font("Tahoma", -14, 700, false),
                small: font("Tahoma", -12, 400, false),
                italic: font("Tahoma", -13, 400, true),
                lang: font("Tahoma", -16, 700, false),
                badge: font("Tahoma", -13, 700, false),
                mono: font("Courier New", -15, 700, false),
                status: font("Courier New", -13, 400, false),
                status_bold: font("Courier New", -14, 700, false),
                glyph: marlett(-11),
            },
            // The bold dialog face of Windows 3.1 (unsmoothed, see `font`).
            Theme::Win31 => {
                let ui = "Microsoft Sans Serif";
                Fonts {
                    title: font(ui, -14, 700, false),
                    group: font(ui, -14, 700, false),
                    body: font(ui, -14, 700, false),
                    bold: font(ui, -14, 700, false),
                    small: font(ui, -12, 400, false),
                    italic: font(ui, -13, 400, false),
                    lang: font(ui, -16, 700, false),
                    badge: font(ui, -13, 700, false),
                    mono: font("Courier New", -15, 700, false),
                    status: font(ui, -12, 400, false),
                    status_bold: font(ui, -13, 700, false),
                    glyph: marlett(-11),
                }
            }
            Theme::Flat => {
                let ui = "Segoe UI";
                Fonts {
                    title: font(ui, -12, 400, false),
                    group: font(ui, -14, 600, false),
                    body: font(ui, -14, 400, false),
                    bold: font(ui, -14, 600, false),
                    small: font(ui, -12, 400, false),
                    italic: font(ui, -13, 400, false),
                    lang: font(ui, -15, 600, false),
                    badge: font(ui, -13, 700, false),
                    mono: font("Consolas", -15, 400, false),
                    status: font(ui, -12, 400, false),
                    status_bold: font(ui, -12, 700, false),
                    glyph: font("Segoe MDL2 Assets", -10, 400, false),
                }
            }
            // Charcoal and Geneva aren't on Windows; Arial and Verdana stand in.
            Theme::Platinum => Fonts {
                title: font("Arial", -13, 700, false),
                group: font("Arial", -13, 700, false),
                body: font("Verdana", -12, 400, false),
                bold: font("Verdana", -12, 700, false),
                small: font("Verdana", -11, 400, false),
                italic: font("Verdana", -11, 400, true),
                lang: font("Arial", -15, 700, false),
                badge: font("Arial", -12, 700, false),
                mono: font("Courier New", -14, 700, false),
                status: font("Verdana", -11, 400, false),
                status_bold: font("Verdana", -11, 700, false),
                glyph: marlett(-11),
            },
        }
    }

    unsafe fn delete(&self) {
        for f in [
            self.title,
            self.group,
            self.body,
            self.bold,
            self.small,
            self.italic,
            self.lang,
            self.badge,
            self.mono,
            self.status,
            self.status_bold,
            self.glyph,
        ] {
            let _ = DeleteObject(f);
        }
    }
}

/// A glyph in `Fonts::glyph`.
#[derive(Clone, Copy)]
enum Glyph {
    Minimize,
    Maximize,
    Close,
    Up,
    Down,
}

fn glyph(g: Glyph) -> &'static str {
    if theme() == Theme::Flat {
        // Segoe MDL2 Assets: the Windows 10/11 caption buttons and chevrons.
        match g {
            Glyph::Minimize => "\u{E921}",
            Glyph::Maximize => "\u{E922}",
            Glyph::Close => "\u{E8BB}",
            Glyph::Up => "\u{E70E}",
            Glyph::Down => "\u{E70D}",
        }
    } else {
        // Marlett.
        match g {
            Glyph::Minimize => "0",
            Glyph::Maximize => "1",
            Glyph::Close => "r",
            Glyph::Up => "5",
            Glyph::Down => "6",
        }
    }
}

unsafe fn paint(hwnd: HWND) {
    // Snapshot the state we render.
    sync_theme();
    let (autostart, step, sound, quick_vn, deferred, abbrev, mods, vk, app_mode, app_list) =
        crate::SETTINGS.with(|s| {
            let s = s.borrow();
            (
                s.start_with_windows,
                s.step_by_step,
                s.play_sound,
                s.quick_vietnamese,
                s.deferred_diacritics,
                s.abbreviations,
                s.hotkey_mods,
                s.hotkey_vk,
                s.app_mode,
                s.active_app_list().to_vec(),
            )
        });
    let (capturing, notice, scroll, active, down) = UI.with(|u| {
        let b = u.borrow();
        match b.as_ref() {
            Some(s) => (
                s.capturing,
                s.notice.clone(),
                s.scroll,
                s.active,
                s.down.filter(|_| s.down_over),
            ),
            None => (false, None, 0, true, None),
        }
    });
    let v = View {
        enabled: crate::hook::is_enabled(),
        fix: crate::hook::is_fix_autocomplete(),
        autostart,
        step,
        sound,
        quick_vn,
        deferred,
        abbrev,
        abbrev_count: crate::hook::abbreviations().len(),
        // No key name contains '+', so this only spaces out the separators.
        hotkey_text: hotkey::describe(mods, vk).replace('+', " + "),
        hotkey_bound: crate::hotkey_bound(),
        app_mode,
        app_list,
        capturing,
        notice,
        active,
        down,
    };
    let p = pal();

    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    let (cw, ch) = (client.right, client.bottom);

    // Viewport between the title bar and the status bar (device pixels).
    let view_top = px(VIEW_TOP);
    let view_bottom = (ch - px(STATUS_H + FRAME)).max(view_top);
    let view_h = view_bottom - view_top;
    let content_px = px(content_height());
    let max_scroll = (content_px - view_h).max(0);
    let scroll = scroll.clamp(0, max_scroll);
    let has_bar = max_scroll > 0;
    let view_left = px(FRAME);
    let view_right = cw - px(FRAME) - if has_bar { px(SCROLLBAR_W) } else { 0 };
    let view = rect_px(view_left, view_top, view_right, view_bottom);

    let fonts = Fonts::new();

    // Content, painted at full height into its own bitmap.
    let cdc = CreateCompatibleDC(hdc);
    let cbmp = CreateCompatibleBitmap(hdc, (view_right - view_left).max(1), content_px.max(1));
    let old_cbmp = SelectObject(cdc, cbmp);
    fill_solid(cdc, rect_px(0, 0, view_right - view_left, content_px), p.bg);
    let mut content_hits = Vec::new();
    paint_content(cdc, unpx(view_right - view_left), &v, &fonts, &mut content_hits);

    // The frame: chrome plus the visible slice of the content.
    let fdc = CreateCompatibleDC(hdc);
    let fbmp = CreateCompatibleBitmap(hdc, cw, ch);
    let old_fbmp = SelectObject(fdc, fbmp);
    fill_solid(fdc, client, p.bg);
    let _ = BitBlt(fdc, view_left, view_top, view.right - view.left, view_h, cdc, 0, scroll, SRCCOPY);

    // Content hits move into client coordinates, clipped to the viewport.
    let mut hits: Vec<(Region, RECT)> = content_hits
        .into_iter()
        .filter_map(|(region, r)| {
            let r = rect_px(
                r.left + view_left,
                r.top + view_top - scroll,
                r.right + view_left,
                r.bottom + view_top - scroll,
            );
            let clipped = rect_px(
                r.left.max(view.left),
                r.top.max(view.top),
                r.right.min(view.right),
                r.bottom.min(view.bottom),
            );
            (clipped.top < clipped.bottom).then_some((region, clipped))
        })
        .collect();

    let geom = paint_chrome(fdc, cw, ch, &v, &fonts, &mut hits, (view, content_px, scroll));

    let _ = BitBlt(hdc, 0, 0, cw, ch, fdc, 0, 0, SRCCOPY);

    SelectObject(cdc, old_cbmp);
    let _ = DeleteObject(cbmp);
    let _ = DeleteDC(cdc);
    SelectObject(fdc, old_fbmp);
    let _ = DeleteObject(fbmp);
    let _ = DeleteDC(fdc);
    fonts.delete();
    let _ = EndPaint(hwnd, &ps);

    // Record hit rectangles (and the clamped scroll) for the next click.
    UI.with(|u| {
        if let Some(s) = u.borrow_mut().as_mut() {
            s.hits = hits;
            s.scroll = scroll;
            s.scroll_geom = geom;
        }
    });
}

/// Paint the scrolling content: the group boxes and the OK button. `cw` is
/// the content width in layout units; hit rects are recorded in
/// content-bitmap device pixels.
unsafe fn paint_content(hdc: HDC, cw: i32, v: &View, f: &Fonts, hits: &mut Vec<(Region, RECT)>) {
    let p = pal();
    let gl = MARGIN;
    let gr = cw - MARGIN;
    let ix = gl + GROUP_PAD;
    let ir = gr - GROUP_PAD;
    let mut y = MARGIN;

    // ----- Group: Input Method Selection -----
    {
        group_box(hdc, lrect(gl, y, gr, y + INPUT_GROUP_H), "Input Method Selection", f.group);
        let mut cy = y + GROUP_HEAD;

        // V / E language buttons.
        let half = (ir - ix - 10) / 2;
        let vi = rect(ix, cy, ix + half, cy + 40);
        let en = rect(ir - half, cy, ir, cy + 40);
        lang_button(hdc, vi, "V", "Tiếng Việt", v.enabled, v.is_down(Region::PillVi), f);
        lang_button(hdc, en, "E", "English", !v.enabled, v.is_down(Region::PillEn), f);
        hits.push((Region::PillVi, vi));
        hits.push((Region::PillEn, en));
        cy += 52;

        // Toggle-shortcut panel: label · current value · [Reset] [Change…].
        let panel = rect(ix, cy, ir, cy + 44);
        fill_solid(hdc, panel, p.panel);
        bevel(hdc, panel, Bevel::Thin);
        SelectObject(hdc, f.bold);
        draw_text(hdc, "Shortcut:", rect(ix + 10, cy, ix + 90, cy + 44), p.text, LEFT);

        let change = rect(ir - 8 - 92, cy + 8, ir - 8, cy + 36);
        let change_down = v.capturing || v.is_down(Region::HotkeyChange);
        push_button(hdc, change, "Change…", f.bold, change_down, false);
        hits.push((Region::HotkeyChange, change));

        // Reset — the only way back from a combination that can't be pressed.
        let reset = rect(ir - 8 - 92 - 6 - 64, cy + 8, ir - 8 - 92 - 6, cy + 36);
        push_button(hdc, reset, "Reset", f.body, v.is_down(Region::HotkeyReset), false);
        hits.push((Region::HotkeyReset, reset));

        let field = rect_px(px(ix + 90), px(cy + 8), reset.left - px(10), px(cy + 36));
        let inner = bevel(hdc, field, Bevel::Sunken);
        fill_solid(hdc, inner, p.field);
        SelectObject(hdc, f.mono);
        let shown = if v.capturing { "Press keys…" } else { v.hotkey_text.as_str() };
        draw_text(hdc, shown, pad_x(inner, px(8)), p.accent_text, LEFT | DT_END_ELLIPSIS);
        cy += 48;

        // Status line: validation feedback, capture hint, or a standing warning
        // that the shortcut never registered.
        let status = match (&v.notice, v.capturing, v.hotkey_bound) {
            (Some((text, kind)), _, _) => Some((text.clone(), *kind)),
            (None, true, _) => Some((
                "Press a combination, or just Ctrl+Shift. Esc to cancel.".to_string(),
                NoticeKind::Hint,
            )),
            (None, false, false) => Some((
                "Not registered — another app owns this combination.".to_string(),
                NoticeKind::Error,
            )),
            (None, false, true) => None,
        };
        if let Some((text, kind)) = status {
            SelectObject(hdc, f.small);
            let color = match kind {
                NoticeKind::Hint => p.text2,
                NoticeKind::Warn => p.warn,
                NoticeKind::Error => p.danger,
            };
            draw_text(hdc, &text, rect(ix + 2, cy, ir, cy + 16), color, LEFT | DT_END_ELLIPSIS);
        }
        cy += 24;

        // Input mode (Telex only, for now): a read-only field.
        SelectObject(hdc, f.body);
        draw_text(hdc, "Input Mode:", rect(ix, cy, ix + 160, cy + 26), p.text, LEFT);
        let mode = rect(ir - 200, cy, ir, cy + 26);
        let inner = bevel(hdc, mode, Bevel::Sunken);
        fill_solid(hdc, inner, p.field);
        draw_text(hdc, "Telex", pad_x(inner, px(8)), p.text, LEFT);
        cy += 36;

        etched_line(hdc, rect(ix, cy, ir, cy));
        cy += 12;

        // Quick Vietnamese, then its sub-option Deferred diacritics — indented,
        // and greyed while Quick Vietnamese is off (clicks are ignored then too).
        let r = checkbox_row(
            hdc,
            (ix, ir, cy),
            "Quick Vietnamese",
            Some("Type w → ư after an initial consonant (tw→tư, chw→chư)"),
            (v.quick_vn, true, v.is_down(Region::ToggleQuickVn)),
            f,
        );
        hits.push((Region::ToggleQuickVn, r));
        cy += 46;
        let r = checkbox_row(
            hdc,
            (ix + 20, ir, cy),
            "Deferred diacritics (Bỏ dấu sau)",
            Some("Marks typed later apply backward (did→đi, thana→thân)"),
            (v.quick_vn && v.deferred, v.quick_vn, v.is_down(Region::ToggleDeferred)),
            f,
        );
        hits.push((Region::ToggleDeferred, r));

        y += INPUT_GROUP_H + GROUP_GAP;
    }

    // ----- Group: Abbreviations (gõ tắt) -----
    {
        group_box(hdc, lrect(gl, y, gr, y + ABBREV_GROUP_H), "Abbreviations", f.group);
        let mut cy = y + GROUP_HEAD;
        let r = checkbox_row(
            hdc,
            (ix, ir, cy),
            "Expand abbreviations",
            Some("Type a short form, then Tab (btw → by the way)"),
            (v.abbrev, true, v.is_down(Region::ToggleAbbrev)),
            f,
        );
        hits.push((Region::ToggleAbbrev, r));
        cy += CAPTIONED_ROW_H + 8;

        let edit = rect(ix, cy, ix + 110, cy + 28);
        push_button(hdc, edit, "Edit List…", f.body, v.is_down(Region::AbbrevEdit), false);
        hits.push((Region::AbbrevEdit, edit));
        let count = match v.abbrev_count {
            0 => "No abbreviations yet".to_string(),
            1 => "1 abbreviation".to_string(),
            n => format!("{n} abbreviations"),
        };
        SelectObject(hdc, f.italic);
        draw_text(hdc, &count, rect(ix + 122, cy, ir, cy + 28), p.text2, LEFT | DT_END_ELLIPSIS);

        y += ABBREV_GROUP_H + GROUP_GAP;
    }

    // ----- Group: Target Application Scope (per-app rules) -----
    {
        let h = scope_group_height(v.app_mode, v.app_list.len());
        group_box(hdc, lrect(gl, y, gr, y + h), "Target Application Scope", f.group);
        let mut cy = y + GROUP_HEAD;

        // Mode buttons: the selected one stays pushed in.
        let gap = 6;
        let seg_w = (ir - ix - 2 * gap) / 3;
        let modes = [
            ("All Apps", AppMode::All, Region::AppModeAll),
            ("Only These", AppMode::Only, Region::AppModeOnly),
            ("All Except", AppMode::Except, Region::AppModeExcept),
        ];
        for (i, (label, mode, region)) in modes.into_iter().enumerate() {
            let left = ix + i as i32 * (seg_w + gap);
            let r = rect(left, cy, left + seg_w, cy + 32);
            if v.app_mode == mode {
                selected_button(hdc, r, label, f);
            } else {
                push_button(hdc, r, label, f.body, v.is_down(region), false);
            }
            hits.push((region, r));
        }
        cy += 32 + 10;

        let caption = match v.app_mode {
            AppMode::All => "Vietnamese typing works in every app.",
            AppMode::Only => "Vietnamese typing only in the apps below.",
            AppMode::Except => "Vietnamese typing everywhere except the apps below.",
        };
        let field = rect(ix, cy, ir, cy + 26);
        let inner = bevel(hdc, field, Bevel::Sunken);
        fill_solid(hdc, inner, p.field_soft);
        SelectObject(hdc, f.italic);
        draw_text(hdc, caption, pad_x(inner, px(8)), p.text2, LEFT | DT_END_ELLIPSIS);
        cy += 26;

        if v.app_mode != AppMode::All {
            cy += 10;
            // List box of app rows, each with its own Remove button.
            let list = rect(ix, cy, ir, cy + app_list_height(v.app_list.len()));
            let inner = bevel(hdc, list, Bevel::Sunken);
            fill_solid(hdc, inner, p.field);
            let mut ry = cy + 2;
            if v.app_list.is_empty() {
                SelectObject(hdc, f.italic);
                draw_text(
                    hdc,
                    "No apps yet — add one below, or from the tray menu.",
                    rect(ix + 10, ry, ir - 10, ry + APP_ROW_H),
                    p.text2,
                    LEFT | DT_END_ELLIPSIS,
                );
            }
            for (i, app) in v.app_list.iter().enumerate() {
                SelectObject(hdc, f.body);
                draw_text(hdc, app, rect(ix + 10, ry, ir - 90, ry + APP_ROW_H), p.text, LEFT | DT_END_ELLIPSIS);
                let remove = rect(ir - 6 - 72, ry + 4, ir - 6, ry + APP_ROW_H - 4);
                let region = Region::AppRemove(i);
                push_button(hdc, remove, "Remove", f.small, v.is_down(region), false);
                hits.push((region, remove));
                ry += APP_ROW_H;
            }
            cy += app_list_height(v.app_list.len()) + 8;

            let add = rect(ix, cy, ix + 110, cy + 28);
            push_button(hdc, add, "Add App…", f.body, v.is_down(Region::AppAdd), false);
            hits.push((Region::AppAdd, add));
        }

        y += h + GROUP_GAP;
    }

    // ----- Group: General & System -----
    {
        group_box(hdc, lrect(gl, y, gr, y + TWO_ROW_GROUP_H), "General & System", f.group);
        let cy = y + GROUP_HEAD;
        let r = checkbox_row(
            hdc,
            (ix, ir, cy),
            "Start automatically with Windows",
            None,
            (v.autostart, true, v.is_down(Region::ToggleAutostart)),
            f,
        );
        hits.push((Region::ToggleAutostart, r));
        let r = checkbox_row(
            hdc,
            (ix, ir, cy + CHECK_ROW_H + 8),
            "Play sound on switch",
            None,
            (v.sound, true, v.is_down(Region::ToggleSound)),
            f,
        );
        hits.push((Region::ToggleSound, r));
        y += TWO_ROW_GROUP_H + GROUP_GAP;
    }

    // ----- Group: Appearance -----
    {
        group_box(hdc, lrect(gl, y, gr, y + APPEARANCE_GROUP_H), "Appearance", f.group);
        let cy = y + GROUP_HEAD;
        SelectObject(hdc, f.body);
        draw_text(hdc, "Theme:", rect(ix, cy, ix + 160, cy + 26), p.text, LEFT);
        let pick = rect(ir - 200, cy, ir, cy + 26);
        combo(hdc, pick, theme().name(), v.is_down(Region::ThemePick), f);
        hits.push((Region::ThemePick, pick));
        y += APPEARANCE_GROUP_H + GROUP_GAP;
    }

    // ----- Group: Compatibility Flags -----
    {
        group_box(hdc, lrect(gl, y, gr, y + TWO_ROW_GROUP_H), "Compatibility Flags", f.group);
        let cy = y + GROUP_HEAD;
        let r = checkbox_row(
            hdc,
            (ix, ir, cy),
            "Fix browser autocomplete",
            None,
            (v.fix, true, v.is_down(Region::ToggleFix)),
            f,
        );
        hits.push((Region::ToggleFix, r));
        let r = checkbox_row(
            hdc,
            (ix, ir, cy + CHECK_ROW_H + 8),
            "Send keys step-by-step",
            None,
            (v.step, true, v.is_down(Region::ToggleStep)),
            f,
        );
        hits.push((Region::ToggleStep, r));
        y += TWO_ROW_GROUP_H + GROUP_GAP;
    }

    // ----- Footer: settings apply at once, so OK just closes -----
    etched_line(hdc, rect(gl, y, gr, y));
    let ok = rect(gr - 96, y + 12, gr, y + 42);
    push_button(hdc, ok, "OK", f.bold, v.is_down(Region::Ok), true);
    hits.push((Region::Ok, ok));
}

/// Paint the fixed chrome over the frame bitmap: window frame, title bar,
/// scroll bar and status bar. `scroll_view` is (viewport, content height,
/// scroll) in device pixels. Returns the scroll-bar geometry.
unsafe fn paint_chrome(
    hdc: HDC,
    cw: i32,
    ch: i32,
    v: &View,
    f: &Fonts,
    hits: &mut Vec<(Region, RECT)>,
    scroll_view: (RECT, i32, i32),
) -> ScrollGeom {
    let p = pal();
    let t = line();
    bevel(hdc, rect_px(0, 0, cw, ch), Bevel::Window);
    if theme() == Theme::Win31 {
        // A dialog frame takes the caption color: navy while active.
        let band = if v.active { p.title_active.0 } else { p.title_inactive.0 };
        ring(hdc, inset(rect_px(0, 0, cw, ch), t), px(FRAME) - 2 * t, band);
    }
    paint_title_bar(hdc, cw, v, f, hits);

    // Scroll bar, when the content is taller than the viewport.
    let (view, content_px, scroll) = scroll_view;
    let view_h = view.bottom - view.top;
    let max_scroll = (content_px - view_h).max(0);
    let mut geom = ScrollGeom {
        travel: 0,
        max_scroll,
        page: (view_h - px(ARROW_STEP)).max(px(ARROW_STEP)),
    };
    if max_scroll > 0 {
        let (l, r) = (view.right, view.right + px(SCROLLBAR_W));
        let arrow = px(SCROLLBAR_W);
        let up = rect_px(l, view.top, r, view.top + arrow);
        let dn = rect_px(l, view.bottom - arrow, r, view.bottom);
        for (b, g, region) in [(up, Glyph::Up, Region::ScrollUp), (dn, Glyph::Down, Region::ScrollDown)] {
            scroll_arrow(hdc, b, g, f);
            hits.push((region, b));
        }

        let track = rect_px(l, up.bottom, r, dn.top);
        fill_solid(hdc, track, p.track);
        let track_len = track.bottom - track.top;
        let thumb_len = if theme() == Theme::Win31 {
            // A square box, whatever the content length.
            arrow.min(track_len)
        } else {
            (track_len * view_h / content_px.max(1)).clamp(px(12).min(track_len), track_len)
        };
        geom.travel = track_len - thumb_len;
        let thumb_top = track.top + geom.travel * scroll / max_scroll;
        let thumb = rect_px(l, thumb_top, r, thumb_top + thumb_len);
        scroll_thumb(hdc, thumb);
        hits.push((Region::ScrollThumb, thumb));
        hits.push((Region::PageUp, rect_px(l, track.top, r, thumb.top)));
        hits.push((Region::PageDown, rect_px(l, thumb.bottom, r, track.bottom)));
    }

    // Status bar: a message pane and the current language.
    let st = rect_px(px(FRAME), ch - px(FRAME + STATUS_H - 4), cw - px(FRAME), ch - px(FRAME));
    let lang = rect_px(st.right - px(52), st.top, st.right, st.bottom);
    let msg = rect_px(st.left, st.top, lang.left - px(3), st.bottom);
    let message = if v.capturing {
        "Recording shortcut…".to_string()
    } else if !v.hotkey_bound {
        "Shortcut not registered".to_string()
    } else {
        format!("NovaKey v{} - Ready", env!("CARGO_PKG_VERSION"))
    };
    let lang_text = if v.enabled { "VN" } else { "EN" };
    if theme() == Theme::Flat {
        // No panes: plain text under a hairline.
        let y = st.top - px(2);
        fill_solid(hdc, rect_px(line(), y, cw - line(), y + line()), p.light);
        SelectObject(hdc, f.status);
        draw_text(hdc, &message, pad_x(msg, px(6)), p.text2, LEFT | DT_END_ELLIPSIS);
        SelectObject(hdc, f.status_bold);
        draw_text(hdc, lang_text, lang, p.accent_text, CENTER);
    } else if theme() == Theme::Win31 {
        // A gray strip under a black rule, holding recessed panes.
        let strip = rect_px(st.left, st.top - px(4), st.right, st.bottom);
        fill_solid(hdc, strip, p.face);
        fill_solid(hdc, rect_px(strip.left, strip.top, strip.right, strip.top + t), p.dark);
        let msg = rect_px(msg.left + px(4), st.top, msg.right, st.bottom - px(4));
        let lang = rect_px(lang.left, st.top, lang.right - px(4), st.bottom - px(4));
        edge(hdc, msg, p.shadow, p.hilite);
        edge(hdc, lang, p.shadow, p.hilite);
        SelectObject(hdc, f.status);
        draw_text(hdc, &message, pad_x(inset(msg, t), px(5)), p.text, LEFT | DT_END_ELLIPSIS);
        SelectObject(hdc, f.status_bold);
        draw_text(hdc, lang_text, inset(lang, t), p.accent_text, CENTER);
    } else {
        let inner = bevel(hdc, msg, Bevel::Thin);
        SelectObject(hdc, f.status);
        draw_text(hdc, &message, pad_x(inner, px(6)), p.text, LEFT | DT_END_ELLIPSIS);
        let inner = bevel(hdc, lang, Bevel::Thin);
        SelectObject(hdc, f.status_bold);
        draw_text(hdc, lang_text, inner, p.accent_text, CENTER);
    }

    geom
}

/// A title-bar button.
#[derive(Clone, Copy, PartialEq)]
enum Caption {
    Minimize,
    /// Inert: the window has a fixed size.
    Maximize,
    Close,
    /// The Windows 3.1 system-menu box, which stands in for a close button.
    SysMenu,
}

impl Caption {
    /// What a click on the button does; `None` for the inert maximize.
    fn region(self) -> Option<Region> {
        match self {
            Caption::Minimize => Some(Region::Minimize),
            Caption::Maximize => None,
            Caption::Close => Some(Region::Close),
            Caption::SysMenu => Some(Region::SysMenu),
        }
    }

    fn glyph(self) -> Glyph {
        match self {
            Caption::Minimize => Glyph::Minimize,
            Caption::Maximize => Glyph::Maximize,
            Caption::Close | Caption::SysMenu => Glyph::Close,
        }
    }
}

/// The title bar, in client device pixels. The flat theme's runs edge to
/// edge; the others sit inside the window frame.
fn title_bar(cw: i32) -> RECT {
    if theme() == Theme::Flat {
        let t = line();
        return rect_px(t, t, cw - t, px(FRAME + TITLE_H));
    }
    rect_px(px(FRAME), px(FRAME), cw - px(FRAME), px(FRAME + TITLE_H))
}

/// The current theme's caption buttons, in client device pixels.
fn title_buttons(cw: i32) -> Vec<(Caption, RECT)> {
    let tb = title_bar(cw);
    let bar_h = tb.bottom - tb.top;
    match theme() {
        Theme::Classic => {
            let (bw, bh) = (px(20), px(18));
            let top = tb.top + (bar_h - bh) / 2;
            let close = rect_px(tb.right - px(3) - bw, top, tb.right - px(3), top + bh);
            let max = rect_px(close.left - px(2) - bw, top, close.left - px(2), top + bh);
            let min = rect_px(max.left - bw, top, max.left, top + bh);
            vec![(Caption::Minimize, min), (Caption::Maximize, max), (Caption::Close, close)]
        }
        // Square buttons the height of the bar: the system-menu box on the
        // left, minimize on the right. A window that cannot be resized shows
        // no maximize, as with Control Panel.
        Theme::Win31 => {
            let sys = rect_px(tb.left, tb.top, tb.left + bar_h, tb.bottom);
            let min = rect_px(tb.right - bar_h, tb.top, tb.right, tb.bottom);
            vec![(Caption::SysMenu, sys), (Caption::Minimize, min)]
        }
        // Wide, full-height buttons; a fixed-size window shows no maximize.
        Theme::Flat => {
            let w = px(46);
            let close = rect_px(tb.right - w, tb.top, tb.right, tb.bottom);
            let min = rect_px(close.left - w, tb.top, close.left, tb.bottom);
            vec![(Caption::Minimize, min), (Caption::Close, close)]
        }
        // Small boxes: close on the left; zoom, then collapse, on the right.
        Theme::Platinum => {
            let b = px(13);
            let top = tb.top + (bar_h - b) / 2;
            let close = rect_px(tb.left + px(6), top, tb.left + px(6) + b, top + b);
            let collapse = rect_px(tb.right - px(6) - b, top, tb.right - px(6), top + b);
            let zoom = rect_px(collapse.left - px(6) - b, top, collapse.left - px(6), top + b);
            vec![(Caption::Close, close), (Caption::Maximize, zoom), (Caption::Minimize, collapse)]
        }
    }
}

/// The title bar: background, title and caption buttons.
unsafe fn paint_title_bar(hdc: HDC, cw: i32, v: &View, f: &Fonts, hits: &mut Vec<(Region, RECT)>) {
    let p = pal();
    let t = line();
    let tb = title_bar(cw);
    let (c1, c2) = if v.active { p.title_active } else { p.title_inactive };
    let color = if v.active { p.title_text } else { p.title_inactive_text };
    gradient(hdc, tb, c1, c2, false);

    // The free stretch of the bar between the buttons on either side.
    let buttons = title_buttons(cw);
    let (mut left, mut right) = (tb.left, tb.right);
    for (_, r) in &buttons {
        if r.right <= cw / 2 {
            left = left.max(r.right);
        } else {
            right = right.min(r.left);
        }
    }

    SelectObject(hdc, f.title);
    let title = format!("NovaKey Settings - v{}", env!("CARGO_PKG_VERSION"));
    match theme() {
        Theme::Classic | Theme::Flat => {
            let icon_sz = px(16);
            let icon_x = left + px(if theme() == Theme::Flat { 10 } else { 4 });
            draw_app_icon(hdc, icon_x, tb.top + (tb.bottom - tb.top - icon_sz) / 2, icon_sz);
            draw_text(
                hdc,
                &title,
                rect_px(icon_x + icon_sz + px(6), tb.top, right - px(4), tb.bottom),
                color,
                LEFT | DT_END_ELLIPSIS,
            );
        }
        Theme::Win31 => {
            draw_text(
                hdc,
                &title,
                rect_px(left + px(4), tb.top, right - px(4), tb.bottom),
                color,
                CENTER | DT_END_ELLIPSIS,
            );
            fill_solid(hdc, rect_px(tb.left, tb.bottom, tb.right, tb.bottom + t), p.dark);
        }
        Theme::Platinum => {
            let (l, r) = (left + px(6), right - px(6));
            if v.active {
                // Pinstripes: a ridge on every third line.
                let mut y = tb.top + px(5);
                while y + 2 * t <= tb.bottom - px(5) {
                    fill_solid(hdc, rect_px(l, y, r, y + t), p.hilite);
                    fill_solid(hdc, rect_px(l, y + t, r, y + 2 * t), p.shadow);
                    y += 3 * t;
                }
            }
            // The title sits on a plain patch cut out of the stripes.
            let w = (text_width(hdc, &title) + px(16)).min(r - l);
            let x = l + (r - l - w) / 2;
            let patch = rect_px(x, tb.top, x + w, tb.bottom);
            fill_solid(hdc, patch, c1);
            draw_text(hdc, &title, patch, color, CENTER | DT_END_ELLIPSIS);
            fill_solid(hdc, rect_px(tb.left, tb.bottom, tb.right, tb.bottom + t), p.shadow);
        }
    }

    for (cap, r) in buttons {
        let region = cap.region();
        let down = region.is_some_and(|reg| v.is_down(reg));
        caption_button(hdc, cap, r, down, v.active, f);
        if let Some(reg) = region {
            hits.push((reg, r));
        }
    }
}

/// One caption button in the current theme.
unsafe fn caption_button(hdc: HDC, cap: Caption, r: RECT, down: bool, active: bool, f: &Fonts) {
    let p = pal();
    let t = line();
    let enabled = cap.region().is_some();
    SelectObject(hdc, f.glyph);
    match theme() {
        Theme::Classic => {
            let inner = bevel(hdc, r, if down { Bevel::Pressed } else { Bevel::Raised });
            fill_solid(hdc, inner, p.face);
            let shift = if down { t } else { 0 };
            let gr = offset(inner, shift, shift);
            if enabled {
                draw_text(hdc, glyph(cap.glyph()), gr, p.text, CENTER);
            } else {
                draw_text_disabled(hdc, glyph(cap.glyph()), gr, CENTER);
            }
        }
        Theme::Win31 if cap == Caption::SysMenu => {
            // A gray box holding the "space bar" handle, ruled off on its right.
            fill_solid(hdc, r, if down { p.shadow } else { p.face });
            fill_solid(hdc, rect_px(r.right - t, r.top, r.right, r.bottom), p.dark);
            let w = (r.right - r.left) * 9 / 16;
            let x = r.left + (r.right - r.left - t - w) / 2;
            let y = (r.top + r.bottom) / 2 - px(2);
            let bar = rect_px(x, y, x + w, y + px(4));
            fill_solid(hdc, offset(bar, t, t), p.dark);
            fill_solid(hdc, bar, p.hilite);
            outline(hdc, bar, p.dark);
        }
        Theme::Win31 => {
            // Arrow buttons, ruled off on their left: ▼ minimize, ▲ maximize.
            fill_solid(hdc, rect_px(r.left, r.top, r.left + t, r.bottom), p.dark);
            let body = rect_px(r.left + t, r.top, r.right, r.bottom);
            let inner = if down {
                edge(hdc, body, p.shadow, p.face);
                inset(body, t)
            } else {
                edge(hdc, body, p.hilite, p.shadow);
                edge(hdc, inset(body, t), p.hilite, p.shadow);
                inset(body, 2 * t)
            };
            fill_solid(hdc, inner, p.face);
            let g = if cap == Caption::Minimize { Glyph::Down } else { Glyph::Up };
            let shift = if down { t } else { 0 };
            let color = if enabled { p.text } else { p.disabled };
            draw_text(hdc, glyph(g), offset(inner, shift, shift), color, CENTER);
        }
        Theme::Flat => {
            let close = cap == Caption::Close;
            if down {
                fill_solid(hdc, r, if close { FLAT_CLOSE } else { p.light });
            }
            let color = if down && close {
                WHITE
            } else if active && enabled {
                p.text
            } else {
                p.disabled
            };
            draw_text(hdc, glyph(cap.glyph()), r, color, CENTER);
        }
        Theme::Platinum => {
            outline(hdc, r, p.dark);
            let inner = inset(r, t);
            if down {
                fill_solid(hdc, inner, p.shadow);
            } else {
                edge(hdc, inner, p.hilite, p.shadow);
                fill_solid(hdc, inset(inner, t), p.face);
            }
            let w = inner.right - inner.left;
            match cap {
                // Zoom: a small box in the top-left corner.
                Caption::Maximize => outline(
                    hdc,
                    rect_px(inner.left + t, inner.top + t, inner.left + w * 3 / 5, inner.top + w * 3 / 5),
                    p.dark,
                ),
                // Collapse (window shade): two bars across the middle.
                Caption::Minimize => {
                    let y = inner.top + w / 2;
                    fill_solid(hdc, rect_px(inner.left + t, y - 2 * t, inner.right - t, y - t), p.dark);
                    fill_solid(hdc, rect_px(inner.left + t, y + t, inner.right - t, y + 2 * t), p.dark);
                }
                Caption::Close | Caption::SysMenu => {}
            }
        }
    }
}

/// A scroll-bar arrow button.
unsafe fn scroll_arrow(hdc: HDC, r: RECT, g: Glyph, f: &Fonts) {
    let p = pal();
    SelectObject(hdc, f.glyph);
    if theme() == Theme::Flat {
        fill_solid(hdc, r, p.track);
        draw_text(hdc, glyph(g), r, p.text2, CENTER);
        return;
    }
    let inner = bevel(hdc, r, Bevel::Raised);
    fill_solid(hdc, inner, p.face);
    if theme() == Theme::Win31 {
        win31_arrow(hdc, inner, matches!(g, Glyph::Up), false, p.text);
        return;
    }
    draw_text(hdc, glyph(g), inner, p.text, CENTER);
}

/// The Windows 3.1 arrow: an arrowhead on a short stem, pointing up or down,
/// with a bar under it when `bar` (the drop-down button). Pixel art scaled by
/// the line width and centered in `r`.
unsafe fn win31_arrow(hdc: HDC, r: RECT, up: bool, bar: bool, color: COLORREF) {
    let t = line();
    // Row widths, top to bottom, of a down arrow.
    let mut rows = [3, 3, 3, 9, 7, 5, 3, 1];
    if up {
        rows.reverse();
    }
    let height = (rows.len() as i32 + if bar { 3 } else { 0 }) * t;
    let cx = (r.left + r.right - t) / 2;
    let mut y = r.top + (r.bottom - r.top - height) / 2;
    let row = |y: i32, w: i32| rect_px(cx - w / 2 * t, y, cx + (w / 2 + 1) * t, y + t);
    for w in rows {
        fill_solid(hdc, row(y, w), color);
        y += t;
    }
    if bar {
        fill_solid(hdc, row(y + t, 9), color);
        fill_solid(hdc, row(y + 2 * t, 9), color);
    }
}

/// The scroll-bar thumb.
unsafe fn scroll_thumb(hdc: HDC, r: RECT) {
    let p = pal();
    let t = line();
    match theme() {
        Theme::Classic | Theme::Win31 => {
            let inner = bevel(hdc, r, Bevel::Raised);
            fill_solid(hdc, inner, p.face);
        }
        // A slim bar.
        Theme::Flat => fill_solid(
            hdc,
            rect_px(r.left + px(5), r.top + px(2), r.right - px(5), r.bottom - px(2)),
            p.dark,
        ),
        // Lavender, with three grip ridges in the middle.
        Theme::Platinum => {
            outline(hdc, r, p.dark);
            let inner = inset(r, t);
            edge(hdc, inner, PLAT_THUMB_LIGHT, p.accent);
            fill_solid(hdc, inset(inner, t), PLAT_THUMB);
            let mid = (r.top + r.bottom) / 2;
            for i in -1..=1 {
                let y = mid + i * 3 * t - t;
                fill_solid(hdc, rect_px(r.left + px(4), y, r.right - px(4), y + t), p.hilite);
                fill_solid(hdc, rect_px(r.left + px(4), y + t, r.right - px(4), y + 2 * t), p.accent);
            }
        }
    }
}

/// A drop-down field showing `text`, drawn pressed while its menu opens.
unsafe fn combo(hdc: HDC, r: RECT, text: &str, down: bool, f: &Fonts) {
    let p = pal();
    let t = line();
    let arrow_w = px(18);
    let (text_r, arrow_r) = match theme() {
        // A sunken field with a push button holding the arrow.
        Theme::Classic => {
            let inner = bevel(hdc, r, Bevel::Sunken);
            fill_solid(hdc, inner, p.field);
            let btn = rect_px(inner.right - arrow_w, inner.top, inner.right, inner.bottom);
            let face = bevel(hdc, btn, if down { Bevel::Pressed } else { Bevel::Raised });
            fill_solid(hdc, face, p.face);
            let shift = if down { t } else { 0 };
            (rect_px(inner.left, inner.top, btn.left, inner.bottom), offset(face, shift, shift))
        }
        // An outlined field and, set apart from it, a square push button
        // holding an arrow over a bar.
        Theme::Win31 => {
            let btn = rect_px(r.right - (r.bottom - r.top), r.top, r.right, r.bottom);
            let inner = bevel(hdc, rect_px(r.left, r.top, btn.left - px(2), r.bottom), Bevel::Sunken);
            fill_solid(hdc, inner, p.field);
            let face = bevel(hdc, btn, if down { Bevel::Pressed } else { Bevel::Raised });
            if !down {
                fill_solid(hdc, face, p.face);
            }
            round_corners(hdc, btn, p.bg);
            let shift = if down { t } else { 0 };
            SelectObject(hdc, f.body);
            draw_text(hdc, text, pad_x(inner, px(6)), p.text, LEFT | DT_END_ELLIPSIS);
            win31_arrow(hdc, offset(face, shift, shift), false, true, p.text);
            return;
        }
        Theme::Flat => {
            fill_solid(hdc, r, if down { p.bg } else { p.field });
            outline(hdc, r, p.shadow);
            let inner = inset(r, t);
            (
                rect_px(inner.left, inner.top, inner.right - arrow_w, inner.bottom),
                rect_px(inner.right - arrow_w - px(4), inner.top, inner.right - px(4), inner.bottom),
            )
        }
        // A pop-up menu button: rounded, the arrow set off by a divider.
        Theme::Platinum => {
            round_rect(hdc, r, p.dark, if down { p.shadow } else { p.face }, px(8));
            let div = r.right - arrow_w - px(4);
            fill_solid(hdc, rect_px(div, r.top + px(4), div + t, r.bottom - px(4)), p.shadow);
            (rect_px(r.left, r.top, div, r.bottom), rect_px(div + t, r.top, r.right - px(2), r.bottom))
        }
    };
    SelectObject(hdc, f.body);
    draw_text(hdc, text, pad_x(text_r, px(8)), p.text, LEFT | DT_END_ELLIPSIS);
    SelectObject(hdc, f.glyph);
    let arrow_color = if theme() == Theme::Flat { p.text2 } else { p.text };
    draw_text(hdc, glyph(Glyph::Down), arrow_r, arrow_color, CENTER);
}

/// A V/E language button. The active one is lit: orange in the classic
/// theme, the accent color in the others.
unsafe fn lang_button(hdc: HDC, r: RECT, letter: &str, label: &str, active: bool, down: bool, f: &Fonts) {
    let p = pal();
    let t = line();
    // Paint the button; keep its face rect and the label, badge, badge-edge
    // and badge-letter colors.
    let (inner, label_color, badge_fill, badge_edge, letter_color) = match (theme(), active) {
        (Theme::Classic, true) => {
            // Heavy black outline, like a default button, around the lit face.
            edge(hdc, r, BLACK, BLACK);
            let inner = bevel(hdc, inset(r, t), Bevel::Raised);
            gradient(hdc, inner, ORANGE_TOP, ORANGE_BOTTOM, true);
            (inner, WHITE, BLACK, WHITE, WHITE)
        }
        (Theme::Win31, true) => {
            // A doubled outline around the highlight color.
            outline(hdc, r, p.dark);
            outline(hdc, inset(r, t), p.dark);
            let inner = inset(r, 2 * t);
            fill_solid(hdc, inner, p.accent);
            round_corners(hdc, r, p.bg);
            (inner, WHITE, WHITE, p.dark, p.accent)
        }
        (Theme::Flat, true) => {
            fill_solid(hdc, r, p.accent);
            (r, WHITE, WHITE, WHITE, p.accent)
        }
        (Theme::Platinum, true) => {
            round_rect(hdc, r, p.dark, p.accent, px(8));
            (r, WHITE, WHITE, p.dark, p.accent)
        }
        (Theme::Classic | Theme::Win31, false) => {
            let inner = bevel(hdc, r, if down { Bevel::Pressed } else { Bevel::Raised });
            fill_solid(hdc, inner, p.face);
            if theme() == Theme::Win31 {
                round_corners(hdc, r, p.bg);
            }
            let shift = if down { t } else { 0 };
            (offset(inner, shift, shift), p.text, p.shadow, p.text2, WHITE)
        }
        (Theme::Flat, false) => {
            fill_solid(hdc, r, if down { p.bg } else { p.face });
            outline(hdc, r, p.shadow);
            (inset(r, t), p.text, p.text2, p.text2, WHITE)
        }
        (Theme::Platinum, false) => {
            round_rect(hdc, r, p.dark, if down { p.shadow } else { p.face }, px(8));
            (r, p.text, p.text2, p.text2, WHITE)
        }
    };

    // Letter badge + label, centered together.
    SelectObject(hdc, f.lang);
    let badge = px(22);
    let gap = px(10);
    let total = badge + gap + text_width(hdc, label);
    let x = inner.left + (inner.right - inner.left - total) / 2;
    let by = inner.top + (inner.bottom - inner.top - badge) / 2;
    let b = rect_px(x, by, x + badge, by + badge);
    fill_solid(hdc, b, badge_fill);
    edge(hdc, b, badge_edge, badge_edge);
    SelectObject(hdc, f.badge);
    draw_text(hdc, letter, b, letter_color, CENTER);
    SelectObject(hdc, f.lang);
    draw_text(
        hdc,
        label,
        rect_px(b.right + gap, inner.top, inner.right, inner.bottom),
        label_color,
        LEFT,
    );
}

/// A checkbox with its label (and optional caption line) at `pos` = (left,
/// right, top) in layout units; `state` = (checked, enabled, pressed). The
/// whole row is clickable, as with a real checkbox; returns that rect.
unsafe fn checkbox_row(
    hdc: HDC,
    pos: (i32, i32, i32),
    label: &str,
    caption: Option<&str>,
    state: (bool, bool, bool),
    f: &Fonts,
) -> RECT {
    let p = pal();
    let (x, right, cy) = pos;
    let (checked, enabled, down) = state;
    let row_h = if caption.is_some() { CAPTIONED_ROW_H } else { CHECK_ROW_H };
    let label_h = if caption.is_some() { 20 } else { CHECK_ROW_H };
    let box_top = cy + (label_h - 16) / 2;
    checkbox(hdc, rect(x, box_top, x + 16, box_top + 16), checked, enabled, down);

    SelectObject(hdc, if caption.is_some() { f.bold } else { f.body });
    let lr = rect(x + 26, cy, right, cy + label_h);
    if enabled {
        draw_text(hdc, label, lr, p.text, LEFT | DT_END_ELLIPSIS);
    } else {
        draw_text_disabled(hdc, label, lr, LEFT | DT_END_ELLIPSIS);
    }
    if let Some(caption) = caption {
        SelectObject(hdc, f.small);
        let cr = rect(x + 26, cy + label_h, right, cy + row_h);
        if enabled {
            draw_text(hdc, caption, cr, p.text2, LEFT | DT_END_ELLIPSIS);
        } else {
            draw_text_disabled(hdc, caption, cr, LEFT | DT_END_ELLIPSIS);
        }
    }
    rect(x, cy, right, cy + row_h)
}

/// A check box, on or off, held down or disabled.
unsafe fn checkbox(hdc: HDC, r: RECT, checked: bool, enabled: bool, down: bool) {
    let p = pal();
    let t = line();
    match theme() {
        // Sunken: blue with a check when on, face-gray while pressed or
        // disabled.
        Theme::Classic => {
            let inner = bevel(hdc, r, Bevel::Sunken);
            let fill = if !enabled || down {
                p.face
            } else if checked {
                p.accent
            } else {
                p.field
            };
            fill_solid(hdc, inner, fill);
            if !checked {
                return;
            }
            let pts = tick_points(inner);
            if enabled && !down {
                // Black tick with a white halo, readable on the blue.
                stroke(hdc, &pts, WHITE, px(4).max(3));
                stroke(hdc, &pts, BLACK, px(2).max(2));
            } else {
                stroke(hdc, &pts, if enabled { BLACK } else { p.shadow }, px(2).max(2));
            }
        }
        // A black square, crossed when on, its border doubled while held.
        Theme::Win31 => {
            outline(hdc, r, if enabled { p.dark } else { p.shadow });
            let inner = inset(r, t);
            fill_solid(hdc, inner, p.field);
            if down {
                outline(hdc, inner, p.dark);
            }
            if checked {
                let color = if enabled { p.dark } else { p.shadow };
                let i = inset(inner, t);
                stroke(hdc, &[POINT { x: i.left, y: i.top }, POINT { x: i.right, y: i.bottom }], color, t);
                stroke(
                    hdc,
                    &[POINT { x: i.left, y: i.bottom - 1 }, POINT { x: i.right, y: i.top - 1 }],
                    color,
                    t,
                );
            }
        }
        // Outlined when off; filled with the accent and a white tick when on.
        Theme::Flat => {
            let (fill, border) = if !enabled {
                (if checked { p.shadow } else { p.bg }, p.shadow)
            } else if checked {
                let c = if down { FLAT_ACCENT_DOWN } else { p.accent };
                (c, c)
            } else {
                (if down { p.light } else { p.field }, p.dark)
            };
            fill_solid(hdc, r, fill);
            outline(hdc, r, border);
            if checked {
                stroke(hdc, &tick_points(inset(r, t)), WHITE, px(2).max(2));
            }
        }
        // A softly beveled square with a black tick.
        Theme::Platinum => {
            outline(hdc, r, if enabled { p.dark } else { p.shadow });
            let inner = inset(r, t);
            if down {
                fill_solid(hdc, inner, p.shadow);
            } else {
                edge(hdc, inner, p.hilite, p.shadow);
                fill_solid(hdc, inset(inner, t), p.light);
            }
            if checked {
                let color = if enabled { BLACK } else { p.shadow };
                stroke(hdc, &tick_points(inner), color, px(2).max(2));
            }
        }
    }
}

/// The three points of a check mark inside `r`.
fn tick_points(r: RECT) -> [POINT; 3] {
    let (l, t) = (r.left, r.top);
    let w = r.right - r.left;
    [
        POINT { x: l + w * 20 / 100, y: t + w * 50 / 100 },
        POINT { x: l + w * 42 / 100, y: t + w * 72 / 100 },
        POINT { x: l + w * 82 / 100, y: t + w * 26 / 100 },
    ]
}

/// A push button. `default` marks the dialog's default button (a heavy
/// outline, the accent color, or the Platinum default ring).
unsafe fn push_button(hdc: HDC, r: RECT, label: &str, font: HFONT, down: bool, default: bool) {
    let p = pal();
    let t = line();
    SelectObject(hdc, font);
    match theme() {
        Theme::Classic | Theme::Win31 => {
            let mut face = r;
            if default {
                outline(hdc, face, p.dark);
                face = inset(face, t);
            }
            let inner = bevel(hdc, face, if down { Bevel::Pressed } else { Bevel::Raised });
            fill_solid(hdc, inner, p.face);
            if theme() == Theme::Win31 {
                round_corners(hdc, r, p.bg);
            }
            let shift = if down { t } else { 0 };
            draw_text(hdc, label, offset(inner, shift, shift), p.text, CENTER);
        }
        Theme::Flat => {
            let (fill, border, color) = match (default, down) {
                (true, false) => (p.accent, p.accent, WHITE),
                (true, true) => (FLAT_ACCENT_DOWN, FLAT_ACCENT_DOWN, WHITE),
                (false, false) => (p.face, p.shadow, p.text),
                (false, true) => (p.bg, p.shadow, p.text2),
            };
            fill_solid(hdc, r, fill);
            outline(hdc, r, border);
            draw_text(hdc, label, r, color, CENTER);
        }
        Theme::Platinum => {
            let mut face = r;
            if default {
                round_rect(hdc, face, p.dark, p.shadow, px(12));
                face = inset(face, px(3));
            }
            // Held down, the button darkens and its label turns white.
            let (fill, color) = if down { (p.text2, WHITE) } else { (p.face, p.text) };
            round_rect(hdc, face, p.dark, fill, px(8));
            if !down {
                // Soft highlight and shadow inside the top and bottom edges.
                let (l, rr) = (face.left + px(4), face.right - px(4));
                fill_solid(hdc, rect_px(l, face.top + t, rr, face.top + 2 * t), p.hilite);
                fill_solid(hdc, rect_px(l, face.bottom - 2 * t, rr, face.bottom - t), p.shadow);
            }
            draw_text(hdc, label, face, color, CENTER);
        }
    }
}

/// The chosen button of a segmented choice (the app-scope modes).
unsafe fn selected_button(hdc: HDC, r: RECT, label: &str, f: &Fonts) {
    let p = pal();
    match theme() {
        // Stays pushed in, with a bullet.
        Theme::Classic => {
            let inner = bevel(hdc, r, Bevel::Sunken);
            fill_solid(hdc, inner, SELECTED);
            SelectObject(hdc, f.bold);
            draw_text(hdc, &format!("• {label}"), offset(inner, line(), line()), p.text, CENTER);
        }
        Theme::Win31 => push_button(hdc, r, &format!("• {label}"), f.bold, true, false),
        Theme::Flat => {
            fill_solid(hdc, r, p.accent);
            SelectObject(hdc, f.bold);
            draw_text(hdc, label, r, WHITE, CENTER);
        }
        Theme::Platinum => push_button(hdc, r, label, f.bold, true, false),
    }
}

/// A group box with its label (`r` in layout units). Etched with the label
/// set into the top edge (classic, Platinum), a black outline (Windows 3.1),
/// or a white card with the label inside (flat).
unsafe fn group_box(hdc: HDC, r: RECT, label: &str, font: HFONT) {
    let p = pal();
    let t = line();
    SelectObject(hdc, font);
    if theme() == Theme::Flat {
        let card = rect(r.left, r.top, r.right, r.bottom);
        fill_solid(hdc, card, p.field);
        outline(hdc, card, p.light);
        draw_text(hdc, label, rect(r.left + 12, r.top + 4, r.right - 12, r.top + 20), p.heading, LEFT);
        return;
    }

    // The label sits on the first 16 units, the border at its middle.
    let frame = rect(r.left, r.top + 8, r.right, r.bottom);
    if theme() == Theme::Win31 {
        outline(hdc, frame, p.dark);
    } else {
        outline(hdc, offset(frame, t, t), p.hilite);
        outline(hdc, rect_px(frame.left, frame.top, frame.right - t, frame.bottom - t), p.shadow);
    }
    let lx = px(r.left + 10);
    let w = text_width(hdc, label);
    let lr = rect_px(lx, px(r.top), lx + w + px(8), px(r.top + 16));
    fill_solid(hdc, lr, p.bg);
    draw_text(hdc, label, lr, p.heading, CENTER);
}

/// A horizontal separator along the top of `r` (device pixels).
unsafe fn etched_line(hdc: HDC, r: RECT) {
    let p = pal();
    let t = line();
    match theme() {
        Theme::Classic | Theme::Platinum => {
            fill_solid(hdc, rect_px(r.left, r.top, r.right, r.top + t), p.shadow);
            fill_solid(hdc, rect_px(r.left, r.top + t, r.right, r.top + 2 * t), p.hilite);
        }
        Theme::Win31 => fill_solid(hdc, rect_px(r.left, r.top, r.right, r.top + t), p.shadow),
        Theme::Flat => fill_solid(hdc, rect_px(r.left, r.top, r.right, r.top + t), p.light),
    }
}

unsafe fn draw_app_icon(hdc: HDC, x: i32, y: i32, size: i32) {
    let Ok(hmod) = GetModuleHandleW(None) else { return };
    // MAKEINTRESOURCE, loaded at the exact size so it stays crisp.
    let name = PCWSTR(crate::tray::APP_ICON_ID as usize as *const u16);
    let Ok(h) = LoadImageW(HINSTANCE(hmod.0), name, IMAGE_ICON, size, size, LR_DEFAULTCOLOR) else {
        return;
    };
    let icon = HICON(h.0);
    let _ = DrawIconEx(hdc, x, y, icon, size, size, 0, HBRUSH::default(), DI_NORMAL);
    let _ = DestroyIcon(icon);
}

// MARK: - Themes

/// A theme's colors. Where themes differ in shape rather than color, the
/// painters above match on `theme()`.
struct Palette {
    /// Dialog background.
    bg: COLORREF,
    /// Button face.
    face: COLORREF,
    /// Bevel and border colors, lightest to darkest.
    hilite: COLORREF,
    light: COLORREF,
    shadow: COLORREF,
    dark: COLORREF,
    text: COLORREF,
    /// Captions and hints.
    text2: COLORREF,
    disabled: COLORREF,
    /// Group-box labels.
    heading: COLORREF,
    /// The shortcut field and the status-bar language.
    accent_text: COLORREF,
    /// The lit language button, the chosen app scope, a checked box.
    accent: COLORREF,
    danger: COLORREF,
    warn: COLORREF,
    /// The shortcut panel.
    panel: COLORREF,
    /// Text fields and the app list.
    field: COLORREF,
    /// The read-only app-scope caption.
    field_soft: COLORREF,
    /// Scroll-bar track.
    track: COLORREF,
    /// Title bar gradients (left, right) and text, active and inactive.
    title_active: (COLORREF, COLORREF),
    title_inactive: (COLORREF, COLORREF),
    title_text: COLORREF,
    title_inactive_text: COLORREF,
}

static CLASSIC: Palette = Palette {
    bg: rgb(192, 192, 192),
    face: rgb(192, 192, 192),
    hilite: rgb(255, 255, 255),
    light: rgb(223, 223, 223),
    shadow: rgb(128, 128, 128),
    dark: rgb(0, 0, 0),
    text: rgb(0, 0, 0),
    text2: rgb(88, 88, 88),
    disabled: rgb(128, 128, 128),
    heading: rgb(0, 0, 128),
    accent_text: rgb(0, 0, 128),
    accent: rgb(32, 96, 232),
    danger: rgb(176, 0, 0),
    warn: rgb(128, 80, 0),
    panel: rgb(180, 180, 180),
    field: rgb(255, 255, 255),
    field_soft: rgb(236, 236, 236),
    track: rgb(224, 224, 224),
    title_active: (rgb(0, 0, 128), rgb(16, 132, 208)),
    title_inactive: (rgb(128, 128, 128), rgb(181, 181, 181)),
    title_text: rgb(255, 255, 255),
    title_inactive_text: rgb(212, 208, 200),
};

/// Windows 3.1's default scheme, within its 16-color palette.
static WIN31: Palette = Palette {
    bg: rgb(255, 255, 255),
    face: rgb(192, 192, 192),
    hilite: rgb(255, 255, 255),
    light: rgb(255, 255, 255),
    shadow: rgb(128, 128, 128),
    dark: rgb(0, 0, 0),
    text: rgb(0, 0, 0),
    text2: rgb(128, 128, 128),
    disabled: rgb(128, 128, 128),
    heading: rgb(0, 0, 0),
    accent_text: rgb(0, 0, 128),
    accent: rgb(0, 0, 128),
    danger: rgb(128, 0, 0),
    warn: rgb(128, 128, 0),
    panel: rgb(255, 255, 255),
    field: rgb(255, 255, 255),
    field_soft: rgb(255, 255, 255),
    track: rgb(192, 192, 192),
    title_active: (rgb(0, 0, 128), rgb(0, 0, 128)),
    title_inactive: (rgb(255, 255, 255), rgb(255, 255, 255)),
    title_text: rgb(255, 255, 255),
    title_inactive_text: rgb(0, 0, 0),
};

/// Windows 10/11 light: a soft gray window, white cards, blue accent.
static FLAT: Palette = Palette {
    bg: rgb(243, 243, 243),
    face: rgb(251, 251, 251),
    hilite: rgb(255, 255, 255),
    light: rgb(229, 229, 229),
    shadow: rgb(204, 204, 204),
    dark: rgb(134, 134, 134),
    text: rgb(26, 26, 26),
    text2: rgb(96, 96, 96),
    disabled: rgb(160, 160, 160),
    heading: rgb(26, 26, 26),
    accent_text: rgb(0, 95, 184),
    accent: rgb(0, 103, 192),
    danger: rgb(196, 43, 28),
    warn: rgb(157, 93, 0),
    panel: rgb(249, 249, 249),
    field: rgb(255, 255, 255),
    field_soft: rgb(249, 249, 249),
    track: rgb(240, 240, 240),
    title_active: (rgb(243, 243, 243), rgb(243, 243, 243)),
    title_inactive: (rgb(243, 243, 243), rgb(243, 243, 243)),
    title_text: rgb(26, 26, 26),
    title_inactive_text: rgb(150, 150, 150),
};

/// Mac OS 9 Platinum: light gray, soft bevels, lavender highlights.
static PLATINUM: Palette = Palette {
    bg: rgb(221, 221, 221),
    face: rgb(221, 221, 221),
    hilite: rgb(255, 255, 255),
    light: rgb(238, 238, 238),
    shadow: rgb(136, 136, 136),
    dark: rgb(51, 51, 51),
    text: rgb(0, 0, 0),
    text2: rgb(85, 85, 85),
    disabled: rgb(153, 153, 153),
    heading: rgb(0, 0, 0),
    accent_text: rgb(51, 51, 153),
    accent: rgb(102, 102, 204),
    danger: rgb(204, 0, 0),
    warn: rgb(153, 102, 0),
    panel: rgb(204, 204, 204),
    field: rgb(255, 255, 255),
    field_soft: rgb(238, 238, 238),
    track: rgb(187, 187, 187),
    title_active: (rgb(221, 221, 221), rgb(221, 221, 221)),
    title_inactive: (rgb(221, 221, 221), rgb(221, 221, 221)),
    title_text: rgb(0, 0, 0),
    title_inactive_text: rgb(136, 136, 136),
};

/// The current theme's colors.
fn pal() -> &'static Palette {
    match theme() {
        Theme::Classic => &CLASSIC,
        Theme::Win31 => &WIN31,
        Theme::Flat => &FLAT,
        Theme::Platinum => &PLATINUM,
    }
}

// Colors used by one theme only.
const WHITE: COLORREF = rgb(255, 255, 255);
const BLACK: COLORREF = rgb(0, 0, 0);
/// Classic: the chosen app-scope button.
const SELECTED: COLORREF = rgb(172, 172, 172);
/// Classic: the lit language button.
const ORANGE_TOP: COLORREF = rgb(244, 146, 62);
const ORANGE_BOTTOM: COLORREF = rgb(212, 98, 22);
/// Flat: the accent while pressed, and the close button while pressed.
const FLAT_ACCENT_DOWN: COLORREF = rgb(0, 90, 158);
const FLAT_CLOSE: COLORREF = rgb(196, 43, 28);
/// Platinum: the lavender scroll thumb.
const PLAT_THUMB: COLORREF = rgb(204, 204, 255);
const PLAT_THUMB_LIGHT: COLORREF = rgb(238, 238, 255);

// MARK: - GDI helpers

const LEFT: DRAW_TEXT_FORMAT = DRAW_TEXT_FORMAT(DT_LEFT.0 | DT_SINGLELINE.0 | DT_VCENTER.0);
const CENTER: DRAW_TEXT_FORMAT = DRAW_TEXT_FORMAT(DT_CENTER.0 | DT_SINGLELINE.0 | DT_VCENTER.0);

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

/// A RECT in 96-DPI layout units (for positioning; scale before drawing).
fn lrect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    RECT {
        left,
        top,
        right,
        bottom,
    }
}

/// A RECT already in device pixels (no scaling).
fn rect_px(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    lrect(left, top, right, bottom)
}

/// A device-pixel RECT from 96-DPI layout coordinates.
fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    rect_px(px(left), px(top), px(right), px(bottom))
}

fn inset(r: RECT, d: i32) -> RECT {
    rect_px(r.left + d, r.top + d, r.right - d, r.bottom - d)
}

fn offset(r: RECT, dx: i32, dy: i32) -> RECT {
    rect_px(r.left + dx, r.top + dy, r.right + dx, r.bottom + dy)
}

/// Shrink `r` horizontally by `d` on both sides (text padding).
fn pad_x(r: RECT, d: i32) -> RECT {
    rect_px(r.left + d, r.top, r.right - d, r.bottom)
}

/// A font scaled to the current DPI; unsmoothed in the Windows 3.1 theme,
/// whose screen fonts were bitmaps.
unsafe fn font(face: &str, height: i32, weight: i32, italic: bool) -> HFONT {
    let face = wide(face);
    let quality = if theme() == Theme::Win31 { 3 } else { 5 };
    // charset=DEFAULT(1), out/clip precis=0, quality=CLEARTYPE(5) or
    // NONANTIALIASED(3), pitch=DEFAULT(0).
    CreateFontW(
        px(height),
        0,
        0,
        0,
        weight,
        italic as u32,
        0,
        0,
        1,
        0,
        0,
        quality,
        0,
        PCWSTR(face.as_ptr()),
    )
}

/// The Marlett symbol font, unsmoothed so its glyphs stay pixel-sharp.
unsafe fn marlett(height: i32) -> HFONT {
    let face = wide("Marlett");
    // charset=SYMBOL(2), quality=NONANTIALIASED(3).
    CreateFontW(px(height), 0, 0, 0, 400, 0, 0, 0, 2, 0, 0, 3, 0, PCWSTR(face.as_ptr()))
}

unsafe fn draw_text(hdc: HDC, s: &str, mut r: RECT, color: COLORREF, fmt: DRAW_TEXT_FORMAT) {
    if s.is_empty() {
        return;
    }
    SetTextColor(hdc, color);
    SetBkMode(hdc, TRANSPARENT);
    let mut buf: Vec<u16> = s.encode_utf16().collect();
    DrawTextW(hdc, &mut buf, &mut r, fmt | DT_NOPREFIX);
}

/// Disabled text: gray, embossed with a white copy one line down-right in
/// the classic theme.
unsafe fn draw_text_disabled(hdc: HDC, s: &str, r: RECT, fmt: DRAW_TEXT_FORMAT) {
    if theme() == Theme::Classic {
        draw_text(hdc, s, offset(r, line(), line()), WHITE, fmt);
    }
    draw_text(hdc, s, r, pal().disabled, fmt);
}

/// Width of `s` in the currently selected font, in device pixels.
unsafe fn text_width(hdc: HDC, s: &str) -> i32 {
    let buf: Vec<u16> = s.encode_utf16().collect();
    let mut size = SIZE::default();
    let _ = GetTextExtentPoint32W(hdc, &buf, &mut size);
    size.cx
}

unsafe fn fill_solid(hdc: HDC, r: RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    FillRect(hdc, &r, brush);
    let _ = DeleteObject(brush);
}

/// A one-line border: `tl` along the top and left, `br` along the bottom and
/// right (which wins at the two mixed corners, as in the classic look).
unsafe fn edge(hdc: HDC, r: RECT, tl: COLORREF, br: COLORREF) {
    let t = line();
    fill_solid(hdc, rect_px(r.left, r.top, r.right, r.top + t), tl);
    fill_solid(hdc, rect_px(r.left, r.top, r.left + t, r.bottom), tl);
    fill_solid(hdc, rect_px(r.left, r.bottom - t, r.right, r.bottom), br);
    fill_solid(hdc, rect_px(r.right - t, r.top, r.right, r.bottom), br);
}

unsafe fn outline(hdc: HDC, r: RECT, color: COLORREF) {
    edge(hdc, r, color, color);
}

/// A border `w` pixels wide just inside `r`.
unsafe fn ring(hdc: HDC, r: RECT, w: i32, color: COLORREF) {
    fill_solid(hdc, rect_px(r.left, r.top, r.right, r.top + w), color);
    fill_solid(hdc, rect_px(r.left, r.bottom - w, r.right, r.bottom), color);
    fill_solid(hdc, rect_px(r.left, r.top, r.left + w, r.bottom), color);
    fill_solid(hdc, rect_px(r.right - w, r.top, r.right, r.bottom), color);
}

/// Clip the four corner pixels of `r` to `color` (Windows 3.1 buttons).
unsafe fn round_corners(hdc: HDC, r: RECT, color: COLORREF) {
    let t = line();
    for (x, y) in [(r.left, r.top), (r.right - t, r.top), (r.left, r.bottom - t), (r.right - t, r.bottom - t)] {
        fill_solid(hdc, rect_px(x, y, x + t, y + t), color);
    }
}

/// A filled rounded rectangle with a one-line border. GDI draws it
/// unsmoothed, which suits the Platinum look.
unsafe fn round_rect(hdc: HDC, r: RECT, border: COLORREF, fill: COLORREF, radius: i32) {
    let pen = CreatePen(PS_INSIDEFRAME, line(), border);
    let brush = CreateSolidBrush(fill);
    let old_pen = SelectObject(hdc, pen);
    let old_brush = SelectObject(hdc, brush);
    let _ = RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(pen);
    let _ = DeleteObject(brush);
}

#[derive(Clone, Copy)]
enum Bevel {
    /// A push button or scroll-bar part.
    Raised,
    /// A push button held down.
    Pressed,
    /// A text field, list box or check box.
    Sunken,
    /// The outer window frame.
    Window,
    /// A shallow one-line inset (status-bar panes, the shortcut panel).
    Thin,
}

/// Draw a border of `kind` on `r` in the current theme and return the area
/// inside it.
unsafe fn bevel(hdc: HDC, r: RECT, kind: Bevel) -> RECT {
    let p = pal();
    let t = line();
    match theme() {
        Theme::Classic => {
            let ((o_tl, o_br), (i_tl, i_br)) = match kind {
                Bevel::Raised => ((p.hilite, p.dark), (p.light, p.shadow)),
                Bevel::Pressed => ((p.dark, p.dark), (p.shadow, p.shadow)),
                Bevel::Sunken => ((p.shadow, p.hilite), (p.dark, p.light)),
                Bevel::Window => ((p.light, p.dark), (p.hilite, p.shadow)),
                Bevel::Thin => {
                    edge(hdc, r, p.shadow, p.hilite);
                    return inset(r, t);
                }
            };
            edge(hdc, r, o_tl, o_br);
            edge(hdc, inset(r, t), i_tl, i_br);
            inset(r, 2 * t)
        }
        Theme::Win31 => match kind {
            // A black outline around a two-line highlight/shadow bevel.
            Bevel::Raised => {
                outline(hdc, r, p.dark);
                edge(hdc, inset(r, t), p.hilite, p.shadow);
                edge(hdc, inset(r, 2 * t), p.hilite, p.shadow);
                inset(r, 3 * t)
            }
            // Held down, the bevel flattens to a shadow along the top and left.
            Bevel::Pressed => {
                outline(hdc, r, p.dark);
                fill_solid(hdc, inset(r, t), p.face);
                edge(hdc, inset(r, t), p.shadow, p.face);
                inset(r, 3 * t)
            }
            Bevel::Sunken => {
                outline(hdc, r, p.dark);
                inset(r, t)
            }
            Bevel::Thin => {
                outline(hdc, r, p.shadow);
                inset(r, t)
            }
            // Black lines either side of a gray band, filling the frame.
            Bevel::Window => {
                let band = px(FRAME) - 2 * t;
                outline(hdc, r, p.dark);
                ring(hdc, inset(r, t), band, p.face);
                outline(hdc, inset(r, t + band), p.dark);
                inset(r, px(FRAME))
            }
        },
        // Flat: a single line.
        Theme::Flat => {
            outline(hdc, r, if matches!(kind, Bevel::Thin) { p.light } else { p.shadow });
            inset(r, t)
        }
        Theme::Platinum => match kind {
            Bevel::Raised | Bevel::Window => {
                outline(hdc, r, p.dark);
                edge(hdc, inset(r, t), p.hilite, p.shadow);
                inset(r, 2 * t)
            }
            Bevel::Pressed => {
                outline(hdc, r, p.dark);
                edge(hdc, inset(r, t), p.shadow, p.light);
                inset(r, 2 * t)
            }
            Bevel::Sunken => {
                edge(hdc, r, p.shadow, p.hilite);
                edge(hdc, inset(r, t), p.dark, p.light);
                inset(r, 2 * t)
            }
            Bevel::Thin => {
                edge(hdc, r, p.shadow, p.hilite);
                inset(r, t)
            }
        },
    }
}

/// Fill `r` with a two-color gradient, top→bottom if `vertical`, else
/// left→right.
unsafe fn gradient(hdc: HDC, r: RECT, from: COLORREF, to: COLORREF, vertical: bool) {
    let verts = [tvtx(r.left, r.top, from), tvtx(r.right, r.bottom, to)];
    let mesh = [GRADIENT_RECT {
        UpperLeft: 0,
        LowerRight: 1,
    }];
    let _ = GradientFill(
        hdc,
        &verts,
        mesh.as_ptr() as *const c_void,
        mesh.len() as u32,
        if vertical { GRADIENT_FILL_RECT_V } else { GRADIENT_FILL_RECT_H },
    );
}

fn tvtx(x: i32, y: i32, c: COLORREF) -> TRIVERTEX {
    let ch = |shift: u32| (((c.0 >> shift) & 0xFF) as u16) << 8;
    TRIVERTEX {
        x,
        y,
        Red: ch(0),
        Green: ch(8),
        Blue: ch(16),
        Alpha: 0xFFFF,
    }
}

/// Draw a polyline through `pts` with a solid pen.
unsafe fn stroke(hdc: HDC, pts: &[POINT], color: COLORREF, width: i32) {
    let pen = CreatePen(PS_SOLID, width, color);
    let old = SelectObject(hdc, pen);
    let _ = Polyline(hdc, pts);
    SelectObject(hdc, old);
    let _ = DeleteObject(pen);
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
