import Loopflow
import SwiftUI

struct MobileNewTaskView: View {
    @State private var model: MobileTaskCreation
    @State private var action: Task<Void, Never>?
    let theme: ThemeDefinition
    let onOpen: (WorkViewPlan.Content) -> Void
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @FocusState private var editing: Bool

    init(model: MobileTaskCreation, theme: ThemeDefinition, onOpen: @escaping (WorkViewPlan.Content) -> Void) {
        _model = State(initialValue: model)
        self.theme = theme
        self.onOpen = onOpen
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("What would you like to work on?")
                            .font(Typography.strong(24))
                            .accessibilityAddTraits(.isHeader)
                        Text("Type or use your keyboard’s microphone to dictate. You can edit before starting.")
                            .font(Typography.body(14))
                            .foregroundStyle(theme.palette.textSecondary)
                        TextEditor(text: Binding(get: { model.draft.text }, set: { model.editText($0) }))
                            .font(Typography.body(17))
                            .scrollContentBackground(.hidden)
                            .frame(minHeight: 170)
                            .padding(8)
                            .background(theme.palette.surface, in: RoundedRectangle(cornerRadius: 12))
                            .overlay(RoundedRectangle(cornerRadius: 12).stroke(theme.palette.border))
                            .focused($editing)
                            .disabled(model.draft.submission != nil || model.storageUnavailable)
                            .accessibilityLabel("Task instructions")
                        if model.instructionsTooLong {
                            Text("Shorten these instructions before starting.")
                                .font(Typography.caption(13)).foregroundStyle(theme.palette.warning)
                        }
                    }
                    destination
                    if let outcome = model.draft.outcome {
                        VStack(alignment: .leading, spacing: 8) {
                            Label(statusTitle(outcome), systemImage: outcome.state == .started ? "checkmark.circle" : "info.circle")
                                .font(Typography.strong(17))
                            if let receipt = outcome.receipt {
                                Text("\(receipt.task.identifier) · \(receipt.task.title)").font(Typography.body(15))
                            }
                            if let message = outcome.message { Text(message).font(Typography.body(14)) }
                        }
                    } else if model.draft.submission != nil {
                        Text("Your request is saved. Check its status before starting anything else.")
                            .font(Typography.body(14))
                    }
                    if let error = model.error {
                        Text(error).font(Typography.body(14)).foregroundStyle(theme.palette.warning)
                            .accessibilityLabel("\(error)")
                    }
                }
                .padding(20)
            }
            .scrollDismissesKeyboard(.interactively)
            .background(theme.palette.background)
            .foregroundStyle(theme.palette.text)
            .safeAreaInset(edge: .bottom) { actions }
            .navigationTitle("New Task")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { if model.saveDraft() || model.storageUnavailable { dismiss() } }
                }
            }
        }
        .tint(theme.palette.accentInk)
        .task { await model.prepare() }
        .task(id: model.draft.text) {
            do { try await Task.sleep(for: .milliseconds(350)); _ = model.saveDraft() }
            catch { /* The next edit or disappearance saves the latest draft. */ }
        }
        .onChange(of: model.draft.directory) { _, _ in _ = model.saveDraft() }
        .onChange(of: scenePhase) { _, phase in if phase != .active { _ = model.saveDraft() } }
        .onDisappear { action?.cancel(); _ = model.saveDraft() }
    }

    private var destination: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("On your Machine").font(Typography.caption(12)).foregroundStyle(theme.palette.textSecondary)
            Label(model.draft.submission?.machineName ?? model.context?.name ?? "Connecting…", systemImage: "desktopcomputer")
                .font(Typography.strong(15))
            if model.draft.submission == nil, let context = model.context {
                if context.repositories.isEmpty {
                    Text("Add a repository in Loopflow on this Machine, then try again.").font(Typography.body(14))
                } else {
                    Picker("Repository", selection: Binding(get: { model.draft.directory }, set: { model.selectRepository($0) })) {
                        Text("Choose a repository").tag("")
                        ForEach(context.repositories.filter { $0.error == nil }) { repository in
                            Text(repository.displayName).tag(repository.path)
                        }
                    }
                    .pickerStyle(.menu)
                    .font(Typography.body(15))
                    if !context.repositories.contains(where: { $0.path == model.draft.directory && $0.error == nil }) {
                        Text("The saved repository is unavailable. Choose a destination.")
                            .font(Typography.body(14))
                    }
                }
            }
            if !model.draft.directory.isEmpty {
                Text(model.draft.submission?.directory ?? model.draft.directory)
                    .font(Typography.caption(12)).foregroundStyle(theme.palette.textSecondary)
                    .textSelection(.enabled)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(theme.palette.surface, in: RoundedRectangle(cornerRadius: 12))
    }

    private var actions: some View {
        VStack(spacing: 8) {
            if model.busy {
                ProgressView(model.draft.submission == nil ? "Loading destinations…" : "Checking your Task…")
                    .frame(minHeight: 44)
            } else if model.settled {
                Button("Open Task") { action = Task { if let task = await model.openTask() { onOpen(task); dismiss() } } }
                    .buttonStyle(.borderedProminent).controlSize(.large)
                    .tint(theme.palette.accent).foregroundStyle(theme.palette.accentText)
            } else if model.draft.submission != nil {
                Button("Check status") { editing = false; action = Task { await submit() } }
                    .buttonStyle(.borderedProminent).controlSize(.large)
                    .tint(theme.palette.accent).foregroundStyle(theme.palette.accentText)
            } else {
                Button("Create & start") { editing = false; action = Task { await submit() } }
                    .buttonStyle(.borderedProminent).controlSize(.large)
                    .tint(theme.palette.accent).foregroundStyle(theme.palette.accentText)
                    .disabled(!model.canCreate)
                if model.context == nil && !model.storageUnavailable {
                    Button("Retry connection") { action = Task { await model.prepare() } }
                }
            }
        }
        .frame(maxWidth: .infinity).padding(16)
        .background(theme.palette.background)
    }

    private func statusTitle(_ outcome: TaskCreateOutcome) -> String {
        switch outcome.state {
        case .inProgress: "Your Task is starting"
        case .started: "Your Task is ready"
        case .failed: "Task created; start needs attention"
        case .unknown: "Task created; start could not be confirmed"
        }
    }

    private func submit() async {
        await model.submit()
        if model.draft.outcome?.state == .started, let task = await model.openTask() {
            onOpen(task)
            dismiss()
        }
    }
}
