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
            if pane == .project && model.work.checkout == nil {
                Text("Choose a checkout to edit its settings.")
            } else if let scope = model.reading.value {
                if pane == .machines {
                    MachineConnections(location: model.location, declaration: model.file?.slots["machines"]?.declared)
                }
                if case .step = model.work.target, scope.files.allSatisfy({ $0.slots["agent"] == nil }) {
                    Section { Text("This Graph step is read-only. Give it an id: in its Graph YAML to edit step preferences.") }
                }
                let definitions = model.catalogue.filter { definition in
                    definition.panes.contains(pane) && model.work.scope(for: definition) == model.work
                }
                ForEach(SettingsSection.allCases) { group in
                    let rows = group.rows.compactMap { row in definitions.first { $0.key == row.key } }
                    if !rows.isEmpty {
                        Section(group.rawValue) {
                            ForEach(rows, id: \.key) { row($0, in: scope) }
                            if group == .policy, pane == .project, let order = scope.settings["agents"] {
                                SettingProvenance(title: "Effective agent order", value: order.value.settingsText, effective: order)
                            }
                        }
                    }
                }
                if pane == .project {
                    if model.work.wave != nil && model.work.target == .general {
                        Section("Git & Release") {
                            Button("Inspect repository settings") { model.select(checkout: model.work.checkout) }
                        }
                    }
                    let other = definitions.filter { $0.section == nil && !$0.isContainer }
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
                        if pane == .project {
                            Text(file.readPath).font(Typography.mono).foregroundStyle(.secondary).textSelection(.enabled)
                        }
                        Button(pane == .project ? "Open file…" : "Open ~/.lf/config.yaml") {
                            openFile(file)
                        }
                        .help(file.readPath)
                        .settingsRow(highlightedFileKey, highlighted: highlightedKey)
                        if file.readPath != file.path {
                            LabeledContent("First change saves to") {
                                Text(file.path).font(Typography.mono).textSelection(.enabled)
                            }
                        }
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
            if !NSWorkspace.shared.open(url) { throw RegistryQueryError("Could not open \(url.path)") }
        } catch { notices.post(Notice(error.localizedDescription)) }
    }

    @ViewBuilder
    private func row(_ definition: ConfigurationDefinition, in scope: ConfigurationScope) -> some View {
        let key = definition.key
        if let effective = scope.settings[key] {
            if let declaration = model.file?.declaration(key) {
                // Search addresses the outer row by catalog key. The editor's draft
                // belongs to the selected file and its Skill or Graph step.
                VStack(alignment: .leading) {
                    ScopedSettingRow(model: model, definition: definition, effective: effective,
                        declaration: declaration, source: scope.source(key), notices: notices, manager: undoManager)
                        .id(declaration.id)
                }
                .settingsRow(key, highlighted: highlightedKey)
            } else {
                SettingProvenance(title: definition.title, value: definition.displayText(for: effective.value), effective: effective, definition: definition)
                    .settingsRow(key, highlighted: highlightedKey)
                if key == "machine", model.work.target == .general {
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
            LabeledContent("Applies to") {
                Menu(scopeTitle) {
                    ForEach(checkouts, id: \.self) { path in
                        Button(URL(fileURLWithPath: path).lastPathComponent) { model.select(checkout: path) }.help(path)
                    }
                    if let checkout = model.work.checkout {
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
                }
                .fixedSize()
            }
            if model.work.target != .general {
                Text("Showing launch settings for this target. Repository and general settings apply to all work.")
                    .foregroundStyle(.secondary)
                Button("Inspect general work") { model.work.target = .general }
            }
            if let checkout = model.work.checkout {
                let owners = model.reading.value?.files.compactMap(\.owner).filter {
                    $0 != .personalGlobal && $0.isWave == (model.work.wave != nil)
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
        var parts = [model.work.checkout.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Choose checkout"]
        if let wave = model.work.wave { parts.append(waves.first { $0.id == wave }?.displayName ?? wave) }
        switch model.work.target {
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

    var rows: KeyValuePairs<String, String> {
        switch self {
        case .profile:
            ["user": "Display name",
             "repo_root": "Repositories folder"]
        case .updates:
            ["stream": "Release channel",
             "logs": "Log Linear requests"]
        case .policy:
            ["agent": "Primary agent",
             "default_agent": "Starting preference",
             "agents": "Fallback order"]
        case .execution:
            ["yolo": "Run without asking",
             "wave.graph": "Default Graph",
             "auto": "Auto-ship new Tasks",
             "automation": "Automatic retries"]
        case .harnesses:
            ["claude.model": "Claude model",
             "codex.service_tier": "Codex service tier",
             "chrome": "Claude Chrome integration"]
        case .diff:
            ["context.diff": "Include diff in agent context",
             "context.diff_files": "Include full file contents",
             "context.paste": "Include clipboard contents"]
        case .files:
            ["context.docs": "Additional documents",
             "context.exclude": "Excluded documents"]
        case .pullRequests:
            ["land": "Landing method",
             "merge_method": "Merge method",
             "pr": "Create pull requests after pushing"]
        case .diffView: ["diff_hidden": "Hide files from Task diffs"]
        case .release: ["repo.tracker": "Repository tracker"]
        case .placement: ["machine": "Preferred Machines"]
        case .machine: ["hub": "Plan hub"]
        }
    }

    static let byKey = Dictionary(uniqueKeysWithValues: allCases.flatMap { section in
        section.rows.map { ($0.key, (section: section, title: $0.value)) }
    })
}

extension ConfigurationDefinition {
    var section: SettingsSection? { SettingsSection.byKey[key]?.section }
    var pane: SettingsPane {
        if key == "theme" { return .appearance }
        if let section { return section.pane }
        if applicability != "personal" { return .project }
        return category == "Machines" ? .machines : .general
    }
    // Agent preferences appear both as personal defaults and project overrides.
    var panes: [SettingsPane] { pane == .agents ? [.agents, .project] : [pane] }
    var opensFile: Bool { section == nil && (pane == .general || pane == .machines) }
    var isContainer: Bool { ["context", "claude", "codex", "forward"].contains(key) }
    var title: String {
        if let row = SettingsSection.byKey[key] { return row.title }
        return switch key {
        case "context.budgets": "Context budgets"
        case "diff_groups": "Diff groups"
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
