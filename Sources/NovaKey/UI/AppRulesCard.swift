// AppRulesCard.swift
// The APPS card in Settings: where Vietnamese composition runs. Three
// modes (All apps / Only these / All except) and, for the two list modes,
// the list of apps with an "Add App…" menu of the apps currently running.
// Apps are identified by bundle identifier; names are resolved for display.

import SwiftUI

struct AppRulesCard: View {
    @Environment(\.novaTheme) private var theme
    @State private var rules: AppRules = AppSettings.shared.appRules
    /// Apps shown in the "Add App…" menu; refreshed when the menu opens.
    @State private var runningApps: [RunningApp] = []

    struct RunningApp: Identifiable, Hashable {
        let id: String   // bundle identifier, normalized
        let name: String
    }

    var body: some View {
        NovaCard(title: "APPS") {
            VStack(alignment: .leading, spacing: 10) {
                NovaSegmented(
                    options: [
                        (label: "All apps", value: AppRuleMode.all),
                        (label: "Only these", value: AppRuleMode.only),
                        (label: "All except", value: AppRuleMode.except),
                    ],
                    selection: $rules.mode
                )
                .onChange(of: rules.mode) { _, _ in save() }

                Text(hint)
                    .font(.caption)
                    .foregroundStyle(theme.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)

                if rules.mode != .all {
                    appList
                    addMenu
                }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .novaKeySettingsChanged)) { _ in
            // The popover's "Vietnamese in <app>" row edits the same rules.
            let stored = AppSettings.shared.appRules
            if stored != rules { rules = stored }
        }
    }

    private var hint: String {
        switch rules.mode {
        case .all: return "Vietnamese typing runs everywhere. Open the menu bar icon in any app to pause it there."
        case .only: return "Vietnamese typing runs only in the apps below; everywhere else NovaKey stays in English."
        case .except: return "Vietnamese typing runs everywhere except the apps below."
        }
    }

    private var appList: some View {
        VStack(spacing: 0) {
            if rules.activeList.isEmpty {
                Text(rules.mode == .only ? "No apps yet — Vietnamese is off everywhere."
                     : "No apps yet — nothing is paused.")
                    .font(.caption)
                    .foregroundStyle(theme.textSecondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 8)
                    .padding(.horizontal, 10)
            }
            ForEach(rules.activeList, id: \.self) { id in
                HStack(spacing: 8) {
                    Text(Self.displayName(for: id))
                        .foregroundStyle(theme.textPrimary)
                        .lineLimit(1)
                    Text(id)
                        .font(.caption)
                        .foregroundStyle(theme.textSecondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer()
                    Button {
                        rules.remove(id)
                        save()
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(theme.textSecondary)
                    }
                    .buttonStyle(.plain)
                    .help("Remove")
                }
                .padding(.vertical, 6)
                .padding(.horizontal, 10)
                if id != rules.activeList.last {
                    Divider().background(theme.divider)
                }
            }
        }
        .background(
            RoundedRectangle(cornerRadius: theme.isRetro ? 0 : 8, style: .continuous)
                .fill(theme.fieldBG)
                .overlay(
                    RoundedRectangle(cornerRadius: theme.isRetro ? 0 : 8, style: .continuous)
                        .stroke(theme.isRetro ? Color.black : Color.clear, lineWidth: 1)
                )
        )
    }

    private var addMenu: some View {
        HStack {
            Menu {
                let candidates = runningApps.filter { !rules.activeList.contains($0.id) }
                if candidates.isEmpty {
                    Text("No other apps are open")
                } else {
                    ForEach(candidates) { app in
                        Button(app.name) {
                            rules.add(app.id)
                            save()
                        }
                    }
                }
            } label: {
                Label("Add App…", systemImage: "plus")
                    .foregroundStyle(theme.textPrimary)
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .onHover { hovering in
                // Refresh the list as the user reaches for the menu.
                if hovering { runningApps = Self.listRunningApps() }
            }
            .onAppear { runningApps = Self.listRunningApps() }
            Text(rules.activeList.count == AppRules.maxApps
                 ? "List is full (\(AppRules.maxApps))."
                 : "Or open the menu bar icon while in the app.")
                .font(.caption)
                .foregroundStyle(theme.textSecondary)
            Spacer()
        }
    }

    private func save() {
        AppSettings.shared.appRules = rules
        NotificationCenter.default.post(name: .novaKeySettingsChanged, object: nil)
    }

    // MARK: - App lookup

    /// The apps the user would see in the Dock / ⌘-Tab, NovaKey excluded,
    /// sorted by name.
    static func listRunningApps() -> [RunningApp] {
        let me = ProcessInfo.processInfo.processIdentifier
        var seen = Set<String>()
        var apps: [RunningApp] = []
        for app in NSWorkspace.shared.runningApplications {
            guard app.activationPolicy == .regular, app.processIdentifier != me,
                  let raw = app.bundleIdentifier,
                  let id = AppRules.normalizeBundleID(raw),
                  seen.insert(id).inserted
            else { continue }
            apps.append(RunningApp(id: id, name: app.localizedName ?? raw))
        }
        return apps.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    /// A display name for a stored bundle identifier: the running app's name,
    /// else the installed app's file name, else the identifier itself.
    static func displayName(for id: String) -> String {
        if let app = NSWorkspace.shared.runningApplications.first(where: {
            $0.bundleIdentifier?.lowercased() == id
        }), let name = app.localizedName {
            return name
        }
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: id) {
            return FileManager.default.displayName(atPath: url.path)
                .replacingOccurrences(of: ".app", with: "")
        }
        return id
    }
}
