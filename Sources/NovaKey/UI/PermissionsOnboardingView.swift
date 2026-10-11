// PermissionsOnboardingView.swift
// Non-modal first-run window that walks the user through granting
// Input Monitoring and Accessibility, one permission at a time.
//
// Deliberately NOT an NSAlert: a blocking modal on top of the async
// system privacy prompt can wedge the whole permission flow.

import SwiftUI

struct PermissionsOnboardingView: View {

    /// Called when the user clicks "Check Again" (or after a grant action).
    var onCheckAgain: () -> Void

    @State private var hasInputMonitoring = AccessibilityPermission.hasInputMonitoring
    @State private var hasAccessibility = AccessibilityPermission.hasAccessibility

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("NovaKey Needs Permissions")
                .font(.title2.bold())

            Text("NovaKey needs two permissions to type Vietnamese. Grant them one at a time, then return here.")
                .font(.callout)
                .foregroundColor(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            // Each Grant button triggers exactly ONE system UI: the standard
            // macOS permission dialog (which has its own "Open System Settings"
            // button). We never open System Settings at the same time —
            // overlapping permission UIs are what caused the first-run hang.
            permissionRow(
                title: "1. Accessibility",
                detail: "Lets NovaKey replace what you type.",
                granted: hasAccessibility,
                enabled: true,
                grantAction: { AccessibilityPermission.requestAccessibility() },
                settingsAction: { AccessibilityPermission.openAccessibilitySettings() }
            )

            permissionRow(
                title: "2. Input Monitoring",
                detail: "Lets NovaKey see your keystrokes. Grant Accessibility first.",
                granted: hasInputMonitoring,
                enabled: hasAccessibility,
                grantAction: { AccessibilityPermission.requestInputMonitoring() },
                settingsAction: { AccessibilityPermission.openInputMonitoringSettings() }
            )

            Text("You may need to restart NovaKey after granting Accessibility.")
                .font(.caption)
                .foregroundColor(.secondary)

            HStack {
                Spacer()
                Button("Check Again") {
                    refresh()
                    onCheckAgain()
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 420)
        .onReceive(NotificationCenter.default.publisher(
            for: NSApplication.didBecomeActiveNotification
        )) { _ in
            refresh()
        }
    }

    private func refresh() {
        hasInputMonitoring = AccessibilityPermission.hasInputMonitoring
        hasAccessibility = AccessibilityPermission.hasAccessibility
        // Both green -> let the app start the tap and close this window.
        if hasInputMonitoring && hasAccessibility {
            onCheckAgain()
        }
    }

    @ViewBuilder
    private func permissionRow(
        title: String,
        detail: String,
        granted: Bool,
        enabled: Bool,
        grantAction: @escaping () -> Void,
        settingsAction: @escaping () -> Void
    ) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: granted ? "checkmark.circle.fill" : "circle")
                .foregroundColor(granted ? .green : .secondary)
                .font(.title3)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.headline)
                Text(detail)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                if !granted && enabled {
                    // Fallback for when macOS suppresses the dialog
                    // (permission was previously denied).
                    Button("Open System Settings…", action: settingsAction)
                        .buttonStyle(.link)
                        .font(.caption)
                }
            }
            Spacer()
            if !granted {
                // Refresh right after the request: if the permission was in
                // fact already granted, macOS shows no dialog and the row
                // would otherwise look like the click did nothing.
                Button("Grant…") {
                    grantAction()
                    refresh()
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) { refresh() }
                }
                .disabled(!enabled)
            }
        }
        .padding(12)
        .background(Color.primary.opacity(0.05))
        .cornerRadius(8)
    }
}
