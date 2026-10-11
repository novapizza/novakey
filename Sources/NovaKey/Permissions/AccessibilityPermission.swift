// AccessibilityPermission.swift
// Checks and requests Input Monitoring and Accessibility permissions
// required for the CGEvent tap to work.
//
// Design: preflight checks never prompt; requests are staged one at a
// time and never combined with a blocking modal. See the permissions
// onboarding window in AppDelegate for the user-driven flow.

import Cocoa

enum AccessibilityPermission {

    // MARK: - Preflight (never prompts)

    /// Whether the app has Input Monitoring permission.
    static var hasInputMonitoring: Bool {
        CGPreflightListenEventAccess()
    }

    /// Whether the app has Accessibility permission.
    static var hasAccessibility: Bool {
        AXIsProcessTrusted()
    }

    /// Whether both permissions are granted.
    static var isGranted: Bool {
        hasInputMonitoring && hasAccessibility
    }

    // MARK: - Staged requests (one at a time, no modal on top)

    /// Request Input Monitoring. Shows the system prompt if not yet determined.
    @discardableResult
    static func requestInputMonitoring() -> Bool {
        let granted = CGRequestListenEventAccess()
        Log.info("CGRequestListenEventAccess: \(granted)")
        return granted
    }

    /// Request Accessibility. Shows the asynchronous system prompt if not
    /// yet determined. Do not present any modal UI on top of this.
    @discardableResult
    static func requestAccessibility() -> Bool {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue(): true] as CFDictionary
        let trusted = AXIsProcessTrustedWithOptions(options)
        Log.info("AXIsProcessTrustedWithOptions: \(trusted)")
        return trusted
    }

    // MARK: - System Settings deep links (non-blocking)

    static func openInputMonitoringSettings() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent") {
            NSWorkspace.shared.open(url)
        }
    }

    static func openAccessibilitySettings() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
            NSWorkspace.shared.open(url)
        }
    }
}
