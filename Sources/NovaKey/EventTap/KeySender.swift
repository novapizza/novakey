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

    /// The kind of the frontmost browser (updated on app activation). The
    /// autocomplete guard is scoped to browsers and tailored per kind. Read/
    /// written on the main thread, so no synchronization is needed.
    var browserKind: BrowserKind = .none

    /// Whether the frontmost app's input field repaints on every discrete
    /// synthetic event (e.g. Telegram Desktop's Qt composer), making the
    /// backspace-and-retype burst flicker. When true, we let macOS coalesce
    /// the burst and always insert text as one batch event, trading a little
    /// fast-typing robustness for far fewer visible repaints. Same threading
    /// contract as `browserKind`.
    var reduceFlicker: Bool = false

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
            // Whether the frontmost app's URL bar needs autocomplete guarding.
            let guardAutocomplete = fixBrowserAutocomplete && browserKind != .none

            // Send backspaces to delete old characters.
            //
            // Inline autocomplete in browser URL/address bars keeps its
            // suggested suffix *selected*, so the first backspace only clears
            // that selection instead of deleting a typed character
            // ("dd" -> "dđ" instead of "đ"). The compensation differs by
            // browser (see BrowserKind):
            //   - .chromium: the omnibox hides its selection from Accessibility,
            //     so prepend a narrow no-break space (U+202F) and send one extra
            //     backspace. The U+202F collapses the selection, then the
            //     backspaces (now N+1) delete it back out along with the real
            //     characters. Self-correcting — harmless when no selection.
            //   - .native (Safari): re-runs autocomplete after an injected char
            //     (defeating the U+202F trick) and doesn't expose the inline
            //     completion via AX selection queries (defeating a probe). Send
            //     a forward-delete instead: it deletes just the selected suffix
            //     when one is present, and is a no-op when the caret sits at the
            //     end of the text with nothing selected. Deletion doesn't
            //     retrigger Safari's inline completion, so the N real backspaces
            //     that follow land on real characters.
            if backspaces > 0 {
                var count = backspaces
                if guardAutocomplete {
                    switch browserKind {
                    case .none:
                        break
                    case .chromium:
                        sendEmptyCharacter(proxy: proxy)
                        count += 1
                    case .native:
                        sendForwardDelete(proxy: proxy)
                    }
                }
                sendBackspaces(count: count, proxy: proxy)
            }

            // Send replacement text.
            //
            // In browsers, always send the whole text as a single event, even
            // in step-by-step mode: the omnibox re-runs inline autocomplete
            // asynchronously on every insertion, so injecting "ếng" as three
            // rapid events lets a completion (with its selected suffix) land
            // *between* our characters and corrupt the word. One event means
            // one text change — any completion lands after the batch, where
            // the next replacement's guard or the user's next real keystroke
            // handles it, exactly like normal typing.
            // Flicker-prone apps additionally force the batch path: one event
            // = one text change = one repaint of the composer.
            if !text.isEmpty {
                if stepByStepMode && !guardAutocomplete && !reduceFlicker {
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
    ///
    /// Exception: in flicker-prone apps (`reduceFlicker`), discrete delivery
    /// is what causes the visible flashing — every event triggers a full
    /// composer repaint — so there we leave the flag off and accept the
    /// coalescing risk.
    private func markNonCoalesced(_ event: CGEvent) {
        guard !reduceFlicker else { return }
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

    /// Send a forward-delete (keycode 0x75) to collapse Safari's inline
    /// autocomplete: it deletes the selected suggestion suffix when present,
    /// and is a no-op when the caret is at the end of the text unselected.
    /// (Rare caveat: if the caret were mid-text with no selection it would eat
    /// the next character, but Safari only inline-completes at the end of the
    /// field, and the engine resets its session on clicks/arrows, so a
    /// replacement can't fire mid-text in practice.)
    private func sendForwardDelete(proxy: CGEventTapProxy) {
        let forwardDeleteKeyCode: CGKeyCode = 0x75

        guard let keyDown = CGEvent(keyboardEventSource: sourceManager.source,
                                    virtualKey: forwardDeleteKeyCode, keyDown: true),
              let keyUp = CGEvent(keyboardEventSource: sourceManager.source,
                                  virtualKey: forwardDeleteKeyCode, keyDown: false) else {
            return
        }
        markNonCoalesced(keyDown)
        markNonCoalesced(keyUp)
        keyDown.tapPostEvent(proxy)
        keyUp.tapPostEvent(proxy)
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
