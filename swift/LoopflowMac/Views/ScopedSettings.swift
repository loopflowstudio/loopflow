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
    @State private var advanced = false

    var body: some View {
        Form {
            if let error = model.reading.failure {
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
                scopeSelector
            }
            if let scope = model.reading.value {
                if pane == .machines {
                    MachineConnections(location: model.location,
                        declaration: scope.files.first { $0.owner == .personalGlobal }?.slots["machines"]?.declared)
                }
                if pane == .git, model.wave != nil {
                    Section {
                        Text("Git & Release settings apply to the repository as a whole.")
                        Button("Inspect repository settings") { model.select(checkout: model.checkout) }
                    }
                } else {
                    if case .step = model.target, pane != .general,
                       scope.files.allSatisfy({ $0.slots["agent"] == nil }) {
                        Section { Text("This Graph step is read-only. Give it an id: in its Graph YAML to edit step preferences.") }
                    }
                    ForEach(SettingsSection.allCases.filter { $0.pane == pane }) { group in
                        Section(group.rawValue) {
                            ForEach(group.keys, id: \.self) { key in
                                if let definition = model.catalogue.first(where: { $0.key == key }) {
                                    row(definition, in: scope)
                                }
                            }
                            if group == .policy, let order = scope.settings["agents"] {
                                SettingProvenance(title: "Effective agent order", value: order.value.settingsText, effective: order)
                            }
                        }
                    }
                    let other = model.catalogue.filter { $0.pane == pane && $0.isAdvanced }
                    if !other.isEmpty {
                        Section {
                            DisclosureGroup("Advanced", isExpanded: Binding(
                                get: { advanced || other.contains { $0.key == highlightedKey } },
                                set: { advanced = $0 }
                            )) {
                                ForEach(other, id: \.key) { row($0, in: scope) }
                            }
                            .onChange(of: highlightedKey, initial: true) { _, key in
                                if other.contains(where: { $0.key == key }) { advanced = true }
                            }
                        }
                    }
                }
            } else if model.reading.isLoading { ProgressView("Loading settings") }
        }
        .settingsForm(highlighted: highlightedKey)
    }

    @ViewBuilder
    private func row(_ definition: ConfigurationDefinition, in scope: ConfigurationScope) -> some View {
        let key = definition.key
        if let effective = scope.settings[key] {
            if let file = model.editableFile(for: definition) {
                // Search addresses the outer row by catalog key. The editor's draft
                // belongs to the selected file and its Skill or Graph step.
                VStack(alignment: .leading) {
                    ScopedSettingRow(model: model, definition: definition, effective: effective,
                        file: file, source: scope.source(key), notices: notices, manager: undoManager)
                        .id(ScopedSettingsModel.RowAddress(file, key))
                }
                .settingsRow(key, highlighted: highlightedKey)
            } else {
                SettingProvenance(title: definition.title, value: effective.value.settingsText, effective: effective)
                    .settingsRow(key, highlighted: highlightedKey)
                if key == "machine", model.target == .general {
                    Text("Machine placement is a personal preference.").foregroundStyle(.secondary)
                    if let owner = scope.files.first(where: { $0.owner?.isWave == model.destination.isWave &&
                        $0.owner != .personalGlobal && $0.slots[key] != nil })?.owner {
                        Button("Switch Save in to \(owner.title)") { model.destination = owner }
                    }
                }
            }
        }
    }

    private var scopeSelector: some View {
        Section {
            Menu {
                Button("Global") { model.select(checkout: nil) }
                ForEach(checkouts, id: \.self) { path in
                    Button(URL(fileURLWithPath: path).lastPathComponent) { model.select(checkout: path) }.help(path)
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
                LabeledContent("Applies to", value: scopeTitle)
            }
            if model.target != .general {
                Button("Inspect general work") { model.target = .general }
            }
            if let checkout = model.checkout {
                let owners = model.reading.value?.files.compactMap(\.owner).filter {
                    $0 != .personalGlobal && $0.isWave == (model.wave != nil)
                } ?? []
                if !owners.isEmpty {
                    Picker("Save in", selection: $model.destination) {
                        ForEach(owners) { Text($0.title).tag($0) }
                    }
                }
                SettingsTargetPicker(model: model, checkout: checkout).id(checkout)
            }
        }
    }

    private var scopeTitle: String {
        var parts = [model.checkout.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Global"]
        if let wave = model.wave { parts.append(waves.first { $0.id == wave }?.displayName ?? wave) }
        switch model.target {
        case .general: break
        case .skill(let name): parts.append("Skill \(name)")
        case .step(let graph, let step): parts.append("\(graph) / \(step)")
        }
        return parts.joined(separator: " · ")
    }
}

enum SettingsSection: String, CaseIterable, Identifiable {
    case profile = "Profile & Workspace", updates = "Updates & Diagnostics"
    case policy = "Agent Policy", execution = "Execution", harnesses = "Harnesses"
    case diff = "Diff & Clipboard", files = "Files & Budgets"
    case pullRequests = "Git & Pull Requests", diffView = "Diff View", release = "Planning & Release"
    case placement = "Placement", machine = "This Machine (Global)"

    var id: Self { self }
    var pane: SettingsPane {
        switch self {
        case .profile, .updates: .general
        case .policy, .execution, .harnesses: .agents
        case .diff, .files: .context
        case .pullRequests, .diffView, .release: .git
        case .placement, .machine: .machines
        }
    }

    var keys: [String] {
        switch self {
        case .profile: ["user", "repo_root"]
        case .updates: ["stream", "logs", "session"]
        case .policy: ["agent", "default_agent", "agents"]
        case .execution: ["yolo", "wave.graph", "auto", "automation", "supported_harnesses"]
        case .harnesses: ["claude.model", "codex.service_tier", "chrome"]
        case .diff: ["context.diff", "context.diff_files", "context.paste"]
        case .files: ["context.docs", "context.exclude", "context.budgets"]
        case .pullRequests: ["land", "merge_method", "pr"]
        case .diffView: ["diff_hidden", "diff_groups"]
        case .release: ["repo.tracker", "release"]
        case .placement: ["machine"]
        case .machine: ["hub", "machines", "connections", "pools", "ondemands", "forward.env", "forward.files"]
        }
    }
}

extension ConfigurationScope.File {
    func hasOverrides(in pane: SettingsPane, catalog: [ConfigurationDefinition]) -> Bool {
        guard owner != .personalGlobal else { return false }
        return catalog.contains { $0.pane == pane && slots[$0.key]?.entries.isEmpty == false }
    }
}

extension ConfigurationDefinition {
    var section: SettingsSection? { SettingsSection.allCases.first { $0.keys.contains(key) } }
    var pane: SettingsPane {
        if key == "theme" { return .appearance }
        if let section { return section.pane }
        return category == "Agents" ? .agents : .general
    }
    var isAdvanced: Bool { section == nil && !isContainer && pane != .appearance }
    var isContainer: Bool { ["context", "claude", "codex", "forward"].contains(key) }
    var title: String {
        switch key {
        case "automation": "Automatic retries"
        case "agent": "Pin"
        case "default_agent": "Starting preference"
        case "agents": "Fallback order"
        case "user": "Display name"
        case "logs": "Log Linear requests"
        case "context.diff": "Include diff in agent context"
        case "context.docs": "Additional documents"
        case "repo.tracker": "Repository tracker"
        default: key.replacingOccurrences(of: "_", with: " ").replacingOccurrences(of: ".", with: " · ").capitalized
        }
    }
}
