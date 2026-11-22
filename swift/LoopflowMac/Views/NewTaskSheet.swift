import Loopflow
import SwiftUI

/// ⌘N: your words, where the Task lives and the workflow it takes up, drawn as
/// the line the Task shows later. Start closes the sheet into the Task's
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
    @State private var filing = false
    /// Why File was refused; the sheet stays open with your words.
    @State private var fileError: String?
    @FocusState private var focused: Bool

    init(model: WorkModel) {
        self.model = model
        _waveID = State(initialValue: model.sidebarWave?.wave.id)
    }

    private var wave: WaveSnapshot? { NewTaskWorkflow.wave(waveID, in: model) }
    private var trimmed: String { request.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            Text("What do you want?")
                .serif(.display)
                .foregroundStyle(palette.emphasisInk)
            TextField("", text: $request, axis: .vertical)
                .lineLimit(3...10)
                .textFieldStyle(.plain)
                .font(Typography.lede)
                .padding(Spacing.md)
                .background(RoundedRectangle(cornerRadius: CornerRadius.control).fill(palette.background))
                .overlay(RoundedRectangle(cornerRadius: CornerRadius.control)
                    .strokeBorder(focused ? palette.accent : palette.borderStrong, lineWidth: 1))
                .focused($focused)
                .accessibilityIdentifier("new-task-request")
            NewTaskWorkflow(model: model, waveID: $waveID, graph: $graph)
            if let fileError {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                    Text("Not filed.").font(Typography.textStrong).foregroundStyle(palette.failure).fixedSize()
                    Text(fileError).font(Typography.mono).foregroundStyle(palette.textSecondary)
                        .textSelection(.enabled).accessibilityIdentifier("new-task-file-error")
                }
            }
            HStack(spacing: Spacing.sm) {
                Spacer()
                Button("Cancel") { dismiss() }
                    .buttonStyle(LoopflowButtonStyle())
                    .keyboardShortcut(.cancelAction)
                Button(filing ? "Filing…" : "File") { file() }
                    .buttonStyle(LoopflowButtonStyle())
                    .disabled(trimmed.isEmpty || filing)
                    .help("Put it in \(model.scopeName(wave))'s backlog without starting it")
                    .accessibilityIdentifier("new-task-file")
                Button("Start ⌘↩") { create() }
                    .buttonStyle(LoopflowButtonStyle(.accent))
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(trimmed.isEmpty)
                    .accessibilityIdentifier("new-task-create")
            }
            .padding(.top, Spacing.sm)
        }
        .padding(Spacing.xxl)
        .frame(width: 540)
        .background(palette.surface)
        .onAppear { focused = true }
        .task { await model.loadGraphCatalog() }
        .task(id: waveID) { await model.loadGraphDefault(wave: wave) }
    }

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
}

/// "In myapp, running code, myapp's default": where the Task lives and the
/// workflow it takes up, which follows that scope unless you choose another,
/// then that workflow as the Task shows it later.
struct NewTaskWorkflow: View {
    let model: WorkModel
    @Binding var waveID: String?
    /// A graph other than the scope's default; nil takes up the default.
    @Binding var graph: String?
    @Environment(\.palette) private var palette

    static func wave(_ id: String?, in model: WorkModel) -> WaveSnapshot? {
        id.flatMap { id in model.visibleRoadmaps.first { $0.wave.id == id }?.wave }
    }

    private var wave: WaveSnapshot? { Self.wave(waveID, in: model) }
    private var scopeName: String { model.scopeName(wave) }
    private var reported: WaveGraph.Default? {
        model.graphDefaults[model.graphDefaultKey(wave: wave)]?.value?.default
    }
    private var chosen: String? { graph ?? reported?.name }
    /// Whose choice the workflow is; nil until the scope's default is read.
    private var whose: String? {
        if graph == nil, reported == nil { return nil }
        return model.workflowChoice(source: chosen == reported?.name ? reported?.source : nil, wave: wave)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                Text("In")
                Menu(scopeName) {
                    Button(model.repositoryName) { waveID = nil }
                    ForEach(model.visibleRoadmaps, id: \.wave.id) { roadmap in
                        Button(roadmap.wave.displayName) { waveID = roadmap.wave.id }
                    }
                }
                .menuStyle(.borderlessButton).font(Typography.textStrong).fixedSize()
                .accessibilityIdentifier("new-task-wave")
                Text(", running").padding(.leading, -Spacing.xs)
                Menu(chosen ?? "…") {
                    StartFlowItems(model: model, chosen: chosen) { name in
                        graph = name == reported?.name ? nil : name
                    }
                }
                .menuStyle(.borderlessButton).font(Typography.textStrong).fixedSize()
                .accessibilityIdentifier("new-task-workflow")
                if let whose {
                    Text(", \(whose)").padding(.leading, -Spacing.xs)
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("new-task-default")
                }
            }
            .font(Typography.text)
            .foregroundStyle(palette.text)
            if let outline = model.workflowOutline(chosen) {
                WorkflowLine(outline: outline).accessibilityIdentifier("new-task-steps")
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
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text(starting.request)
                    .font(Typography.lede)
                    .textSelection(.enabled)
                if let error = starting.error {
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                        Text("Not started.").font(Typography.textStrong).foregroundStyle(palette.failure).fixedSize()
                        Text(error).font(Typography.mono).foregroundStyle(palette.textSecondary)
                            .textSelection(.enabled).accessibilityIdentifier("new-task-error")
                    }
                    HStack(spacing: Spacing.sm) {
                        Button("Try again") {
                            Self.start(model: model, request: starting.request, wave: starting.wave, graph: starting.graph)
                        }
                        .buttonStyle(LoopflowButtonStyle(.accent))
                        Button("Discard") { model.navigation.startingTask = nil }
                            .buttonStyle(LoopflowButtonStyle())
                    }
                    .padding(.top, Spacing.sm)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Spacing.xxxl)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(palette.background)
        .accessibilityIdentifier("starting-task-conversation")
    }
}
