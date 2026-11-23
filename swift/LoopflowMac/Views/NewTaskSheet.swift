import Loopflow
import SwiftUI

/// The request and Start choice stay together while the sheet is open.
@MainActor @Observable
private final class NewTaskDraft {
    var request = ""
    var waveID: String?
    var graph: String?
    var pending: NewTaskAction?
    var refusal: (action: NewTaskAction, reason: String)?

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
        _draft = State(initialValue: NewTaskDraft(waveID: model.navigation.sidebarWaveID))
    }

    private var wave: WaveSnapshot? { draft.waveID.flatMap { model.wave(id: $0)?.wave } }
    private var destination: WorkNavigation.Board? {
        guard draft.waveID != nil else { return .repository }
        return wave.map { .wave($0.id) }
    }
    private var canSubmit: Bool { !trimmed.isEmpty && destination != nil && draft.pending == nil }
    private var trimmed: String { draft.request.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var scopeDefault: WaveGraph.Default? {
        destination == nil ? nil : model.defaultWorkflow(in: wave)
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
            if destination == nil {
                Text("Choose another Wave or the repository.")
                    .foregroundStyle(palette.textSecondary)
            }
            workflowPicker
                .task(id: destination) {
                    if destination != nil { await model.loadGraphDefault(wave: wave) }
                }
            if let refusal = draft.refusal {
                FailureLine(refusal.action == .file ? "Not filed." : "Not started.", machine: refusal.reason)
                    .accessibilityElement(children: .contain)
                    .accessibilityIdentifier(refusal.action == .file ? "new-task-file-error" : "new-task-error")
            }
            HStack(spacing: Spacing.sm) {
                Button(draft.pending == .file ? "Filing…" : "File to backlog") { submit(.file) }
                    .buttonStyle(LoopflowButtonStyle())
                    .disabled(!canSubmit)
                    .accessibilityIdentifier("new-task-file")
                Text("Choose a workflow when you start.")
                    .foregroundStyle(palette.textSecondary)
            }
            HStack(spacing: Spacing.sm) {
                Spacer()
                Button("Cancel") { dismiss() }
                    .buttonStyle(LoopflowButtonStyle())
                    .keyboardShortcut(.cancelAction)
                Button(startLabel) { submit(.start(graph: draft.graph)) }
                    .buttonStyle(LoopflowButtonStyle(.accent))
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(!canSubmit)
                    .accessibilityIdentifier("new-task-create")
            }
        }
        .font(Typography.text)
        .foregroundStyle(palette.text)
        .disabled(draft.pending != nil)
        .interactiveDismissDisabled(draft.pending != nil)
        .padding(Spacing.xxl)
        .frame(width: 540)
        .background(palette.surface)
        .onAppear { focused = true }
        .task { await model.loadGraphCatalog() }
    }

    private var startLabel: String {
        if case .start = draft.pending { return "Starting…" }
        return startWorkflow.map { "Start with \($0) ⌘↩" } ?? "Start ⌘↩"
    }

    private var scopePicker: some View {
        HStack(spacing: Spacing.xs) {
            Text("In")
            Menu(destination == nil ? "Wave unavailable" : model.scopeName(wave)) {
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
                Menu(startWorkflow ?? (destination == nil ? "Use default" : "…")) {
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

    private func submit(_ action: NewTaskAction) {
        guard canSubmit else { return }
        let request = trimmed
        let wave = self.wave
        let requestID = UUID().uuidString
        let owner = model.navigation.performanceOwner
        draft.pending = action
        draft.refusal = nil
        func send() {
            // Capture the submitting sheet and repository before navigation can change.
            Task.immediate {
                do {
                    try await model.submitTask(action, request: request, wave: wave, requestID: requestID)
                } catch {
                    draft.refusal = (action, error.localizedDescription)
                    draft.pending = nil
                    if case .start = action { Perf.finish(.startTask, id: requestID, owner: owner, outcome: .failed) }
                }
            }
        }
        switch action {
        case .file: send()
        case .start:
            Perf.userAction(.startTask, scenario: "new_task_to_typing", id: requestID, repository: model.repoPath,
                            machine: model.machineKind, liveSessions: model.liveSessionCount, owner: owner) {
                Perf.acknowledged(.startTask, id: requestID, owner: owner)
                send()
            }
        }
    }
}
