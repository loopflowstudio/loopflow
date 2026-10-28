import Loopflow
import SwiftUI

/// ⌘N: your words, where the Task lives and the workflow it takes up, drawn as
/// its steps with yours named. Start runs `lf task create REQUEST --run` and
/// opens the Task's conversation.
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
    @State private var choosingWorkflow = false
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
    private var chosen: String? { graph ?? reported?.name }
    private var workflow: WorkflowDefinition? {
        chosen.flatMap { name in catalog.first { $0.name == name }?.workflow }
    }
    private var trimmed: String { request.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 14) {
                Text("What do you want?")
                    .font(Typography.sectionTitle(28))
                    .foregroundStyle(palette.emphasisInk)
                TextField("", text: $request, axis: .vertical)
                    .lineLimit(4...10)
                    .textFieldStyle(.plain)
                    .font(Typography.body(15))
                    .padding(12)
                    .background(RoundedRectangle(cornerRadius: 12).fill(palette.background))
                    .overlay(RoundedRectangle(cornerRadius: 12)
                        .strokeBorder(focused ? palette.accent : palette.borderStrong, lineWidth: 1))
                    .focused($focused)
                    .accessibilityIdentifier("new-task-request")
                selector
                if let workflow {
                    WorkflowSteps(workflow: workflow).accessibilityIdentifier("new-task-steps")
                }
                if let error {
                    Text(error).font(Typography.body(12)).foregroundStyle(palette.warning)
                        .textSelection(.enabled).accessibilityIdentifier("new-task-error")
                }
            }
            .padding(22)
            Divider()
            HStack(spacing: 10) {
                Spacer()
                Button("Cancel") { dismiss() }
                    .buttonStyle(.plain).foregroundStyle(palette.textSecondary)
                    .keyboardShortcut(.cancelAction)
                Button { create() } label: {
                    Text(creating ? "Starting…" : "Start  ⌘↩")
                        .font(Typography.strong(13))
                        .padding(.horizontal, 15).padding(.vertical, 7)
                        .foregroundStyle(palette.accentText)
                        .background(Capsule().fill(palette.accent))
                }
                .buttonStyle(.plain)
                .keyboardShortcut(.return, modifiers: .command)
                .disabled(trimmed.isEmpty || creating)
                .opacity(trimmed.isEmpty || creating ? 0.45 : 1)
                .accessibilityIdentifier("new-task-create")
            }
            .padding(.horizontal, 22).padding(.vertical, 12)
        }
        .frame(width: 540)
        .background(palette.surface)
        .onAppear { focused = true }
        .task { await model.loadGraphCatalog() }
        .task(id: waveID) { await model.loadGraphDefault(wave: wave) }
    }

    /// "In myapp, running code · myapp's default": where the Task lives and the
    /// workflow it takes up, which follows that scope unless you choose another.
    private var selector: some View {
        HStack(alignment: .center, spacing: 4) {
            Text("In")
            Menu {
                Button(repositoryName) { waveID = nil }
                ForEach(model.visibleRoadmaps, id: \.wave.id) { roadmap in
                    Button(roadmap.wave.displayName) { waveID = roadmap.wave.id }
                }
            } label: {
                pickerLabel(scopeName)
            }
            .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
            .accessibilityIdentifier("new-task-wave")
            Text(", running").padding(.leading, -4)
            Button { choosingWorkflow = true } label: {
                pickerLabel(chosen ?? "…")
            }
            .buttonStyle(.plain).fixedSize()
            .accessibilityIdentifier("new-task-workflow")
            .popover(isPresented: $choosingWorkflow, arrowEdge: .bottom) {
                WorkflowChooser(catalog: catalog, defaultName: reported?.name,
                                defaultPhrase: reported.map { Self.defaultPhrase($0, scope: scopeName) },
                                scope: scopeName, chosen: chosen) { name in
                    graph = name == reported?.name ? nil : name
                    choosingWorkflow = false
                }
            }
            if let graph, let reported, graph != reported.name {
                Text("· not \(reported.name)").foregroundStyle(palette.textSecondary)
                Button { self.graph = nil } label: { Image(systemName: "arrow.uturn.backward") }
                    .buttonStyle(.plain).foregroundStyle(palette.emphasisInk)
                    .help("Use \(Self.defaultPhrase(reported, scope: scopeName)), \(reported.name)")
            } else if let reported {
                Text("· \(Self.defaultPhrase(reported, scope: scopeName))").foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("new-task-default")
            }
        }
        .font(Typography.body(13.5))
        .foregroundStyle(palette.text)
    }

    private func pickerLabel(_ title: String) -> some View {
        HStack(spacing: 3) {
            Text(title).font(Typography.strong(13.5))
            Image(systemName: "chevron.down").font(.system(size: 9, weight: .semibold))
        }
        .foregroundStyle(palette.text)
        .contentShape(Rectangle())
    }

    private var repositoryName: String {
        model.repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "repository"
    }
    private var scopeName: String { wave?.displayName ?? repositoryName }

    private func create() {
        creating = true
        error = nil
        let (request, wave, graph) = (trimmed, wave, graph)
        Task {
            defer { creating = false }
            do { try await model.createTask(request: request, wave: wave, graph: graph) }
            catch { self.error = error.localizedDescription }
        }
    }

    /// Whose default the reported workflow is, named for the scope it follows.
    static func defaultPhrase(_ reported: WaveGraph.Default, scope: String) -> String {
        switch reported.source {
        case "personal", "global": "your default"
        case "wave", "repo": "\(scope)'s default"
        default: "the default"
        }
    }
}

/// A workflow as one line of steps: the runs between unnamed, the steps where
/// you take part named and marked.
struct WorkflowSteps: View {
    let workflow: WorkflowDefinition
    @Environment(\.palette) private var palette

    private struct Step: Identifiable {
        let id: Int
        let you: String?
    }

    private var steps: [Step] {
        let columns = [TaskGraph.start] + workflow.steps.map(\.name) + [TaskGraph.end]
        var out: [Step] = []
        for index in columns.indices.dropLast() {
            if workflow.runs.contains(where: { $0.from == columns[index] && $0.to == columns[index + 1] && !$0.name.isEmpty }) {
                out.append(Step(id: out.count, you: nil))
            }
            if index < workflow.steps.count { out.append(Step(id: out.count, you: workflow.steps[index].name)) }
        }
        return out
    }

    var body: some View {
        let steps = steps
        HStack(alignment: .top, spacing: 0) {
            ForEach(steps) { step in
                VStack(spacing: 5) {
                    Circle()
                        .fill(step.you == nil ? palette.surface : palette.accent)
                        .overlay(Circle().strokeBorder(step.you == nil ? palette.borderActive : .clear, lineWidth: 1.5))
                        .frame(width: step.you == nil ? 9 : 11, height: step.you == nil ? 9 : 11)
                        .padding(.top, step.you == nil ? 1 : 0)
                    Text(step.you ?? " ")
                        .font(Typography.sectionTitle(15))
                        .foregroundStyle(palette.emphasisInk)
                        .fixedSize()
                }
                .frame(width: 11)
                if step.id < steps.count - 1 {
                    Rectangle().fill(palette.borderStrong).frame(height: 1).frame(maxWidth: .infinity).padding(.top, 5)
                }
            }
        }
        .padding(.horizontal, 4)
        .overlay(alignment: .trailing) {
            if workflow.steps.isEmpty {
                Text("runs on its own").font(Typography.caption()).foregroundStyle(palette.textSecondary).offset(y: 12)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(workflow.steps.isEmpty ? "Runs on its own" : "You take part at " + workflow.steps.map(\.name).joined(separator: ", "))
    }
}

/// Every workflow in the catalog, searchable; the scope's default first.
private struct WorkflowChooser: View {
    let catalog: [WorkflowCatalogEntry]
    let defaultName: String?
    let defaultPhrase: String?
    let scope: String
    let chosen: String?
    let choose: (String) -> Void
    @Environment(\.palette) private var palette
    @State private var query = ""

    private var matches: [WorkflowCatalogEntry] {
        let needle = query.trimmingCharacters(in: .whitespaces).lowercased()
        return catalog.filter { needle.isEmpty || $0.name.lowercased().contains(needle) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let defaultName {
                Text("New Tasks in \(scope) run \(defaultName). Pick another for this Task only.")
                    .font(Typography.body(12.5)).foregroundStyle(palette.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(12)
                Divider()
            }
            TextField("Find a workflow", text: $query)
                .textFieldStyle(.plain).font(Typography.body(13)).padding(10)
                .onSubmit { if let first = matches.first { choose(first.name) } }
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    section("Built in", matches.filter { $0.source == nil })
                    section("This repository", matches.filter { $0.source != nil })
                    if matches.isEmpty {
                        Text("No workflow matches").font(Typography.body(12.5)).foregroundStyle(palette.textSecondary).padding(9)
                    }
                }.padding(4)
            }
            .frame(maxHeight: 300)
        }
        .frame(width: 340)
    }

    @ViewBuilder private func section(_ title: String, _ entries: [WorkflowCatalogEntry]) -> some View {
        if !entries.isEmpty {
            Text(title).font(Typography.strong(12.5)).foregroundStyle(palette.textSecondary)
                .padding(.horizontal, 9).padding(.top, 8).padding(.bottom, 2)
            ForEach(entries.sorted { ($0.name == defaultName ? 0 : 1, $0.name) < ($1.name == defaultName ? 0 : 1, $1.name) }) { entry in
                Button { choose(entry.name) } label: {
                    VStack(alignment: .leading, spacing: 6) {
                        HStack(spacing: 6) {
                            Text(entry.name).font(Typography.strong(13))
                            if entry.name == defaultName, let defaultPhrase { Text(defaultPhrase).foregroundStyle(palette.textSecondary) }
                            if entry.name == chosen { Image(systemName: "checkmark").foregroundStyle(palette.emphasisInk) }
                        }.font(Typography.body(11.5))
                        if let workflow = entry.workflow { WorkflowSteps(workflow: workflow).scaleEffect(0.9, anchor: .leading) }
                    }
                    .padding(9)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(RoundedRectangle(cornerRadius: 7))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("new-task-workflow-\(entry.name)")
            }
        }
    }
}
