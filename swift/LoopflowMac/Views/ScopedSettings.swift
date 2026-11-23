import AppKit
import Loopflow
import SwiftUI

struct ScopedSettings: View {
    @Bindable var model: ScopedSettingsModel
    let pane: SettingsPane
    let checkouts: [String]
    let waves: [Wave]
    let notices: NoticeCenter
    var highlightedKey: String?
    @Environment(\.undoManager) private var undoManager

    var body: some View {
        Form {
            if let error = model.editError ?? model.reading.failure {
                Section {
                    Text(error).textSelection(.enabled)
                    Button("Refresh") { model.refresh() }
                }
            }
            if pane == .general {
                Section("Task Files") {
                    AutosaveToggle(notices: notices, undoManager: undoManager)
                        .settingsRow(SettingsMatch.autosave.id, highlighted: highlightedKey)
                }
            } else {
                Section {
                    Menu {
                        Button("Global") { model.select(checkout: nil) }
                        ForEach(checkouts, id: \.self) { path in
                            Button(path) { model.select(checkout: path) }
                        }
                        if let checkout = model.checkout {
                            let origin = WaveOrigin.resolve(checkout).normalizedFilePath
                            let choices = waves.filter { WaveOrigin.resolve($0.repo).normalizedFilePath == origin }
                            if !choices.isEmpty {
                                Section("Waves") {
                                    ForEach(choices) { wave in
                                        Button(wave.displayName) { model.select(checkout: checkout, wave: wave.id) }
                                    }
                                }
                            }
                        }
                        Button("Choose Checkout…") {
                            let panel = NSOpenPanel()
                            panel.canChooseDirectories = true
                            panel.canChooseFiles = false
                            if panel.runModal() == .OK { model.select(checkout: panel.url?.path) }
                        }
                    } label: {
                        LabeledContent("Applies to") {
                            Text([model.checkout ?? "Global", model.wave.map { id in waves.first { $0.id == id }?.displayName ?? id }]
                                .compactMap { $0 }.joined(separator: " / "))
                        }
                    }
                    if model.checkout != nil {
                        Picker("Save in", selection: $model.destination) {
                            ForEach((model.reading.value?.files.compactMap(\.owner) ?? []).filter { owner in
                                owner == model.destination || (owner != .personalGlobal && owner.isWave == (model.wave != nil))
                            }) { owner in
                                Text(owner.title).tag(owner)
                            }
                        }
                        if let checkout = model.checkout { SettingsTargetPicker(model: model, checkout: checkout).id(checkout) }
                    }
                }
            }
            if pane == .machines { MachineConnections(location: model.location) }

            if let scope = model.reading.value, let file = model.file(for: pane) {
                let definitions = model.catalogue.filter { $0.pane == pane && !$0.isContainer }
                ForEach(definitions, id: \.key) { definition in
                    if let effective = scope.settings[definition.key] {
                        if file.slots[definition.key] != nil {
                            ScopedSettingRow(model: model, definition: definition, effective: effective,
                                file: file, source: scope.source(definition.key), notices: notices, manager: undoManager, highlightedKey: highlightedKey)
                                .id(RowAddress(key: file.slots[definition.key]?.key ?? definition.key, destination: file.destination))
                                .disabled(model.editsDisabled)
                        } else {
                            Section {
                                LabeledContent(definition.title, value: effective.value.settingsText)
                                    .settingsRow(definition.key, highlighted: highlightedKey)
                                ForEach(scope.files.filter { $0.slots[definition.key] != nil }) { other in
                                    if let owner = other.owner {
                                        Button("Edit in \(owner.title)") { model.destination = owner }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if model.reading.isLoading {
                ProgressView("Loading settings")
            }
        }
        .formStyle(.grouped)
    }

    private struct RowAddress: Hashable { let key: String; let destination: JSONValue }
}

extension ConfigurationScope.File {
    func hasOverrides(in pane: SettingsPane, catalog: [ConfigurationDefinition]) -> Bool {
        guard owner != .personalGlobal else { return false }
        return catalog.contains { $0.pane == pane && slots[$0.key]?.entries.isEmpty == false }
    }
}

extension ConfigurationDefinition {
    var pane: SettingsPane {
        if key == "theme" { return .appearance }
        if key.hasPrefix("context") { return .context }
        if category == "Machines" { return .machines }
        if ["Planning", "Release"].contains(category) || ["diff_hidden", "diff_groups"].contains(key) { return .git }
        if ["Desktop", "Loopflow"].contains(category) { return .general }
        return .agents
    }

    var isContainer: Bool { ["context", "claude", "codex", "forward"].contains(key) }
    var title: String {
        switch key {
        case "automation": "Automatic retries"
        case "agent": "Pin"
        case "default_agent": "Starting preference"
        case "agents": "Fallback order"
        case "context.diff": "Include diff in agent context"
        case "context.docs": "Additional documents"
        case "repo.tracker": "Repository tracker"
        default: key.replacingOccurrences(of: "_", with: " ").replacingOccurrences(of: ".", with: " · ").capitalized
        }
    }
}
