import Loopflow
import SwiftUI

/// The request and Start choice stay together while the sheet is open.
@MainActor @Observable
private final class NewTaskDraft {
    var request = ""
    var waveID: String?
    var graph: String?
    var filing = false
    var fileError: String?

    init(waveID: String?) { self.waveID = waveID }
}

struct NewTaskSheet: View {
    let model: WorkModel
    @Environment(\.palette) private var palette
    @Environment(\.dismiss) private var dismiss
    @State private var draft: NewTaskDraft
    @FocusState private var focused: Bool

    init(model: WorkModel) {
        self.model = model
        _draft = State(initialValue: NewTaskDraft(waveID: model.sidebarWave?.wave.id))
    }

    private var wave: WaveSnapshot? { draft.waveID.flatMap { model.wave(id: $0)?.wave } }
    private var trimmed: String { draft.request.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var scopeDefault: WaveGraph.Default? {
        model.defaultWorkflow(in: wave)
    }
    private var startWorkflow: String? { draft.graph ?? scopeDefault?.name }
    private var workflowSource: String? {
        if draft.graph != nil { return "chosen for this start" }
        return scopeDefault.map { model.workflowChoice(source: $0.source, wave: wave) }
    }

    var body: some View {
        @Bindable var draft = self.draft
        VStack(alignment: .leading, spacing: Spacing.lg) {
            Text("What do you want?")
                .serif(.display)
                .foregroundStyle(palette.emphasisInk)
            TextField("", text: $draft.request, axis: .vertical)
                .lineLimit(3...10)
                .textFieldStyle(.plain)
                .font(Typography.lede)
                .padding(Spacing.md)
                .background(RoundedRectangle(cornerRadius: CornerRadius.control).fill(palette.background))
                .overlay(RoundedRectangle(cornerRadius: CornerRadius.control)
                    .strokeBorder(focused ? palette.accent : palette.borderStrong, lineWidth: 1))
                .focused($focused)
                .accessibilityIdentifier("new-task-request")
            scopePicker
            workflowPicker
            if let fileError = draft.fileError {
                FailureLine("Not filed.", machine: fileError)
                    .accessibilityElement(children: .contain)
                    .accessibilityIdentifier("new-task-file-error")
            }
            HStack(spacing: Spacing.sm) {
                Button(draft.filing ? "Filing…" : "File to backlog") { Task { await file() } }
                    .buttonStyle(LoopflowButtonStyle())
                    .disabled(trimmed.isEmpty)
                    .accessibilityIdentifier("new-task-file")
                Text("Choose a workflow when you start.")
                    .foregroundStyle(palette.textSecondary)
            }
            HStack(spacing: Spacing.sm) {
                Spacer()
                Button("Cancel") { dismiss() }
                    .buttonStyle(LoopflowButtonStyle())
                    .keyboardShortcut(.cancelAction)
                Button(startWorkflow.map { "Start with \($0) ⌘↩" } ?? "Start ⌘↩") {
                    StartingTaskView.start(model: model, request: trimmed, wave: wave, graph: draft.graph)
                }
                .buttonStyle(LoopflowButtonStyle(.accent))
                .keyboardShortcut(.return, modifiers: .command)
                .disabled(trimmed.isEmpty)
                .accessibilityIdentifier("new-task-create")
            }
        }
        .font(Typography.text)
        .foregroundStyle(palette.text)
        .disabled(draft.filing)
        .padding(Spacing.xxl)
        .frame(width: 540)
        .background(palette.surface)
        .onAppear { focused = true }
        .task { await model.loadGraphCatalog() }
        .task(id: draft.waveID) { await model.loadGraphDefault(wave: wave) }
    }

    private var scopePicker: some View {
        HStack(spacing: Spacing.xs) {
            Text("In")
            Menu(model.scopeName(wave)) {
                Button(model.repositoryName) { draft.waveID = nil }
                ForEach(model.visibleRoadmaps, id: \.wave.id) { roadmap in
                    Button(roadmap.wave.displayName) { draft.waveID = roadmap.wave.id }
                }
            }
            .menuStyle(.borderlessButton).font(Typography.textStrong).fixedSize()
            .accessibilityIdentifier("new-task-wave")
        }
    }

    private var workflowPicker: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Text("Workflow for Start")
                Menu(startWorkflow ?? "…") {
                    Toggle("Use default", isOn: Binding(get: { draft.graph == nil }, set: { _ in draft.graph = nil }))
                    Divider()
                    StartFlowItems(model: model, chosen: draft.graph) { draft.graph = $0 }
                }
                .menuStyle(.borderlessButton).font(Typography.textStrong).fixedSize()
                .accessibilityIdentifier("new-task-workflow")
                if let workflowSource {
                    Text(workflowSource)
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("new-task-default")
                }
            }
            if let outline = model.workflowOutline(startWorkflow) {
                WorkflowLine(outline: outline).accessibilityIdentifier("new-task-steps")
            }
        }
    }

    func file() async {
        guard !draft.filing else { return }
        draft.filing = true
        defer { draft.filing = false }
        draft.fileError = nil
        do {
            try await model.fileTask(request: trimmed, wave: wave)
        } catch { draft.fileError = error.localizedDescription }
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
                    FailureLine("Not started.", machine: error)
                        .accessibilityElement(children: .contain)
                        .accessibilityIdentifier("new-task-error")
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
