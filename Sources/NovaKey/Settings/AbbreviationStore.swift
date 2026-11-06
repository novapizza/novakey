// AbbreviationStore.swift
// Holds the user's abbreviation list and keeps it on disk at
// ~/Library/Application Support/NovaKey/abbreviations.txt -- a plain text
// file (one `abbreviation<TAB>expansion` per line, see `Abbreviations.parse`)
// rather than a UserDefaults blob, so the user can edit it by hand and share
// it with the Windows app, which reads the same format.

import Foundation

final class AbbreviationStore {

    static let shared = AbbreviationStore()

    /// Largest file read, so a stray huge file cannot stall startup.
    static let maxFileSize: UInt64 = 4 * 1024 * 1024

    /// The current table. Read from the event tap on Tab; written only on the
    /// main thread by the editor, which is also where the tap runs.
    private(set) var table = Abbreviations()

    /// Where the list lives; nil if Application Support cannot be found.
    let fileURL: URL?

    init(fileURL: URL? = AbbreviationStore.defaultFileURL()) {
        self.fileURL = fileURL
    }

    static func defaultFileURL() -> URL? {
        guard let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
        else { return nil }
        return base.appendingPathComponent(AppConstants.appName, isDirectory: true)
            .appendingPathComponent("abbreviations.txt", isDirectory: false)
    }

    /// Load the list from disk (empty if missing, unreadable or too large).
    func load() {
        table = Self.read(from: fileURL)
    }

    static func read(from url: URL?) -> Abbreviations {
        guard let url else { return Abbreviations() }
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: url.path),
              let size = attrs[.size] as? UInt64, size <= maxFileSize
        else { return Abbreviations() }
        guard let text = try? String(contentsOf: url, encoding: .utf8) else { return Abbreviations() }
        return Abbreviations.parse(text)
    }

    /// Replace the table and save it. Written atomically (to a temporary file
    /// renamed over the old one), so a failed write never leaves a truncated
    /// list behind. Returns whether it was saved; the in-memory table is
    /// updated either way so typing keeps working.
    @discardableResult
    func replace(with newTable: Abbreviations) -> Bool {
        table = newTable
        NotificationCenter.default.post(name: .novaKeyAbbreviationsChanged, object: nil)
        return Self.write(newTable, to: fileURL)
    }

    static func write(_ table: Abbreviations, to url: URL?) -> Bool {
        guard let url else { return false }
        do {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data(table.serialize().utf8).write(to: url, options: .atomic)
            return true
        } catch {
            return false
        }
    }
}

extension Notification.Name {
    /// The abbreviation list changed (posted by `AbbreviationStore.replace`).
    static let novaKeyAbbreviationsChanged = Notification.Name("NovaKeyAbbreviationsChanged")
}
