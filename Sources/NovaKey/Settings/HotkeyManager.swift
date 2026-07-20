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

    /// Human-readable description of a hotkey.
    static func describe(keyCode: UInt16, modifiers: CGEventFlags) -> String {
        var parts: [String] = []

        if modifiers.contains(.maskControl) { parts.append("Ctrl") }
        if modifiers.contains(.maskAlternate) { parts.append("Option") }
        if modifiers.contains(.maskShift) { parts.append("Shift") }
        if modifiers.contains(.maskCommand) { parts.append("Cmd") }

        if let key = KeyCode(rawValue: keyCode), let letter = key.asciiLetter {
            parts.append(String(letter).uppercased())
        } else {
            parts.append("Key(\(keyCode))")
        }

        return parts.joined(separator: "+")
    }

    /// Get the current hotkey description from settings.
    static var currentDescription: String {
        let settings = AppSettings.shared
        return describe(
            keyCode: settings.toggleHotkeyKeyCode,
            modifiers: CGEventFlags(rawValue: settings.toggleHotkeyModifiers)
        )
    }

    /// Symbol chips for a hotkey, e.g. ["⌥", "Z"]. Suitable for the recorder UI.
    static func symbols(keyCode: UInt16, modifiers: CGEventFlags) -> [String] {
        var parts: [String] = []
        if modifiers.contains(.maskControl) { parts.append("⌃") }
        if modifiers.contains(.maskAlternate) { parts.append("⌥") }
        if modifiers.contains(.maskShift) { parts.append("⇧") }
        if modifiers.contains(.maskCommand) { parts.append("⌘") }
        parts.append(keyLabel(keyCode))
        return parts
    }

    /// Current hotkey rendered as symbol chips.
    static var currentSymbols: [String] {
        let settings = AppSettings.shared
        return symbols(
            keyCode: settings.toggleHotkeyKeyCode,
            modifiers: CGEventFlags(rawValue: settings.toggleHotkeyModifiers)
        )
    }

    /// Human-readable label for the main (non-modifier) key of a hotkey.
    static func keyLabel(_ keyCode: UInt16) -> String {
        if let key = KeyCode(rawValue: keyCode), let letter = key.asciiLetter {
            return String(letter).uppercased()
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
