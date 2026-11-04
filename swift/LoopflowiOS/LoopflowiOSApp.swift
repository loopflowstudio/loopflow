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
    private let onChangeAddress: (() -> Void)?
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.scenePhase) private var scenePhase
    private var theme: ThemeDefinition { colorScheme == .dark ? .dark : .light }
    private var visibleTasks: [WorkViewPlan.Content] {
        workspace.tasks.filter { $0.matchesMobileSearch(search) }
    }

    init(client: any MobileClient, connectionLabel: String = "Offline fixture", onForget: (() -> Void)? = nil,
         onChangeAddress: (() -> Void)? = nil) {
        _workspace = State(initialValue: MobileWorkspace(client: client))
        creationClient = client as? any MobileTaskCreationClient
        self.connectionLabel = connectionLabel
        self.onForget = onForget
        self.onChangeAddress = onChangeAddress
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
                            if let onChangeAddress {
                                Button("Change address", action: onChangeAddress)
                            }
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
    @Environment(\.verticalSizeClass) private var verticalSize
    @State private var opening: Task<Void, Never>?
    @State private var showingSessions = false
    @State private var chosenSession: SessionRecord?
    @State private var showingDetails = false

    private var compact: Bool { verticalSize == .compact }

    var body: some View {
        VStack(spacing: 0) {
            if case .unavailable(let reason) = workspace.state {
                ContentUnavailableView("Session unavailable", systemImage: "terminal",
                    description: Text(reason))
            } else if workspace.state == .notRunning {
                ContentUnavailableView {
                    Label("Continue this Session", systemImage: "terminal")
                } description: {
                    Text("Its conversation is saved on your Machine. Open it to continue where you left off.")
                } actions: {
                    if workspace.canStartSession {
                        openSessionButton
                            .buttonStyle(.borderedProminent)
                            .tint(theme.palette.accent)
                            .foregroundStyle(theme.palette.accentText)
                    }
                }
            } else {
                if !compact {
                    header
                        .padding(.horizontal, 16)
                        .padding(.vertical, 8)
                    Divider().overlay(theme.palette.border)
                }
                SessionTerminal(workspace: workspace, theme: theme)
            }
        }
        .background(theme.palette.background)
        .foregroundStyle(theme.palette.text)
        .navigationTitle(task.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if compact {
                ToolbarItem(placement: .principal) {
                    Button { showingDetails = true } label: {
                        HStack(spacing: 6) {
                            if activityLabel != nil {
                                Image(systemName: "circle.fill")
                                    .font(.system(size: 6))
                                    .foregroundStyle(activityColor)
                                    .accessibilityHidden(true)
                            }
                            Text(workspace.state.label).font(Typography.body(15)).lineLimit(1)
                            Image(systemName: "chevron.down").font(.caption2)
                        }
                    }
                    .accessibilityLabel("Session details")
                    .accessibilityValue([workspace.state.label, activityLabel].compactMap { $0 }.joined(separator: ", "))
                }
                if workspace.state != .notRunning {
                    ToolbarItem(placement: .topBarTrailing) { sessionActions }
                }
            }
            if workspace.sessionListingClient != nil {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Sessions", systemImage: "bubble.left.and.bubble.right") { showingSessions = true }
                }
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button("Find in Session", systemImage: "magnifyingglass") { workspace.findInTerminal?() }
                    .disabled(workspace.findInTerminal == nil)
            }
        }
        .task { await workspace.open(task) }
        .onDisappear { opening?.cancel(); workspace.leave() }
        .sheet(isPresented: $showingSessions) {
            if let client = workspace.sessionListingClient {
                MobileSessionsView(client: client, task: task, currentID: workspace.sessionID, theme: theme) { session in
                    chosenSession = session
                    guard session.id != workspace.sessionID else { return }
                    opening?.cancel()
                    opening = Task { await workspace.open(task, sessionID: session.id) }
                }
            }
        }
        .alert("Session details", isPresented: $showingDetails) {
            Button("Close", role: .cancel) {}
        } message: {
            Text(([task.title, chosenSession?.title, connectionLabel, workspace.state.label,
                   activityLabel, activityLabel == nil ? nil : activityDetail]
                .compactMap { $0 }).joined(separator: "\n\n"))
        }
    }

    private var header: some View {
        let layout = textSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 8))
            : AnyLayout(HStackLayout(alignment: .center, spacing: 12))
        return layout {
            VStack(alignment: .leading, spacing: 3) {
                if let chosenSession {
                    Text(chosenSession.title)
                        .font(Typography.strong(14))
                        .lineLimit(2)
                }
                if let activityLabel {
                    Button { showingDetails = true } label: {
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            Image(systemName: "circle.fill")
                                .font(.system(size: 6))
                                .accessibilityHidden(true)
                            Text(activityLabel)
                        }
                            .font(Typography.strong(14))
                            .foregroundStyle(activityColor)
                            .lineLimit(2)
                            .frame(minHeight: 44, alignment: .leading)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityHint("Read the program's reported activity")
                }
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
            sessionActions
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private var sessionActions: some View {
        if workspace.state == .loading || workspace.state == .requestingControl {
            ProgressView().accessibilityLabel("Opening Session")
        }
        if workspace.state == .viewing {
            Button { workspace.requestControl() } label: { actionLabel("Use keyboard") }
        }
        if case .disconnected = workspace.state {
            Button { Task { await workspace.reconnect() } } label: { actionLabel("Reconnect") }
        }
        if workspace.canStartSession { openSessionButton }
    }

    private var activityLabel: String? {
        guard let report = workspace.programStatus?.summary else { return nil }
        let provider = workspace.programStatus?.app(for: report).map { ProgramStatusReport.displaySafe($0.capitalized) }
        let state = report.state.rawValue.capitalized
        let kind = report.kind.map { $0.rawValue.capitalized }
        let progress = report.progress.map { "\($0)%" }
        return [provider, state, kind, progress].compactMap { $0 }.joined(separator: " · ")
    }

    private var activityColor: Color {
        switch workspace.programStatus?.summary?.state {
        case .working: return theme.palette.working
        case .blocked, .error: return theme.palette.warning
        default: return theme.palette.available
        }
    }

    private var activityDetail: String {
        guard let report = workspace.programStatus?.summary else {
            return "No current program report."
        }
        let title = report.title.map(ProgramStatusReport.displaySafe)
        return ([title, report.displayText].compactMap { $0 }.joined(separator: "\n"))
            + "\n\nReported by the program in this Session."
    }

    private var openSessionButton: some View {
        Button { opening = Task { await workspace.startSession() } } label: { actionLabel("Open Session") }
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
