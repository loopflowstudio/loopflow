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
            } else if pane == .project {
                scopeSelector
            }
            if pane == .project && model.checkout == nil {
                Text("Choose a checkout to edit its settings.")
            } else if let scope = model.reading.value {
                if pane == .machines {
                    MachineConnections(location: model.location, declaration: model.file?.slots["machines"]?.declared)
                }
                if case .step = model.target, scope.files.allSatisfy({ $0.slots["agent"] == nil }) {
                    Section { Text("This Graph step is read-only. Give it an id: in its Graph YAML to edit step preferences.") }
                }
                ForEach(SettingsSection.allCases.filter { $0.appears(in: pane, wave: model.wave) }) { group in
                    Section(group.rawValue) {
                        ForEach(group.keys, id: \.self) { key in
                            if let definition = model.catalogue.first(where: { $0.key == key }) {
                                row(definition, in: scope)
                            }
                        }
                        if group == .policy, pane == .project, let order = scope.settings["agents"] {
                            SettingProvenance(title: "Effective agent order", value: order.value.settingsText, effective: order)
                        }
                    }
                }
                if pane == .project {
                    if model.wave != nil {
                        Section("Git & Release") {
                            Button("Inspect repository settings") { model.select(checkout: model.checkout) }
                        }
                    }
                    let other = model.catalogue.filter {
                        $0.pane == .project && $0.section == nil && !$0.isContainer &&
                            (model.wave == nil || $0.applicability != "repository")
                    }
                    if !other.isEmpty {
                        Section {
                            DisclosureGroup("Advanced settings", isExpanded: $advanced) {
                                ForEach(other, id: \.key) { row($0, in: scope) }
                            }
                            .onChange(of: highlightedKey, initial: true) { _, key in
                                if other.contains(where: { $0.key == key }) { advanced = true }
                            }
                        }
                    }
                }
                if let file = model.file {
                    Section {
                        Button(pane == .project ? "Open file…" : "Open ~/.lf/config.yaml") {
                            openFile(file)
                        }
                        .help(file.path)
                        .settingsRow(highlightedFileKey, highlighted: highlightedKey)
                    }
                }
            } else if model.reading.isLoading { ProgressView("Loading settings") }
        }
        .settingsForm(highlighted: highlightedKey)
    }

    private var highlightedFileKey: String {
        if let highlightedKey, model.catalogue.contains(where: { $0.key == highlightedKey && $0.opensFile }) {
            return highlightedKey
        }
        return "configuration-file"
    }

    private func openFile(_ file: ConfigurationScope.File) {
        do {
            let url = try file.prepareForOpening()
            if !NSWorkspace.shared.open(url) { throw RegistryQueryError("Could not open \(file.path)") }
        } catch { notices.post(Notice(error.localizedDescription)) }
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
        var parts = [model.checkout.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Choose checkout"]
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
    case diff = "Context", files = "Context Files"
    case pullRequests = "Git & Release", diffView = "Diff View", release = "Planning & Release"
    case placement = "Machine Placement", machine = "Planning"

    var id: Self { self }
    var pane: SettingsPane {
        switch self {
        case .profile, .updates: .general
        case .policy, .execution, .harnesses: .agents
        case .diff, .files, .pullRequests, .diffView, .release, .placement: .project
        case .machine: .machines
        }
    }

    func appears(in selected: SettingsPane, wave: String?) -> Bool {
        guard selected == .project else { return selected == pane }
        switch self {
        case .pullRequests, .diffView, .release: return wave == nil
        default: return pane == .project || pane == .agents
        }
    }

    var keys: [String] {
        switch self {
        case .profile: ["user", "repo_root"]
        case .updates: ["stream", "logs"]
        case .policy: ["agent", "default_agent", "agents"]
        case .execution: ["yolo", "wave.graph", "auto", "automation"]
        case .harnesses: ["claude.model", "codex.service_tier", "chrome"]
        case .diff: ["context.diff", "context.diff_files", "context.paste"]
        case .files: ["context.docs", "context.exclude"]
        case .pullRequests: ["land", "merge_method", "pr"]
        case .diffView: ["diff_hidden"]
        case .release: ["repo.tracker"]
        case .placement: ["machine"]
        case .machine: ["hub"]
        }
    }
}

extension ConfigurationDefinition {
    var section: SettingsSection? { SettingsSection.allCases.first { $0.keys.contains(key) } }
    var pane: SettingsPane {
        if key == "theme" { return .appearance }
        if let section { return section.pane }
        if applicability != "personal" { return .project }
        return category == "Machines" ? .machines : .general
    }
    var opensFile: Bool { section == nil && (pane == .general || pane == .machines) }
    var isContainer: Bool { ["context", "claude", "codex", "forward"].contains(key) }
    var title: String {
        switch key {
        case "user": "Display name"
        case "repo_root": "Repositories folder"
        case "stream": "Release channel"
        case "logs": "Log Linear requests"
        case "agent": "Primary agent"
        case "default_agent": "Starting preference"
        case "agents": "Fallback order"
        case "yolo": "Run without asking"
        case "wave.graph": "Default Graph"
        case "auto": "Auto-ship new Tasks"
        case "automation": "Automatic retries"
        case "claude.model": "Claude model"
        case "codex.service_tier": "Codex service tier"
        case "chrome": "Claude Chrome integration"
        case "hub": "Plan hub"
        case "machine": "Preferred Machines"
        case "context.diff": "Include diff in agent context"
        case "context.diff_files": "Include full file contents"
        case "context.paste": "Include clipboard contents"
        case "context.docs": "Additional documents"
        case "context.exclude": "Excluded documents"
        case "context.budgets": "Context budgets"
        case "land": "Landing method"
        case "merge_method": "Merge method"
        case "pr": "Create pull requests after pushing"
        case "diff_hidden": "Hide files from Task diffs"
        case "diff_groups": "Diff groups"
        case "repo.tracker": "Repository tracker"
        case "release": "Release targets"
        case "session": "Session lifetime"
        case "machines": "Registered Machines"
        case "connections": "Connection addresses"
        case "pools": "Machine pools"
        case "ondemands": "On-demand Machines"
        case "forward.env": "Forward environment variables"
        case "forward.files": "Forward files"
        case "auth_browsers": "Account browser profiles"
        case "portfolio": "Portfolio selection"
        case "supported_harnesses": "Available harnesses"
        case "isolate": "Isolated launches"
        case "summaries": "Session summaries"
        case "summary_tokens": "Summary token budget"
        default: description
        }
    }
}
