// EventTapManager.swift
// Creates and manages the global CGEvent tap that intercepts keyboard events.
// Handles lifecycle: create, enable, disable, destroy, sleep/wake recovery.

import Cocoa
import Carbon // IsSecureEventInputEnabled()

/// Manages the CGEventTap lifecycle and integrates it with the run loop.
final class EventTapManager {

    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var retainedSelf: Unmanaged<EventTapManager>?
    private(set) var isRunning = false

    /// How often the watchdog polls the tap's enabled state.
    private static let healthCheckInterval: TimeInterval = 2.0

    /// Tap callbacks slower than this are logged — a slow callback is what makes
    /// macOS disable the tap in the first place.
    private static let slowCallbackThreshold: TimeInterval = 0.100

    /// Watchdog timer; created only once `start()` succeeds, torn down in `stop()`.
    private var healthTimer: Timer?

    /// Diagnostics counters (see `logDiagnostics()`).
    private(set) var timeoutNotificationCount = 0
    private(set) var watchdogRecoveryCount = 0
    private(set) var slowCallbackCount = 0

    /// Dump state every ~30s (15 * 2s) so a user's log shows what the tap was
    /// doing at the moment input died, without needing them to reproduce on cue.
    private static let diagnosticsEveryNChecks = 15
    private var healthChecksSinceDiag = 0

    /// Key-downs the tap has actually received. `keyDownsSinceDiag` resets on
    /// every diagnostics dump: if it stays 0 while the user is typing, events
    /// aren't reaching the tap at all (secure input, or a wedged main thread) --
    /// which no amount of re-enabling can fix.
    private(set) var keyDownCount = 0
    private var keyDownsSinceDiag = 0

    /// Last observed secure-input state, so the watchdog can log transitions.
    private var lastSecureInputState = false

    let sourceManager: EventSourceManager
    let engine: TelexEngine
    let keySender: KeySender

    /// Callback reference to update UI when mode changes.
    var onModeChanged: ((Bool) -> Void)?

    /// Settings
    var toggleHotkeyKeyCode: UInt16 = KeyCode.z.rawValue
    var toggleHotkeyModifiers: CGEventFlags = .maskAlternate  // Option+Z

    /// Whether the Fn (Globe) key toggles Vietnamese/English mode.
    var switchWithFnKey: Bool = true

    /// Tracks Fn-key press state so we only toggle on the down edge.
    private var fnKeyWasDown = false

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

        // Bridge `self` to the C callback via userInfo pointer. The retain
        // that keeps this pointer valid is only taken once the tap and run
        // loop source are confirmed created (see below), so failed attempts
        // don't leak a retain per retry.
        let userInfo = Unmanaged.passUnretained(self).toOpaque()

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

        guard let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0) else {
            Log.error("Failed to create run loop source")
            CFMachPortInvalidate(tap)
            return false
        }

        // Everything created successfully -- now take the retain that keeps
        // `self` alive while the tap is active. Released in stop().
        self.retainedSelf = Unmanaged.passRetained(self)
        self.eventTap = tap
        self.runLoopSource = source
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)

        isRunning = true
        lastSecureInputState = IsSecureEventInputEnabled()
        startHealthTimer()
        Log.info("Event tap STARTED (stateID: \(sourceManager.stateID), secureInput: \(lastSecureInputState))")
        return true
    }

    /// Stop intercepting keyboard events.
    func stop() {
        guard isRunning else { return }

        healthTimer?.invalidate()
        healthTimer = nil

        if let source = runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .commonModes)
        }
        if let tap = eventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
            // Disabling alone leaves the mach port alive. `start()` creates a
            // fresh tap, so without invalidating here every stop/start cycle
            // (notably sleep/wake) would leak a port for the app's lifetime.
            CFMachPortInvalidate(tap)
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
    ///
    /// Always resets the engine session first: while the tap was dead the user's
    /// keystrokes reached the app without us seeing them, so the syllable buffer
    /// no longer matches what's on screen. Replaying a replacement against a
    /// stale buffer would backspace over the wrong characters.
    func reenable() {
        guard let tap = eventTap else { return }
        engine.resetSession()
        CGEvent.tapEnable(tap: tap, enable: true)
        Log.info("Event tap re-enabled after system timeout")
    }

    // MARK: - Watchdog

    private func startHealthTimer() {
        healthTimer?.invalidate()
        let timer = Timer(timeInterval: Self.healthCheckInterval, repeats: true) { [weak self] _ in
            self?.checkHealth()
        }
        // .common so the watchdog keeps firing during modal/tracking run loops
        // (menu open, alert up) — the same modes the tap source is added in.
        RunLoop.main.add(timer, forMode: .common)
        healthTimer = timer
    }

    /// Poll the tap's enabled state and revive it if macOS disabled it.
    ///
    /// The reactive `.tapDisabledByTimeout` handling in `handleEvent` is the
    /// fastest path, but it only runs if that notification is actually delivered
    /// to our callback. If it's missed, nothing else re-enables the tap and
    /// Vietnamese input stays dead until NovaKey is relaunched. This is the
    /// backstop for that case.
    func checkHealth() {
        guard isRunning, let tap = eventTap else { return }

        // Secure input can flip without the tap's state changing at all, so
        // check it on every pass and log the edges. A transition to `true` that
        // never comes back is the "typing died in Terminal" smoking gun.
        let secureNow = IsSecureEventInputEnabled()
        if secureNow != lastSecureInputState {
            lastSecureInputState = secureNow
            let frontmost = NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "?"
            Log.info("SECURE INPUT \(secureNow ? "ENABLED" : "disabled") (frontmost: \(frontmost)) -- while active, NO event tap receives key events")
        }

        guard !CGEvent.tapIsEnabled(tap: tap) else {
            // Tap looks healthy -- but "healthy" is exactly what a secure-input
            // or hung-main-thread failure also looks like, so dump state
            // periodically regardless. `keysSeen` is the discriminator: zero
            // while the user is typing means events aren't reaching us at all.
            healthChecksSinceDiag += 1
            if healthChecksSinceDiag >= Self.diagnosticsEveryNChecks {
                healthChecksSinceDiag = 0
                logDiagnostics(context: "periodic")
            }
            return
        }

        let frontmost = NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "?"
        let secureInput = IsSecureEventInputEnabled()
        Log.error("WATCHDOG: tap found DISABLED (frontmost: \(frontmost), secureInput: \(secureInput)) -- reviving")

        // Same stale-buffer reasoning as `reenable()`.
        engine.resetSession()
        CGEvent.tapEnable(tap: tap, enable: true)
        watchdogRecoveryCount += 1

        if CGEvent.tapIsEnabled(tap: tap) {
            Log.info("WATCHDOG: tap re-enabled (recoveries: \(watchdogRecoveryCount))")
        } else {
            Log.error("WATCHDOG: tapEnable did NOT stick -- tap still disabled")
        }
    }

    /// Dump the diagnostic counters plus the current secure-input state.
    ///
    /// Secure Keyboard Entry (Terminal.app/iTerm2, and any password field) blocks
    /// key events from reaching *every* event tap while leaving the tap reporting
    /// itself as enabled — so a healthy-looking tap that receives nothing is the
    /// signature of that state, and this log line is our only visibility into it.
    /// Note it belongs to whichever process turned it on: relaunching NovaKey
    /// cannot clear it.
    func logDiagnostics(context: String) {
        let keysSeen = keyDownsSinceDiag
        keyDownsSinceDiag = 0
        Log.info("""
            DIAG(\(context)): running=\(isRunning) \
            enabled=\(eventTap.map { CGEvent.tapIsEnabled(tap: $0) }.map(String.init) ?? "no-tap") \
            secureInput=\(IsSecureEventInputEnabled()) \
            vi=\(engine.isVietnameseMode) \
            frontmost=\(NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "?") \
            keysSeen=\(keysSeen) keysTotal=\(keyDownCount) \
            timeouts=\(timeoutNotificationCount) \
            watchdogRecoveries=\(watchdogRecoveryCount) \
            slowCallbacks=\(slowCallbackCount)
            """)
    }

    // MARK: - Event Processing

    /// Called from the global C callback. Times the real work so we can spot the
    /// slow callbacks that get the tap disabled in the first place.
    func handleEvent(proxy: CGEventTapProxy, type: CGEventType, event: CGEvent) -> Unmanaged<CGEvent>? {
        let started = CFAbsoluteTimeGetCurrent()
        let result = processEvent(proxy: proxy, type: type, event: event)
        let elapsed = CFAbsoluteTimeGetCurrent() - started

        if elapsed > Self.slowCallbackThreshold {
            slowCallbackCount += 1
            let ms = Int(elapsed * 1000)
            let count = slowCallbackCount
            // Resolving the frontmost app is itself not free, and the file write
            // is already deferred by Log — keep both off the hot path entirely.
            DispatchQueue.main.async {
                let frontmost = NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "?"
                Log.error("SLOW tap callback: \(ms)ms type=\(type.rawValue) frontmost=\(frontmost) (total: \(count))")
            }
        }
        return result
    }

    /// Processes a single event.
    /// Returns nil to suppress the event, or the (possibly modified) event to pass through.
    private func processEvent(proxy: CGEventTapProxy, type: CGEventType, event: CGEvent) -> Unmanaged<CGEvent>? {
        let passThrough = Unmanaged.passUnretained(event)

        // Handle tap disabled by system timeout
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            timeoutNotificationCount += 1
            Log.error("Tap disabled by \(type == .tapDisabledByTimeout ? "timeout" : "user input") (count: \(timeoutNotificationCount))")
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

        // Fn (Globe) key: toggle mode on the down edge so NovaKey stays in
        // sync with the system language switch. We never suppress the event —
        // the system's own Globe behavior (if any) still runs.
        if type == .flagsChanged {
            let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
            if keyCode == KeyCode.function.rawValue {
                let fnDown = event.flags.contains(.maskSecondaryFn)
                if fnDown && !fnKeyWasDown {
                    fnKeyWasDown = true
                    if switchWithFnKey {
                        Log.info("Fn key language switch detected")
                        toggleMode()
                    }
                } else if !fnDown {
                    fnKeyWasDown = false
                }
            }
            return passThrough
        }

        // Only process key-down events for the engine
        guard type == .keyDown else {
            return passThrough
        }

        keyDownCount += 1
        keyDownsSinceDiag += 1

        let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
        let flags = event.flags
        Log.verbose({
            let keyChar = KeyCode(rawValue: keyCode)?.asciiLetter.map(String.init) ?? "?"
            let flagStr = [
                flags.contains(.maskShift) ? "Shift" : nil,
                flags.contains(.maskAlphaShift) ? "Caps" : nil,
                flags.contains(.maskControl) ? "Ctrl" : nil,
                flags.contains(.maskAlternate) ? "Opt" : nil,
                flags.contains(.maskCommand) ? "Cmd" : nil,
            ].compactMap { $0 }.joined(separator: "+")
            return "keyDown: \(keyChar) (0x\(String(keyCode, radix: 16))) flags=[\(flagStr)] vi=\(engine.isVietnameseMode) buffer='\(engine.buffer.text)'"
        }())

        // Check for hotkey toggle (Option+Z by default)
        if isToggleHotkey(keyCode: keyCode, flags: flags) {
            Log.info("HOTKEY TOGGLE detected")
            toggleMode()
            return nil // Suppress the hotkey event
        }

        // Effective letter case = Shift XOR CapsLock. Without folding CapsLock
        // (.maskAlphaShift) in, replaced text would be rebuilt lowercase while
        // pass-through consonants stay uppercase -- e.g. "CHào" instead of
        // "CHÀO" when composing with CapsLock on.
        let isShift = flags.contains(.maskShift) != flags.contains(.maskAlphaShift)
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
            Log.verbose("REPLACE: \(bs) backspaces + '\(text)' (browser=\(keySender.browserKind), flicker=\(keySender.reduceFlicker))")
            keySender.execute(result: result, proxy: proxy)
            return nil

        case .restore(let bs, let text):
            // Invalid syllable at word-break: emit the raw-keystroke restore
            // synthetically, then let the original word-break key pass through.
            Log.verbose("RESTORE: \(bs) backspaces + '\(text)'")
            keySender.execute(result: .replace(backspaces: bs, text: text), proxy: proxy)
            return passThrough
        }
    }

    // MARK: - Mode Toggle

    /// Flip Vietnamese/English mode, reset the syllable buffer, and notify the UI.
    private func toggleMode() {
        engine.isVietnameseMode.toggle()
        engine.resetSession()
        onModeChanged?(engine.isVietnameseMode)
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
