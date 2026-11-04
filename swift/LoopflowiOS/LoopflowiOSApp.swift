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
                ContentUnavailableView("Machine connection unavailable", systemImage: "network.slash",
                    description: Text("This build contains an offline fixture. Live Machine pairing is not configured."))
            }
        }
    }
}

struct MobileTaskList: View {
    @State private var workspace: MobileWorkspace
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.scenePhase) private var scenePhase
    private var theme: ThemeDefinition { colorScheme == .dark ? .dark : .light }

    init(client: any MobileClient) { _workspace = State(initialValue: MobileWorkspace(client: client)) }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Text("Working Tasks").font(Typography.display).frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 20)
                Text("Offline fixture · no Machine connection")
                    .font(Typography.caption(13))
                    .foregroundStyle(theme.palette.textSecondary)
                    .padding(12)
                if workspace.isLoadingTasks && workspace.tasks.isEmpty {
                    ProgressView("Loading Tasks…").frame(maxHeight: .infinity)
                } else {
                    List {
                        if let error = workspace.tasksError {
                            Text("Couldn’t update · \(error)").foregroundStyle(theme.palette.failure)
                        }
                        ForEach(workspace.tasks, id: \.work) { task in
                            NavigationLink {
                                MobileSessionView(task: task, workspace: workspace, theme: theme)
                            } label: {
                                HStack(spacing: 12) {
                                    RoundedRectangle(cornerRadius: 2)
                                        .fill(task.states.contains(.busy) ? theme.palette.working : theme.palette.available)
                                        .frame(width: 3)
                                    VStack(alignment: .leading, spacing: 6) {
                                        Text(task.title).font(Typography.strong(17)).foregroundStyle(theme.palette.text)
                                        Text(task.states.map(\.rawValue).joined(separator: " · ").capitalized)
                                            .font(Typography.caption(13)).foregroundStyle(theme.palette.textSecondary)
                                    }
                                    Spacer()
                                }
                                .frame(minHeight: 60)
                            }
                            .listRowBackground(theme.palette.surface)
                            .accessibilityHint("Open the existing Session terminal")
                        }
                        if workspace.tasks.isEmpty && workspace.tasksError == nil {
                            Text("No open Tasks").font(Typography.body(17))
                        }
                    }
                    .scrollContentBackground(.hidden)
                    .refreshable { await workspace.refresh() }
                }
            }
            .background(theme.palette.background)
            .navigationTitle("Tasks")
            .navigationBarTitleDisplayMode(.inline)
        }
        .tint(theme.palette.accentInk)
        .environment(\.palette, theme.palette)
        .task { await workspace.refresh() }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background { workspace.suspend() }
        }
    }
}

struct MobileSessionView: View {
    let task: WorkViewPlan.Content
    let workspace: MobileWorkspace
    let theme: ThemeDefinition

    var body: some View {
        VStack(spacing: 0) {
            Text("Offline fixture").font(Typography.caption(12)).padding(.top, 4)
            HStack {
                Text(workspace.state.label).font(Typography.body(14))
                Spacer()
                if workspace.state == .viewing {
                    Button("Use keyboard") { workspace.requestControl() }
                        .frame(minHeight: 44)
                }
                if case .disconnected = workspace.state {
                    Button("Reconnect") { Task { await workspace.reconnect() } }
                        .frame(minHeight: 44)
                }
            }.padding(.horizontal, 12)
            SessionTerminal(workspace: workspace, theme: theme)
        }
        .background(theme.palette.background)
        .foregroundStyle(theme.palette.text)
        .navigationTitle(task.title)
        .navigationBarTitleDisplayMode(.inline)
        .task { await workspace.open(task) }
        .onDisappear { workspace.leave() }
    }
}
