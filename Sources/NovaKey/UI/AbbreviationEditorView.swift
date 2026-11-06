// AbbreviationEditorView.swift
// The abbreviation editor (Settings > Abbreviations > Edit List…): a list of
// entries, two text fields and Add/Remove. Every change is saved at once.
//
// The key field takes keys literally while it has the focus, so the stored
// key is exactly what the user presses ("omw", not the "ơm" Telex would make
// of it); the expansion field composes Vietnamese as usual.

import SwiftUI

struct AbbreviationEditorView: View {
    @Environment(\.novaTheme) private var theme
    @State private var table = AbbreviationStore.shared.table
    @State private var selectedKey: String?
    @State private var keyText = ""
    @State private var expansionText = ""
    @State private var status: (text: String, isError: Bool)?
    @FocusState private var focusedField: Field?

    private enum Field { case key, expansion }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Type an abbreviation, then press Tab to replace it with its full text: “btw” → “by the way”. It works in both Vietnamese and English mode.")
                .font(.caption)
                .foregroundStyle(theme.textSecondary)
                .fixedSize(horizontal: false, vertical: true)

            entryList

            HStack(alignment: .top, spacing: 10) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Abbreviation").font(.caption).foregroundStyle(theme.textSecondary)
                    NovaTextField(placeholder: "btw", text: $keyText)
                        .focused($focusedField, equals: .key)
                        .onSubmit { focusedField = .expansion }
                }
                .frame(width: 150)
                VStack(alignment: .leading, spacing: 4) {
                    Text("Expands to").font(.caption).foregroundStyle(theme.textSecondary)
                    NovaTextField(placeholder: "by the way", text: $expansionText)
                        .focused($focusedField, equals: .expansion)
                        .onSubmit(add)
                }
            }

            HStack {
                Text(statusText)
                    .font(.caption)
                    .foregroundStyle(status?.isError == true ? theme.danger : theme.textSecondary)
                    .lineLimit(2)
                Spacer()
                Button("Remove", action: remove)
                    .buttonStyle(NovaButtonStyle())
                    .disabled(selectedKey == nil)
                Button(isReplacing ? "Replace" : "Add", action: add)
                    .buttonStyle(NovaButtonStyle())
                    .keyboardShortcut(.defaultAction)
                    .disabled(keyText.trimmingCharacters(in: .whitespaces).isEmpty
                              || expansionText.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(16)
        .background(theme.windowBG)
        .onChange(of: focusedField) { _, field in
            // Only the key field is literal: the expansion is free text.
            NotificationCenter.default.post(
                name: .novaKeyLiteralInputChanged, object: nil,
                userInfo: ["active": field == .key])
        }
        .onDisappear {
            NotificationCenter.default.post(
                name: .novaKeyLiteralInputChanged, object: nil, userInfo: ["active": false])
        }
        .onAppear { focusedField = .key }
    }

    // MARK: - Pieces

    private var entryList: some View {
        List(selection: $selectedKey) {
            ForEach(table.sortedEntries, id: \.key) { entry in
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(entry.key)
                        .font(.system(.body, design: .monospaced))
                        .frame(width: 120, alignment: .leading)
                    Text(entry.expansion)
                        .lineLimit(1)
                        .truncationMode(.tail)
                }
                .foregroundStyle(theme.textPrimary)
                .tag(entry.key)
            }
        }
        .scrollContentBackground(.hidden)
        .background(theme.fieldBG)
        .frame(minHeight: 160)
        .clipShape(RoundedRectangle(cornerRadius: theme.isRetro ? 0 : 8, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: theme.isRetro ? 0 : 8, style: .continuous)
                .stroke(theme.isRetro ? Color.black : Color.clear, lineWidth: 1)
        )
        .onChange(of: selectedKey) { _, key in
            guard let key, let expansion = table.get(key) else { return }
            keyText = key
            expansionText = expansion
            status = nil
        }
    }

    private var isReplacing: Bool {
        table.get(keyText) != nil
    }

    private var statusText: String {
        if let status { return status.text }
        switch table.count {
        case 0: return "No abbreviations yet"
        case 1: return "1 abbreviation"
        default: return "\(table.count) abbreviations"
        }
    }

    // MARK: - Actions

    private func add() {
        var next = table
        switch next.insert(keyText, expansionText) {
        case nil:
            commit(next)
            status = nil
            selectedKey = nil
            keyText = ""
            expansionText = ""
            focusedField = .key
        case .invalidKey?:
            status = ("Abbreviations are \(Abbreviations.minKeyLength)–\(Abbreviations.maxKeyLength) letters, no spaces, digits or punctuation.", true)
        case .invalidExpansion?:
            status = ("The expansion must be one line of up to \(Abbreviations.maxExpansionLength) characters.", true)
        case .full?:
            status = ("The list is full (\(Abbreviations.maxEntries) entries).", true)
        }
    }

    private func remove() {
        guard let key = selectedKey else { return }
        var next = table
        next.remove(key)
        commit(next)
        selectedKey = nil
        keyText = ""
        expansionText = ""
        status = nil
    }

    private func commit(_ next: Abbreviations) {
        table = next
        if !AbbreviationStore.shared.replace(with: next) {
            status = ("Saved for this session only: the list could not be written to disk.", true)
        }
    }
}

extension Notification.Name {
    /// The abbreviation editor's key field gained (`active: true`) or lost
    /// the focus; while active the event tap passes keys through untouched.
    static let novaKeyLiteralInputChanged = Notification.Name("NovaKeyLiteralInputChanged")
    /// Settings asked for the abbreviation editor window.
    static let novaKeyEditAbbreviationsRequested = Notification.Name("NovaKeyEditAbbreviationsRequested")
}
