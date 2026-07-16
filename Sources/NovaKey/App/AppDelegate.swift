// AppDelegate.swift
// Main application delegate. Sets up the event tap, status bar, and handles lifecycle.

import Cocoa
import ServiceManagement
import SwiftUI

final class AppDelegate: NSObject, NSApplicationDelegate {

    private var engine: TelexEngine!
    private var eventTapManager: EventTapManager!
    private var statusBarController: StatusBarController!
    private var settingsWindow: NSWindow?
    private let browserWatcher = BrowserWatcher()

    // MARK: - App Lifecycle

    func applicationDidFinishLaunching(_ notification: Notification) {
        Log.setup()
        Log.info("NovaKey starting...")

        // Permission is checked later when event tap starts.
        // Only prompt if it actually fails.

        // Initialize engine
        engine = TelexEngine()
        engine.isVietnameseMode = AppSettings.shared.isVietnameseMode
        Log.info("Engine initialized, Vietnamese mode: \(engine.isVietnameseMode)")

        // Initialize event tap
        guard let tapManager = EventTapManager(engine: engine) else {
            Log.error("Failed to create EventTapManager")
            showFatalError("Failed to initialize event source. Please restart NovaKey.")
            return
        }
        eventTapManager = tapManager
        Log.info("EventTapManager created")

        // Apply settings to event tap
        applySettings()

        // Set up mode change callback
        eventTapManager.onModeChanged = { [weak self] isVietnamese in
            DispatchQueue.main.async {
                self?.statusBarController.updateIcon(isVietnamese: isVietnamese)
                AppSettings.shared.isVietnameseMode = isVietnamese
                NotificationCenter.default.post(
                    name: .novaKeyModeChanged, object: nil,
                    userInfo: ["isVietnamese": isVietnamese]
                )
                if AppSettings.shared.playSoundOnSwitch {
                    NSSound(named: NSSound.Name("Tink"))?.play()
                }
                Log.info("Mode toggled to: \(isVietnamese ? "Vietnamese" : "English")")
            }
        }

        // Settings changed: re-apply to event tap.
        NotificationCenter.default.addObserver(
            forName: .novaKeySettingsChanged, object: nil, queue: .main
        ) { [weak self] _ in
            self?.applySettings()
        }

        // Set up status bar
        statusBarController = StatusBarController(engine: engine)
        statusBarController.setup()
        statusBarController.onOpenSettings = { [weak self] in
            self?.showSettings()
        }
        statusBarController.onQuit = {
            NSApplication.shared.terminate(nil)
        }

        // Track the frontmost app so the autocomplete guard is scoped to
        // browsers. Push the flag into the sender on every activation change.
        browserWatcher.onChange = { [weak self] kind in
            self?.eventTapManager.keySender.browserKind = kind
        }
        browserWatcher.start()
        eventTapManager.keySender.browserKind = browserWatcher.kind

        // Start event tap
        startEventTap()

        // Register for sleep/wake notifications
        let workspace = NSWorkspace.shared.notificationCenter
        workspace.addObserver(self, selector: #selector(handleSleep),
                              name: NSWorkspace.willSleepNotification, object: nil)
        workspace.addObserver(self, selector: #selector(handleWake),
                              name: NSWorkspace.didWakeNotification, object: nil)

        // Enable launch at login
        enableLaunchAtLogin()

        Log.info("NovaKey started successfully")
    }

    func applicationWillTerminate(_ notification: Notification) {
        eventTapManager?.stop()
        Log.info("NovaKey terminated")
    }

    // MARK: - Event Tap

    private func startEventTap() {
        guard eventTapManager != nil else {
            Log.error("Cannot start: EventTapManager is nil")
            return
        }

        if eventTapManager.start() {
            Log.info("Event tap started OK")
        } else {
            Log.error("Event tap FAILED -- requesting permission...")
            AccessibilityPermission.requestAccess()
            AccessibilityPermission.showPermissionAlert()
            pollForPermission()
        }
    }

    /// Poll every 2 seconds until permission is granted, then start the event tap.
    private func pollForPermission() {
        Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] timer in
            if AccessibilityPermission.isGranted {
                timer.invalidate()
                Log.info("Permission granted via polling")
                self?.startEventTap()
            }
        }
    }

    // MARK: - Settings

    private func applySettings() {
        let settings = AppSettings.shared
        eventTapManager.keySender.fixBrowserAutocomplete = settings.fixBrowserAutocomplete
        eventTapManager.keySender.stepByStepMode = settings.sendKeyStepByStep
        eventTapManager.toggleHotkeyKeyCode = settings.toggleHotkeyKeyCode
        eventTapManager.toggleHotkeyModifiers = CGEventFlags(rawValue: settings.toggleHotkeyModifiers)
    }

    private func showSettings() {
        if let window = settingsWindow {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let settingsView = SettingsView(onToggleMode: { [weak self] vietnamese in
            guard let self else { return }
            if self.engine.isVietnameseMode != vietnamese {
                self.engine.isVietnameseMode = vietnamese
                self.engine.resetSession()
                AppSettings.shared.isVietnameseMode = vietnamese
                self.statusBarController.updateIcon(isVietnamese: vietnamese)
                NotificationCenter.default.post(
                    name: .novaKeyModeChanged, object: nil,
                    userInfo: ["isVietnamese": vietnamese]
                )
            }
        })
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 460, height: 560),
            styleMask: [.titled, .closable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "NovaKey Settings"
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isMovableByWindowBackground = true
        window.appearance = NSAppearance(named: .darkAqua)
        window.backgroundColor = NSColor(red: 0.11, green: 0.11, blue: 0.12, alpha: 1.0)
        window.contentView = NSHostingView(rootView: settingsView)
        window.center()
        window.isReleasedWhenClosed = false
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        self.settingsWindow = window
    }

    // MARK: - Sleep / Wake

    @objc private func handleSleep(_ notification: Notification) {
        eventTapManager?.stop()
        Log.info("Stopped for sleep")
    }

    @objc private func handleWake(_ notification: Notification) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) { [weak self] in
            self?.startEventTap()
        }
    }

    // MARK: - Launch at Login

    private func enableLaunchAtLogin() {
        do {
            try SMAppService.mainApp.register()
            Log.info("Launch at login: enabled")
        } catch {
            Log.error("Launch at login failed: \(error.localizedDescription)")
        }
    }

    // MARK: - Error Handling

    private func showFatalError(_ message: String) {
        Log.error("Fatal: \(message)")
        let alert = NSAlert()
        alert.messageText = "NovaKey Error"
        alert.informativeText = message
        alert.alertStyle = .critical
        alert.addButton(withTitle: "Quit")
        alert.runModal()
    }
}
