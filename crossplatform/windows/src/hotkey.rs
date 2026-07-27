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

/// Every modifier bit we accept. Anything else in a `mods` value is bogus
/// (`MOD_NOREPEAT` is added at registration time, never stored).
pub const ALL_MODS: u32 = MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN;

/// Default toggle hotkey: **Ctrl + Space** (VK_SPACE = 0x20).
pub const DEFAULT_MODS: u32 = MOD_CONTROL;
pub const DEFAULT_VK: u32 = 0x20;

/// `vk` value meaning "no main key" — a modifier-only shortcut such as
/// Ctrl+Shift, which is what most Vietnamese IMEs bind by default.
/// `RegisterHotKey` cannot express this, so those are detected in the
/// low-level keyboard hook instead (see `ComboWatcher`).
pub const VK_NONE: u32 = 0;

/// Whether a virtual-key is a modifier that can't stand as the main key.
/// Shift/Ctrl/Alt (and their L/R variants), CapsLock, and both Win keys.
pub fn is_modifier_vk(vk: u32) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0x14 | 0x5B | 0x5C | 0xA0..=0xA5)
}

/// Whether a stored pair describes a modifier-only shortcut.
pub fn is_modifier_only(mods: u32, vk: u32) -> bool {
    vk == VK_NONE && mods != 0
}

/// The `MOD_*` bit a virtual-key belongs to, if it is a modifier. Low-level
/// hook events report the side-specific keys (VK_LSHIFT etc.), so both the
/// generic and the L/R codes map here.
pub fn mod_bit(vk: u32) -> Option<u32> {
    match vk {
        0x10 | 0xA0 | 0xA1 => Some(MOD_SHIFT),
        0x11 | 0xA2 | 0xA3 => Some(MOD_CONTROL),
        0x12 | 0xA4 | 0xA5 => Some(MOD_ALT),
        0x5B | 0x5C => Some(MOD_WIN),
        _ => None,
    }
}

/// F1–F24 — the only keys usable with no modifier at all.
fn is_function_key(vk: u32) -> bool {
    (0x70..=0x87).contains(&vk)
}

/// The verdict on a candidate hotkey. `Warn` still binds; `Reject` never does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Validity {
    Ok,
    Warn(&'static str),
    Reject(&'static str),
}

/// Check a `mods`+`vk` pair before it is registered or persisted.
///
/// The important rule is the modifier-less one: `RegisterHotKey` happily accepts
/// `mods = 0`, which would bind a bare key system-wide and swallow it in every
/// application. Function keys are the one sane exception.
pub fn validate(mods: u32, vk: u32) -> Validity {
    if mods & !ALL_MODS != 0 {
        return Validity::Reject("That key can't be used as a shortcut.");
    }

    // Modifier-only shortcut (Ctrl+Shift and friends). Two modifiers minimum:
    // a single one fires constantly during ordinary typing.
    if vk == VK_NONE {
        if mods.count_ones() < 2 {
            return Validity::Reject("Hold at least two modifiers, e.g. Ctrl+Shift.");
        }
        if mods == MOD_ALT | MOD_SHIFT {
            return Validity::Warn("Windows also uses Alt+Shift to switch keyboard layouts.");
        }
        if mods & MOD_WIN != 0 {
            return Validity::Warn("Windows may claim this combination for itself.");
        }
        return Validity::Ok;
    }

    if vk > 0xFF {
        return Validity::Reject("That key can't be used as a shortcut.");
    }
    if is_modifier_vk(vk) {
        return Validity::Reject("Hold the modifiers and press another key.");
    }
    if mods == 0 && !is_function_key(vk) {
        return Validity::Reject("Add Ctrl, Alt, Shift or Win — a plain key would be swallowed everywhere.");
    }
    if mods == MOD_SHIFT && !is_function_key(vk) {
        return Validity::Reject("Shift alone isn't enough — add Ctrl or Alt.");
    }
    if mods & MOD_WIN != 0 {
        return Validity::Warn("Windows may claim this combination for itself.");
    }
    Validity::Ok
}

/// Whether a pair is safe to register/persist (i.e. not rejected).
pub fn is_valid(mods: u32, vk: u32) -> bool {
    !matches!(validate(mods, vk), Validity::Reject(_))
}

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
    // A modifier-only shortcut is just its modifiers ("Ctrl+Shift").
    if vk != VK_NONE {
        parts.push(vk_name(vk));
    }
    parts.join("+")
}

/// Detects modifier-only shortcuts from the raw key stream.
///
/// The combination fires on *release*, and only if nothing else was pressed
/// while the modifiers were held — so Ctrl+Shift toggles the language, while
/// Ctrl+Shift+S stays a normal shortcut for whatever app has focus. Pure state
/// machine: no Win32, driven by the low-level hook.
pub struct ComboWatcher {
    /// The modifier set to watch for; 0 disables detection.
    combo: u32,
    /// Modifier bits currently held down.
    held: u32,
    /// False once a non-modifier key joins in, until every modifier is released.
    armed: bool,
}

impl ComboWatcher {
    pub const fn new() -> Self {
        ComboWatcher {
            combo: 0,
            held: 0,
            armed: false,
        }
    }

    /// Watch for `mods` (0 to disable). Resets any in-flight press.
    pub fn set_combo(&mut self, mods: u32) {
        self.combo = mods;
        self.held = 0;
        self.armed = false;
    }

    /// Feed one key event. Returns true when the combination just completed.
    pub fn on_key(&mut self, vk: u32, down: bool) -> bool {
        let bit = mod_bit(vk);

        if down {
            match bit {
                Some(b) => {
                    if self.held == 0 {
                        self.armed = true;
                    }
                    self.held |= b;
                }
                // A real keystroke: this is a normal shortcut, not a toggle.
                None => self.armed = false,
            }
            return false;
        }

        let Some(bit) = bit else { return false };
        let was_held = self.held;
        self.held &= !bit;

        let fires = self.armed && self.combo != 0 && was_held == self.combo;
        // Fire once per press, and re-arm only after everything is released.
        if fires || self.held == 0 {
            self.armed = false;
        }
        fires
    }
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

    #[test]
    fn rejects_modifier_less_ordinary_keys() {
        // The bug this guards: RegisterHotKey(0, 'Z') binds a bare Z globally.
        assert!(matches!(validate(0, 0x5A), Validity::Reject(_)));
        assert!(matches!(validate(0, DEFAULT_VK), Validity::Reject(_)));
        assert!(!is_valid(0, 0x5A));
    }

    #[test]
    fn allows_modifier_less_function_keys() {
        assert_eq!(validate(0, 0x70), Validity::Ok); // F1
        assert_eq!(validate(0, 0x87), Validity::Ok); // F24
    }

    #[test]
    fn rejects_shift_only_combos() {
        assert!(matches!(validate(MOD_SHIFT, 0x41), Validity::Reject(_)));
        // ...but Shift alongside Ctrl is fine.
        assert_eq!(validate(MOD_SHIFT | MOD_CONTROL, DEFAULT_VK), Validity::Ok);
    }

    #[test]
    fn rejects_modifier_keys_and_out_of_range_vks() {
        assert!(matches!(validate(MOD_CONTROL, 0x10), Validity::Reject(_))); // Shift
        assert!(matches!(validate(MOD_CONTROL, 0x5B), Validity::Reject(_))); // LWin
        assert!(matches!(validate(MOD_CONTROL, 0), Validity::Reject(_)));
        assert!(matches!(validate(MOD_CONTROL, 0x1_0000), Validity::Reject(_)));
    }

    #[test]
    fn rejects_unknown_modifier_bits() {
        // e.g. MOD_NOREPEAT leaking into a persisted value.
        assert!(matches!(validate(0x4000 | MOD_CONTROL, 0x41), Validity::Reject(_)));
    }

    #[test]
    fn warns_but_allows_win_combos() {
        assert!(matches!(validate(MOD_WIN | MOD_CONTROL, 0x41), Validity::Warn(_)));
        assert!(is_valid(MOD_WIN | MOD_CONTROL, 0x41));
    }

    #[test]
    fn default_binding_is_valid() {
        assert_eq!(validate(DEFAULT_MODS, DEFAULT_VK), Validity::Ok);
    }

    // MARK: - Modifier-only shortcuts (Ctrl+Shift and friends)

    #[test]
    fn accepts_two_modifier_combos() {
        assert_eq!(validate(MOD_CONTROL | MOD_SHIFT, VK_NONE), Validity::Ok);
        assert_eq!(validate(MOD_CONTROL | MOD_ALT, VK_NONE), Validity::Ok);
        assert!(is_modifier_only(MOD_CONTROL | MOD_SHIFT, VK_NONE));
    }

    #[test]
    fn rejects_single_modifier_combos() {
        // One modifier would fire during ordinary typing.
        assert!(matches!(validate(MOD_SHIFT, VK_NONE), Validity::Reject(_)));
        assert!(matches!(validate(MOD_CONTROL, VK_NONE), Validity::Reject(_)));
        assert!(matches!(validate(0, VK_NONE), Validity::Reject(_)));
    }

    #[test]
    fn warns_on_alt_shift_layout_chord() {
        assert!(matches!(validate(MOD_ALT | MOD_SHIFT, VK_NONE), Validity::Warn(_)));
    }

    #[test]
    fn describes_modifier_only_combos() {
        assert_eq!(describe(MOD_CONTROL | MOD_SHIFT, VK_NONE), "Ctrl+Shift");
    }

    /// Press and release a list of `(vk, down)` events, returning how many times
    /// the watcher fired.
    fn run(watcher: &mut ComboWatcher, events: &[(u32, bool)]) -> usize {
        events
            .iter()
            .filter(|(vk, down)| watcher.on_key(*vk, *down))
            .count()
    }

    const L_CTRL: u32 = 0xA2;
    const L_SHIFT: u32 = 0xA0;
    const L_ALT: u32 = 0xA4;
    const KEY_S: u32 = 0x53;

    #[test]
    fn watcher_fires_once_on_release() {
        let mut w = ComboWatcher::new();
        w.set_combo(MOD_CONTROL | MOD_SHIFT);
        let fired = run(
            &mut w,
            &[
                (L_CTRL, true),
                (L_SHIFT, true),
                (L_SHIFT, false),
                (L_CTRL, false),
            ],
        );
        assert_eq!(fired, 1);
    }

    #[test]
    fn watcher_ignores_combos_used_as_a_shortcut() {
        // Ctrl+Shift+S must stay a normal shortcut for the focused app.
        let mut w = ComboWatcher::new();
        w.set_combo(MOD_CONTROL | MOD_SHIFT);
        let fired = run(
            &mut w,
            &[
                (L_CTRL, true),
                (L_SHIFT, true),
                (KEY_S, true),
                (KEY_S, false),
                (L_SHIFT, false),
                (L_CTRL, false),
            ],
        );
        assert_eq!(fired, 0);
    }

    #[test]
    fn watcher_ignores_partial_and_extra_modifiers() {
        let mut w = ComboWatcher::new();
        w.set_combo(MOD_CONTROL | MOD_SHIFT);
        // Ctrl alone.
        assert_eq!(run(&mut w, &[(L_CTRL, true), (L_CTRL, false)]), 0);
        // Ctrl+Alt+Shift — a superset is not the combination.
        let fired = run(
            &mut w,
            &[
                (L_CTRL, true),
                (L_ALT, true),
                (L_SHIFT, true),
                (L_SHIFT, false),
                (L_ALT, false),
                (L_CTRL, false),
            ],
        );
        assert_eq!(fired, 0);
    }

    #[test]
    fn watcher_rearms_between_presses() {
        let mut w = ComboWatcher::new();
        w.set_combo(MOD_CONTROL | MOD_SHIFT);
        let press = [
            (L_CTRL, true),
            (L_SHIFT, true),
            (L_SHIFT, false),
            (L_CTRL, false),
        ];
        assert_eq!(run(&mut w, &press), 1);
        assert_eq!(run(&mut w, &press), 1);
    }

    #[test]
    fn watcher_is_inert_without_a_combo() {
        let mut w = ComboWatcher::new();
        let fired = run(
            &mut w,
            &[
                (L_CTRL, true),
                (L_SHIFT, true),
                (L_SHIFT, false),
                (L_CTRL, false),
            ],
        );
        assert_eq!(fired, 0);
    }
}
