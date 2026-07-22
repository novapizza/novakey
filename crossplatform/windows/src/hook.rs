//! hook.rs
//! The WH_KEYBOARD_LL / WH_MOUSE_LL callbacks and the shared engine state.
//!
//! All hooks are installed on — and their callbacks delivered to — the single
//! thread that runs the message pump, so the engine lives in a `thread_local!`
//! `RefCell` with no locking. The callback must never block (a >300 ms stall
//! makes Windows silently drop the hook), so it does no I/O or logging.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CAPITAL, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use novakey_core::{EngineResult, TelexEngine};

use crate::sender::{self, NOVAKEY_MAGIC};
use crate::vk;

thread_local! {
    static ENGINE: RefCell<TelexEngine> = RefCell::new(TelexEngine::new());
}

/// Whether Vietnamese composition is active (toggled from the tray/hotkey).
static ENABLED: AtomicBool = AtomicBool::new(true);
/// Send replacement text one char at a time (compatibility mode).
static STEP_BY_STEP: AtomicBool = AtomicBool::new(false);
/// Whether to defeat browser URL-bar autocomplete with the U+202F prefix trick.
static FIX_AUTOCOMPLETE: AtomicBool = AtomicBool::new(true);
/// Whether the current foreground app is a web browser (updated on foreground
/// change so the hot keydown path only reads a cheap atomic).
static IS_BROWSER: AtomicBool = AtomicBool::new(false);

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

/// Record whether the foreground app is a browser (called from the foreground
/// WinEvent hook).
pub fn set_is_browser(on: bool) {
    IS_BROWSER.store(on, Ordering::Relaxed);
}

/// Whether the autocomplete-defeating prefix should be applied right now.
fn should_guard_autocomplete() -> bool {
    FIX_AUTOCOMPLETE.load(Ordering::Relaxed) && IS_BROWSER.load(Ordering::Relaxed)
}

/// Reset the composition session — called on mouse click and foreground change.
pub fn reset_engine() {
    ENGINE.with(|e| e.borrow_mut().reset_session());
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

    // Skip our own injected events (equivalent of the macOS source stateID).
    // We intentionally do NOT skip all LLKHF_INJECTED — only our tagged input.
    if kb.dwExtraInfo == NOVAKEY_MAGIC {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let msg = wparam.0 as u32;
    if msg != WM_KEYDOWN && msg != WM_SYSKEYDOWN {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    // Disabled -> behave like a plain keyboard.
    if !is_enabled() {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let vk_code = kb.vk_code_u32();
    let key = match vk::classify(vk_code) {
        Some(k) => k,
        None => return CallNextHookEx(None, code, wparam, lparam), // modifier/toggle
    };

    let ctrl = key_down(VK_CONTROL.0) || key_down(VK_LWIN.0) || key_down(VK_RWIN.0);
    let alt = key_down(VK_MENU.0);
    let shift = shift_state_for_letter();

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
            let inputs = if backspaces > 0 && should_guard_autocomplete() {
                // U+202F collapses the URL-bar autocomplete selection; +1 BS
                // deletes it back out.
                let mut v = sender::build_autocomplete_prefix();
                v.extend(sender::build_replace(backspaces + 1, &text));
                v
            } else {
                sender::build_replace(backspaces, &text)
            };
            sender::send(&inputs);
            LRESULT(1) // suppress the original key
        }
        EngineResult::Restore { backspaces, text } => {
            // Suppress the original word-break key and re-inject it after the
            // restore text so ordering is preserved.
            let inputs = if backspaces > 0 && should_guard_autocomplete() {
                let mut v = sender::build_autocomplete_prefix();
                v.extend(sender::build_restore(backspaces + 1, &text, vk_code as u16));
                v
            } else {
                sender::build_restore(backspaces, &text, vk_code as u16)
            };
            sender::send(&inputs);
            LRESULT(1)
        }
    }
}

/// Low-level mouse hook callback — any button/press resets the session so we
/// don't compose across a click that moved the caret.
pub unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
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
