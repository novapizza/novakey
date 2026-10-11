import Foundation

enum AppConstants {
    static let bundleIdentifier = "com.novakey.inputmethod"
    static let appName = "NovaKey"
    static let version = "0.2.7"

    // UserDefaults keys
    enum Defaults {
        static let isVietnameseMode = "NovaKey.isVietnameseMode"
        static let toggleHotkeyKeyCode = "NovaKey.toggleHotkeyKeyCode"
        static let toggleHotkeyModifiers = "NovaKey.toggleHotkeyModifiers"
        static let toggleHotkeyModifierOnly = "NovaKey.toggleHotkeyModifierOnly"
        static let fixBrowserAutocomplete = "NovaKey.fixBrowserAutocomplete"
        static let sendKeyStepByStep = "NovaKey.sendKeyStepByStep"
        static let launchAtLogin = "NovaKey.launchAtLogin"
        static let playSoundOnSwitch = "NovaKey.playSoundOnSwitch"
        static let switchWithFnKey = "NovaKey.switchWithFnKey"
        static let quickVietnamese = "NovaKey.quickVietnamese"
        static let deferredDiacritics = "NovaKey.deferredDiacritics"
    }
}
