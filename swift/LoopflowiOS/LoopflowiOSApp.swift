import Loopflow
import SwiftUI

@main
struct LoopflowiOSApp: App {
    private let fixture = ProcessInfo.processInfo.arguments.contains("--fixture")

    init() { bootstrapLoopflowApp() }

    var body: some Scene {
        WindowGroup {
            if fixture {
                MobileTaskList(client: FixtureClient())
            } else {
                MachineConnectionView()
            }
        }
    }
}

struct MobileTaskList: View {
    @State private var workspace: MobileWorkspace
    @State private var search = ""
    @State private var listVisible = false
    @State private var creating = false
    @State private var createdTask: WorkViewPlan.Content?
    @State private var showCreatedTask = false
    private let creationClient: (any MobileTaskCreationClient)?
    private let connectionLabel: String
    private let onForget: (() -> Void)?
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.scenePhase) private var scenePhase
    private var theme: ThemeDefinition { colorScheme == .dark ? .dark : .light }
    private var visibleTasks: [WorkViewPlan.Content] {
        workspace.tasks.filter { $0.matchesMobileSearch(search) }
    }

    init(client: any MobileClient, connectionLabel: String = "Offline fixture", onForget: (() -> Void)? = nil) {
        _workspace = State(initialValue: MobileWorkspace(client: client))
        creationClient = client as? any MobileTaskCreationClient
        self.connectionLabel = connectionLabel
        self.onForget = onForget
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    if let error = workspace.tasksError {
                        VStack(alignment: .leading, spacing: 8) {
                            Label("Couldn’t update Tasks", systemImage: "wifi.exclamationmark")
                                .font(Typography.strong(15))
                            Text(error).font(Typography.caption(13))
                            if !workspace.tasks.isEmpty {
                                Text("Showing the last received Tasks.").font(Typography.caption(13))
                            }
                            Button("Try again") { Task { await workspace.refresh() } }
                                .disabled(workspace.isLoadingTasks)
                        }
                        .foregroundStyle(theme.palette.textSecondary)
                        .listRowBackground(theme.palette.surface)
                    }
                    ForEach(visibleTasks, id: \.work) { task in
                        NavigationLink {
                            MobileSessionView(task: task, workspace: workspace, theme: theme, connectionLabel: connectionLabel)
                        } label: {
                            MobileTaskRow(task: task, theme: theme)
                        }
                        .listRowInsets(EdgeInsets(top: 12, leading: 20, bottom: 12, trailing: 16))
                        .listRowBackground(theme.palette.background)
                        .accessibilityHint(task.primarySession == nil
                            ? "Show why this Task has no Session"
                            : "Open the existing Session terminal")
                    }
                } header: {
                    HStack {
                        Text("Working Tasks")
                        Spacer()
                        Text("\(workspace.tasks.count)")
                            .monospacedDigit()
                    }
                    .font(Typography.caption(12))
                    .foregroundStyle(theme.palette.textSecondary)
                    .textCase(nil)
                    .accessibilityElement(children: .combine)
                } footer: {
                    Label(connectionLabel, systemImage: "iphone")
                        .font(Typography.caption(12))
                        .foregroundStyle(theme.palette.textSecondary)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.top, 16)
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .scrollDismissesKeyboard(.interactively)
            .background(theme.palette.background)
            .overlay {
                if workspace.isLoadingTasks && workspace.tasks.isEmpty {
                    ProgressView("Loading Tasks…")
                } else if visibleTasks.isEmpty && workspace.tasksError == nil {
                    if search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        ContentUnavailableView("No open Tasks", systemImage: "checkmark.circle",
                            description: Text("Your open Tasks will appear here."))
                    } else {
                        ContentUnavailableView.search(text: search)
                    }
                }
            }
            .refreshable { await workspace.refresh() }
            .onAppear { listVisible = true }
            .onDisappear { listVisible = false; workspace.stopTaskUpdates() }
            .searchable(text: $search, prompt: "Find a Task")
            .navigationTitle("Tasks")
            .navigationBarTitleDisplayMode(.large)
            .navigationDestination(isPresented: $showCreatedTask) {
                if let createdTask {
                    MobileSessionView(task: createdTask, workspace: workspace, theme: theme, connectionLabel: connectionLabel)
                }
            }
            .sheet(isPresented: $creating, onDismiss: {
                if createdTask != nil { showCreatedTask = true }
            }) {
                if let creationClient {
                    MobileNewTaskView(model: MobileTaskCreation(client: creationClient), theme: theme) { createdTask = $0 }
                }
            }
            .toolbar {
                if creationClient != nil {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button { createdTask = nil; creating = true } label: { Image(systemName: "plus") }
                            .accessibilityLabel("New Task")
                    }
                }
                if let onForget {
                    ToolbarItem(placement: .topBarTrailing) {
                        Menu {
                            Text(connectionLabel)
                            Button("Forget Machine", role: .destructive) {
                                workspace.stopTaskUpdates()
                                workspace.leave()
                                onForget()
                            }
                        } label: { Image(systemName: "network") }
                        .accessibilityLabel("Machine connection")
                    }
                }
            }
        }
        .tint(theme.palette.accentInk)
        .environment(\.palette, theme.palette)
        .task(id: listVisible && scenePhase == .active) {
            if listVisible && scenePhase == .active { await workspace.startTaskUpdates() }
            else { workspace.stopTaskUpdates() }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background { workspace.suspend() }
        }
        .task(id: scenePhase == .background) {
            if scenePhase != .background { await workspace.restoreForegroundSession() }
        }
    }
}

private struct MobileTaskRow: View {
    let task: WorkViewPlan.Content
    let theme: ThemeDefinition

    private var statusColor: Color {
        if task.primarySession == nil || !task.unavailable.isEmpty { return theme.palette.textSecondary }
        if task.states.contains(.blocked) { return theme.palette.warning }
        if task.states.contains(.busy) { return theme.palette.working }
        return theme.palette.available
    }

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            RoundedRectangle(cornerRadius: 2)
                .fill(statusColor)
                .frame(width: 3)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 7) {
                Text(task.title)
                    .font(Typography.strong(17))
                    .foregroundStyle(theme.palette.text)
                    .fixedSize(horizontal: false, vertical: true)
                Label(task.mobileStatus, systemImage: task.mobileStatusSymbol)
                    .font(Typography.caption(13))
                    .foregroundStyle(statusColor)
                    .fixedSize(horizontal: false, vertical: true)
                if let location = task.location {
                    Text(location.cwd)
                        .font(Typography.caption(12))
                        .foregroundStyle(theme.palette.textSecondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }
}

struct MobileSessionView: View {
    let task: WorkViewPlan.Content
    let workspace: MobileWorkspace
    let theme: ThemeDefinition
    var connectionLabel: String = "Offline fixture"
    @Environment(\.dynamicTypeSize) private var textSize

    var body: some View {
        VStack(spacing: 0) {
            if case .unavailable(let reason) = workspace.state {
                ContentUnavailableView("Session unavailable", systemImage: "terminal",
                    description: Text(reason))
            } else {
                header
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                Divider().overlay(theme.palette.border)
                SessionTerminal(workspace: workspace, theme: theme)
            }
        }
        .background(theme.palette.background)
        .foregroundStyle(theme.palette.text)
        .navigationTitle(task.title)
        .navigationBarTitleDisplayMode(.inline)
        .task { await workspace.open(task) }
        .onDisappear { workspace.leave() }
    }

    private var header: some View {
        let layout = textSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 8))
            : AnyLayout(HStackLayout(alignment: .center, spacing: 12))
        return layout {
            VStack(alignment: .leading, spacing: 3) {
                Text(workspace.state.label)
                    .font(Typography.body(15))
                    .foregroundStyle(theme.palette.text)
                    .fixedSize(horizontal: false, vertical: true)
                Text(connectionLabel)
                    .font(Typography.caption(12))
                    .foregroundStyle(theme.palette.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if !textSize.isAccessibilitySize { Spacer(minLength: 0) }
            if workspace.state == .loading || workspace.state == .requestingControl {
                ProgressView().accessibilityLabel("Opening Session")
            }
            if workspace.state == .viewing {
                Button { workspace.requestControl() } label: { actionLabel("Use keyboard") }
            }
            if case .disconnected = workspace.state {
                Button { Task { await workspace.reconnect() } } label: { actionLabel("Reconnect") }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func actionLabel(_ title: String) -> some View {
        Text(title)
            .font(Typography.strong(14))
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 8)
            .frame(minHeight: 44)
            .contentShape(Rectangle())
    }
}
