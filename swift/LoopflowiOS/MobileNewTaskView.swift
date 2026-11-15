import Loopflow
import SwiftUI

struct MobileNewTaskView: View {
    @State private var model: MobileTaskCreation
    let theme: ThemeDefinition
    let onStart: (CreationOperation) -> Void
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @FocusState private var editing: Bool

    init(model: MobileTaskCreation, theme: ThemeDefinition, onStart: @escaping (CreationOperation) -> Void) {
        _model = State(initialValue: model)
        self.theme = theme
        self.onStart = onStart
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
                            .disabled(model.storageUnavailable)
                            .accessibilityLabel("Task instructions")
                        if model.instructionsTooLong {
                            Text("Shorten these instructions before starting.")
                                .font(Typography.caption(13)).foregroundStyle(theme.palette.warning)
                        }
                    }
                    destination
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
                    Button("Close") {
                        if model.saveDraft() || model.storageUnavailable {
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
        .onDisappear { _ = model.saveDraft() }
    }

    private var destination: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("On your Machine").font(Typography.caption(12)).foregroundStyle(theme.palette.textSecondary)
            Label(model.context?.name
                ?? (model.busy ? "Connecting…" : "Machine unavailable"), systemImage: "desktopcomputer")
                .font(Typography.strong(15))
            if let context = model.context {
                NavigationLink {
                    MobileRepositoryPicker(model: model, theme: theme)
                } label: {
                    HStack(spacing: 12) {
                        Text(context.repositories.first(where: { $0.path == model.draft.directory })?.displayName
                            ?? "Choose a repository")
                            .font(Typography.strong(17))
                            .fixedSize(horizontal: false, vertical: true)
                        Spacer(minLength: 0)
                        Image(systemName: "chevron.right").font(.system(size: 13, weight: .semibold))
                    }
                    .frame(minHeight: 44)
                    .contentShape(Rectangle())
                }
                .disabled(model.busy || model.storageUnavailable)
                .accessibilityHint("Search repositories or refresh destinations on this Machine")
                if !model.draft.directory.isEmpty,
                   !context.repositories.contains(where: { $0.path == model.draft.directory && $0.error == nil }) {
                    Text("The saved repository is unavailable. Choose a destination.")
                        .font(Typography.body(14))
                }
            }
            if !model.draft.directory.isEmpty {
                Text(model.draft.directory)
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
                ProgressView("Loading destinations…").frame(minHeight: 44)
            } else {
                Button("Create & start") {
                    editing = false
                    model.begin(select: onStart)
                }
                .buttonStyle(.borderedProminent).controlSize(.large)
                .tint(theme.palette.accent).foregroundStyle(theme.palette.accentText)
                .disabled(!model.canCreate).accessibilityIdentifier("task-create")
                if model.context == nil && !model.storageUnavailable {
                    Button("Retry connection") { Task { await model.prepare() } }
                }
            }
        }
        .frame(maxWidth: .infinity).padding(16).background(theme.palette.background)
    }
}

/// The route remains the operation even after its receipt binds real identities.
/// Fetching or attaching its content never changes a later route.
struct MobileCreationConversation: View {
    let client: RemoteMobileClient
    let operation: CreationOperation
    let theme: ThemeDefinition
    let onEdit: (CreationOperation) -> Void
    @State private var task: WorkViewPlan.Content?
    @State private var error: String?
    @State private var reply = ""
    @State private var attempt = 0

    var body: some View {
        Group {
            if let task {
                TaskWorkspaceView(client: client, task: task, theme: theme, draft: $reply)
                    .task { try? await client.interaction.presented(operation.id) }
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 20) {
                        Text(operation.request).textSelection(.enabled)
                        if let receipt = operation.receipt {
                            Text("\(receipt.task.identifier) · \(receipt.task.title)")
                        }
                        Text(error ?? operation.error ?? "Starting conversation…")
                        if let evidence = operation.evidence {
                            DisclosureGroup("Receipt") {
                                Text(String(decoding: (try? JSONEncoder().encode(evidence)) ?? Data(), as: UTF8.self))
                                    .textSelection(.enabled)
                            }
                        } else if let legacy = operation.legacyOutcome {
                            DisclosureGroup("Saved outcome") {
                                Text(String(decoding: (try? JSONEncoder().encode(legacy)) ?? Data(), as: UTF8.self))
                                    .textSelection(.enabled)
                            }
                        }
                        ForEach(operation.recovery?.messages ?? [], id: \.self) { Text($0) }
                        if client.interaction.canEditCreation(operation.id) {
                            Button("Edit request") { onEdit(operation) }
                        }
                        if operation.sessionID != nil {
                            Button("Retry opening") { attempt += 1 }
                        }
                        Button("Check status") { client.interaction.retry(operation.id) }
                    }.padding(20).frame(maxWidth: .infinity, alignment: .leading)
                }
                .navigationTitle("New Task")
            }
        }
        .onAppear { PerfClock.interaction(.taskCreate, .contentMounted, id: operation.id) }
        .task(id: "\(operation.sessionID ?? ""):\(operation.taskID ?? ""):\(attempt)") { await open() }
    }

    private func open() async {
        guard let id = operation.taskID, let session = operation.sessionID else { return }
        do {
            guard operation.destination == client.savedMachine.machineID else {
                throw RegistryQueryError("Reconnect the original Machine to open this request")
            }
            let plan = try await client.query.viewPlan(expression: "task:\(id)", repoPath: operation.directory)
            guard let content = try mobileTasks(ProjectedWorkPlan(plan)).first(where: { $0.work.id == id }) else {
                throw RegistryQueryError("The filed Task is not available yet")
            }
            if content.primarySession != session {
                throw RegistryQueryError("The original conversation is not available yet")
            }
            try Task.checkCancellation()
            task = content
            error = nil
        } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
    }
}
