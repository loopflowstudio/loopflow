import Loopflow
import SwiftUI

/// The request and Start choice stay together while the sheet is open.
@MainActor @Observable
private final class NewTaskDraft {
    var request = ""
    var scope: CreationWave?
    var replacing: String?
    var unavailableWaves: Set<String> = []
    var graph: String?
    enum Submission: Equatable {
        case editing
        case sending(NewTaskAction)
        case refused(NewTaskAction, String)
    }
    var submission = Submission.editing

    var isSending: Bool {
        if case .sending = submission { return true }
        return false
    }

    init(waveID: String?, recovered: RefusedTaskDraft?) {
        if let recovered {
            request = recovered.request
            scope = recovered.wave
            graph = recovered.graph
            replacing = recovered.id
            refuse(recovered.run ? .start(graph: graph) : .file, error: recovered.refusal)
        } else {
            scope = waveID.map { .existing(id: $0) }
        }
    }

    func refuse(_ action: NewTaskAction, error: Error) {
        let parked = error as? RefusedTaskDraft
        if let parked { replacing = parked.id }
        if let refusal = parked?.refusal ?? (error as? ServerError), refusal.code == .notFound,
           case .object(let detail) = refusal.data, case .string(let id) = detail["wave_id"] {
            unavailableWaves.insert(id)
        }
        submission = .refused(action, error.localizedDescription)
    }
}

struct NewTaskSheet: View {
    let model: WorkModel
    let recovery: CreationRecovery
    @Environment(\.palette) private var palette
    @Environment(\.dismiss) private var dismiss
    @State private var draft: NewTaskDraft
    @FocusState private var focused: Bool

    init(model: WorkModel, recovered: RefusedTaskDraft? = nil, recovery: CreationRecovery = .shared) {
        self.recovery = recovery
        self.model = model
        _draft = State(initialValue: NewTaskDraft(waveID: model.navigation.sidebarWaveID, recovered: recovered))
    }

    private var wave: WaveSnapshot? {
        guard case .existing(let id) = draft.scope else { return nil }
        return model.wave(id: id)?.wave
    }
    private var destination: WorkNavigation.Board? {
        guard let scope = draft.scope else { return .repository }
        guard case .existing(let id) = scope, !draft.unavailableWaves.contains(id) else { return nil }
        return wave.map { .wave($0.id) }
    }
    private var canSubmit: Bool { !trimmed.isEmpty && destination != nil && !draft.isSending }
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
            if case .refused(let action, let reason) = draft.submission {
                FailureLine(action == .file ? "Not filed." : "Not started.", machine: reason)
                    .accessibilityElement(children: .contain)
                    .accessibilityIdentifier(action == .file ? "new-task-file-error" : "new-task-error")
            }
            HStack(spacing: Spacing.sm) {
                Button(draft.submission == .sending(.file) ? "Filing…" : "File to backlog") { submit(.file) }
                    .buttonStyle(LoopflowButtonStyle())
                    .disabled(!canSubmit)
                    .accessibilityIdentifier("new-task-file")
                Text("Choose a workflow when you start.")
                    .foregroundStyle(palette.textSecondary)
            }
            HStack(spacing: Spacing.sm) {
                if let id = draft.replacing {
                    Button("Discard draft", role: .destructive) {
                        Task {
                            do {
                                try await recovery.discardDraft(id)
                                dismiss()
                            } catch {
                                model.navigation.notices.post(Notice("Couldn’t discard draft. \(error.localizedDescription)"))
                            }
                        }
                    }
                    .buttonStyle(LoopflowButtonStyle())
                }
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
        .disabled(draft.isSending)
        .interactiveDismissDisabled(draft.isSending)
        .padding(Spacing.xxl)
        .frame(width: 540)
        .background(palette.surface)
        .onAppear { focused = true }
        .task { await model.loadGraphCatalog() }
    }

    private var startLabel: String {
        if case .sending(.start) = draft.submission { return "Starting…" }
        return startWorkflow.map { "Start with \($0) ⌘↩" } ?? "Start ⌘↩"
    }

    private var scopePicker: some View {
        HStack(spacing: Spacing.xs) {
            Text("In")
            Menu(destination == nil ? "Wave unavailable" : model.scopeName(wave)) {
                Button(model.repositoryName) { draft.scope = nil }
                ForEach(model.visibleRoadmaps, id: \.wave.id) { roadmap in
                    Button(roadmap.wave.displayName) { draft.scope = .existing(id: roadmap.wave.id) }
                        .disabled(draft.unavailableWaves.contains(roadmap.wave.id))
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
        draft.submission = .sending(action)
        let requestID = UUID().uuidString
        switch action {
        case .file:
            send(action, requestID: requestID)
        case .start:
            let owner = model.navigation.performanceOwner
            Perf.userAction(.startTask, scenario: "new_task_to_typing", id: requestID, repository: model.repoPath,
                            machine: model.machineKind, liveSessions: model.liveSessionCount, owner: owner) {
                Perf.acknowledged(.startTask, id: requestID, owner: owner)
                send(action, requestID: requestID)
            }
        }
    }

    private func send(_ action: NewTaskAction, requestID: String) {
        // Begin in this sheet's repository before navigation can change.
        Task.immediate {
            do {
                try await model.submitTask(action, request: trimmed, wave: wave, requestID: requestID,
                                           fileWorkflow: action == .file ? draft.graph : nil, replacing: draft.replacing)
            } catch {
                draft.refuse(action, error: error)
            }
        }
    }
}
