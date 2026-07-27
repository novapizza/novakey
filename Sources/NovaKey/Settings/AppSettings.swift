// AppSettings.swift
// Persists user preferences via UserDefaults.

import Cocoa

/// Observable settings model for the app.
final class AppSettings {

    static let shared = AppSettings()

    private let defaults = UserDefaults.standard

    // MARK: - Properties

    /// Whether Vietnamese input mode is active. Defaults to true.
    var isVietnameseMode: Bool {
        get { defaults.object(forKey: AppConstants.Defaults.isVietnameseMode) as? Bool ?? true }
        set { defaults.set(newValue, forKey: AppConstants.Defaults.isVietnameseMode) }
    }

    /// Whether to send an invisible character before backspaces to fix browser autocomplete.
    var fixBrowserAutocomplete: Bool {
        get { defaults.object(forKey: AppConstants.Defaults.fixBrowserAutocomplete) as? Bool ?? true }
        set { defaults.set(newValue, forKey: AppConstants.Defaults.fixBrowserAutocomplete) }
    }

    /// Whether to send each character as a separate CGEvent (for compatibility).
    var sendKeyStepByStep: Bool {
        get { defaults.bool(forKey: AppConstants.Defaults.sendKeyStepByStep) }
        set { defaults.set(newValue, forKey: AppConstants.Defaults.sendKeyStepByStep) }
    }

    /// Whether to play a subtle system sound when toggling V/E.
    var playSoundOnSwitch: Bool {
        get { defaults.bool(forKey: AppConstants.Defaults.playSoundOnSwitch) }
        set { defaults.set(newValue, forKey: AppConstants.Defaults.playSoundOnSwitch) }
    }

    /// Whether pressing the Fn (Globe) key toggles Vietnamese/English mode,
    /// keeping NovaKey in sync with the system language switch. Defaults to true.
    var switchWithFnKey: Bool {
        get { defaults.object(forKey: AppConstants.Defaults.switchWithFnKey) as? Bool ?? true }
        set { defaults.set(newValue, forKey: AppConstants.Defaults.switchWithFnKey) }
    }

    /// Whether "Quick Vietnamese" is enabled: a lone `w` right after an initial
    /// consonant becomes `ư` (e.g. "tw" -> "tư"), plus a real-time revert of
    /// invalid mid-word transformations. Off by default.
    var quickVietnamese: Bool {
        get { defaults.bool(forKey: AppConstants.Defaults.quickVietnamese) }
        set { defaults.set(newValue, forKey: AppConstants.Defaults.quickVietnamese) }
    }

    /// Whether "Deferred diacritics" (Bỏ dấu sau) is enabled: a modifier key
    /// typed later in the word applies backward ("did" -> "đi", "thana" ->
    /// "thân"). Sub-option of Quick Vietnamese -- inert unless it is also on.
    /// Off by default.
    var deferredDiacritics: Bool {
        get { defaults.bool(forKey: AppConstants.Defaults.deferredDiacritics) }
        set { defaults.set(newValue, forKey: AppConstants.Defaults.deferredDiacritics) }
    }

    /// Hotkey keycode for toggling Vietnamese/English.
    ///
    /// Presence, not value, decides whether a keycode was ever recorded: the
    /// ANSI virtual keycode for `A` is 0x00, so treating 0 as "unset" would make
    /// ⌥A silently fall back to the default.
    var toggleHotkeyKeyCode: UInt16 {
        get {
            guard let val = defaults.object(forKey: AppConstants.Defaults.toggleHotkeyKeyCode) as? Int,
                  (0...0xFFFF).contains(val)
            else { return HotkeyManager.defaultKeyCode }
            return UInt16(val)
        }
        set { defaults.set(Int(newValue), forKey: AppConstants.Defaults.toggleHotkeyKeyCode) }
    }

    /// Hotkey modifier flags for toggling. A stored combination that would be
    /// unusable — a bare key, or a single modifier for a modifier-only shortcut
    /// — falls back to the default instead of being bound.
    var toggleHotkeyModifiers: UInt64 {
        get {
            guard let val = defaults.object(forKey: AppConstants.Defaults.toggleHotkeyModifiers) as? UInt64
            else { return HotkeyManager.defaultModifiers.rawValue }

            let flags = CGEventFlags(rawValue: val)
            let verdict = toggleHotkeyModifierOnly
                ? HotkeyManager.validateModifierOnly(flags)
                : HotkeyManager.validate(keyCode: toggleHotkeyKeyCode, modifiers: flags)
            return verdict.isReject ? HotkeyManager.defaultModifiers.rawValue : val
        }
        set { defaults.set(newValue, forKey: AppConstants.Defaults.toggleHotkeyModifiers) }
    }

    /// Whether the shortcut is modifier-only (⌃⇧ and friends): the main key is
    /// ignored and the toggle fires when the modifiers are released together.
    var toggleHotkeyModifierOnly: Bool {
        get { defaults.bool(forKey: AppConstants.Defaults.toggleHotkeyModifierOnly) }
        set { defaults.set(newValue, forKey: AppConstants.Defaults.toggleHotkeyModifierOnly) }
    }

    /// Restore the language-toggle shortcut to ⌥Z.
    func resetToggleHotkey() {
        toggleHotkeyKeyCode = HotkeyManager.defaultKeyCode
        toggleHotkeyModifiers = HotkeyManager.defaultModifiers.rawValue
        toggleHotkeyModifierOnly = false
    }

    // MARK: - Init

    private init() {
        // Register defaults
        defaults.register(defaults: [
            AppConstants.Defaults.isVietnameseMode: true,
            AppConstants.Defaults.fixBrowserAutocomplete: true,
            AppConstants.Defaults.sendKeyStepByStep: true,
            AppConstants.Defaults.switchWithFnKey: true,
        ])
    }
}
