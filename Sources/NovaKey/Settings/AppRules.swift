// AppRules.swift
// Per-app rules (Settings > Apps): where Vietnamese composition runs. Three
// modes -- every app, only the listed apps, every app except the listed
// ones -- each with its own list of bundle identifiers. Pure logic; the
// event tap consults `allows(_:)` on every app switch, never per keystroke.
//
// The rules never change the user's V/E choice: where NovaKey is paused the
// menu bar shows E, and the setting itself is untouched for the other apps.

import Foundation

/// Which apps NovaKey composes Vietnamese in.
enum AppRuleMode: Int, CaseIterable {
    /// Every app (the default).
    case all = 0
    /// Only the apps in `AppRules.only`.
    case only = 1
    /// Every app except those in `AppRules.except`.
    case except = 2
}

struct AppRules: Equatable {

    /// Upper bound on each list, so a hand-edited preferences file cannot
    /// make the Settings window (one row per app) unboundedly tall.
    static let maxApps = 64

    var mode: AppRuleMode = .all
    /// Bundle identifiers, normalized (see `normalizeBundleID`), sorted.
    var only: [String] = []
    var except: [String] = []

    init() {}

    init(mode: AppRuleMode, only: [String] = [], except: [String] = []) {
        self.mode = mode
        self.only = Self.cleanList(only)
        self.except = Self.cleanList(except)
    }

    /// The list the current mode consults (empty for `.all`).
    var activeList: [String] {
        switch mode {
        case .all: return []
        case .only: return only
        case .except: return except
        }
    }

    /// Whether Vietnamese composition should run in the app `bundleID`
    /// (nil when the frontmost app could not be identified).
    func allows(_ bundleID: String?) -> Bool {
        let id = bundleID.flatMap(Self.normalizeBundleID)
        func listed(_ list: [String]) -> Bool { id.map(list.contains) ?? false }
        switch mode {
        case .all: return true
        case .only: return listed(only)
        case .except: return !listed(except)
        }
    }

    /// Add `bundleID` to the current mode's list. Returns false if the name
    /// is not a valid identifier, the mode has no list, the list is full, or
    /// it is already listed.
    @discardableResult
    mutating func add(_ bundleID: String) -> Bool {
        guard let id = Self.normalizeBundleID(bundleID) else { return false }
        return withActiveList { list -> Bool in
            guard list.count < Self.maxApps, !list.contains(id) else { return false }
            list.append(id)
            list.sort()
            return true
        } ?? false
    }

    /// Remove `bundleID` from the current mode's list.
    mutating func remove(_ bundleID: String) {
        guard let id = Self.normalizeBundleID(bundleID) else { return }
        withActiveList { list in list.removeAll { $0 == id } }
    }

    /// Turn Vietnamese composition on (`on`) or off in `bundleID`, editing
    /// whichever list the current mode uses. From "All apps", turning one app
    /// off switches to "All except" with that app listed -- the only way to
    /// honour the request.
    mutating func setEnabled(_ bundleID: String, _ on: Bool) {
        guard let id = Self.normalizeBundleID(bundleID) else { return }
        if allows(id) == on { return }
        if mode == .all { mode = .except }
        // Only-mode lists the apps that are on; Except-mode the ones that are off.
        if (mode == .only) == on {
            add(id)
        } else {
            remove(id)
        }
    }

    @discardableResult
    private mutating func withActiveList<T>(_ body: (inout [String]) -> T) -> T? {
        switch mode {
        case .all: return nil
        case .only: return body(&only)
        case .except: return body(&except)
        }
    }

    // MARK: - Validation

    /// Canonical form of an app identifier: a trimmed, lowercased bundle
    /// identifier (`com.apple.Safari` -> `com.apple.safari`; Launch Services
    /// treats them case-insensitively). Rejects anything implausible: empty,
    /// over 255 characters, or containing whitespace or control characters.
    static func normalizeBundleID(_ raw: String) -> String? {
        let id = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !id.isEmpty, id.count <= 255 else { return nil }
        let bad = CharacterSet.whitespacesAndNewlines.union(.controlCharacters)
        guard !id.unicodeScalars.contains(where: { bad.contains($0) }) else { return nil }
        return id
    }

    /// A stored list, with invalid and duplicate entries dropped, sorted and
    /// capped.
    static func cleanList(_ list: [String]) -> [String] {
        var seen = Set<String>()
        var out: [String] = []
        for raw in list {
            guard let id = normalizeBundleID(raw), seen.insert(id).inserted else { continue }
            out.append(id)
        }
        out.sort()
        return Array(out.prefix(maxApps))
    }

    // MARK: - Persistence (UserDefaults-shaped dictionary)

    static let modeKey = "mode"
    static let onlyKey = "only"
    static let exceptKey = "except"

    /// A property-list dictionary for UserDefaults.
    var dictionary: [String: Any] {
        [Self.modeKey: mode.rawValue, Self.onlyKey: only, Self.exceptKey: except]
    }

    /// Rebuild from `dictionary`; unknown or malformed values fall back to
    /// the defaults rather than failing the whole thing.
    init(dictionary: [String: Any]?) {
        let mode = (dictionary?[Self.modeKey] as? Int).flatMap(AppRuleMode.init(rawValue:)) ?? .all
        self.init(
            mode: mode,
            only: dictionary?[Self.onlyKey] as? [String] ?? [],
            except: dictionary?[Self.exceptKey] as? [String] ?? []
        )
    }
}
