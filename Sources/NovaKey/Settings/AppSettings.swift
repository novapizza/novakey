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
    var toggleHotkeyKeyCode: UInt16 {
        get {
            let val = defaults.integer(forKey: AppConstants.Defaults.toggleHotkeyKeyCode)
            return val == 0 ? KeyCode.z.rawValue : UInt16(val)
        }
        set { defaults.set(Int(newValue), forKey: AppConstants.Defaults.toggleHotkeyKeyCode) }
    }

    /// Hotkey modifier flags for toggling.
    var toggleHotkeyModifiers: UInt64 {
        get {
            let val = defaults.object(forKey: AppConstants.Defaults.toggleHotkeyModifiers) as? UInt64
            return val ?? CGEventFlags.maskAlternate.rawValue
        }
        set { defaults.set(newValue, forKey: AppConstants.Defaults.toggleHotkeyModifiers) }
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
