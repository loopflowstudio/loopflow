import Loopflow
import SwiftUI

struct MobileNewTaskView: View {
    @State private var model: MobileTaskCreation
    @State private var action: Task<Void, Never>?
    @Environment(\.theme) private var theme
    let onOpen: (MobileTaskOpening) -> Void
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @FocusState private var editing: Bool

    init(model: MobileTaskCreation, onOpen: @escaping (MobileTaskOpening) -> Void) {
        _model = State(initialValue: model)
        self.onOpen = onOpen
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: Spacing.xxl) {
                    if !model.draft.savedRequests.isEmpty {
                        NavigationLink {
                            MobileSavedRequestsView(model: model, onOpen: onOpen)
                        } label: {
                            Label("Saved requests (\(model.draft.savedRequests.count))", systemImage: "tray")
                                .font(Typography.textStrong)
                                .frame(minHeight: HitTarget.touch)
                        }
                    }
                    VStack(alignment: .leading, spacing: Spacing.md) {
                        Text("What do you want?")
                            .serif(.display)
                            .accessibilityAddTraits(.isHeader)
                        Text("Type or use your keyboard’s microphone to dictate. You can edit before starting.")
                            .font(Typography.meta)
                            .foregroundStyle(theme.palette.textSecondary)
                        TextEditor(text: Binding(get: { model.draft.text }, set: { model.editText($0) }))
                            .font(Typography.lede)
                            .scrollContentBackground(.hidden)
                            .frame(minHeight: 170)
                            .padding(Spacing.sm)
                            .background(theme.palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.pane))
                            .overlay(RoundedRectangle(cornerRadius: CornerRadius.pane).stroke(theme.palette.border))
                            .focused($editing)
                            .disabled(model.draft.submission != nil || model.storageUnavailable)
                            .accessibilityLabel("Task instructions")
                        if model.instructionsTooLong {
                            Text("Shorten these instructions before starting.")
                                .font(Typography.meta).foregroundStyle(theme.palette.warning)
                        }
                    }
                    destination
                    if let outcome = model.draft.outcome {
                        MobileCreationResult(outcome: outcome)
                    } else if model.draft.submission != nil {
                        Text("Your request may already be running. Check its status now or save it for later. Saving does not cancel work on your Machine.")
                            .font(Typography.text)
                    }
                    if let error = model.error {
                        Text(error).font(Typography.text).foregroundStyle(theme.palette.warning)
                            .accessibilityLabel("\(error)")
                    }
                }
                .padding(Spacing.xl)
            }
            .scrollDismissesKeyboard(.interactively)
            .background(theme.palette.background)
            .foregroundStyle(theme.palette.text)
            .safeAreaInset(edge: .bottom) { actions }
            .navigationTitle("New Task")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") {
                        if model.saveDraft() || model.storageUnavailable {
                            action?.cancel()
                            dismiss()
                        }
                    }
                }
            }
        }
        .tint(theme.palette.accentInk)
        .interactiveDismissDisabled(model.hasUnsavedChanges)
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
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("On your Machine").font(Typography.meta).foregroundStyle(theme.palette.textSecondary)
            Label(model.draft.submission?.machineName ?? model.context?.name
                ?? (model.busy ? "Connecting…" : "Machine unavailable"), systemImage: "desktopcomputer")
                .font(Typography.textStrong)
            if model.draft.submission == nil, let context = model.context {
                NavigationLink {
                    MobileRepositoryPicker(model: model)
                } label: {
                    HStack(spacing: Spacing.md) {
                        Text(context.repositories.first(where: { $0.path == model.draft.directory })?.displayName
                            ?? "Choose a repository")
                            .font(Typography.textStrong)
                            .fixedSize(horizontal: false, vertical: true)
                        Spacer(minLength: 0)
                        Image(systemName: "chevron.right").font(Typography.meta).fontWeight(.semibold)
                    }
                    .frame(minHeight: HitTarget.touch)
                    .contentShape(Rectangle())
                }
                .disabled(model.busy || model.storageUnavailable)
                .accessibilityHint("Search repositories or refresh destinations on this Machine")
                if !model.draft.directory.isEmpty,
                   !context.repositories.contains(where: { $0.path == model.draft.directory && $0.error == nil }) {
                    Text("The saved repository is unavailable. Choose a destination.")
                        .font(Typography.text)
                }
            }
            if !model.draft.directory.isEmpty {
                Text(model.draft.submission?.directory ?? model.draft.directory)
                    .font(Typography.meta).foregroundStyle(theme.palette.textSecondary)
                    .textSelection(.enabled)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Spacing.lg)
        .background(theme.palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.pane))
    }

    private var actions: some View {
        VStack(spacing: Spacing.sm) {
            if model.busy {
                ProgressView(model.starting ? "Starting…"
                    : model.draft.submission == nil ? "Loading destinations…" : "Checking your Task…")
                    .frame(minHeight: HitTarget.touch)
            } else if model.settled {
                Button("Open Task") { action = Task { if let task = await model.openTask() { onOpen(task) } } }
                    .buttonStyle(LoopflowButtonStyle(.accent))
            } else if model.draft.submission != nil {
                Button("Check status") { editing = false; action = Task { await submit() } }
                    .buttonStyle(LoopflowButtonStyle(.accent))
            } else {
                Button("Create & start") { editing = false; action = Task { await submit() } }
                    .buttonStyle(LoopflowButtonStyle(.accent))
                    .disabled(!model.canCreate)
                if model.context == nil && !model.storageUnavailable {
                    Button("Retry connection") { action = Task { await model.prepare() } }
                        .buttonStyle(LoopflowButtonStyle())
                }
            }
            if !model.busy && model.draft.submission != nil {
                Button("Save for later") { editing = false; model.saveForLater() }
                    .buttonStyle(LoopflowButtonStyle())
            }
        }
        .frame(maxWidth: .infinity).padding(Spacing.lg)
        .background(theme.palette.background)
    }

    private func submit() async {
        await model.submit()
        if model.draft.outcome?.state == .started, let task = await model.openTask() {
            onOpen(task)
        }
    }
}

private struct MobileCreationResult: View {
    let outcome: MobileTaskOutcome

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Label(Self.title(outcome), systemImage: outcome.state == .started ? "checkmark.circle" : "info.circle")
                .font(Typography.textStrong)
            if let receipt = outcome.receipt {
                Text("\(receipt.task.identifier) · \(receipt.task.title)").font(Typography.text)
            }
            if let message = outcome.message { Text(message).font(Typography.text) }
        }
    }

    static func title(_ outcome: MobileTaskOutcome) -> String {
        switch outcome.state {
        case .inProgress: "Your Task is starting"
        case .started: "Your Task is ready"
        case .failed: "Task created; start needs attention"
        case .unknown: "Task created; start could not be confirmed"
        }
    }
}

struct MobileSavedRequestsView: View {
    let model: MobileTaskCreation
    @Environment(\.theme) private var theme
    let onOpen: (MobileTaskOpening) -> Void

    var body: some View {
        List {
            Section {
                ForEach(model.draft.savedRequests) { request in
                    NavigationLink {
                        MobileSavedTaskRequestView(id: request.id, model: model, onOpen: onOpen)
                    } label: {
                        VStack(alignment: .leading, spacing: Spacing.sm) {
                            Text(request.outcome?.receipt?.task.title ?? request.submission.request)
                                .font(Typography.textStrong).lineLimit(3)
                            Label(request.submission.machineName, systemImage: "desktopcomputer")
                                .font(Typography.meta).foregroundStyle(theme.palette.textSecondary)
                            Text(request.outcome.map(MobileCreationResult.title) ?? "Result not confirmed")
                                .font(Typography.meta).foregroundStyle(theme.palette.textSecondary)
                        }
                        .padding(.vertical, Spacing.xs)
                    }
                    .listRowBackground(theme.palette.surface)
                }
            } footer: {
                Text("Saving a request does not cancel it. Reconnect its original Machine to check the result or open the Task. Your new draft stays separate.")
                    .font(Typography.meta)
            }
        }
        .scrollContentBackground(.hidden)
        .background(theme.palette.background)
        .foregroundStyle(theme.palette.text)
        .navigationTitle("Saved requests")
        .navigationBarTitleDisplayMode(.inline)
    }
}

struct MobileSavedTaskRequestView: View {
    let id: UUID
    let model: MobileTaskCreation
    @Environment(\.theme) private var theme
    let onOpen: (MobileTaskOpening) -> Void
    @State private var action: Task<Void, Never>?

    var body: some View {
        ScrollView {
            if let request = model.savedRequest(id) {
                VStack(alignment: .leading, spacing: Spacing.xl) {
                    Label(request.submission.machineName, systemImage: "desktopcomputer")
                        .font(Typography.textStrong)
                    Text(request.submission.directory)
                        .font(Typography.meta).foregroundStyle(theme.palette.textSecondary)
                        .textSelection(.enabled)
                    Text(request.submission.request).font(Typography.text).textSelection(.enabled)
                    if let outcome = request.outcome {
                        MobileCreationResult(outcome: outcome)
                    } else {
                        Text("The result hasn’t been confirmed. Checking status recovers this exact request on its original Machine.")
                            .font(Typography.text)
                    }
                    if let context = model.context, context.machineID != request.submission.machineID {
                        Text("Reconnect \(request.submission.machineName) to continue. Nothing will be sent to \(context.name).")
                            .font(Typography.meta).foregroundStyle(theme.palette.textSecondary)
                    }
                    if let error = model.error {
                        Text(error).font(Typography.text).foregroundStyle(theme.palette.warning)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading).padding(Spacing.xl)
            }
        }
        .background(theme.palette.background)
        .foregroundStyle(theme.palette.text)
        .navigationTitle("Saved request")
        .navigationBarTitleDisplayMode(.inline)
        .safeAreaInset(edge: .bottom) {
            VStack {
                if model.busy {
                    ProgressView("Checking your Task…").frame(minHeight: HitTarget.touch)
                } else if let request = model.savedRequest(id) {
                    if let outcome = request.outcome, outcome.state != .inProgress {
                        Button("Open Task") { action = Task { await open() } }
                            .buttonStyle(LoopflowButtonStyle(.accent))
                    } else {
                        Button("Check status") {
                            action = Task {
                                await model.submit(savedRequestID: id)
                                if model.savedRequest(id)?.outcome?.state == .started { await open() }
                            }
                        }
                        .buttonStyle(LoopflowButtonStyle(.accent))
                    }
                }
            }
            .frame(maxWidth: .infinity).padding(Spacing.lg).background(theme.palette.background)
        }
        .onAppear { model.clearError() }
        .onDisappear { action?.cancel(); model.clearError() }
    }

    private func open() async {
        if let task = await model.openTask(savedRequestID: id) { onOpen(task) }
    }
}
