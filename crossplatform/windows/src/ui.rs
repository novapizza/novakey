//! ui.rs
//! An owner-drawn, dark card-based Settings window that mirrors the macOS
//! SwiftUI preferences pane (INPUT METHOD / GENERAL / COMPATIBILITY / ABOUT).
//!
//! Everything is drawn by hand with GDI into an off-screen bitmap (flicker-free
//! double buffering); interactive elements (V/E pills, toggles, the Change
//! button) are hit-tested manually against rectangles recorded during paint.
//! No GUI dependency is added — this keeps the binary tiny.
//!
//! The window is created on, and its messages pump on, the same thread that
//! owns `crate::SETTINGS` and the hooks, so it reads/writes shared state without
//! locking and talks to the main message window via `SendMessageW`.

use std::cell::RefCell;
use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW,
    CreateRoundRectRgn, CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, Ellipse, EndPaint,
    FillRect, GetStockObject, GradientFill, InvalidateRect, RoundRect, SelectClipRgn, SelectObject,
    SetBkMode, SetTextColor, UpdateWindow, DRAW_TEXT_FORMAT, DT_CENTER, DT_LEFT, DT_NOPREFIX,
    DT_RIGHT, DT_SINGLELINE, DT_VCENTER, GRADIENT_FILL_RECT_H, GRADIENT_RECT, HBITMAP, HBRUSH, HDC,
    HFONT, HGDIOBJ, HRGN, NULL_PEN, PAINTSTRUCT, SRCCOPY, TRANSPARENT, TRIVERTEX,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRect, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect,
    GetSystemMetrics, IsWindow, LoadCursorW, RegisterClassW, SetForegroundWindow,
    SetWindowPos, ShowWindow, HMENU, IDC_ARROW, SM_CXSCREEN, SM_CYSCREEN,
    SWP_NOSIZE, SWP_NOZORDER, SW_SHOW, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_KEYUP,
    WM_KILLFOCUS, WM_LBUTTONDOWN, WM_PAINT, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW, WS_CAPTION,
    WS_SYSMENU,
};

use crate::hotkey;
use crate::tray::{
    CMD_AUTOCOMPLETE, CMD_AUTOSTART, CMD_DEFERRED, CMD_PLAYSOUND, CMD_QUICKVN, CMD_STEP,
    CMD_TOGGLE,
};

// MARK: - Layout constants (client pixels)

const CLIENT_W: i32 = 460;
const CLIENT_H: i32 = 786;
const PAD: i32 = 18;
const CARD_PAD: i32 = 16;
const CARD_RADIUS: i32 = 14;
const GAP: i32 = 14;

// MARK: - Interactive regions

#[derive(Clone, Copy, PartialEq)]
enum Region {
    PillVi,
    PillEn,
    HotkeyChange,
    HotkeyReset,
    ToggleQuickVn,
    ToggleDeferred,
    ToggleAutostart,
    ToggleSound,
    ToggleFix,
    ToggleStep,
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

struct UiState {
    hwnd: HWND,
    main_hwnd: HWND,
    /// True while waiting for the user to press a new toggle shortcut.
    capturing: bool,
    /// Status line drawn under the toggle-shortcut row.
    notice: Option<(String, NoticeKind)>,
    /// Clickable rectangles recorded during the last paint.
    hits: Vec<(Region, RECT)>,
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

    let style = WS_CAPTION | WS_SYSMENU;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: CLIENT_W,
        bottom: CLIENT_H,
    };
    let _ = AdjustWindowRect(&mut rect, style, false);
    let win_w = rect.right - rect.left;
    let win_h = rect.bottom - rect.top;

    let title = wide("NovaKey Settings");
    let hwnd = match CreateWindowExW(
        Default::default(),
        PCWSTR(class_name.as_ptr()),
        PCWSTR(title.as_ptr()),
        style,
        0,
        0,
        win_w,
        win_h,
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
        });
    });

    // Center on the primary monitor.
    let sw = GetSystemMetrics(SM_CXSCREEN);
    let sh = GetSystemMetrics(SM_CYSCREEN);
    let _ = SetWindowPos(
        hwnd,
        HWND::default(),
        (sw - win_w) / 2,
        (sh - win_h) / 3,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER,
    );

    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = UpdateWindow(hwnd);
    let _ = SetForegroundWindow(hwnd);
}

/// Repaint the Settings window if it is open (called after a state change made
/// elsewhere — e.g. the tray/hotkey toggling the language).
pub unsafe fn refresh() {
    UI.with(|u| {
        if let Some(s) = u.borrow().as_ref() {
            let _ = InvalidateRect(s.hwnd, None, false);
        }
    });
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
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            on_click(hwnd, x, y);
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if on_key(hwnd, wparam.0 as u32) {
                return LRESULT(0); // consumed during capture
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

// MARK: - Interaction

unsafe fn on_click(hwnd: HWND, x: i32, y: i32) {
    let (main_hwnd, hits) =
        UI.with(|u| {
            let b = u.borrow();
            match b.as_ref() {
                Some(s) => (s.main_hwnd, s.hits.clone()),
                None => (HWND::default(), Vec::new()),
            }
        });

    let region = hits
        .iter()
        .find(|(_, r)| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
        .map(|(reg, _)| *reg);

    let Some(region) = region else { return };

    match region {
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
        Region::ToggleAutostart => send_cmd(main_hwnd, CMD_AUTOSTART),
        Region::ToggleSound => send_cmd(main_hwnd, CMD_PLAYSOUND),
        Region::ToggleFix => send_cmd(main_hwnd, CMD_AUTOCOMPLETE),
        Region::ToggleStep => send_cmd(main_hwnd, CMD_STEP),
    }

    let _ = InvalidateRect(hwnd, None, false);
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

unsafe fn paint(hwnd: HWND) {
    // Snapshot the state we render.
    let enabled = crate::hook::is_enabled();
    let fix = crate::hook::is_fix_autocomplete();
    let (autostart, step, sound, quick_vn, deferred, mods, vk) = crate::SETTINGS.with(|s| {
        let s = s.borrow();
        (
            s.start_with_windows,
            s.step_by_step,
            s.play_sound,
            s.quick_vietnamese,
            s.deferred_diacritics,
            s.hotkey_mods,
            s.hotkey_vk,
        )
    });
    let (capturing, notice) = UI.with(|u| {
        let b = u.borrow();
        match b.as_ref() {
            Some(s) => (s.capturing, s.notice.clone()),
            None => (false, None),
        }
    });
    let hotkey_text = hotkey::describe(mods, vk);

    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    let cw = client.right;
    let ch = client.bottom;

    // Off-screen buffer.
    let mem = CreateCompatibleDC(hdc);
    let bmp = CreateCompatibleBitmap(hdc, cw, ch);
    let old_bmp = SelectObject(mem, bmp);

    // Background.
    fill_solid(mem, client, WINDOW_BG);

    // Fonts.
    let f_title = font(-19, 600);
    let f_section = font(-12, 700);
    let f_row = font(-16, 400);
    let f_pill = font(-15, 700);
    let f_value = font(-14, 600);
    let f_small = font(-13, 400);

    let mut hits: Vec<(Region, RECT)> = Vec::new();
    let mut y = 12;

    // Title.
    SelectObject(mem, f_title);
    draw_text(
        mem,
        "NovaKey Settings",
        rect(PAD, y, cw - PAD, y + 24),
        TEXT,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER,
    );
    y += 34;

    // ----- Card: INPUT METHOD -----
    {
        let card = rect(PAD, y, cw - PAD, y + 340);
        fill_round(mem, card, CARD_RADIUS, CARD_BG);
        let ix = card.left + CARD_PAD;
        let ir = card.right - CARD_PAD;
        let mut cy = card.top + CARD_PAD;

        SelectObject(mem, f_section);
        draw_text(mem, "INPUT METHOD", rect(ix, cy, ir, cy + 16), SECTION, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        cy += 28;

        // V / E pills.
        let pill_h = 38;
        let vi = rect(ix, cy, ix + 130, cy + pill_h);
        let en = rect(ix + 140, cy, ix + 140 + 110, cy + pill_h);
        draw_pill(mem, vi, "V   Tiếng Việt", enabled, f_pill);
        draw_pill(mem, en, "E   English", !enabled, f_pill);
        hits.push((Region::PillVi, vi));
        hits.push((Region::PillEn, en));
        cy += pill_h + 14;

        // Divider.
        fill_solid(mem, rect(ix, cy, ir, cy + 1), DIVIDER);
        cy += 14;

        // Toggle-shortcut row: label · current value · [Reset] [Change].
        SelectObject(mem, f_row);
        draw_text(mem, "Toggle shortcut", rect(ix, cy, ix + 200, cy + 28), TEXT, DT_LEFT | DT_SINGLELINE | DT_VCENTER);

        let btn_w = 92;
        let btn = rect(ir - btn_w, cy, ir, cy + 28);
        let btn_label = if capturing { "Press keys…" } else { "Change" };
        fill_round(mem, btn, 8, if capturing { SECTION } else { PILL_OFF });
        SelectObject(mem, f_value);
        draw_text(
            mem,
            btn_label,
            btn,
            if capturing { WINDOW_BG } else { TEXT },
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );
        hits.push((Region::HotkeyChange, btn));

        // Reset — the only way back from a combination that can't be pressed.
        let reset = rect(btn.left - 8 - 60, cy, btn.left - 8, cy + 28);
        fill_round(mem, reset, 8, PILL_OFF);
        draw_text(mem, "Reset", reset, TEXT2, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
        hits.push((Region::HotkeyReset, reset));

        // Current hotkey text, right-aligned before the buttons.
        draw_text(
            mem,
            &hotkey_text,
            rect(ix + 130, cy, reset.left - 12, cy + 28),
            TEXT2,
            DT_RIGHT | DT_SINGLELINE | DT_VCENTER,
        );
        cy += 28 + 2;

        // Status line: validation feedback, capture hint, or a standing warning
        // that the shortcut never registered.
        let status = match (&notice, capturing, crate::hotkey_bound()) {
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
            SelectObject(mem, f_small);
            draw_text(
                mem,
                &text,
                rect(ix, cy, ir, cy + 16),
                match kind {
                    NoticeKind::Hint => TEXT2,
                    NoticeKind::Warn => SECTION,
                    NoticeKind::Error => DANGER,
                },
                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
            );
        }
        cy += 16 + 12;

        // Input method (Telex only, for now).
        SelectObject(mem, f_row);
        draw_text(mem, "Input Method", rect(ix, cy, ix + 200, cy + 24), TEXT, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        draw_text(mem, "Telex", rect(ix, cy, ir, cy + 24), TEXT2, DT_RIGHT | DT_SINGLELINE | DT_VCENTER);
        cy += 24 + 12;

        // Divider before the Quick Vietnamese option.
        fill_solid(mem, rect(ix, cy, ir, cy + 1), DIVIDER);
        cy += 14;

        // Quick Vietnamese toggle + caption.
        SelectObject(mem, f_row);
        let qr = toggle_row(mem, ix, ir, cy, "Quick Vietnamese", quick_vn);
        hits.push((Region::ToggleQuickVn, qr));
        cy += 30;
        SelectObject(mem, f_small);
        draw_text(
            mem,
            "Type w → ư after an initial consonant (tw→tư, chw→chư)",
            rect(ix, cy, ir, cy + 18),
            TEXT2,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
        cy += 18 + 10;

        // Deferred diacritics — indented sub-option, greyed while Quick
        // Vietnamese is off (clicks are ignored in that state too).
        let sub_ix = ix + 16;
        SelectObject(mem, f_row);
        draw_text(
            mem,
            "Deferred diacritics (Bỏ dấu sau)",
            rect(sub_ix, cy, ir - 60, cy + 28),
            if quick_vn { TEXT } else { TEXT2 },
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
        let dr = draw_toggle(mem, ir, cy + 14, quick_vn && deferred);
        hits.push((Region::ToggleDeferred, dr));
        cy += 30;
        SelectObject(mem, f_small);
        draw_text(
            mem,
            "Marks typed later apply backward (did→đi, thana→thân)",
            rect(sub_ix, cy, ir, cy + 18),
            TEXT2,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );

        y = card.bottom + GAP;
    }

    // ----- Card: GENERAL -----
    {
        let card = rect(PAD, y, cw - PAD, y + 128);
        fill_round(mem, card, CARD_RADIUS, CARD_BG);
        let ix = card.left + CARD_PAD;
        let ir = card.right - CARD_PAD;
        let mut cy = card.top + CARD_PAD;

        SelectObject(mem, f_section);
        draw_text(mem, "GENERAL", rect(ix, cy, ir, cy + 16), SECTION, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        cy += 26;

        SelectObject(mem, f_row);
        let r1 = toggle_row(mem, ix, ir, cy, "Start with Windows", autostart);
        hits.push((Region::ToggleAutostart, r1));
        cy += 38;
        let r2 = toggle_row(mem, ix, ir, cy, "Play sound on switch", sound);
        hits.push((Region::ToggleSound, r2));

        y = card.bottom + GAP;
    }

    // ----- Card: COMPATIBILITY -----
    {
        let card = rect(PAD, y, cw - PAD, y + 128);
        fill_round(mem, card, CARD_RADIUS, CARD_BG);
        let ix = card.left + CARD_PAD;
        let ir = card.right - CARD_PAD;
        let mut cy = card.top + CARD_PAD;

        SelectObject(mem, f_section);
        draw_text(mem, "COMPATIBILITY", rect(ix, cy, ir, cy + 16), SECTION, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        cy += 26;

        SelectObject(mem, f_row);
        let r1 = toggle_row(mem, ix, ir, cy, "Fix browser autocomplete", fix);
        hits.push((Region::ToggleFix, r1));
        cy += 38;
        let r2 = toggle_row(mem, ix, ir, cy, "Send keys step-by-step", step);
        hits.push((Region::ToggleStep, r2));

        y = card.bottom + GAP;
    }

    // ----- Card: ABOUT -----
    {
        let card = rect(PAD, y, cw - PAD, y + 84);
        fill_round(mem, card, CARD_RADIUS, CARD_BG);
        let ix = card.left + CARD_PAD;
        let ir = card.right - CARD_PAD;
        let mut cy = card.top + CARD_PAD;

        SelectObject(mem, f_section);
        draw_text(mem, "ABOUT", rect(ix, cy, ir, cy + 16), SECTION, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        cy += 24;

        SelectObject(mem, f_row);
        let ver = format!("NovaKey v{}", env!("CARGO_PKG_VERSION"));
        draw_text(mem, &ver, rect(ix, cy, ir, cy + 20), TEXT, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
        cy += 20;
        SelectObject(mem, f_small);
        draw_text(
            mem,
            "Vietnamese Input Method for Windows",
            rect(ix, cy, ir, cy + 18),
            TEXT2,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
    }

    // Blit and clean up.
    let _ = BitBlt(hdc, 0, 0, cw, ch, mem, 0, 0, SRCCOPY);
    SelectObject(mem, old_bmp);
    let _ = DeleteObject(bmp);
    let _ = DeleteDC(mem);
    for f in [f_title, f_section, f_row, f_pill, f_value, f_small] {
        let _ = DeleteObject(f);
    }
    let _ = EndPaint(hwnd, &ps);

    // Record hit rectangles for the next click.
    UI.with(|u| {
        if let Some(s) = u.borrow_mut().as_mut() {
            s.hits = hits;
        }
    });
}

/// A labelled row with a switch on the right. Returns the switch's hit rect.
unsafe fn toggle_row(hdc: HDC, ix: i32, ir: i32, cy: i32, label: &str, on: bool) -> RECT {
    draw_text(hdc, label, rect(ix, cy, ir - 60, cy + 28), TEXT, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
    draw_toggle(hdc, ir, cy + 14, on)
}

// MARK: - GDI helpers

const WINDOW_BG: COLORREF = rgb(28, 28, 31);
const CARD_BG: COLORREF = rgb(41, 41, 46);
const SECTION: COLORREF = rgb(255, 184, 69);
const TEXT: COLORREF = rgb(255, 255, 255);
const TEXT2: COLORREF = rgb(150, 150, 156);
const DANGER: COLORREF = rgb(255, 107, 107);
const PILL_OFF: COLORREF = rgb(48, 48, 54);
const TOGGLE_OFF: COLORREF = rgb(66, 66, 72);
const DIVIDER: COLORREF = rgb(52, 52, 58);

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    RECT {
        left,
        top,
        right,
        bottom,
    }
}

unsafe fn font(height: i32, weight: i32) -> HFONT {
    let face = wide("Segoe UI");
    // charset=DEFAULT(1), out/clip precis=0, quality=CLEARTYPE(5), pitch=DEFAULT(0).
    CreateFontW(
        height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        1,
        0,
        0,
        5,
        0,
        PCWSTR(face.as_ptr()),
    )
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

unsafe fn fill_solid(hdc: HDC, r: RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    FillRect(hdc, &r, brush);
    let _ = DeleteObject(brush);
}

unsafe fn fill_round(hdc: HDC, r: RECT, radius: i32, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    let old_brush = SelectObject(hdc, brush);
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    let _ = RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    let _ = DeleteObject(brush);
}

/// Fill a rounded "pill" with the brand red→orange→yellow horizontal gradient.
unsafe fn fill_gradient_pill(hdc: HDC, r: RECT) {
    let h = r.bottom - r.top;
    let rgn: HRGN = CreateRoundRectRgn(r.left, r.top, r.right + 1, r.bottom + 1, h, h);
    SelectClipRgn(hdc, rgn);

    let mid = (r.left + r.right) / 2;
    let verts = [
        tvtx(r.left, r.top, 230, 38, 51),
        tvtx(mid, r.bottom, 250, 110, 41),
        tvtx(mid, r.top, 250, 110, 41),
        tvtx(r.right, r.bottom, 255, 199, 71),
    ];
    let mesh = [
        GRADIENT_RECT {
            UpperLeft: 0,
            LowerRight: 1,
        },
        GRADIENT_RECT {
            UpperLeft: 2,
            LowerRight: 3,
        },
    ];
    let _ = GradientFill(
        hdc,
        &verts,
        mesh.as_ptr() as *const c_void,
        mesh.len() as u32,
        GRADIENT_FILL_RECT_H,
    );

    SelectClipRgn(hdc, HRGN::default());
    let _ = DeleteObject(rgn);
}

fn tvtx(x: i32, y: i32, r: u8, g: u8, b: u8) -> TRIVERTEX {
    TRIVERTEX {
        x,
        y,
        Red: (r as u16) << 8,
        Green: (g as u16) << 8,
        Blue: (b as u16) << 8,
        Alpha: 0xFFFF,
    }
}

/// Draw a language pill; a font must already be selected into `hdc`.
unsafe fn draw_pill(hdc: HDC, r: RECT, label: &str, active: bool, pill_font: HFONT) {
    let h = r.bottom - r.top;
    if active {
        fill_gradient_pill(hdc, r);
    } else {
        fill_round(hdc, r, h, PILL_OFF);
    }
    SelectObject(hdc, pill_font);
    let color = if active { TEXT } else { TEXT2 };
    draw_text(hdc, label, r, color, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
}

/// Draw a 44×24 switch whose right edge sits at `right_edge`, vertically
/// centered on `cy`. Returns the switch rect for hit-testing.
unsafe fn draw_toggle(hdc: HDC, right_edge: i32, cy: i32, on: bool) -> RECT {
    let (w, h) = (44, 24);
    let track = rect(right_edge - w, cy - h / 2, right_edge, cy + h / 2);
    if on {
        fill_gradient_pill(hdc, track);
    } else {
        fill_round(hdc, track, h, TOGGLE_OFF);
    }

    let knob = 18;
    let m = 3;
    let kx = if on {
        track.right - m - knob
    } else {
        track.left + m
    };
    let ky = track.top + m;
    let brush = CreateSolidBrush(rgb(255, 255, 255));
    let ob = SelectObject(hdc, brush);
    let op = SelectObject(hdc, GetStockObject(NULL_PEN));
    let _ = Ellipse(hdc, kx, ky, kx + knob, ky + knob);
    SelectObject(hdc, ob);
    SelectObject(hdc, op);
    let _ = DeleteObject(brush);
    track
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// Silence unused-type warnings for handles only produced transiently.
#[allow(dead_code)]
fn _touch(_: HBITMAP, _: HBRUSH, _: HGDIOBJ) {}
