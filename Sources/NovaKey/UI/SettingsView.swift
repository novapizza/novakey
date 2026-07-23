// SettingsView.swift
// Dark, card-based SwiftUI preferences window for NovaKey.

import ServiceManagement
import SwiftUI

// MARK: - Design tokens

enum NovaTheme {
    static let windowBG = Color(red: 0.11, green: 0.11, blue: 0.12)
    static let cardBG = Color(red: 0.16, green: 0.16, blue: 0.18)
    static let cardStroke = Color.white.opacity(0.06)
    static let sectionLabel = Color(red: 1.0, green: 0.72, blue: 0.27)
    static let textPrimary = Color.white
    static let textSecondary = Color.white.opacity(0.55)
    static let pillOffBG = Color.white.opacity(0.08)

    static let brandGradient = LinearGradient(
        colors: [
            Color(red: 0.90, green: 0.15, blue: 0.20),
            Color(red: 0.98, green: 0.43, blue: 0.16),
            Color(red: 1.0, green: 0.78, blue: 0.28),
        ],
        startPoint: .leading, endPoint: .trailing
    )
}

// MARK: - Main view

struct SettingsView: View {
    @State private var isVietnamese: Bool = AppSettings.shared.isVietnameseMode
    @State private var launchAtLogin: Bool = (SMAppService.mainApp.status == .enabled)
    @State private var playSound: Bool = AppSettings.shared.playSoundOnSwitch
    @State private var fixBrowserAutocomplete: Bool = AppSettings.shared.fixBrowserAutocomplete
    @State private var sendKeyStepByStep: Bool = AppSettings.shared.sendKeyStepByStep
    @State private var switchWithFnKey: Bool = AppSettings.shared.switchWithFnKey
    @State private var quickVietnamese: Bool = AppSettings.shared.quickVietnamese
    @State private var deferredDiacritics: Bool = AppSettings.shared.deferredDiacritics

    /// Callback to toggle Vietnamese mode in the engine (wired from AppDelegate).
    var onToggleMode: ((Bool) -> Void)? = nil

    var body: some View {
        ZStack {
            NovaTheme.windowBG.ignoresSafeArea()

            VStack(spacing: 14) {
                Text("NovaKey Settings")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(NovaTheme.textPrimary)
                    .frame(maxWidth: .infinity)
                    .padding(.top, 6)
                inputMethodCard
                generalCard
                compatibilityCard
                aboutCard
            }
            .padding(.horizontal, 18)
            .padding(.bottom, 18)
            .padding(.top, 8)
        }
        .frame(width: 480)
        .onReceive(NotificationCenter.default.publisher(for: .novaKeyModeChanged)) { note in
            if let v = note.userInfo?["isVietnamese"] as? Bool {
                isVietnamese = v
            }
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
            Divider().background(Color.white.opacity(0.06))
            HStack {
                Text("Input Method")
                    .foregroundStyle(NovaTheme.textPrimary)
                Spacer()
                InputMethodMenu()
            }
            Divider().background(Color.white.opacity(0.06))
            VStack(alignment: .leading, spacing: 4) {
                row("Quick Vietnamese", trailing: {
                    NovaToggle(isOn: $quickVietnamese)
                        .onChange(of: quickVietnamese) { _, v in
                            AppSettings.shared.quickVietnamese = v
                            NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)
                        }
                })
                Text("Type faster: a lone “w” after a consonant becomes “ư” (e.g. tw → tư). Mixed English words like “huawei” stay literal.")
                    .font(.caption)
                    .foregroundStyle(NovaTheme.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)

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
                    Text("Add marks after the word: “did” → “đi”, “thana” → “thân”. Some English words that look Vietnamese may transform; press the key again to escape.")
                        .font(.caption)
                        .foregroundStyle(NovaTheme.textSecondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.leading, 16)
                .padding(.top, 6)
                .disabled(!quickVietnamese)
                .opacity(quickVietnamese ? 1 : 0.4)
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
                        .foregroundStyle(NovaTheme.textPrimary)
                    Text("Vietnamese Input Method for macOS")
                        .font(.caption)
                        .foregroundStyle(NovaTheme.textSecondary)
                }
                Spacer()
            }
        }
    }

    // MARK: Helpers

    @ViewBuilder
    private func row<Trailing: View>(_ title: String, @ViewBuilder trailing: () -> Trailing) -> some View {
        HStack {
            Text(title).foregroundStyle(NovaTheme.textPrimary)
            Spacer()
            trailing()
        }
    }

    private func setMode(_ vietnamese: Bool) {
        guard vietnamese != isVietnamese else { return }
        isVietnamese = vietnamese
        onToggleMode?(vietnamese)
    }
}

// MARK: - Card container

struct NovaCard<Content: View>: View {
    let title: String
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .tracking(1.2)
                .foregroundStyle(NovaTheme.sectionLabel)
            content()
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .fill(NovaTheme.cardBG)
                .overlay(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .stroke(NovaTheme.cardStroke, lineWidth: 1)
                )
        )
    }
}

// MARK: - Language pill (V / E selector)

struct LanguagePill: View {
    let label: String
    let letter: String
    let isActive: Bool

    var body: some View {
        HStack(spacing: 8) {
            Text(letter)
                .font(.system(size: 14, weight: .heavy, design: .rounded))
                .foregroundStyle(isActive ? .white : NovaTheme.textSecondary)
            Text(label)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(isActive ? .white : NovaTheme.textSecondary)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 9)
        .background(
            Group {
                if isActive {
                    Capsule().fill(NovaTheme.brandGradient)
                        .shadow(color: Color.red.opacity(0.45), radius: 10, y: 2)
                } else {
                    Capsule().fill(NovaTheme.pillOffBG)
                }
            }
        )
        .contentShape(Capsule())
    }
}

// MARK: - Small gradient V/E badge (for popover header + status bar image)

struct GradientLetterBadge: View {
    let letter: String
    var size: CGFloat = 22
    var body: some View {
        Text(letter)
            .font(.system(size: size * 0.58, weight: .heavy, design: .rounded))
            .foregroundStyle(.white)
            .frame(width: size, height: size)
            .background(Circle().fill(NovaTheme.brandGradient))
            .shadow(color: Color.red.opacity(0.35), radius: 4, y: 1)
    }
}

// MARK: - Hotkey recorder (click to change the toggle shortcut)

/// Shows the current toggle hotkey as key chips. Click to record a new one:
/// press a modifier + key combination, or Escape to cancel.
struct HotkeyRecorder: View {
    @State private var symbols: [String] = HotkeyManager.currentSymbols
    @State private var isRecording = false
    @State private var monitor: Any?

    var body: some View {
        Button(action: toggleRecording) {
            Group {
                if isRecording {
                    Text("Press keys…")
                        .font(.system(size: 12, weight: .semibold, design: .rounded))
                        .foregroundStyle(NovaTheme.sectionLabel)
                        .padding(.horizontal, 10)
                        .frame(height: 26)
                        .background(
                            RoundedRectangle(cornerRadius: 7, style: .continuous)
                                .fill(Color.white.opacity(0.09))
                                .overlay(
                                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                                        .stroke(NovaTheme.sectionLabel.opacity(0.7), lineWidth: 1)
                                )
                        )
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
        .onDisappear(perform: stop)
    }

    private func badgeKey(_ s: String) -> some View {
        Text(s)
            .font(.system(size: 12, weight: .semibold, design: .rounded))
            .frame(minWidth: 26, minHeight: 26)
            .padding(.horizontal, s.count > 1 ? 6 : 0)
            .background(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(Color.white.opacity(0.09))
            )
            .foregroundStyle(NovaTheme.textPrimary)
    }

    private func toggleRecording() {
        isRecording ? stop() : start()
    }

    private func start() {
        isRecording = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { event in
            // Escape cancels recording without changing the shortcut.
            if event.keyCode == KeyCode.escape.rawValue {
                stop()
                return nil
            }

            let mods = event.modifierFlags.intersection([.command, .option, .control, .shift])
            // Require at least one modifier so we don't capture plain typing.
            guard !mods.isEmpty else { return nil }

            let cgFlags = Self.cgFlags(from: mods)
            AppSettings.shared.toggleHotkeyKeyCode = event.keyCode
            AppSettings.shared.toggleHotkeyModifiers = cgFlags.rawValue
            NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)

            symbols = HotkeyManager.symbols(keyCode: event.keyCode, modifiers: cgFlags)
            stop()
            return nil  // swallow the event so it isn't typed anywhere
        }
    }

    private func stop() {
        isRecording = false
        if let m = monitor {
            NSEvent.removeMonitor(m)
            monitor = nil
        }
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
    var body: some View {
        Menu {
            Button("Telex") {}
        } label: {
            HStack(spacing: 4) {
                Text("Telex")
                    .foregroundStyle(NovaTheme.textPrimary)
                Image(systemName: "chevron.down")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(NovaTheme.textSecondary)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(Color.white.opacity(0.08))
            )
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
    }
}

// MARK: - Gradient toggle

struct NovaToggle: View {
    @Binding var isOn: Bool

    var body: some View {
        ZStack(alignment: isOn ? .trailing : .leading) {
            Capsule()
                .fill(isOn ? AnyShapeStyle(NovaTheme.brandGradient) : AnyShapeStyle(Color.white.opacity(0.12)))
                .frame(width: 44, height: 24)
            Circle()
                .fill(Color.white)
                .frame(width: 18, height: 18)
                .padding(3)
                .shadow(color: .black.opacity(0.25), radius: 1.5, y: 1)
        }
        .onTapGesture {
            withAnimation(.spring(response: 0.22, dampingFraction: 0.8)) { isOn.toggle() }
        }
        .contentShape(Capsule())
    }
}

// MARK: - App logo

/// Renders the AppLogo.png from the app bundle. Falls back to a gradient tile.
struct AppLogoView: View {
    let size: CGFloat
    var cornerRadius: CGFloat = 8

    var body: some View {
        Group {
            if let ns = NSImage(named: "AppLogo") {
                Image(nsImage: ns)
                    .resizable()
                    .interpolation(.high)
            } else {
                NovaTheme.brandGradient
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
