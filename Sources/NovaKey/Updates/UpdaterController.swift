// UpdaterController.swift
// Thin wrapper over Sparkle's standard updater. Reads SUFeedURL / SUPublicEDKey
// from Info.plist; SUEnableAutomaticChecks drives the daily background check.

import Foundation
import Sparkle

final class UpdaterController {
    static let shared = UpdaterController()

    private let controller: SPUStandardUpdaterController

    private init() {
        // startingUpdater: true begins the scheduled background check loop.
        controller = SPUStandardUpdaterController(
            startingUpdater: true,
            updaterDelegate: nil,
            userDriverDelegate: nil
        )
    }

    /// User-initiated check — shows Sparkle's UI (progress, release notes).
    func checkForUpdates() {
        controller.checkForUpdates(nil)
    }
}
