//! sender.rs
//! Builds and dispatches the `SendInput` batch that implements the
//! backspace-and-replace technique. Every synthetic event is tagged with
//! `dwExtraInfo = NOVAKEY_MAGIC` so the hook can recognise and skip its own
//! injected input (the Windows equivalent of the macOS CGEventSource stateID).

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_BACK,
};

/// Marker stored in `dwExtraInfo` on every event we inject, so the low-level
/// hook can ignore its own output. "NOVA" in ASCII.
pub const NOVAKEY_MAGIC: usize = 0x4E4F5641;

/// Narrow no-break space — an (effectively invisible) character inserted then
/// immediately deleted to collapse a browser autocomplete selection before our
/// backspaces run. See `build_autocomplete_prefix`.
const NNBSP: u16 = 0x202F;

/// One VK key event (down or up), tagged as ours.
fn vk_event(vk: u16, key_up: bool) -> INPUT {
    let flags = if key_up { KEYEVENTF_KEYUP } else { Default::default() };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: NOVAKEY_MAGIC,
            },
        },
    }
}

/// One Unicode UTF-16 code-unit event (down or up), tagged as ours.
fn unicode_event(unit: u16, key_up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: NOVAKEY_MAGIC,
            },
        },
    }
}

/// Build the input batch for a `Replace`: N backspaces, then the replacement
/// text as KEYEVENTF_UNICODE units. Output is guaranteed single-scalar BMP by
/// the engine, so 1 char == 1 UTF-16 unit == 1 backspace.
pub fn build_replace(backspaces: usize, text: &str) -> Vec<INPUT> {
    let mut inputs = Vec::with_capacity(backspaces * 2 + text.len() * 2);
    for _ in 0..backspaces {
        inputs.push(vk_event(VK_BACK.0, false));
        inputs.push(vk_event(VK_BACK.0, true));
    }
    for unit in text.encode_utf16() {
        inputs.push(unicode_event(unit, false));
        inputs.push(unicode_event(unit, true));
    }
    inputs
}

/// Build the input batch for a `Restore`: the replacement (backspaces + raw
/// text) followed by a re-injection of the original word-break key. Because
/// our injected input queues *behind* the (suppressed) original on Windows, we
/// must append the original key ourselves to preserve typing order.
pub fn build_restore(backspaces: usize, text: &str, original_vk: u16) -> Vec<INPUT> {
    let mut inputs = build_replace(backspaces, text);
    inputs.push(vk_event(original_vk, false));
    inputs.push(vk_event(original_vk, true));
    inputs
}

/// The two events that collapse a browser autocomplete selection: type a
/// narrow no-break space (replacing any selected suggestion). The caller must
/// then send one extra backspace to delete this character. Prepend this to a
/// `build_replace(backspaces + 1, ..)` / `build_restore(backspaces + 1, ..)`
/// batch. Self-correcting: works whether or not a selection was present.
pub fn build_autocomplete_prefix() -> Vec<INPUT> {
    vec![unicode_event(NNBSP, false), unicode_event(NNBSP, true)]
}

/// Dispatch a batch in a single `SendInput` call. No-op on empty input.
pub fn send(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock-sender style assertion: verify the batch shape per EngineResult
    /// without actually calling SendInput.
    fn shape(inputs: &[INPUT]) -> Vec<(u16, u16, bool, bool)> {
        // (wVk, wScan, is_unicode, is_keyup)
        inputs
            .iter()
            .map(|i| unsafe {
                let ki = i.Anonymous.ki;
                (
                    ki.wVk.0,
                    ki.wScan,
                    (ki.dwFlags & KEYEVENTF_UNICODE).0 != 0,
                    (ki.dwFlags & KEYEVENTF_KEYUP).0 != 0,
                )
            })
            .collect()
    }

    #[test]
    fn replace_one_backspace_one_char() {
        // "dd" -> "đ": 1 backspace + đ (U+0111).
        let inputs = build_replace(1, "\u{0111}");
        let s = shape(&inputs);
        assert_eq!(s.len(), 4); // bs down/up + đ down/up
        assert_eq!(s[0], (VK_BACK.0, 0, false, false));
        assert_eq!(s[1], (VK_BACK.0, 0, false, true));
        assert_eq!(s[2], (0, 0x0111, true, false));
        assert_eq!(s[3], (0, 0x0111, true, true));
    }

    #[test]
    fn replace_two_backspaces_two_chars() {
        // "uow" -> "ươ": 2 backspaces + 2 chars.
        let inputs = build_replace(2, "\u{01B0}\u{01A1}");
        assert_eq!(inputs.len(), 2 * 2 + 2 * 2);
    }

    #[test]
    fn all_tagged_with_magic() {
        let inputs = build_replace(1, "\u{0111}");
        for i in &inputs {
            unsafe {
                assert_eq!(i.Anonymous.ki.dwExtraInfo, NOVAKEY_MAGIC);
            }
        }
    }

    #[test]
    fn autocomplete_guarded_replace_shape() {
        // Browser-guarded "dd" -> "đ": U+202F, then 2 backspaces (N+1), then đ.
        let mut inputs = build_autocomplete_prefix();
        inputs.extend(build_replace(1 + 1, "\u{0111}"));
        let s = shape(&inputs);
        assert_eq!(s.len(), 2 + 4 + 2); // nnbsp + 2×BS + đ
        assert_eq!(s[0], (0, NNBSP, true, false)); // U+202F down (unicode)
        assert_eq!(s[1], (0, NNBSP, true, true)); // U+202F up
        assert_eq!(s[2], (VK_BACK.0, 0, false, false)); // BS 1
        assert_eq!(s[4], (VK_BACK.0, 0, false, false)); // BS 2 (the extra)
        assert_eq!(s[6], (0, 0x0111, true, false)); // đ
    }

    #[test]
    fn restore_appends_original_key() {
        // "wd" + space -> restore 2 "wd" then re-inject VK_SPACE (0x20).
        let inputs = build_restore(2, "wd", 0x20);
        let s = shape(&inputs);
        // 2 backspaces (4) + "wd" (4) + space down/up (2) = 10
        assert_eq!(s.len(), 10);
        assert_eq!(s[8], (0x20, 0, false, false)); // space down (VK, not unicode)
        assert_eq!(s[9], (0x20, 0, false, true)); // space up
    }
}
