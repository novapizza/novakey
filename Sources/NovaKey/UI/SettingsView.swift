// SettingsView.swift
// Card-based SwiftUI preferences window for NovaKey, in the theme chosen
// under Appearance (see NovaTheme.swift).

import ServiceManagement
import SwiftUI

// MARK: - Main view

struct SettingsView: View {
    @Environment(\.novaTheme) private var theme
    @State private var isVietnamese: Bool = AppSettings.shared.isVietnameseMode
    @State private var launchAtLogin: Bool = (SMAppService.mainApp.status == .enabled)
    @State private var playSound: Bool = AppSettings.shared.playSoundOnSwitch
    @State private var fixBrowserAutocomplete: Bool = AppSettings.shared.fixBrowserAutocomplete
    @State private var sendKeyStepByStep: Bool = AppSettings.shared.sendKeyStepByStep
    @State private var switchWithFnKey: Bool = AppSettings.shared.switchWithFnKey
    @State private var quickVietnamese: Bool = AppSettings.shared.quickVietnamese
    @State private var deferredDiacritics: Bool = AppSettings.shared.deferredDiacritics
    @State private var abbreviationsEnabled: Bool = AppSettings.shared.abbreviationsEnabled
    @State private var abbreviationCount: Int = AbbreviationStore.shared.table.count
    @State private var settingsTheme: SettingsTheme = AppSettings.shared.settingsTheme

    /// Callback to toggle Vietnamese mode in the engine (wired from AppDelegate).
    var onToggleMode: ((Bool) -> Void)? = nil

    var body: some View {
        ZStack {
            theme.windowBG.ignoresSafeArea()

            ScrollView(.vertical) {
                VStack(spacing: 14) {
                    NovaHeader(title: "NovaKey Settings")
                    inputMethodCard
                    abbreviationsCard
                    AppRulesCard()
                    generalCard
                    appearanceCard
                    compatibilityCard
                    aboutCard
                }
                .padding(.horizontal, 18)
                .padding(.bottom, 18)
                .padding(.top, 8)
            }
        }
        .frame(width: 480)
        .onReceive(NotificationCenter.default.publisher(for: .novaKeyModeChanged)) { note in
            if let v = note.userInfo?["isVietnamese"] as? Bool {
                isVietnamese = v
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .novaKeyAbbreviationsChanged)) { _ in
            abbreviationCount = AbbreviationStore.shared.table.count
        }
    }

    // MARK: Sections

    private var inputMethodCard: some View {
        NovaCard(title: "INPUT METHOD") {
            HStack(spacing: 10) {
                LanguagePill(label: "Tiếng Việt", letter: "V", isActive: isVietnamese)
                    .onTapGesture { setMode(true) }
                LanguagePill(label: "English", letter: "E", isActive: !isVietnamese)
                    .onTapGesture { setMode(false) }
                Spacer()
                HotkeyRecorder()
            }
            Divider().background(theme.divider)
            HStack {
                Text("Input Method")
                    .foregroundStyle(theme.textPrimary)
                Spacer()
                InputMethodMenu()
            }
            Divider().background(theme.divider)
            VStack(alignment: .leading, spacing: 4) {
                row("Quick Vietnamese", trailing: {
                    NovaToggle(isOn: $quickVietnamese)
                        .onChange(of: quickVietnamese) { _, v in
                            AppSettings.shared.quickVietnamese = v
                            NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)
                        }
                })
                caption("Type faster: a lone “w” after a consonant becomes “ư” (e.g. tw → tư). Mixed English words like “huawei” stay literal.")

                // Sub-option of Quick Vietnamese: indented, and greyed out /
                // disabled while Quick Vietnamese is off.
                VStack(alignment: .leading, spacing: 4) {
                    row("Deferred diacritics (Bỏ dấu sau)", trailing: {
                        NovaToggle(isOn: $deferredDiacritics)
                            .onChange(of: deferredDiacritics) { _, v in
                                AppSettings.shared.deferredDiacritics = v
                                NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)
                            }
                    })
                    caption("Add marks after the word: “did” → “đi”, “thana” → “thân”. Some English words that look Vietnamese may transform; press the key again to escape.")
                }
                .padding(.leading, 16)
                .padding(.top, 6)
                .disabled(!quickVietnamese)
                .opacity(quickVietnamese ? 1 : 0.4)
            }
        }
    }

    private var abbreviationsCard: some View {
        NovaCard(title: "ABBREVIATIONS") {
            VStack(alignment: .leading, spacing: 4) {
                row("Expand abbreviations", trailing: {
                    NovaToggle(isOn: $abbreviationsEnabled)
                        .onChange(of: abbreviationsEnabled) { _, v in
                            AppSettings.shared.abbreviationsEnabled = v
                            NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)
                        }
                })
                caption("Type a short form, then Tab (btw → by the way). Works in both Vietnamese and English mode.")
            }
            HStack(spacing: 12) {
                Button("Edit List…") {
                    NotificationCenter.default.post(name: .novaKeyEditAbbreviationsRequested, object: nil)
                }
                .buttonStyle(NovaButtonStyle())
                Text(abbreviationCount == 0 ? "No abbreviations yet"
                     : abbreviationCount == 1 ? "1 abbreviation"
                     : "\(abbreviationCount) abbreviations")
                    .font(.caption)
                    .foregroundStyle(theme.textSecondary)
                Spacer()
            }
        }
    }

    private var generalCard: some View {
        NovaCard(title: "GENERAL") {
            row("Launch at login", trailing: {
                NovaToggle(isOn: $launchAtLogin)
                    .onChange(of: launchAtLogin) { _, v in
                        do {
                            if v { try SMAppService.mainApp.register() }
                            else { try SMAppService.mainApp.unregister() }
                            // An explicit choice: never re-register on launch.
                            UserDefaults.standard.set(true, forKey: AppConstants.Defaults.launchAtLoginConfigured)
                        } catch {
                            launchAtLogin = !v
                        }
                    }
            })
            row("Play sound on switch", trailing: {
                NovaToggle(isOn: $playSound)
                    .onChange(of: playSound) { _, v in
                        AppSettings.shared.playSoundOnSwitch = v
                    }
            })
            row("Switch with Fn (Globe) key", trailing: {
                NovaToggle(isOn: $switchWithFnKey)
                    .onChange(of: switchWithFnKey) { _, v in
                        AppSettings.shared.switchWithFnKey = v
                        NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)
                    }
            })
        }
    }

    private var appearanceCard: some View {
        NovaCard(title: "APPEARANCE") {
            VStack(alignment: .leading, spacing: 6) {
                row("Theme", trailing: {
                    NovaSegmented(
                        options: SettingsTheme.allCases.map { (label: $0.title, value: $0) },
                        selection: $settingsTheme
                    )
                    .onChange(of: settingsTheme) { _, v in
                        AppSettings.shared.settingsTheme = v
                        NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)
                    }
                })
                caption(settingsTheme == .platinum
                        ? "The gray, bevelled look of classic Mac OS for NovaKey's windows and menu."
                        : "The dark look for NovaKey's windows and menu.")
            }
        }
    }

    private var compatibilityCard: some View {
        NovaCard(title: "COMPATIBILITY") {
            row("Fix browser autocomplete", trailing: {
                NovaToggle(isOn: $fixBrowserAutocomplete)
                    .onChange(of: fixBrowserAutocomplete) { _, v in
                        AppSettings.shared.fixBrowserAutocomplete = v
                        NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)
                    }
            })
            row("Send keys step-by-step", trailing: {
                NovaToggle(isOn: $sendKeyStepByStep)
                    .onChange(of: sendKeyStepByStep) { _, v in
                        AppSettings.shared.sendKeyStepByStep = v
                        NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)
                    }
            })
        }
    }

    private var aboutCard: some View {
        NovaCard(title: "ABOUT") {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("NovaKey v\(AppConstants.version)")
                        .foregroundStyle(theme.textPrimary)
                    Text("Vietnamese Input Method for macOS")
                        .font(.caption)
                        .foregroundStyle(theme.textSecondary)
                }
                Spacer()
            }
        }
    }

    // MARK: Helpers

    @ViewBuilder
    private func row<Trailing: View>(_ title: String, @ViewBuilder trailing: () -> Trailing) -> some View {
        HStack {
            Text(title).foregroundStyle(theme.textPrimary)
            Spacer()
            trailing()
        }
    }

    private func caption(_ text: String) -> some View {
        Text(text)
            .font(.caption)
            .foregroundStyle(theme.textSecondary)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func setMode(_ vietnamese: Bool) {
        guard vietnamese != isVietnamese else { return }
        isVietnamese = vietnamese
        onToggleMode?(vietnamese)
    }
}

// MARK: - Language pill (V / E selector)

/// The V/E language button. A glowing gradient capsule in Modern; a
/// bevelled push button, lit in the accent colour, in Platinum.
struct LanguagePill: View {
    @Environment(\.novaTheme) private var theme
    let label: String
    let letter: String
    let isActive: Bool

    var body: some View {
        let textColor: Color = isActive ? .white : (theme.isRetro ? theme.textPrimary : theme.textSecondary)
        HStack(spacing: 8) {
            Text(letter)
                .font(.system(size: 14, weight: .heavy, design: .rounded))
                .foregroundStyle(textColor)
            Text(label)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(textColor)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, theme.isRetro ? 6 : 9)
        .background(
            Group {
                if theme.isRetro {
                    RoundedRectangle(cornerRadius: 6)
                        .fill(isActive ? theme.accent : theme.pillOffBG)
                        .overlay(
                            Bevel(light: isActive ? Color.white.opacity(0.55) : theme.bevelLight,
                                  shadow: isActive ? theme.bevelDark : theme.bevelShadow)
                                .clipShape(RoundedRectangle(cornerRadius: 6))
                        )
                        .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.black, lineWidth: 1))
                } else if isActive {
                    Capsule().fill(theme.accentGradient)
                        .shadow(color: theme.accentGlow, radius: 10, y: 2)
                } else {
                    Capsule().fill(theme.pillOffBG)
                }
            }
        )
        .contentShape(Rectangle())
    }
}

// MARK: - Small V/E badge (for popover header + status bar image)

struct GradientLetterBadge: View {
    @Environment(\.novaTheme) private var theme
    let letter: String
    var size: CGFloat = 22
    var body: some View {
        Text(letter)
            .font(.system(size: size * 0.58, weight: .heavy, design: .rounded))
            .foregroundStyle(.white)
            .frame(width: size, height: size)
            .background(Circle().fill(theme.accentGradient))
            .shadow(color: theme.accentGlow.opacity(0.8), radius: 4, y: 1)
    }
}

// MARK: - Hotkey recorder (click to change the toggle shortcut)

/// Shows the current toggle hotkey as key chips. Click to record a new one:
/// press a modifier + key combination, or Escape to cancel.
struct HotkeyRecorder: View {
    @Environment(\.novaTheme) private var theme
    @State private var symbols: [String] = HotkeyManager.currentSymbols
    @State private var isRecording = false
    @State private var monitor: Any?
    /// Validation feedback for the last attempt: the message and whether it
    /// blocked the change.
    @State private var notice: (text: String, isError: Bool)?
    /// Modifiers held during recording, so a release can be recognised.
    @State private var heldModifiers: CGEventFlags = []

    private var isDefault: Bool {
        AppSettings.shared.toggleHotkeyKeyCode == HotkeyManager.defaultKeyCode
            && AppSettings.shared.toggleHotkeyModifiers == HotkeyManager.defaultModifiers.rawValue
            && !AppSettings.shared.toggleHotkeyModifierOnly
    }

    var body: some View {
        VStack(alignment: .trailing, spacing: 4) {
            HStack(spacing: 6) {
                Button(action: toggleRecording) {
                    Group {
                        if isRecording {
                            Text("Press keys…")
                                .font(.system(size: 12, weight: .semibold, design: .rounded))
                                .foregroundStyle(theme.isRetro ? theme.accent : theme.sectionLabel)
                                .padding(.horizontal, 10)
                                .frame(height: 26)
                                .background(chip(highlighted: true))
                        } else {
                            HStack(spacing: 4) {
                                ForEach(Array(symbols.enumerated()), id: \.offset) { _, s in
                                    badgeKey(s)
                                }
                            }
                        }
                    }
                }
                .buttonStyle(.plain)
                .help("Click to change the language toggle shortcut")

                // Only offered once the shortcut has moved off the default —
                // the escape hatch from a combination that can't be pressed.
                if !isDefault {
                    Button(action: restoreDefault) {
                        Image(systemName: "arrow.uturn.backward")
                            .font(.system(size: 11, weight: .semibold))
                            .frame(width: 26, height: 26)
                            .background(chip(highlighted: false))
                            .foregroundStyle(theme.textSecondary)
                    }
                    .buttonStyle(.plain)
                    .help("Restore the default shortcut (⌥Z)")
                }
            }

            if let notice {
                Text(notice.text)
                    .font(.caption)
                    .foregroundStyle(notice.isError ? theme.danger : theme.sectionLabel)
                    .multilineTextAlignment(.trailing)
                    .fixedSize(horizontal: false, vertical: true)
            } else if isRecording {
                Text("Press a combination, or just ⌃⇧. Esc to cancel.")
                    .font(.caption)
                    .foregroundStyle(theme.textSecondary)
            }
        }
        .onDisappear(perform: stop)
    }

    /// A key-cap chip: soft in Modern, a sunken white field in Platinum.
    private func chip(highlighted: Bool) -> some View {
        RoundedRectangle(cornerRadius: theme.controlRadius, style: .continuous)
            .fill(theme.chipBG)
            .overlay(
                RoundedRectangle(cornerRadius: theme.controlRadius, style: .continuous)
                    .stroke(theme.isRetro ? theme.bevelShadow
                            : (highlighted ? theme.sectionLabel.opacity(0.7) : Color.clear), lineWidth: 1)
            )
    }

    private func badgeKey(_ s: String) -> some View {
        Text(s)
            .font(.system(size: 12, weight: .semibold, design: .rounded))
            .frame(minWidth: 26, minHeight: 26)
            .padding(.horizontal, s.count > 1 ? 6 : 0)
            .background(chip(highlighted: false))
            .foregroundStyle(theme.textPrimary)
    }

    private func toggleRecording() {
        isRecording ? stop() : start()
    }

    private func start() {
        isRecording = true
        notice = nil
        heldModifiers = []
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged]) { event in
            // Letting go of two or more modifiers with nothing else pressed
            // records a modifier-only shortcut (⌃⇧ and friends).
            if event.type == .flagsChanged {
                return handleFlagsChanged(event)
            }

            // Escape cancels recording without changing the shortcut.
            if event.keyCode == KeyCode.escape.rawValue {
                stop()
                return nil
            }

            let mods = event.modifierFlags.intersection([.command, .option, .control, .shift])
            let cgFlags = Self.cgFlags(from: mods)

            // Refuse combinations that would be unusable once global; stay in
            // recording mode so the user can simply try another one.
            switch HotkeyManager.validate(keyCode: event.keyCode, modifiers: cgFlags) {
            case .reject(let why):
                notice = (why, true)
                return nil
            case .warn(let why):
                notice = (why, false)
            case .ok:
                notice = nil
            }

            AppSettings.shared.toggleHotkeyKeyCode = event.keyCode
            AppSettings.shared.toggleHotkeyModifiers = cgFlags.rawValue
            AppSettings.shared.toggleHotkeyModifierOnly = false
            NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)

            symbols = HotkeyManager.symbols(keyCode: event.keyCode, modifiers: cgFlags)
            stop()
            return nil  // swallow the event so it isn't typed anywhere
        }
    }

    /// Track modifiers while recording; commit on the release edge.
    private func handleFlagsChanged(_ event: NSEvent) -> NSEvent? {
        let now = Self.cgFlags(
            from: event.modifierFlags.intersection([.command, .option, .control, .shift])
        )
        let previous = heldModifiers
        heldModifiers = now

        // Only a release interests us — pressing more modifiers just builds up.
        guard HotkeyManager.modifierCount(now) < HotkeyManager.modifierCount(previous) else {
            return event
        }
        // One modifier on its own is held constantly while typing; wait for the
        // user to add a second rather than rejecting outright.
        guard HotkeyManager.modifierCount(previous) >= 2 else { return event }

        switch HotkeyManager.validateModifierOnly(previous) {
        case .reject(let why):
            notice = (why, true)
            return event
        case .warn(let why):
            notice = (why, false)
        case .ok:
            notice = nil
        }

        AppSettings.shared.toggleHotkeyModifiers = previous.rawValue
        AppSettings.shared.toggleHotkeyModifierOnly = true
        NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)

        symbols = HotkeyManager.symbols(
            keyCode: AppSettings.shared.toggleHotkeyKeyCode,
            modifiers: previous,
            modifierOnly: true
        )
        stop()
        return event  // modifiers alone do nothing downstream; let them through
    }

    private func stop() {
        isRecording = false
        heldModifiers = []
        if let m = monitor {
            NSEvent.removeMonitor(m)
            monitor = nil
        }
    }

    private func restoreDefault() {
        stop()
        AppSettings.shared.resetToggleHotkey()
        NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)
        symbols = HotkeyManager.currentSymbols
        notice = nil
    }

    /// Map AppKit modifier flags to Quartz CGEventFlags for persistence.
    private static func cgFlags(from mods: NSEvent.ModifierFlags) -> CGEventFlags {
        var flags: CGEventFlags = []
        if mods.contains(.control) { flags.insert(.maskControl) }
        if mods.contains(.option) { flags.insert(.maskAlternate) }
        if mods.contains(.shift) { flags.insert(.maskShift) }
        if mods.contains(.command) { flags.insert(.maskCommand) }
        return flags
    }
}

// MARK: - Input method menu (Telex only for now)

struct InputMethodMenu: View {
    @Environment(\.novaTheme) private var theme
    var body: some View {
        Menu {
            Button("Telex") {}
        } label: {
            HStack(spacing: 4) {
                Text("Telex")
                    .foregroundStyle(theme.textPrimary)
                Image(systemName: "chevron.down")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(theme.textSecondary)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: theme.controlRadius + 1, style: .continuous)
                    .fill(theme.isRetro ? theme.fieldBG : theme.pillOffBG)
                    .overlay(
                        RoundedRectangle(cornerRadius: theme.controlRadius + 1, style: .continuous)
                            .stroke(theme.isRetro ? Color.black : Color.clear, lineWidth: 1)
                    )
            )
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
    }
}

// MARK: - App logo

/// Renders the AppLogo.png from the app bundle. Falls back to a gradient tile.
struct AppLogoView: View {
    @Environment(\.novaTheme) private var theme
    let size: CGFloat
    var cornerRadius: CGFloat = 8

    var body: some View {
        Group {
            if let ns = NSImage(named: "AppLogo") {
                Image(nsImage: ns)
                    .resizable()
                    .interpolation(.high)
            } else {
                theme.accentGradient
                    .overlay(
                        Image(systemName: "sparkle")
                            .font(.system(size: size * 0.5, weight: .bold))
                            .foregroundStyle(.white)
                    )
            }
        }
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
    }
}

// MARK: - Notifications

extension Notification.Name {
    static let novaKeySettingsChanged = Notification.Name("NovaKeySettingsChanged")
    static let novaKeyModeChanged = Notification.Name("NovaKeyModeChanged")
    static let novaKeyModeToggleRequested = Notification.Name("NovaKeyModeToggleRequested")
}
