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
    private var permissionsWindow: NSWindow?
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
        engine.quickVietnamese = AppSettings.shared.quickVietnamese
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
        browserWatcher.onFlickerProneChange = { [weak self] prone in
            self?.eventTapManager.keySender.reduceFlicker = prone
        }
        browserWatcher.start()
        eventTapManager.keySender.browserKind = browserWatcher.kind
        eventTapManager.keySender.reduceFlicker = browserWatcher.isFlickerProne

        // Start the event tap only once permissions are verified; otherwise
        // show the non-modal onboarding window. No system prompts fire here.
        if AccessibilityPermission.isGranted {
            startEventTap()
        } else {
            Log.info("Permissions missing -- showing onboarding")
            showPermissionsOnboarding()
        }

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

    /// Recheck permissions whenever the user comes back to NovaKey (e.g.
    /// after granting one in System Settings). Replaces the old 2s polling.
    func applicationDidBecomeActive(_ notification: Notification) {
        guard eventTapManager != nil, !eventTapManager.isRunning else { return }
        if AccessibilityPermission.isGranted {
            startEventTap()
        }
    }

    // MARK: - Event Tap

    /// Start the tap. Never prompts or polls; on failure it (re-)shows the
    /// non-modal onboarding window so the user drives the permission flow.
    private func startEventTap() {
        guard eventTapManager != nil else {
            Log.error("Cannot start: EventTapManager is nil")
            return
        }

        if eventTapManager.start() {
            Log.info("Event tap started OK")
            permissionsWindow?.close()
            permissionsWindow = nil
        } else {
            Log.error("Event tap FAILED -- showing permissions onboarding")
            showPermissionsOnboarding()
        }
    }

    // MARK: - Permissions Onboarding

    private func showPermissionsOnboarding() {
        if let window = permissionsWindow {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let view = PermissionsOnboardingView(onCheckAgain: { [weak self] in
            guard let self else { return }
            if AccessibilityPermission.isGranted {
                self.startEventTap()
            }
        })
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 420, height: 320),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "NovaKey Setup"
        window.contentView = NSHostingView(rootView: view)
        window.center()
        window.isReleasedWhenClosed = false
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        self.permissionsWindow = window
    }

    // MARK: - Settings

    private func applySettings() {
        let settings = AppSettings.shared
        eventTapManager.keySender.fixBrowserAutocomplete = settings.fixBrowserAutocomplete
        eventTapManager.keySender.stepByStepMode = settings.sendKeyStepByStep
        eventTapManager.toggleHotkeyKeyCode = settings.toggleHotkeyKeyCode
        eventTapManager.toggleHotkeyModifiers = CGEventFlags(rawValue: settings.toggleHotkeyModifiers)
        eventTapManager.switchWithFnKey = settings.switchWithFnKey
        engine.quickVietnamese = settings.quickVietnamese
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
