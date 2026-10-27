//! hook.rs
//! The WH_KEYBOARD_LL / WH_MOUSE_LL callbacks and the shared engine state.
//!
//! All hooks are installed on — and their callbacks delivered to — the single
//! thread that runs the message pump, so the engine lives in a `thread_local!`
//! `RefCell` with no locking. The callback must never block (a >300 ms stall
//! makes Windows silently drop the hook), so it does no I/O or logging.

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, VK_CAPITAL, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_SPACE,
    VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use novakey_core::{Abbreviations, EngineResult, KeyClass, TelexEngine, WordTracker};

use crate::hotkey::ComboWatcher;
use crate::sender::{self, NOVAKEY_MAGIC};
use crate::vk;

thread_local! {
    static ENGINE: RefCell<TelexEngine> = RefCell::new(TelexEngine::new());

    /// Watches for a modifier-only toggle shortcut (e.g. Ctrl+Shift), which
    /// `RegisterHotKey` cannot express. Same thread as the hook callback.
    static COMBO: RefCell<ComboWatcher> = const { RefCell::new(ComboWatcher::new()) };

    /// The user's abbreviation list (Settings > Abbreviations).
    static ABBREVIATIONS: RefCell<Abbreviations> = RefCell::new(Abbreviations::new());

    /// The word being typed, for abbreviations: whether it started at a clean
    /// boundary, and its letters while Vietnamese composition is off.
    static WORD: RefCell<WordTracker> = RefCell::new(WordTracker::new());

    /// A NovaKey text box that takes keys literally — no composition, no
    /// abbreviations — so the abbreviation editor stores exactly the keys the
    /// user presses ("omw", not the "ơm" Telex would make of it). 0 if none.
    static LITERAL_FIELD: Cell<isize> = const { Cell::new(0) };
}

/// Arm the modifier-only toggle shortcut (0 disables it).
pub fn set_mod_combo(mods: u32) {
    COMBO.with(|c| c.borrow_mut().set_combo(mods));
}

/// Whether Vietnamese composition is active (toggled from the tray/hotkey).
static ENABLED: AtomicBool = AtomicBool::new(true);
/// Send replacement text one char at a time (compatibility mode).
static STEP_BY_STEP: AtomicBool = AtomicBool::new(false);
/// Whether to defeat browser URL-bar autocomplete with the U+202F prefix trick.
static FIX_AUTOCOMPLETE: AtomicBool = AtomicBool::new(true);
/// Whether the per-app rules (Settings > Apps) allow composition in the
/// current foreground app. Separate from `ENABLED` — the user's V/E choice —
/// so switching apps never changes or persists the language setting.
static APP_ALLOWED: AtomicBool = AtomicBool::new(true);
/// Whether the current foreground app is a web browser (updated on foreground
/// change so the hot keydown path only reads a cheap atomic).
static IS_BROWSER: AtomicBool = AtomicBool::new(false);
/// Timestamp (GetTickCount clock, same as `GetLastInputInfo`) of the last event
/// either hook saw. The watchdog in main.rs compares it with the system's last
/// input time to detect a hook Windows silently removed.
static LAST_HOOK_TIME: AtomicU32 = AtomicU32::new(0);
/// Whether abbreviations expand (Settings > Abbreviations).
static ABBREVIATIONS_ON: AtomicBool = AtomicBool::new(false);
/// Whether one of NovaKey's own windows has the focus. Abbreviations never
/// expand there, or typing "btw" into the abbreviation editor would expand it.
static IN_OWN_WINDOW: AtomicBool = AtomicBool::new(false);

pub fn last_hook_time() -> u32 {
    LAST_HOOK_TIME.load(Ordering::Relaxed)
}

pub fn mark_hook_alive(time: u32) {
    LAST_HOOK_TIME.store(time, Ordering::Relaxed);
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    // Starting fresh avoids composing across a mode switch.
    reset_engine();
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn toggle_enabled() -> bool {
    let new = !ENABLED.load(Ordering::Relaxed);
    set_enabled(new);
    new
}

pub fn set_step_by_step(on: bool) {
    STEP_BY_STEP.store(on, Ordering::Relaxed);
}

pub fn set_fix_autocomplete(on: bool) {
    FIX_AUTOCOMPLETE.store(on, Ordering::Relaxed);
}

pub fn is_fix_autocomplete() -> bool {
    FIX_AUTOCOMPLETE.load(Ordering::Relaxed)
}

/// Record whether the per-app rules allow composition in the foreground app.
pub fn set_app_allowed(on: bool) {
    APP_ALLOWED.store(on, Ordering::Relaxed);
}

pub fn is_app_allowed() -> bool {
    APP_ALLOWED.load(Ordering::Relaxed)
}

/// Record whether the foreground app is a browser (called from the foreground
/// WinEvent hook).
pub fn set_is_browser(on: bool) {
    IS_BROWSER.store(on, Ordering::Relaxed);
}

/// Whether the autocomplete-defeating prefix should be applied right now.
fn should_guard_autocomplete() -> bool {
    FIX_AUTOCOMPLETE.load(Ordering::Relaxed) && IS_BROWSER.load(Ordering::Relaxed)
}

/// Record whether one of NovaKey's own windows is in the foreground.
pub fn set_in_own_window(on: bool) {
    IN_OWN_WINDOW.store(on, Ordering::Relaxed);
}

/// Turn abbreviation expansion on or off.
pub fn set_abbreviations_enabled(on: bool) {
    ABBREVIATIONS_ON.store(on, Ordering::Relaxed);
}

/// Replace the abbreviation list.
pub fn set_abbreviations(table: Abbreviations) {
    ABBREVIATIONS.with(|t| *t.borrow_mut() = table);
}

/// A copy of the abbreviation list, for the editor.
pub fn abbreviations() -> Abbreviations {
    ABBREVIATIONS.with(|t| t.borrow().clone())
}

/// Make `hwnd` (a NovaKey text box) take keys literally while it has the
/// focus; `None` clears it.
pub fn set_literal_field(hwnd: Option<isize>) {
    LITERAL_FIELD.with(|f| f.set(hwnd.unwrap_or(0)));
}

/// Whether the focused window is the literal text box. `GetFocus` only sees
/// this thread's windows, which is where the editor lives.
fn in_literal_field() -> bool {
    let field = LITERAL_FIELD.with(|f| f.get());
    field != 0 && unsafe { GetFocus() }.0 as isize == field
}

/// Reset the composition session — called on mouse click and foreground change.
pub fn reset_engine() {
    ENGINE.with(|e| e.borrow_mut().reset_session());
    WORD.with(|w| w.borrow_mut().boundary());
}

/// Enable/disable "Quick Vietnamese" on the engine (persists across resets).
pub fn set_quick_vietnamese(on: bool) {
    ENGINE.with(|e| e.borrow_mut().set_quick_vietnamese(on));
}

/// Enable/disable "Deferred diacritics" (Bỏ dấu sau) on the engine — only
/// effective while Quick Vietnamese is also on (persists across resets).
pub fn set_deferred_diacritics(on: bool) {
    ENGINE.with(|e| e.borrow_mut().set_deferred_diacritics(on));
}

/// Low-level keyboard hook callback.
pub unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    mark_hook_alive(kb.time);

    // Skip our own injected events (equivalent of the macOS source stateID).
    // We intentionally do NOT skip all LLKHF_INJECTED — only our tagged input.
    if kb.dwExtraInfo == NOVAKEY_MAGIC {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let msg = wparam.0 as u32;
    let vk_code = kb.vk_code_u32();

    // Modifier-only toggle (Ctrl+Shift, …). Fed every key event, including
    // releases, and checked before the enabled/keydown filters below so the
    // shortcut still turns Vietnamese back *on*. The key is never suppressed —
    // modifiers on their own do nothing in the focused app anyway.
    let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
    if down || msg == WM_KEYUP || msg == WM_SYSKEYUP {
        if COMBO.with(|c| c.borrow_mut().on_key(vk_code, down)) {
            crate::post_toggle();
        }
    }

    if !down {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    // Paused in this app by the per-app rules, or typing into the literal
    // text box -> behave like a plain keyboard.
    if !is_app_allowed() || in_literal_field() {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let key = match vk::classify(vk_code) {
        Some(k) => k,
        None => return CallNextHookEx(None, code, wparam, lparam), // modifier/toggle
    };

    let ctrl = key_down(VK_CONTROL.0) || key_down(VK_LWIN.0) || key_down(VK_RWIN.0);
    let alt = key_down(VK_MENU.0);
    let shift = shift_state_for_letter();
    let vietnamese = is_enabled();

    // Tab after an abbreviation replaces it with its expansion, in either
    // language. The Tab is consumed, so it does not also move the focus or
    // trigger a shell's completion; a Tab that expands nothing passes through.
    if vk_code == VK_TAB.0 as u32 && !ctrl && !alt && !key_down(VK_SHIFT.0) {
        if let Some(EngineResult::Restore { backspaces, text }) = expand_abbreviation(vietnamese) {
            send_replace(backspaces, &text);
            return LRESULT(1);
        }
    }
    track_word(key, shift, ctrl || alt, vk_code);

    // English: nothing to compose.
    if !vietnamese {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let result = ENGINE.with(|e| e.borrow_mut().process_key(key, shift, ctrl, alt));

    match result {
        EngineResult::PassThrough | EngineResult::WordBreak => {
            CallNextHookEx(None, code, wparam, lparam)
        }
        EngineResult::Replace { backspaces, text } => {
            // KEYEVENTF_UNICODE already emits each code unit as its own event, so
            // the batch is inherently "step by step" at the unit level; the flag
            // is retained for parity with macOS and future tuning.
            let _ = STEP_BY_STEP.load(Ordering::Relaxed);
            send_replace(backspaces, &text);
            LRESULT(1) // suppress the original key
        }
        EngineResult::Restore { backspaces, text } => {
            send_restore(backspaces, &text, vk_code);
            LRESULT(1)
        }
    }
}

/// Send a `Replace`: delete `backspaces` characters and type `text`.
fn send_replace(backspaces: usize, text: &str) {
    let inputs = if backspaces > 0 && should_guard_autocomplete() {
        // U+202F collapses the URL-bar autocomplete selection; +1 BS
        // deletes it back out.
        let mut v = sender::build_autocomplete_prefix();
        v.extend(sender::build_replace(backspaces + 1, text));
        v
    } else {
        sender::build_replace(backspaces, text)
    };
    sender::send(&inputs);
}

/// Send a `Restore`: suppress the original word-break key and re-inject it
/// after the replacement text so ordering is preserved.
fn send_restore(backspaces: usize, text: &str, vk_code: u32) {
    let inputs = if backspaces > 0 && should_guard_autocomplete() {
        let mut v = sender::build_autocomplete_prefix();
        v.extend(sender::build_restore(backspaces + 1, text, vk_code as u16));
        v
    } else {
        sender::build_restore(backspaces, text, vk_code as u16)
    };
    sender::send(&inputs);
}

/// The replacement for the word Tab is ending, if it is an abbreviation
/// that started at a clean boundary. In Vietnamese the engine knows both what
/// is on screen and the keys behind it; in English the tracker's letters are
/// what is on screen.
fn expand_abbreviation(vietnamese: bool) -> Option<EngineResult> {
    if !ABBREVIATIONS_ON.load(Ordering::Relaxed) || IN_OWN_WINDOW.load(Ordering::Relaxed) {
        return None;
    }
    if !WORD.with(|w| w.borrow().is_clean()) {
        return None;
    }
    let result = ABBREVIATIONS.with(|t| {
        let t = t.borrow();
        if vietnamese {
            ENGINE.with(|e| e.borrow_mut().expand_abbreviation(&t))
        } else {
            WORD.with(|w| w.borrow().word().and_then(|word| t.expand_word(word)))
        }
    })?;
    if let EngineResult::Restore { text, .. } = &result {
        WORD.with(|w| w.borrow_mut().expanded(text));
    }
    Some(result)
}

/// Feed a keydown to the abbreviation word tracker. `modified`: Ctrl, Alt or
/// Win is held, so the key is a shortcut rather than typing. A shortcut starts
/// a fresh word — Ctrl+C, Ctrl+U or Ctrl+L leave a terminal on a clean prompt,
/// Ctrl+Backspace and Ctrl+arrows land on a word edge — except paste, which
/// leaves unseen text glued before the caret.
fn track_word(key: KeyClass, shift: bool, modified: bool, vk_code: u32) {
    WORD.with(|w| {
        let mut w = w.borrow_mut();
        match key {
            _ if modified && vk_code == u32::from(b'V') => w.interrupt(),
            _ if modified => w.boundary(),
            KeyClass::Letter(c) => w.letter(if shift { c.to_ascii_uppercase() } else { c }),
            KeyClass::Backspace => w.backspace(),
            KeyClass::WordBreak if vk_code == VK_SPACE.0 as u32 => w.space(),
            KeyClass::WordBreak if vk::starts_word(vk_code) => w.boundary(),
            _ => w.interrupt(),
        }
    });
}

/// Low-level mouse hook callback — any button/press resets the session so we
/// don't compose across a click that moved the caret.
pub unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        mark_hook_alive((*(lparam.0 as *const MSLLHOOKSTRUCT)).time);
        // WM_LBUTTONDOWN(0x0201), RBUTTONDOWN(0x0204), MBUTTONDOWN(0x0207)
        let msg = wparam.0 as u32;
        if matches!(msg, 0x0201 | 0x0204 | 0x0207) {
            reset_engine();
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Effective case for a letter: Shift XOR CapsLock.
fn shift_state_for_letter() -> bool {
    let shift = key_down(VK_SHIFT.0);
    let caps = unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 0x0001 != 0;
    shift ^ caps
}

/// Whether a virtual key is currently held (high bit of GetKeyState).
fn key_down(vk: u16) -> bool {
    (unsafe { GetKeyState(vk as i32) } as u16 & 0x8000) != 0
}

/// Small helper to read the vkCode field regardless of the field's exact type.
trait VkCode {
    fn vk_code_u32(&self) -> u32;
}
impl VkCode for KBDLLHOOKSTRUCT {
    fn vk_code_u32(&self) -> u32 {
        self.vkCode
    }
}
