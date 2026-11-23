import Loopflow
import SwiftUI

enum SettingsMatch: Identifiable {
    case pane(SettingsPane)
    case autosave, appearanceMode, theme, mobileAccess
    case setting(ConfigurationDefinition, SettingsPane)

    var pane: SettingsPane {
        switch self {
        case .pane(let pane): pane
        case .autosave: .general
        case .appearanceMode, .theme: .appearance
        case .mobileAccess: .mobile
        case .setting(_, let pane): pane
        }
    }

    var key: String? {
        switch self {
        case .pane: nil
        case .autosave: UserDefaults.taskFileAutosaveKey
        case .appearanceMode: "appearance-mode"
        case .theme: "theme"
        case .mobileAccess: "mobile-access"
        case .setting(let definition, _): definition.key
        }
    }

    var id: String {
        if case .setting(let definition, .project) = self, definition.pane != .project {
            return "project." + definition.key
        }
        return key ?? pane.rawValue
    }

    var title: String {
        switch self {
        case .pane(let pane): pane.rawValue
        case .autosave: "Autosave Task Files"
        case .appearanceMode: "Appearance mode"
        case .theme: "Theme"
        case .mobileAccess: "Mobile access"
        case .setting(let definition, _): definition.title
        }
    }

    var terms: [String] {
        switch self {
        case .pane: [title]
        case .autosave: [title, id, "Save Task files automatically"]
        case .appearanceMode: [title, "System Light Dark"]
        case .theme: [title, "Customize or import a theme"]
        case .mobileAccess: [title, "Tailscale iPhone pairing QR code and Machine address"]
        case .setting(let definition, _):
            [title, definition.key, definition.description, definition.lifecycle, definition.section?.rawValue,
             definition.editedField.map { definition.key + "." + $0 }].compactMap { $0 } + definition.shorthands
        }
    }
}

extension SettingsNavigation {
    func matches(in catalogue: [ConfigurationDefinition]) -> [SettingsMatch] {
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return [] }
        var rows = SettingsPane.allCases.map(SettingsMatch.pane)
        rows += [.autosave, .appearanceMode, .theme, .mobileAccess]
        for definition in catalogue where !definition.isContainer && definition.pane != .appearance {
            rows.append(.setting(definition, definition.pane))
            if definition.pane == .agents { rows.append(.setting(definition, .project)) }
        }
        let found = rows.filter { row in
            var terms = row.terms
            if case .theme = row, let theme = catalogue.first(where: { $0.key == row.key }) {
                terms += SettingsMatch.setting(theme, .appearance).terms
            }
            return terms.contains { $0.localizedCaseInsensitiveContains(query) }
        }
        // A query naming a row's key or title exactly leads, so Return opens that row.
        let named = { (row: SettingsMatch) in
            [row.key, row.title].contains { $0?.caseInsensitiveCompare(query) == .orderedSame }
        }
        return found.filter(named) + found.filter { !named($0) }
    }
}

extension View {
    func settingsForm(highlighted key: String?) -> some View {
        ScrollViewReader { proxy in
            self.formStyle(.grouped)
                .task(id: key) {
                    guard let key else { return }
                    await Task.yield()
                    if !Task.isCancelled { proxy.scrollTo(key, anchor: .center) }
                }
        }
    }

    func settingsRow(_ key: String, highlighted: String?) -> some View {
        background(key == highlighted ? Color.accentColor.opacity(0.18) : .clear,
                   in: RoundedRectangle(cornerRadius: CornerRadius.row))
            .accessibilityAddTraits(key == highlighted ? .isSelected : [])
            .id(key)
    }
}

struct SettingsSearchRow: View {
    let result: SettingsMatch
    let scope: ScopedSettingsModel
    let mobile: MobileSetup
    let notices: NoticeCenter
    let manager: UndoManager?
    let selected: Bool
    let open: () -> Void

    private var booleanToggle: ScopedBooleanToggle? {
        guard case .setting(let definition, _) = result, definition.isSwitch,
              let file = scope.editableFile(for: definition),
              let effective = scope.reading.value?.settings[definition.key] else { return nil }
        return ScopedBooleanToggle(model: scope, definition: definition, effective: effective, file: file,
                                   notices: notices, manager: manager)
    }

    private func destinationLabel(for toggle: ScopedBooleanToggle) -> String {
        let file = toggle.file
        var parts = [file.owner?.title ?? file.path]
        if file.owner != .personalGlobal, let checkout = scope.checkout {
            parts.append(URL(fileURLWithPath: checkout).lastPathComponent)
        }
        if file.owner?.isWave == true, let wave = scope.wave { parts.append(wave) }
        if let key = file.slots[toggle.definition.key]?.key, key != toggle.definition.key { parts.append(key) }
        return parts.joined(separator: " · ")
    }

    var body: some View {
        let toggle = booleanToggle
        HStack {
            Button(action: open) {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text(result.title)
                    if case .setting(let definition, _) = result, definition.lifecycle == "inactive" {
                        Text("Inactive").font(Typography.meta).foregroundStyle(.secondary)
                    }
                    if let toggle {
                        Text(destinationLabel(for: toggle))
                            .font(Typography.meta).foregroundStyle(.secondary)
                            .help(toggle.file.path)
                        if let error = scope.errors[.init(toggle.file, toggle.definition.key)] {
                            Text(error.message).font(Typography.meta).foregroundStyle(.secondary)
                        }
                    } else if result.key != nil {
                        Text(result.pane.rawValue).font(Typography.meta).foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(selected ? .isSelected : [])
            switch result {
            case .autosave:
                AutosaveToggle(notices: notices, undoManager: manager).labelsHidden()
            case .mobileAccess:
                MobileAccessToggle(setup: mobile).labelsHidden()
                    .disabled(mobile.status == nil || mobile.error != nil)
            default:
                if let toggle {
                    toggle.labelsHidden().accessibilityHint(toggle.file.path)
                }
            }
        }
    }
}
