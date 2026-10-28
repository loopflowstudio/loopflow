import Loopflow
import SwiftUI

/// ⌘N: your words, the Wave it is filed in, the workflow it takes up and where
/// that stops for you. Create runs `lf task create REQUEST --run`, the line
/// shown at the bottom, and opens the Task's conversation.
struct NewTaskSheet: View {
    let model: WorkModel
    @Environment(\.palette) private var palette
    @Environment(\.dismiss) private var dismiss
    @State private var request = ""
    /// The Wave it is filed in; nil files it in the repository.
    @State private var waveID: String?
    /// A graph other than the scope's default; nil takes up the default.
    @State private var graph: String?
    @State private var creating = false
    @State private var error: String?
    @FocusState private var focused: Bool

    init(model: WorkModel) {
        self.model = model
        _waveID = State(initialValue: model.sidebarWave?.wave.id)
    }

    private var wave: WaveSnapshot? {
        waveID.flatMap { id in model.visibleRoadmaps.first { $0.wave.id == id }?.wave }
    }
    private var reported: WaveGraph.Default? {
        model.graphDefaults[model.graphDefaultKey(wave: wave)]?.value?.default
    }
    private var catalog: [WorkflowCatalogEntry] {
        (model.workflowCatalog.value ?? []).filter { $0.workflow != nil }
    }
    private var workflow: WorkflowDefinition? {
        (graph ?? reported?.name).flatMap { name in catalog.first { $0.name == name }?.workflow }
    }
    private var trimmed: String { request.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("New Task").font(Typography.strong(15))
            TextField("What do you want? Say it the way you'd say it to a teammate.", text: $request, axis: .vertical)
                .lineLimit(4...10)
                .textFieldStyle(.plain)
                .font(Typography.body(13))
                .padding(10)
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(palette.border))
                .focused($focused)
                .accessibilityIdentifier("new-task-request")
            Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 10) {
                GridRow {
                    Text("Wave").foregroundStyle(palette.textSecondary)
                    Picker("Wave", selection: $waveID) {
                        Text("No Wave (\(repositoryName))").tag(String?.none)
                        ForEach(model.visibleRoadmaps, id: \.wave.id) { roadmap in
                            Text(roadmap.wave.displayName).tag(Optional(roadmap.wave.id))
                        }
                    }
                    .labelsHidden()
                    .accessibilityIdentifier("new-task-wave")
                }
                GridRow(alignment: .firstTextBaseline) {
                    Text("Workflow").foregroundStyle(palette.textSecondary)
                    VStack(alignment: .leading, spacing: 4) {
                        Picker("Workflow", selection: $graph) {
                            Text(Self.defaultLabel(reported)).tag(String?.none)
                            ForEach(catalog.filter { $0.name != reported?.name }) { entry in
                                Text(entry.name).tag(Optional(entry.name))
                            }
                        }
                        .labelsHidden()
                        .accessibilityIdentifier("new-task-workflow")
                        if let stops = Self.stops(workflow) {
                            Text(stops).foregroundStyle(palette.textSecondary)
                                .accessibilityIdentifier("new-task-stops")
                        }
                    }
                }
            }.font(Typography.body(13))
            if let error {
                Text(error).font(Typography.body(12)).foregroundStyle(palette.warning)
                    .textSelection(.enabled).accessibilityIdentifier("new-task-error")
            }
            HStack(spacing: 8) {
                Text(command).font(Typography.code(11)).foregroundStyle(palette.textSecondary)
                    .lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                    .help("The same Task from a terminal")
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button(creating ? "Creating…" : "Create") { create() }
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(trimmed.isEmpty || creating)
                    .accessibilityIdentifier("new-task-create")
            }
        }
        .padding(20)
        .frame(width: 520)
        .onAppear { focused = true }
        .task { await model.loadGraphCatalog() }
        .task(id: waveID) { await model.loadGraphDefault(wave: wave) }
    }

    private var repositoryName: String {
        model.repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "repository"
    }

    /// The terminal line that files and starts the same Task.
    private var command: String {
        let first = trimmed.split(separator: "\n").first.map(String.init) ?? "…"
        var line = "lf task create \"\(first.replacingOccurrences(of: "\"", with: "\\\""))\" --run"
        if let wave { line += " --wave \(wave.name)" }
        if let graph { line += " --graph \(graph)" }
        return line
    }

    private func create() {
        guard !trimmed.isEmpty, !creating else { return }
        creating = true
        error = nil
        let (request, wave, graph) = (trimmed, wave, graph)
        Task {
            defer { creating = false }
            do { try await model.createTask(request: request, wave: wave, graph: graph) }
            catch { self.error = error.localizedDescription }
        }
    }

    static func defaultLabel(_ reported: WaveGraph.Default?) -> String {
        guard let reported else { return "Default" }
        return switch reported.source {
        case "personal": "\(reported.name) · your default"
        case "wave": "\(reported.name) · Wave default"
        case "repo": "\(reported.name) · repository default"
        default: "\(reported.name) · default"
        }
    }

    /// Where `workflow` stops for you, from Rust's compiled definition.
    static func stops(_ workflow: WorkflowDefinition?) -> String? {
        guard let workflow else { return nil }
        let steps = workflow.steps.map(\.name)
        return steps.isEmpty ? "runs to its end without stopping" : "stops for you at \(steps.joined(separator: ", "))"
    }
}
