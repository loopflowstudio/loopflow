import Loopflow
import Observation
import SwiftUI

/// The Machine's open Tasks, kept current while the list is on screen.
@MainActor
@Observable
final class TaskFeed {
    private(set) var tasks: [WorkViewPlan.Content] = []
    private(set) var loaded = false
    private(set) var error: String?
    @ObservationIgnored private let client: RemoteMobileClient
    @ObservationIgnored private var observation: RemoteTaskObservation?
    @ObservationIgnored private var reader: Task<Void, Never>?
    @ObservationIgnored private var generation = 0

    init(client: RemoteMobileClient) { self.client = client }

    func start() async {
        stop()
        let current = generation
        do {
            let observation = try await client.observeTasks()
            guard current == generation, !Task.isCancelled else { observation.close(); return }
            self.observation = observation
            reader = Task { [weak self] in
                do {
                    for try await reading in observation.readings {
                        guard let self, current == self.generation else { return }
                        switch reading {
                        case .success(let tasks):
                            self.tasks = tasks
                            self.loaded = true
                            self.error = nil
                        case .failure(let error): self.error = error.message
                        }
                    }
                    throw RegistryQueryError("Task updates ended.")
                } catch {
                    guard let self, current == self.generation else { return }
                    self.stop()
                    self.error = machineConnectionMessage(error)
                }
            }
        } catch {
            guard current == generation, !Task.isCancelled else { return }
            self.error = machineConnectionMessage(error)
        }
    }

    func refresh() async {
        guard let observation else { await start(); return }
        do { try await observation.refresh() } catch {
            stop()
            self.error = machineConnectionMessage(error)
        }
    }

    func stop() {
        generation += 1
        reader?.cancel()
        reader = nil
        observation?.close()
        observation = nil
    }
}

extension WorkViewPlan.Content {
    var mobileStatus: String {
        if primarySession == nil { return "No Session" }
        if !unavailable.isEmpty { return "Unavailable" }
        if states.contains(.blocked) { return "Needs a response" }
        if states.contains(.busy) { return "Working" }
        if states.contains(.available) { return "Ready" }
        if states.contains(.shipping) { return "Shipping" }
        return "Open"
    }
}

struct TaskListView: View {
    let client: RemoteMobileClient
    let onForget: () -> Void
    let onChangeAddress: () -> Void
    @State private var feed: TaskFeed
    @State private var search = ""
    @State private var drafts: [WorkWorkspaceKey: String] = [:]
    @State private var creation: MobileTaskCreation?
    @State private var created: WorkViewPlan.Content?
    @State private var showingCreated = false
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.scenePhase) private var scenePhase
    private var theme: ThemeDefinition { colorScheme == .dark ? .dark : .light }

    init(client: RemoteMobileClient, onForget: @escaping () -> Void, onChangeAddress: @escaping () -> Void) {
        self.client = client
        self.onForget = onForget
        self.onChangeAddress = onChangeAddress
        _feed = State(initialValue: TaskFeed(client: client))
    }

    private var visible: [WorkViewPlan.Content] {
        let term = search.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !term.isEmpty else { return feed.tasks }
        return feed.tasks.filter {
            $0.title.localizedStandardContains(term) || $0.location?.cwd.localizedStandardContains(term) == true
        }
    }

    var body: some View {
        NavigationStack {
            List {
                if let error = feed.error {
                    VStack(alignment: .leading, spacing: 8) {
                        Label("Couldn’t update Tasks", systemImage: "wifi.exclamationmark").font(Typography.strong(15))
                        Text(error).font(Typography.caption(13))
                        Button("Try again") { Task { await feed.start() } }
                    }
                    .foregroundStyle(theme.palette.textSecondary)
                    .listRowBackground(theme.palette.surface)
                }
                ForEach(visible, id: \.work) { task in
                    NavigationLink { session(task) } label: { row(task) }
                        .listRowInsets(EdgeInsets(top: 12, leading: 20, bottom: 12, trailing: 16))
                        .listRowBackground(theme.palette.background)
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .background(theme.palette.background)
            .overlay {
                if !feed.loaded && feed.error == nil {
                    ProgressView()
                } else if visible.isEmpty && feed.error == nil {
                    if search.isEmpty {
                        ContentUnavailableView("No open Tasks", systemImage: "checkmark.circle")
                    } else {
                        ContentUnavailableView.search(text: search)
                    }
                }
            }
            .refreshable { await feed.refresh() }
            .navigationTitle("Tasks")
            .searchable(text: $search, prompt: "Search Tasks")
            .navigationDestination(isPresented: $showingCreated) {
                if let created { session(created) }
            }
            .sheet(item: $creation, onDismiss: { showingCreated = created != nil }) { model in
                MobileNewTaskView(model: model, theme: theme) { opening in
                    opening.presented()
                    created = opening.task
                    creation = nil
                }
            }
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        created = nil
                        creation = MobileTaskCreation(client: client)
                    } label: { Image(systemName: "plus") }
                        .accessibilityLabel("New Task")
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Text(client.savedMachine.endpoint.host ?? "Machine")
                        Button("Change address", action: onChangeAddress)
                        Button("Forget Machine", role: .destructive) {
                            feed.stop()
                            onForget()
                        }
                    } label: { Image(systemName: "desktopcomputer") }
                        .accessibilityLabel("Machine")
                }
            }
        }
        .tint(theme.palette.accentInk)
        .environment(\.palette, theme.palette)
        .task(id: scenePhase == .active) {
            if scenePhase == .active { await feed.start() } else { feed.stop() }
        }
    }

    private func session(_ task: WorkViewPlan.Content) -> some View {
        SessionView(client: client, task: task, theme: theme, draft: Binding(
            get: { drafts[task.work] ?? "" },
            set: { drafts[task.work] = $0 }))
    }

    private func statusColor(_ task: WorkViewPlan.Content) -> Color {
        if task.primarySession == nil || !task.unavailable.isEmpty { return theme.palette.textSecondary }
        if task.states.contains(.blocked) { return theme.palette.warning }
        if task.states.contains(.busy) { return theme.palette.working }
        return theme.palette.available
    }

    private func row(_ task: WorkViewPlan.Content) -> some View {
        HStack(alignment: .top, spacing: 12) {
            RoundedRectangle(cornerRadius: 2)
                .fill(statusColor(task))
                .frame(width: 3)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 6) {
                Text(task.title)
                    .font(Typography.strong(17))
                    .foregroundStyle(theme.palette.text)
                    .fixedSize(horizontal: false, vertical: true)
                Text(task.mobileStatus)
                    .font(Typography.caption(13))
                    .foregroundStyle(statusColor(task))
            }
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }
}
