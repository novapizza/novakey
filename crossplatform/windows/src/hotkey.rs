//! hotkey.rs
//! Pure (no `windows` crate) helpers describing the language-toggle hotkey.
//!
//! A hotkey is stored as two integers matching Win32 `RegisterHotKey`:
//!   * `mods` — a bitmask of the `MOD_*` flags below.
//!   * `vk`   — a virtual-key code.
//! Keeping this module numeric-only lets it be unit-tested without linking Win32.

/// `RegisterHotKey` modifier flags (values fixed by Win32 — do not change).
pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;

/// Default toggle hotkey: **Ctrl + Space** (VK_SPACE = 0x20).
pub const DEFAULT_MODS: u32 = MOD_CONTROL;
pub const DEFAULT_VK: u32 = 0x20;

/// Human-readable name for a single virtual-key code, e.g. `0x20 -> "Space"`.
pub fn vk_name(vk: u32) -> String {
    match vk {
        0x41..=0x5A => ((b'A' + (vk as u8 - 0x41)) as char).to_string(), // A-Z
        0x30..=0x39 => ((b'0' + (vk as u8 - 0x30)) as char).to_string(), // 0-9
        0x60..=0x69 => format!("Num{}", vk - 0x60),                       // numpad 0-9
        0x70..=0x87 => format!("F{}", vk - 0x70 + 1),                     // F1-F24
        0x20 => "Space".into(),
        0x0D => "Enter".into(),
        0x09 => "Tab".into(),
        0x08 => "Backspace".into(),
        0x2E => "Delete".into(),
        0x2D => "Insert".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        other => format!("Key{:#04X}", other),
    }
}

/// A `Ctrl+Shift+Space`-style description of a `mods`+`vk` combination.
pub fn describe(mods: u32, vk: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    if mods & MOD_CONTROL != 0 {
        parts.push("Ctrl".into());
    }
    if mods & MOD_ALT != 0 {
        parts.push("Alt".into());
    }
    if mods & MOD_SHIFT != 0 {
        parts.push("Shift".into());
    }
    if mods & MOD_WIN != 0 {
        parts.push("Win".into());
    }
    parts.push(vk_name(vk));
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_default() {
        assert_eq!(describe(DEFAULT_MODS, DEFAULT_VK), "Ctrl+Space");
    }

    #[test]
    fn describes_combo_in_canonical_order() {
        // Order is always Ctrl, Alt, Shift, Win, then the key — regardless of
        // the order the flags happen to be OR'd together.
        let mods = MOD_SHIFT | MOD_WIN | MOD_CONTROL | MOD_ALT;
        assert_eq!(describe(mods, 0x5A), "Ctrl+Alt+Shift+Win+Z");
    }

    #[test]
    fn names_letters_and_function_keys() {
        assert_eq!(vk_name(0x41), "A");
        assert_eq!(vk_name(0x70), "F1");
        assert_eq!(vk_name(0x87), "F24");
    }
}
