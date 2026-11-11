import Loopflow
import Observation
import SwiftUI

/// Which Tasks the list asks the Machine for. `started` is Desktop's sidebar:
/// Working Tasks, then Autonomous ones.
enum TaskFilter: String, CaseIterable, Identifiable {
    case started, available, busy, backlog, done

    var id: String { rawValue }

    var title: String {
        switch self {
        case .started: "Started"
        case .available: "Ready for you"
        case .busy: "Working now"
        case .backlog: "Backlog"
        case .done: "Done"
        }
    }

    var expression: String {
        switch self {
        case .started: "kind:task state:working or kind:task state:autonomous"
        case .available: "kind:task state:available"
        case .busy: "kind:task state:busy"
        case .backlog: "kind:task state:backlog"
        case .done: "kind:task state:done"
        }
    }
}

/// The Machine's Tasks under one filter, kept current while the list is on screen.
@MainActor
@Observable
final class TaskFeed {
    private(set) var repositories: [RepositoryTasks] = []
    private(set) var loaded = false
    private(set) var error: String?
    /// Repository names by durable ID, filled in as readings name them.
    private(set) var names: [String: String] = [:]
    var filter = TaskFilter.started
    /// The last reading under each filter, shown at once while the Machine answers.
    private struct Saved: Codable {
        let repositories: [RepositoryTasks]
        let names: [String: String]
    }
    @ObservationIgnored private var waiting: Perf.Wait?
    @ObservationIgnored private let client: RemoteMobileClient
    @ObservationIgnored private var observation: RemoteTaskObservation?
    @ObservationIgnored private var reader: Task<Void, Never>?
    @ObservationIgnored private var generation = 0

    init(client: RemoteMobileClient) { self.client = client }

    func start() async {
        stop()
        let current = generation
        waiting = Perf.begin("tasks_current")
        if let saved = try? JSONDecoder().decode(Saved.self, from: Data(contentsOf: savedURL)) {
            repositories = saved.repositories
            names.merge(saved.names) { _, saved in saved }
            loaded = true
            Perf.launch("tasks_saved")
        } else {
            repositories = []
            loaded = false
        }
        do {
            let observation = try await client.observeTasks(expression: filter.expression)
            guard current == generation, !Task.isCancelled else { observation.close(); return }
            self.observation = observation
            reader = Task { [weak self] in
                do {
                    for try await reading in observation.readings {
                        guard let self, current == self.generation else { return }
                        switch reading {
                        case .success(let repositories):
                            if repositories != self.repositories { self.repositories = repositories }
                            self.loaded = true
                            self.error = nil
                            Perf.launch("tasks_current")
                            if let waiting = self.waiting { Perf.end(waiting, self.filter.rawValue) }
                            self.waiting = nil
                            await self.name(repositories.map(\.repository))
                            self.save()
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

    /// Removes what this Machine's readings left on the phone.
    func forget() {
        stop()
        for filter in TaskFilter.allCases { try? FileManager.default.removeItem(at: savedURL(filter)) }
    }

    private var savedURL: URL { savedURL(filter) }

    private func savedURL(_ filter: TaskFilter) -> URL {
        URL.cachesDirectory.appending(path: "tasks-\(client.savedMachine.machineID)-\(filter.rawValue).json")
    }

    private func save() {
        try? JSONEncoder().encode(Saved(repositories: repositories, names: names))
            .write(to: savedURL, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    }

    private func name(_ ids: [String]) async {
        for id in ids where names[id] == nil {
            guard let path = try? await client.repositoryPath(id: id) else { continue }
            names[id] = URL(fileURLWithPath: path).lastPathComponent
        }
    }
}

extension WorkViewPlan.Content {
    var mobileStatus: String {
        if primarySession == nil { return "No conversation yet" }
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
    /// The repository shown, by durable ID; empty shows every repository.
    @AppStorage("repository") private var repository = ""
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.scenePhase) private var scenePhase
    private var theme: ThemeDefinition { colorScheme == .dark ? .dark : .light }

    init(client: RemoteMobileClient, onForget: @escaping () -> Void, onChangeAddress: @escaping () -> Void) {
        self.client = client
        self.onForget = onForget
        self.onChangeAddress = onChangeAddress
        _feed = State(initialValue: TaskFeed(client: client))
    }

    /// Repositories with matching Tasks, each split into Tasks with a conversation and those without.
    private var sections: [(repository: String, open: [WorkViewPlan.Content], quiet: [WorkViewPlan.Content])] {
        let term = search.trimmingCharacters(in: .whitespacesAndNewlines)
        return feed.repositories.compactMap { entry in
            guard shown.isEmpty || entry.repository == shown else { return nil }
            let tasks = term.isEmpty ? entry.tasks : entry.tasks.filter {
                $0.title.localizedStandardContains(term) || $0.location?.cwd.localizedStandardContains(term) == true
            }
            guard !tasks.isEmpty else { return nil }
            return (entry.repository, tasks.filter { $0.primarySession != nil }, tasks.filter { $0.primarySession == nil })
        }
    }

    /// The saved choice, unless this Machine no longer has that repository.
    private var shown: String {
        feed.repositories.contains { $0.repository == repository } ? repository : ""
    }

    private var title: String { shown.isEmpty ? "All repositories" : name(shown) }

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
                ForEach(sections, id: \.repository) { section in
                    Section {
                        ForEach(section.open, id: \.work) { link($0) }
                        if !section.quiet.isEmpty {
                            // Started Tasks nobody has opened a conversation for stay one tap away.
                            DisclosureGroup("\(section.quiet.count) without a conversation") {
                                ForEach(section.quiet, id: \.work) { link($0) }
                            }
                            .font(Typography.caption(14))
                            .foregroundStyle(theme.palette.textSecondary)
                            .listRowBackground(theme.palette.background)
                        }
                    } header: {
                        if shown.isEmpty {
                            Text(feed.names[section.repository] ?? "Repository")
                                .font(Typography.strong(14))
                                .foregroundStyle(theme.palette.textSecondary)
                                .textCase(nil)
                        }
                    }
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .background(theme.palette.background)
            .overlay {
                if !feed.loaded && feed.error == nil {
                    ProgressView()
                } else if sections.isEmpty && feed.error == nil {
                    if search.isEmpty {
                        ContentUnavailableView("No \(feed.filter.title.lowercased()) Tasks", systemImage: "checkmark.circle")
                    } else {
                        ContentUnavailableView.search(text: search)
                    }
                }
            }
            .refreshable { await feed.refresh() }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbarTitleMenu {
                Picker("Repository", selection: $repository) {
                    Text("All repositories").tag("")
                    ForEach(feed.repositories.map(\.repository).sorted { name($0) < name($1) }, id: \.self) {
                        Text(name($0)).tag($0)
                    }
                }
            }
            .searchable(text: $search, prompt: "Search Tasks")
            .navigationDestination(isPresented: $showingCreated) {
                if let created { workspace(created) }
            }
            .sheet(item: $creation, onDismiss: { showingCreated = created != nil }) { model in
                MobileNewTaskView(model: model, theme: theme) { opening in
                    opening.presented()
                    created = opening.task
                    creation = nil
                }
            }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Menu {
                        Picker("Show", selection: $feed.filter) {
                            ForEach(TaskFilter.allCases) { Text($0.title).tag($0) }
                        }
                    } label: {
                        Image(systemName: feed.filter == .started
                            ? "line.3.horizontal.decrease.circle" : "line.3.horizontal.decrease.circle.fill")
                    }
                    .accessibilityLabel("Show")
                    .accessibilityValue(feed.filter.title)
                }
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
                            feed.forget()
                            onForget()
                        }
                    } label: { Image(systemName: "desktopcomputer") }
                        .accessibilityLabel("Machine")
                }
            }
        }
        .tint(theme.palette.accentInk)
        .environment(\.palette, theme.palette)
        .task(id: [feed.filter.rawValue, String(scenePhase == .active)]) {
            if scenePhase == .active { await feed.start() } else { feed.stop() }
        }
    }

    private func name(_ repository: String) -> String { feed.names[repository] ?? "Repository" }

    private func link(_ task: WorkViewPlan.Content) -> some View {
        NavigationLink { workspace(task) } label: { row(task) }
            .listRowInsets(EdgeInsets(top: 12, leading: 20, bottom: 12, trailing: 16))
            .listRowBackground(theme.palette.background)
    }

    private func workspace(_ task: WorkViewPlan.Content) -> some View {
        TaskWorkspaceView(client: client, task: task, theme: theme, draft: Binding(
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
