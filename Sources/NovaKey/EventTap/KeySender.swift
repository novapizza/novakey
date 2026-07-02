// KeySender.swift
// Sends synthetic keyboard events (backspaces and Unicode characters)
// using CGEvent. These events are posted through the event tap proxy
// so they appear as normal keystrokes to the target application.

import Cocoa

/// Sends synthetic keystrokes to simulate the backspace-and-replace technique.
final class KeySender {

    private let sourceManager: EventSourceManager

    /// Whether to send each character as a separate event (slower, more compatible)
    /// vs batching into a single CGEvent with CGEventKeyboardSetUnicodeString.
    var stepByStepMode: Bool = false

    /// Whether to send a narrow no-break space before backspaces
    /// to defeat browser autocomplete interference.
    var fixBrowserAutocomplete: Bool = true

    init(sourceManager: EventSourceManager) {
        self.sourceManager = sourceManager
    }

    // MARK: - Public API

    /// Execute an engine result: send backspaces then replacement text.
    func execute(result: EngineResult, proxy: CGEventTapProxy) {
        switch result {
        case .passThrough, .wordBreak:
            break

        case .restore(let backspaces, let text):
            // Restore behaves identically to .replace at the keystroke level;
            // the caller is responsible for letting the original event pass.
            execute(result: .replace(backspaces: backspaces, text: text), proxy: proxy)

        case .replace(let backspaces, let text):
            // Send backspaces to delete old characters.
            // Inline autocomplete (browser URL bars, Spotlight) keeps its
            // suggested suffix as selected text, and the first backspace only
            // clears that selection instead of deleting a typed character
            // ("dd" -> "dđ" instead of "đ"). Probe the focused element's
            // selection to compensate exactly.
            if backspaces > 0 {
                var count = backspaces
                if fixBrowserAutocomplete {
                    if let selectionLength = focusedSelectionLength() {
                        if selectionLength > 0 {
                            count += 1
                            Log.debug("Autocomplete selection of \(selectionLength) chars; sending \(count) backspaces")
                        }
                    } else {
                        // Focused element doesn't answer accessibility queries;
                        // fall back to the invisible-character trick.
                        sendEmptyCharacter(proxy: proxy)
                        count += 1
                    }
                }
                sendBackspaces(count: count, proxy: proxy)
            }

            // Send replacement text
            if !text.isEmpty {
                if stepByStepMode {
                    sendTextStepByStep(text, proxy: proxy)
                } else {
                    sendTextBatch(text, proxy: proxy)
                }
            }
        }
    }

    // MARK: - Backspace Sending

    /// Apply the kCGEventFlagMaskNonCoalesced flag so macOS doesn't coalesce
    /// rapidly-fired synthetic events. Without this, fast typing can cause the
    /// OS to merge or drop our backspaces / character events, producing
    /// duplicated or missing letters.
    private func markNonCoalesced(_ event: CGEvent) {
        event.flags = event.flags.union(.maskNonCoalesced)
    }

    /// Send N backspace key events.
    /// Keycode 0x33 (51) = Backspace/Delete on macOS.
    private func sendBackspaces(count: Int, proxy: CGEventTapProxy) {
        let backspaceKeyCode: CGKeyCode = 0x33

        for _ in 0..<count {
            guard let keyDown = CGEvent(keyboardEventSource: sourceManager.source,
                                        virtualKey: backspaceKeyCode, keyDown: true),
                  let keyUp = CGEvent(keyboardEventSource: sourceManager.source,
                                      virtualKey: backspaceKeyCode, keyDown: false) else {
                continue
            }
            markNonCoalesced(keyDown)
            markNonCoalesced(keyUp)
            keyDown.tapPostEvent(proxy)
            keyUp.tapPostEvent(proxy)
        }
    }

    // MARK: - Text Sending (Batch)

    /// Send text as a single CGEvent using CGEventKeyboardSetUnicodeString.
    /// Faster, works with most apps. Falls back to step-by-step for long strings.
    private func sendTextBatch(_ text: String, proxy: CGEventTapProxy) {
        let utf16 = Array(text.utf16)

        // CGEventKeyboardSetUnicodeString has a practical limit of ~20 characters
        // For longer strings, chunk it
        let chunkSize = 16
        for start in stride(from: 0, to: utf16.count, by: chunkSize) {
            let end = min(start + chunkSize, utf16.count)
            var chunk = Array(utf16[start..<end])

            guard let keyDown = CGEvent(keyboardEventSource: sourceManager.source,
                                        virtualKey: 0, keyDown: true),
                  let keyUp = CGEvent(keyboardEventSource: sourceManager.source,
                                      virtualKey: 0, keyDown: false) else {
                continue
            }

            keyDown.keyboardSetUnicodeString(stringLength: chunk.count, unicodeString: &chunk)
            keyUp.keyboardSetUnicodeString(stringLength: chunk.count, unicodeString: &chunk)

            markNonCoalesced(keyDown)
            markNonCoalesced(keyUp)
            keyDown.tapPostEvent(proxy)
            keyUp.tapPostEvent(proxy)
        }
    }

    // MARK: - Text Sending (Step by Step)

    /// Send each character as a separate CGEvent. Slower but more compatible
    /// with apps that don't handle multi-character CGEvents well.
    private func sendTextStepByStep(_ text: String, proxy: CGEventTapProxy) {
        for char in text {
            var utf16 = Array(String(char).utf16)

            guard let keyDown = CGEvent(keyboardEventSource: sourceManager.source,
                                        virtualKey: 0, keyDown: true),
                  let keyUp = CGEvent(keyboardEventSource: sourceManager.source,
                                      virtualKey: 0, keyDown: false) else {
                continue
            }

            keyDown.keyboardSetUnicodeString(stringLength: utf16.count, unicodeString: &utf16)
            keyUp.keyboardSetUnicodeString(stringLength: utf16.count, unicodeString: &utf16)

            markNonCoalesced(keyDown)
            markNonCoalesced(keyUp)
            keyDown.tapPostEvent(proxy)
            keyUp.tapPostEvent(proxy)
        }
    }

    // MARK: - Browser Fix

    /// System-wide accessibility element, used to locate the focused text field.
    /// The short messaging timeout keeps the event-tap callback fast even when
    /// the focused app is slow to answer AX queries.
    private lazy var systemWideElement: AXUIElement = {
        let element = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(element, 0.05)
        return element
    }()

    /// Length of the selected text in the currently focused UI element,
    /// or nil if the element can't be queried via accessibility.
    ///
    /// A non-zero selection at replacement time means inline autocomplete is
    /// showing (the engine resets its session on clicks and non-letter keys,
    /// so a user-made selection can't survive into a replacement).
    private func focusedSelectionLength() -> Int? {
        var focusedRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(systemWideElement,
                                            kAXFocusedUIElementAttribute as CFString,
                                            &focusedRef) == .success,
              let focused = focusedRef,
              CFGetTypeID(focused) == AXUIElementGetTypeID() else {
            return nil
        }
        let element = focused as! AXUIElement

        var rangeRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element,
                                            kAXSelectedTextRangeAttribute as CFString,
                                            &rangeRef) == .success,
              let rangeValue = rangeRef,
              CFGetTypeID(rangeValue) == AXValueGetTypeID() else {
            return nil
        }
        var range = CFRange()
        guard AXValueGetValue((rangeValue as! AXValue), .cfRange, &range) else {
            return nil
        }
        return range.length
    }

    /// Send a narrow no-break space (U+202F) to defeat browser URL bar autocomplete
    /// that interferes with the backspace technique.
    private func sendEmptyCharacter(proxy: CGEventTapProxy) {
        var utf16: [UniChar] = [0x202F]

        guard let keyDown = CGEvent(keyboardEventSource: sourceManager.source,
                                    virtualKey: 0, keyDown: true),
              let keyUp = CGEvent(keyboardEventSource: sourceManager.source,
                                  virtualKey: 0, keyDown: false) else {
            return
        }

        keyDown.keyboardSetUnicodeString(stringLength: 1, unicodeString: &utf16)
        keyUp.keyboardSetUnicodeString(stringLength: 1, unicodeString: &utf16)

        markNonCoalesced(keyDown)
        markNonCoalesced(keyUp)
        keyDown.tapPostEvent(proxy)
        keyUp.tapPostEvent(proxy)
    }
}
