// StatusBarController.swift
// Status bar icon + custom popover menu (SwiftUI).

import Cocoa
import SwiftUI

final class StatusBarController {

    private var statusItem: NSStatusItem?
    private var popover: NSPopover?
    private var eventMonitor: Any?
    private let engine: TelexEngine

    var onOpenSettings: (() -> Void)?
    var onQuit: (() -> Void)?

    init(engine: TelexEngine) {
        self.engine = engine
    }

    // MARK: Setup

    func setup() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        updateIcon(isVietnamese: engine.isVietnameseMode)

        if let button = statusItem?.button {
            button.action = #selector(handleClick(_:))
            button.target = self
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }

        let popover = NSPopover()
        popover.behavior = .transient
        popover.animates = true
        let rootView = MenuBarPopoverView(
            engine: engine,
            onToggleMode: { [weak self] in self?.toggleMode() },
            onOpenSettings: { [weak self] in
                self?.closePopover()
                self?.onOpenSettings?()
            },
            onQuit: { [weak self] in self?.onQuit?() }
        )
        popover.contentViewController = NSHostingController(rootView: rootView)
        popover.contentSize = NSSize(width: 280, height: 340)
        self.popover = popover
    }

    func updateIcon(isVietnamese: Bool) {
        guard let button = statusItem?.button else { return }
        button.title = ""
        button.image = StatusBarController.makeIcon(letter: isVietnamese ? "V" : "E",
                                                   colored: isVietnamese)
        button.image?.isTemplate = !isVietnamese // English = template (monochrome), VN = colored
        button.toolTip = isVietnamese ? "NovaKey: Vietnamese (Telex)" : "NovaKey: English"
    }

    /// Render a small gradient V or grey E as an NSImage for the status bar.
    private static func makeIcon(letter: String, colored: Bool) -> NSImage {
        let size = NSSize(width: 18, height: 18)
        let image = NSImage(size: size, flipped: false) { rect in
            let path = NSBezierPath(ovalIn: rect.insetBy(dx: 0.5, dy: 0.5))
            if colored {
                // Red→orange→yellow radial-ish gradient via NSGradient
                let gradient = NSGradient(colors: [
                    NSColor(red: 0.90, green: 0.15, blue: 0.20, alpha: 1),
                    NSColor(red: 0.98, green: 0.43, blue: 0.16, alpha: 1),
                    NSColor(red: 1.00, green: 0.78, blue: 0.28, alpha: 1),
                ])!
                gradient.draw(in: path, angle: 0)
            } else {
                NSColor.labelColor.withAlphaComponent(0.0).setFill() // transparent; template mode handles coloring
                path.fill()
            }
            let font = NSFont.systemFont(ofSize: 12, weight: .heavy)
            let color: NSColor = colored ? .white : .labelColor
            let attrs: [NSAttributedString.Key: Any] = [
                .font: font, .foregroundColor: color,
            ]
            let str = NSAttributedString(string: letter, attributes: attrs)
            let textSize = str.size()
            let pt = NSPoint(x: (rect.width - textSize.width) / 2,
                             y: (rect.height - textSize.height) / 2 - 0.5)
            str.draw(at: pt)
            return true
        }
        return image
    }

    // MARK: Actions

    @objc private func handleClick(_ sender: Any?) {
        guard let popover = popover, let button = statusItem?.button else { return }
        if popover.isShown {
            closePopover()
        } else {
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
            // Close when clicking elsewhere
            eventMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
                self?.closePopover()
            }
        }
    }

    private func closePopover() {
        popover?.performClose(nil)
        if let monitor = eventMonitor {
            NSEvent.removeMonitor(monitor)
            eventMonitor = nil
        }
    }

    private func toggleMode() {
        engine.isVietnameseMode.toggle()
        engine.resetSession()
        let v = engine.isVietnameseMode
        updateIcon(isVietnamese: v)
        AppSettings.shared.isVietnameseMode = v
        NotificationCenter.default.post(
            name: .novaKeyModeChanged, object: nil,
            userInfo: ["isVietnamese": v]
        )
        if AppSettings.shared.playSoundOnSwitch {
            NSSound(named: NSSound.Name("Tink"))?.play()
        }
    }
}

// MARK: - SwiftUI popover content

struct MenuBarPopoverView: View {
    let engine: TelexEngine
    let onToggleMode: () -> Void
    let onOpenSettings: () -> Void
    let onQuit: () -> Void

    @State private var isVietnamese: Bool = AppSettings.shared.isVietnameseMode
    @State private var hotkeyText: String = HotkeyManager.currentDescription
    @State private var fnEnabled: Bool = AppSettings.shared.switchWithFnKey

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            pillSection
            Divider().background(Color.white.opacity(0.08))
            menuRows
        }
        .background(Color(red: 0.13, green: 0.12, blue: 0.14))
        .frame(width: 280)
        .onReceive(NotificationCenter.default.publisher(for: .novaKeyModeChanged)) { note in
            if let v = note.userInfo?["isVietnamese"] as? Bool { isVietnamese = v }
        }
        .onReceive(NotificationCenter.default.publisher(for: .novaKeySettingsChanged)) { _ in
            hotkeyText = HotkeyManager.currentDescription
            fnEnabled = AppSettings.shared.switchWithFnKey
        }
    }

    private var header: some View {
        HStack(spacing: 10) {
            AppLogoView(size: 34, cornerRadius: 9)
            VStack(alignment: .leading, spacing: 2) {
                Text("NovaKey")
                    .font(.system(size: 15, weight: .bold))
                    .foregroundStyle(.white)
                HStack(spacing: 5) {
                    GradientLetterBadge(letter: isVietnamese ? "V" : "E", size: 14)
                    Text(isVietnamese ? "Tiếng Việt" : "English")
                        .font(.system(size: 11))
                        .foregroundStyle(.white.opacity(0.65))
                }
            }
            Spacer()
        }
        .padding(.horizontal, 14)
        .padding(.top, 14)
        .padding(.bottom, 10)
    }

    private var pillSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                LanguagePill(label: "Tiếng Việt", letter: "V", isActive: isVietnamese)
                    .onTapGesture { if !isVietnamese { onToggleMode() } }
                LanguagePill(label: "English", letter: "E", isActive: !isVietnamese)
                    .onTapGesture { if isVietnamese { onToggleMode() } }
            }
            HStack(spacing: 6) {
                Image(systemName: "keyboard")
                    .font(.system(size: 10, weight: .semibold))
                Text(fnEnabled ? "\(hotkeyText) or Fn to toggle" : "\(hotkeyText) to toggle")
                    .font(.system(size: 11))
            }
            .foregroundStyle(.white.opacity(0.45))
            .padding(.leading, 2)
        }
        .padding(.horizontal, 14)
        .padding(.bottom, 12)
    }

    private var menuRows: some View {
        VStack(spacing: 0) {
            MenuRow(title: "Input Method", trailing: AnyView(
                HStack(spacing: 4) {
                    Text("Telex").font(.system(size: 12)).foregroundStyle(.white.opacity(0.5))
                    Image(systemName: "chevron.right")
                        .font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(.white.opacity(0.4))
                }
            ), action: {})

            Divider().background(Color.white.opacity(0.08)).padding(.vertical, 4)

            MenuRow(title: "Settings…", trailing: AnyView(
                Image(systemName: "arrow.right")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.white.opacity(0.5))
            ), action: onOpenSettings)

            MenuRow(title: "About NovaKey", trailing: AnyView(EmptyView()), action: {})

            Divider().background(Color.white.opacity(0.08)).padding(.vertical, 4)

            MenuRow(title: "Quit", trailing: AnyView(
                Text("⇧Q")
                    .font(.system(size: 11, weight: .medium, design: .rounded))
                    .foregroundStyle(.white.opacity(0.4))
            ), action: onQuit)
        }
        .padding(.vertical, 6)
        .padding(.bottom, 8)
    }
}

/// One row in the popover menu, with hover highlight.
struct MenuRow: View {
    let title: String
    let trailing: AnyView
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        HStack(spacing: 8) {
            Text(title)
                .font(.system(size: 13))
                .foregroundStyle(.white)
            Spacer()
            trailing
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 7)
        .contentShape(Rectangle())
        .background(hovering ? Color.white.opacity(0.06) : Color.clear)
        .onHover { hovering = $0 }
        .onTapGesture(perform: action)
    }
}

