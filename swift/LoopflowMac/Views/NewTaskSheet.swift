import Loopflow
import SwiftUI

/// ⌘N: your words, where the Task lives and the workflow it takes up, drawn as
/// its steps with yours named. Start closes the sheet into the Task's
/// conversation and runs `lf task create REQUEST --run`. File runs it without
/// `--run`: the Task lands in that backlog, selected, and nothing starts.
struct NewTaskSheet: View {
    let model: WorkModel
    @Environment(\.palette) private var palette
    @Environment(\.dismiss) private var dismiss
    @State private var request = ""
    /// The Wave it is filed in; nil files it in the repository.
    @State private var waveID: String?
    /// A graph other than the scope's default; nil takes up the default.
    @State private var graph: String?
    @State private var choosingWorkflow = false
    @State private var filing = false
    /// Why File was refused; the sheet stays open with your words.
    @State private var fileError: String?
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
                    .serif(.title)
                    .foregroundStyle(palette.emphasisInk)
                TextField("", text: $request, axis: .vertical)
                    .lineLimit(4...10)
                    .textFieldStyle(.plain)
                    .font(Typography.text)
                    .padding(12)
                    .background(RoundedRectangle(cornerRadius: CornerRadius.pane).fill(palette.background))
                    .overlay(RoundedRectangle(cornerRadius: CornerRadius.pane)
                        .strokeBorder(focused ? palette.accent : palette.borderStrong, lineWidth: 1))
                    .focused($focused)
                    .accessibilityIdentifier("new-task-request")
                selector
                if let workflow {
                    WorkflowSteps(workflow: workflow).accessibilityIdentifier("new-task-steps")
                }
            }
            .padding(22)
            Divider()
            HStack(spacing: 10) {
                if let fileError {
                    Text("Not filed · \(fileError)").font(Typography.text).foregroundStyle(palette.warning)
                        .lineLimit(2).textSelection(.enabled).accessibilityIdentifier("new-task-file-error")
                }
                Spacer()
                Button("Cancel") { dismiss() }
                    .buttonStyle(.plain).foregroundStyle(palette.textSecondary)
                    .keyboardShortcut(.cancelAction)
                Button(filing ? "Filing…" : "File") { file() }
                    .buttonStyle(LoopflowButtonStyle())
                    .disabled(trimmed.isEmpty || filing)
                    .help("Put it in \(scopeName)'s backlog without starting it")
                    .accessibilityIdentifier("new-task-file")
                Button { create() } label: {
                    Text("Start  ⌘↩")
                        .font(Typography.textStrong)
                        .padding(.horizontal, 15).padding(.vertical, 7)
                        .foregroundStyle(palette.accentText)
                        .background(Capsule().fill(palette.accent))
                }
                .buttonStyle(.plain)
                .keyboardShortcut(.return, modifiers: .command)
                .disabled(trimmed.isEmpty)
                .opacity(trimmed.isEmpty ? 0.45 : 1)
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
                WorkflowChooser(flows: StartFlows(model.graphCatalog.value ?? []), defaultName: reported?.name,
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
        .font(Typography.text)
        .foregroundStyle(palette.text)
    }

    private func pickerLabel(_ title: String) -> some View {
        HStack(spacing: 3) {
            Text(title).font(Typography.textStrong)
            Image(systemName: "chevron.down").font(Typography.meta).fontWeight(.semibold)
        }
        .foregroundStyle(palette.text)
        .contentShape(Rectangle())
    }

    private var repositoryName: String {
        model.repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "repository"
    }
    private var scopeName: String { wave?.displayName ?? repositoryName }

    private func create() {
        StartingTaskView.start(model: model, request: trimmed, wave: wave, graph: graph)
    }

    /// File it unstarted. The sheet waits for Rust's answer, so a refusal keeps your words.
    private func file() {
        filing = true
        fileError = nil
        Task {
            do {
                try await model.fileTask(request: trimmed, wave: wave, graph: graph)
                if model.navigation.palette == .newTask { model.navigation.palette = nil }
            } catch { fileError = error.localizedDescription }
            filing = false
        }
    }

    /// Whose default the reported workflow is, named for the scope it follows.
    static func defaultPhrase(_ reported: WaveGraph.Default, scope: String) -> String {
        switch reported.source {
        case "personal": "your default"
        case "repository": "\(scope)'s default"
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
                        .font(Typography.lede)
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
                Text("runs on its own").font(Typography.meta).foregroundStyle(palette.textSecondary).offset(y: 12)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(workflow.steps.isEmpty ? "Runs on its own" : "You take part at " + workflow.steps.map(\.name).joined(separator: ", "))
    }
}

/// The workflows a Task can start on, searchable, laid out as Start's menu is:
/// each with what starting it commits you to and its first sentence.
private struct WorkflowChooser: View {
    let flows: StartFlows
    let defaultName: String?
    let defaultPhrase: String?
    let scope: String
    let chosen: String?
    let choose: (String) -> Void
    @Environment(\.palette) private var palette
    @State private var query = ""

    private func matches(_ flows: [StartFlow]) -> [StartFlow] {
        let needle = query.trimmingCharacters(in: .whitespaces).lowercased()
        return flows.filter { needle.isEmpty || $0.name.lowercased().contains(needle) }
            .sorted { ($0.name == defaultName ? 0 : 1, $0.name) < ($1.name == defaultName ? 0 : 1, $1.name) }
    }

    var body: some View {
        let sections = [(nil, matches(flows.stopping)), ("Run without stopping", matches(flows.unattended)),
                        ("More", matches(flows.more))]
        VStack(alignment: .leading, spacing: 0) {
            if let defaultName {
                Text("New Tasks in \(scope) run \(defaultName). Pick another for this Task only.")
                    .font(Typography.text).foregroundStyle(palette.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(12)
                Divider()
            }
            TextField("Find a workflow", text: $query)
                .textFieldStyle(.plain).font(Typography.text).padding(10)
                .onSubmit { if let first = sections.flatMap(\.1).first { choose(first.name) } }
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(sections, id: \.0) { title, flows in section(title, flows) }
                    if sections.allSatisfy(\.1.isEmpty) {
                        Text("No workflow matches").font(Typography.text).foregroundStyle(palette.textSecondary).padding(9)
                    }
                }.padding(4)
            }
            .frame(maxHeight: 300)
        }
        .frame(width: 340)
    }

    @ViewBuilder private func section(_ title: String?, _ flows: [StartFlow]) -> some View {
        if !flows.isEmpty {
            if let title {
                Text(title).font(Typography.textStrong).foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, 9).padding(.top, 8).padding(.bottom, 2)
            }
            ForEach(flows) { flow in
                Button { choose(flow.name) } label: {
                    VStack(alignment: .leading, spacing: 3) {
                        HStack(spacing: 6) {
                            Text(flow.name).font(Typography.textStrong)
                            Text(flow.consequence).foregroundStyle(palette.textSecondary)
                            if flow.name == defaultName, let defaultPhrase { Text("· \(defaultPhrase)").foregroundStyle(palette.textSecondary) }
                            if flow.name == chosen { Image(systemName: "checkmark").foregroundStyle(palette.emphasisInk) }
                        }.font(Typography.meta)
                        if let summary = flow.summary {
                            Text(summary).font(Typography.meta).foregroundStyle(palette.textSecondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                    .padding(9)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(RoundedRectangle(cornerRadius: CornerRadius.row))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("new-task-workflow-\(flow.name)")
            }
        }
    }
}

/// A New Task between the click and its installed conversation: the words you
/// typed, and the failure if the request could not be filed.
struct StartingTaskView: View {
    let model: WorkModel
    let starting: StartingTask
    @Environment(\.palette) private var palette

    /// The sheet closes at once; the answer opens the Task's conversation.
    @MainActor
    static func start(model: WorkModel, request: String, wave: WaveSnapshot?, graph: String?) {
        let owner = model.navigation.performanceOwner
        let requestID = UUID().uuidString
        Perf.userAction(.startTask, scenario: "new_task_to_typing", id: requestID, repository: model.repoPath,
                        machine: model.machineKind, liveSessions: model.liveSessionCount, owner: owner) {
            Perf.acknowledged(.startTask, id: requestID, owner: owner)
            if model.navigation.palette == .newTask { model.navigation.palette = nil }
            Task {
                do { try await model.createTask(request: request, wave: wave, graph: graph, requestID: requestID) }
                catch { Perf.finish(.startTask, id: requestID, owner: owner, outcome: .failed) }
            }
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            ScrollView {
                Text(starting.request)
                    .font(Typography.text)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            if let error = starting.error {
                Text(error).font(Typography.text).foregroundStyle(palette.warning)
                    .textSelection(.enabled).accessibilityIdentifier("new-task-error")
                HStack {
                    Button("Try again") {
                        Self.start(model: model, request: starting.request, wave: starting.wave, graph: starting.graph)
                    }
                    Button("Discard") { model.navigation.startingTask = nil }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(28)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(palette.background)
        .accessibilityIdentifier("starting-task-conversation")
    }
}
