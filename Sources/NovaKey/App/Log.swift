// Log.swift
// Simple file-based logger for debugging.
// Writes to ~/Library/Logs/NovaKey/novakey.log
//
// Log calls happen inside the CGEventTap callback (the keystroke hot path),
// so all file I/O is deferred to a background serial queue and the log file
// handle is opened once and kept open. A slow tap callback makes macOS
// disable the tap (tapDisabledByTimeout) and drop keystrokes mid-word.

import Foundation

enum Log {
    private static var logDir: String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        return "\(home)/Library/Logs/NovaKey"
    }
    private static var logFile: String { "\(logDir)/novakey.log" }

    /// Serial queue owning all logging state (handle + formatter).
    private static let queue = DispatchQueue(label: "com.novakey.log", qos: .utility)

    /// Kept open for the app's lifetime; only touched on `queue`.
    private static var handle: FileHandle?

    /// Only used on `queue` (DateFormatter is not thread-safe).
    private static let dateFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss.SSS"
        return f
    }()

    static func setup() {
        // Create log directory with user-only permissions (0700)
        let fm = FileManager.default
        if !fm.fileExists(atPath: logDir) {
            try? fm.createDirectory(atPath: logDir, withIntermediateDirectories: true)
            // Set directory to owner-only access
            try? fm.setAttributes([.posixPermissions: 0o700], ofItemAtPath: logDir)
        }
        // Clear old log on startup, create with owner-only permissions (0600)
        fm.createFile(atPath: logFile, contents: nil, attributes: [.posixPermissions: 0o600])
        queue.async {
            handle = FileHandle(forWritingAtPath: logFile)
        }
        info("=== NovaKey Log Started ===")
    }

    static func info(_ message: String) {
        write("INFO", message)
    }

    static func error(_ message: String) {
        write("ERROR", message)
    }

    /// Debug logs only in DEBUG builds. The autoclosure means the message
    /// string (often with interpolation) is never even built in release.
    static func debug(_ message: @autoclosure () -> String) {
        #if DEBUG
        write("DEBUG", message())
        #endif
    }

    private static func write(_ level: String, _ message: String) {
        let now = Date()
        queue.async {
            let timestamp = dateFormatter.string(from: now)
            let line = "[\(timestamp)] \(level): \(message)\n"
            if let data = line.data(using: .utf8) {
                handle?.write(data)
            }
        }
    }
}
