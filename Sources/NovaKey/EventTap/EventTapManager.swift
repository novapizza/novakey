// EventTapManager.swift
// Creates and manages the global CGEvent tap that intercepts keyboard events.
// Handles lifecycle: create, enable, disable, destroy, sleep/wake recovery.

import Cocoa

/// Manages the CGEventTap lifecycle and integrates it with the run loop.
final class EventTapManager {

    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var retainedSelf: Unmanaged<EventTapManager>?
    private var isRunning = false

    let sourceManager: EventSourceManager
    let engine: TelexEngine
    let keySender: KeySender

    /// Callback reference to update UI when mode changes.
    var onModeChanged: ((Bool) -> Void)?

    /// Settings
    var toggleHotkeyKeyCode: UInt16 = KeyCode.z.rawValue
    var toggleHotkeyModifiers: CGEventFlags = .maskAlternate  // Option+Z

    init?(engine: TelexEngine) {
        guard let srcMgr = EventSourceManager() else {
            Log.error("Failed to create CGEventSource")
            return nil
        }
        self.sourceManager = srcMgr
        self.engine = engine
        self.keySender = KeySender(sourceManager: srcMgr)
    }

    deinit {
        stop()
    }

    // MARK: - Start / Stop

    /// Start intercepting keyboard events.
    func start() -> Bool {
        guard !isRunning else { return true }

        Log.info("Attempting event tap creation...")
        Log.info("  InputMonitoring: \(AccessibilityPermission.hasInputMonitoring)")
        Log.info("  Accessibility: \(AccessibilityPermission.hasAccessibility)")

        // Events to intercept
        let eventMask: CGEventMask =
            (1 << CGEventType.keyDown.rawValue) |
            (1 << CGEventType.keyUp.rawValue) |
            (1 << CGEventType.flagsChanged.rawValue) |
            (1 << CGEventType.leftMouseDown.rawValue) |
            (1 << CGEventType.rightMouseDown.rawValue)

        // Bridge `self` to the C callback via userInfo pointer.
        // Use passRetained to prevent use-after-free if manager is deallocated
        // while the tap is active. Released in stop().
        let retained = Unmanaged.passRetained(self)
        self.retainedSelf = retained
        let userInfo = retained.toOpaque()

        // Try cgSessionEventTap first
        var tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: eventMask,
            callback: globalEventTapCallback,
            userInfo: userInfo
        )

        if tap == nil {
            Log.info("cgSessionEventTap failed, trying cghidEventTap...")
            tap = CGEvent.tapCreate(
                tap: .cghidEventTap,
                place: .headInsertEventTap,
                options: .defaultTap,
                eventsOfInterest: eventMask,
                callback: globalEventTapCallback,
                userInfo: userInfo
            )
        }

        guard let tap = tap else {
            Log.error("All CGEvent.tapCreate attempts FAILED")
            return false
        }

        self.eventTap = tap

        guard let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0) else {
            Log.error("Failed to create run loop source")
            self.eventTap = nil
            return false
        }

        self.runLoopSource = source
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)

        isRunning = true
        Log.info("Event tap STARTED (stateID: \(sourceManager.stateID))")
        return true
    }

    /// Stop intercepting keyboard events.
    func stop() {
        guard isRunning else { return }

        if let source = runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .commonModes)
        }
        if let tap = eventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
        }

        runLoopSource = nil
        eventTap = nil
        isRunning = false

        // Release the retained self reference
        retainedSelf?.release()
        retainedSelf = nil

        Log.info("Event tap stopped")
    }

    /// Re-enable the event tap if macOS disabled it due to timeout.
    func reenable() {
        guard let tap = eventTap else { return }
        CGEvent.tapEnable(tap: tap, enable: true)
        Log.info("Event tap re-enabled after system timeout")
    }

    // MARK: - Event Processing

    /// Called from the global C callback. Processes a single event.
    /// Returns nil to suppress the event, or the (possibly modified) event to pass through.
    func handleEvent(proxy: CGEventTapProxy, type: CGEventType, event: CGEvent) -> Unmanaged<CGEvent>? {
        let passThrough = Unmanaged.passUnretained(event)

        // Handle tap disabled by system timeout
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            reenable()
            return passThrough
        }

        // Skip our own synthetic events (self-event detection)
        if sourceManager.isSelfGenerated(event) {
            return passThrough
        }

        // Mouse clicks reset the session
        if type == .leftMouseDown || type == .rightMouseDown {
            engine.resetSession()
            return passThrough
        }

        // Only process key-down events for the engine
        guard type == .keyDown else {
            return passThrough
        }

        let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
        let flags = event.flags
        #if DEBUG
        let keyChar = KeyCode(rawValue: keyCode)?.asciiLetter.map(String.init) ?? "?"
        let flagStr = [
            flags.contains(.maskShift) ? "Shift" : nil,
            flags.contains(.maskControl) ? "Ctrl" : nil,
            flags.contains(.maskAlternate) ? "Opt" : nil,
            flags.contains(.maskCommand) ? "Cmd" : nil,
        ].compactMap { $0 }.joined(separator: "+")
        Log.debug("keyDown: \(keyChar) (0x\(String(keyCode, radix: 16))) flags=[\(flagStr)] vi=\(engine.isVietnameseMode)")
        #endif

        // Check for hotkey toggle (Option+Z by default)
        if isToggleHotkey(keyCode: keyCode, flags: flags) {
            Log.info("HOTKEY TOGGLE detected")
            engine.isVietnameseMode.toggle()
            engine.resetSession()
            onModeChanged?(engine.isVietnameseMode)
            return nil // Suppress the hotkey event
        }

        let isShift = flags.contains(.maskShift)
        let hasCmd = flags.contains(.maskCommand)
        let hasCtrl = flags.contains(.maskControl)
        let hasOption = flags.contains(.maskAlternate)

        let result = engine.processKey(
            keyCode: keyCode,
            isShift: isShift,
            hasCommandOrControl: hasCmd || hasCtrl,
            hasOption: hasOption
        )

        switch result {
        case .passThrough, .wordBreak:
            return passThrough

        case .replace(let bs, let text):
            Log.debug("REPLACE: \(bs) backspaces + '\(text)'")
            keySender.execute(result: result, proxy: proxy)
            return nil

        case .restore(let bs, let text):
            // Invalid syllable at word-break: emit the raw-keystroke restore
            // synthetically, then let the original word-break key pass through.
            Log.debug("RESTORE: \(bs) backspaces + '\(text)'")
            keySender.execute(result: .replace(backspaces: bs, text: text), proxy: proxy)
            return passThrough
        }
    }

    // MARK: - Hotkey Detection

    private func isToggleHotkey(keyCode: UInt16, flags: CGEventFlags) -> Bool {
        guard keyCode == toggleHotkeyKeyCode else { return false }

        // Check that the required modifier is pressed
        let relevantFlags = flags.intersection([.maskShift, .maskControl, .maskAlternate, .maskCommand])
        Log.debug("Hotkey check: relevantFlags=0x\(String(relevantFlags.rawValue, radix: 16)) expected=0x\(String(toggleHotkeyModifiers.rawValue, radix: 16))")
        return relevantFlags == toggleHotkeyModifiers
    }
}

// MARK: - Global C Callback

/// The global event tap callback function.
/// CGEvent.tapCreate requires a C-compatible function pointer.
/// We bridge to the EventTapManager instance via the userInfo pointer.
private func globalEventTapCallback(
    proxy: CGEventTapProxy,
    type: CGEventType,
    event: CGEvent,
    userInfo: UnsafeMutableRawPointer?
) -> Unmanaged<CGEvent>? {
    guard let userInfo = userInfo else {
        return Unmanaged.passUnretained(event)
    }

    let manager = Unmanaged<EventTapManager>.fromOpaque(userInfo).takeUnretainedValue()
    return manager.handleEvent(proxy: proxy, type: type, event: event)
}
