// BrowserWatcher.swift
// Tracks whether the frontmost application is a web browser, and which kind, so
// the autocomplete-defeating logic can be scoped and tailored per browser.
//
// Mirrors the Windows port's `set_is_browser` on foreground change: rather than
// probing the focused element on every replacement, we cache a cheap value on
// each app activation and read it from the hot event-tap path.
//
// Two browser families need different handling for URL-bar inline autocomplete:
//   - Chromium-family (Chrome, Edge, Brave, Arc, ...) render the omnibox as a
//     custom view that hides its selection from Accessibility. The blind U+202F
//     trick is the only thing that works there.
//   - Safari uses a native text field that re-runs autocomplete right after an
//     injected character, defeating the U+202F trick — but it *does* answer AX
//     selection queries correctly, so we probe the selection instead.

import Cocoa

/// How the frontmost browser (if any) exposes its URL-bar autocomplete.
enum BrowserKind {
    /// Not a browser — no autocomplete guard needed.
    case none
    /// Chromium/Gecko-family: custom-rendered omnibox, hidden from AX.
    /// Use the self-correcting U+202F trick.
    case chromium
    /// Safari-family: native text field, answers AX selection queries.
    /// Probe the selection and add one backspace only when it's non-empty.
    case native
}

/// Observes app activation and caches the frontmost browser's kind.
final class BrowserWatcher {

    /// Bundle IDs of Safari-family browsers (native field, AX-probed).
    private static let nativeBrowserIDs: Set<String> = [
        "com.apple.Safari",
        "com.apple.SafariTechnologyPreview",
    ]

    /// Bundle IDs of Chromium/Gecko-family browsers (custom omnibox, U+202F).
    private static let chromiumBrowserIDs: Set<String> = [
        "com.google.Chrome",
        "com.google.Chrome.canary",
        "com.microsoft.edgemac",
        "org.mozilla.firefox",
        "org.mozilla.firefoxdeveloperedition",
        "com.brave.Browser",
        "com.brave.Browser.beta",
        "com.vivaldi.Vivaldi",
        "com.operasoftware.Opera",
        "org.chromium.Chromium",
        "company.thebrowser.Browser", // Arc
    ]

    /// The kind of the current frontmost app. Read from the event-tap callback;
    /// updated only on the main thread via the activation notification, so a
    /// plain stored value is sufficient (no synchronization needed).
    private(set) var kind: BrowserKind = .none

    /// Invoked whenever `kind` changes (used to push it into KeySender).
    var onChange: ((BrowserKind) -> Void)?

    private var observer: NSObjectProtocol?

    func start() {
        // Seed from the currently-active app.
        update(app: NSWorkspace.shared.frontmostApplication)

        let center = NSWorkspace.shared.notificationCenter
        observer = center.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification,
            object: nil, queue: .main
        ) { [weak self] note in
            let app = note.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication
            self?.update(app: app)
        }
    }

    func stop() {
        if let observer {
            NSWorkspace.shared.notificationCenter.removeObserver(observer)
        }
        observer = nil
    }

    deinit {
        stop()
    }

    private func update(app: NSRunningApplication?) {
        let bundleID = app?.bundleIdentifier
        let newKind: BrowserKind
        if let bundleID, Self.nativeBrowserIDs.contains(bundleID) {
            newKind = .native
        } else if let bundleID, Self.chromiumBrowserIDs.contains(bundleID) {
            newKind = .chromium
        } else {
            newKind = .none
        }
        guard newKind != kind else { return }
        kind = newKind
        Log.debug("Frontmost app '\(bundleID ?? "?")' browserKind=\(newKind)")
        onChange?(newKind)
    }
}
