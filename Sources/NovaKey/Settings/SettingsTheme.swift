// SettingsTheme.swift
// Which look NovaKey's windows use (Settings > Appearance). The tokens and
// drawing live in UI/NovaTheme.swift; this is the stored choice.

import Foundation

enum SettingsTheme: Int, CaseIterable {
    /// The dark, card-based look (the default).
    case modern = 0
    /// The gray, bevelled look of classic Mac OS.
    case platinum = 1

    var title: String {
        switch self {
        case .modern: return "Modern"
        case .platinum: return "Platinum"
        }
    }
}
