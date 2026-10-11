// HotkeyManager.swift
// Manages the global hotkey for toggling Vietnamese/English mode.

import Cocoa

/// Provides hotkey configuration for the language toggle.
/// The actual detection happens in EventTapManager's callback,
/// this class just manages the settings.
enum HotkeyManager {

    /// Default toggle hotkey: Option + Z
    static let defaultKeyCode: UInt16 = KeyCode.z.rawValue
    static let defaultModifiers: CGEventFlags = .maskAlternate

    /// The modifier bits a shortcut may use. Everything else in a CGEventFlags
    /// value (CapsLock, Fn, numeric-pad, coalescing bits) is ignored when
    /// matching, so a stray flag can never stop the hotkey from firing.
    static let recognizedModifiers: CGEventFlags = [
        .maskControl, .maskAlternate, .maskShift, .maskCommand,
    ]

    /// The verdict on a candidate shortcut. `.warn` still binds; `.reject` never
    /// does.
    enum Validity: Equatable {
        case ok
        case warn(String)
        case reject(String)

        var isReject: Bool {
            if case .reject = self { return true }
            return false
        }

        /// Message to show the user, if any.
        var message: String? {
            switch self {
            case .ok: return nil
            case .warn(let m), .reject(let m): return m
            }
        }
    }

    /// F1–F20 keycodes, in order. They are not in `KeyCode` (the engine has no
    /// use for them) and their virtual keycodes are famously unordered.
    private static let functionKeyCodes: [UInt16] = [
        0x7A, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6D,  // F1–F10
        0x67, 0x6F, 0x69, 0x6B, 0x71, 0x6A, 0x40, 0x4F, 0x50, 0x5A,  // F11–F20
    ]

    /// Function keys are the only sensible modifier-free shortcuts.
    private static let functionKeys = Set(functionKeyCodes)

    /// System shortcuts that NovaKey's tap sits in front of. Binding one of
    /// these works, but it stops the system from ever seeing it — worth saying
    /// out loud before the user loses ⌘Q.
    private static let reserved: [(keyCode: UInt16, modifiers: CGEventFlags, name: String)] = [
        (KeyCode.q.rawValue, .maskCommand, "Quit"),
        (KeyCode.w.rawValue, .maskCommand, "Close Window"),
        (KeyCode.space.rawValue, .maskCommand, "Spotlight"),
        (KeyCode.tab.rawValue, .maskCommand, "App Switcher"),
        (KeyCode.h.rawValue, .maskCommand, "Hide"),
        (KeyCode.m.rawValue, .maskCommand, "Minimise"),
        (KeyCode.c.rawValue, .maskCommand, "Copy"),
        (KeyCode.v.rawValue, .maskCommand, "Paste"),
        (KeyCode.x.rawValue, .maskCommand, "Cut"),
        (KeyCode.z.rawValue, .maskCommand, "Undo"),
        (KeyCode.a.rawValue, .maskCommand, "Select All"),
        (KeyCode.s.rawValue, .maskCommand, "Save"),
    ]

    /// Check a shortcut before it is stored. The one hard rule is that an
    /// ordinary key needs a modifier — the tap swallows whatever it matches, so
    /// a bare letter would be unusable everywhere.
    static func validate(keyCode: UInt16, modifiers: CGEventFlags) -> Validity {
        let mods = modifiers.intersection(recognizedModifiers)

        if mods.isEmpty {
            guard functionKeys.contains(keyCode) else {
                return .reject("Add ⌃, ⌥, ⇧ or ⌘ — a plain key would be swallowed everywhere.")
            }
            return .ok
        }

        if mods == .maskShift, !functionKeys.contains(keyCode) {
            return .reject("Shift alone isn't enough — add ⌃, ⌥ or ⌘.")
        }

        if let hit = reserved.first(where: { $0.keyCode == keyCode && mods == $0.modifiers }) {
            return .warn("This is the system's \(hit.name) shortcut — NovaKey will take it over.")
        }

        return .ok
    }

    /// Human-readable description of a hotkey. `modifierOnly` shortcuts (⌃⇧ and
    /// friends) are named by their modifiers alone.
    static func describe(keyCode: UInt16, modifiers: CGEventFlags, modifierOnly: Bool = false) -> String {
        var parts: [String] = []

        if modifiers.contains(.maskControl) { parts.append("Ctrl") }
        if modifiers.contains(.maskAlternate) { parts.append("Option") }
        if modifiers.contains(.maskShift) { parts.append("Shift") }
        if modifiers.contains(.maskCommand) { parts.append("Cmd") }

        if !modifierOnly { parts.append(keyLabel(keyCode)) }

        return parts.joined(separator: "+")
    }

    /// Get the current hotkey description from settings.
    static var currentDescription: String {
        let settings = AppSettings.shared
        return describe(
            keyCode: settings.toggleHotkeyKeyCode,
            modifiers: CGEventFlags(rawValue: settings.toggleHotkeyModifiers),
            modifierOnly: settings.toggleHotkeyModifierOnly
        )
    }

    /// How many of the four recognized modifiers a flag set carries.
    static func modifierCount(_ modifiers: CGEventFlags) -> Int {
        [CGEventFlags.maskControl, .maskAlternate, .maskShift, .maskCommand]
            .filter { modifiers.contains($0) }
            .count
    }

    /// Check a modifier-only shortcut. Two modifiers minimum: a single one is
    /// held constantly during ordinary typing, so it would fire nonstop.
    static func validateModifierOnly(_ modifiers: CGEventFlags) -> Validity {
        let mods = modifiers.intersection(recognizedModifiers)
        guard modifierCount(mods) >= 2 else {
            return .reject("Hold at least two modifiers, e.g. ⌃⇧.")
        }
        return .ok
    }

    /// Symbol chips for a hotkey, e.g. ["⌥", "Z"]. Suitable for the recorder UI.
    static func symbols(keyCode: UInt16, modifiers: CGEventFlags, modifierOnly: Bool = false) -> [String] {
        var parts: [String] = []
        if modifiers.contains(.maskControl) { parts.append("⌃") }
        if modifiers.contains(.maskAlternate) { parts.append("⌥") }
        if modifiers.contains(.maskShift) { parts.append("⇧") }
        if modifiers.contains(.maskCommand) { parts.append("⌘") }
        if !modifierOnly { parts.append(keyLabel(keyCode)) }
        return parts
    }

    /// Current hotkey rendered as symbol chips.
    static var currentSymbols: [String] {
        let settings = AppSettings.shared
        return symbols(
            keyCode: settings.toggleHotkeyKeyCode,
            modifiers: CGEventFlags(rawValue: settings.toggleHotkeyModifiers),
            modifierOnly: settings.toggleHotkeyModifierOnly
        )
    }

    /// Detects modifier-only shortcuts (⌃⇧ and friends) from the modifier
    /// stream.
    ///
    /// The combination fires on *release*, and only if nothing else was pressed
    /// while the modifiers were held — so ⌃⇧ toggles the language while ⌃⇧S
    /// stays a normal shortcut for whatever app has focus. Pure state machine:
    /// no event tap, so it can be unit-tested.
    struct ComboWatcher {
        private var combo: CGEventFlags = []
        private var held: CGEventFlags = []
        private var armed = false

        init() {}

        /// Watch for `mods` (empty to disable). Resets any in-flight press.
        mutating func setCombo(_ mods: CGEventFlags) {
            combo = mods.intersection(recognizedModifiers)
            held = []
            armed = false
        }

        /// Feed the flags carried by a flagsChanged event. Returns true when the
        /// combination just completed.
        mutating func flagsChanged(to flags: CGEventFlags) -> Bool {
            let now = flags.intersection(recognizedModifiers)
            defer { held = now }

            if now == held { return false }

            // A modifier went down.
            if now.isSuperset(of: held) {
                if held.isEmpty { armed = true }
                return false
            }

            // Release edge: fire if exactly the wanted set was held, untouched.
            let fires = armed && !combo.isEmpty && held == combo
            if fires || now.isEmpty { armed = false }
            return fires
        }

        /// A real keystroke while the modifiers are held cancels the
        /// combination — it was a shortcut, not a toggle.
        mutating func keyPressed() {
            armed = false
        }
    }

    /// Human-readable label for the main (non-modifier) key of a hotkey.
    static func keyLabel(_ keyCode: UInt16) -> String {
        if let key = KeyCode(rawValue: keyCode), let letter = key.asciiLetter {
            return String(letter).uppercased()
        }
        if let fn = functionKeyCodes.firstIndex(of: keyCode) {
            return "F\(fn + 1)"
        }
        switch KeyCode(rawValue: keyCode) {
        case .zero: return "0"
        case .one: return "1"
        case .two: return "2"
        case .three: return "3"
        case .four: return "4"
        case .five: return "5"
        case .six: return "6"
        case .seven: return "7"
        case .eight: return "8"
        case .nine: return "9"
        case .space: return "Space"
        case .returnKey: return "↩"
        case .tab: return "⇥"
        case .escape: return "⎋"
        case .delete: return "⌫"
        case .grave: return "`"
        case .minus: return "-"
        case .equal: return "="
        case .leftBracket: return "["
        case .rightBracket: return "]"
        case .backslash: return "\\"
        case .semicolon: return ";"
        case .quote: return "'"
        case .comma: return ","
        case .period: return "."
        case .slash: return "/"
        case .leftArrow: return "←"
        case .rightArrow: return "→"
        case .upArrow: return "↑"
        case .downArrow: return "↓"
        default: return "Key(\(keyCode))"
        }
    }
}
