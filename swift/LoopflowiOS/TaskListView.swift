import Loopflow
import Observation
import SwiftUI

extension TaskList {
    var title: String {
        switch self {
        case .started: "Started"
        case .available: "Ready for you"
        case .busy: "Working now"
        case .backlog: "Backlog"
        case .done: "Done"
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
    private(set) var waves: [Wave] = []
    var filter = TaskList.started
    /// The Wave whose Tasks are listed; nil lists every Wave's.
    var wave: String?
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
        // The last reading under each filter, shown at once while the Machine answers.
        // It is kept as wire bytes and read by the live decoder, so a file an older
        // app wrote fails to load instead of showing a stale shape.
        let url = savedURL
        let saved = await Task.detached(priority: .userInitiated) {
            (try? RemoteTaskObservation.decode(frame: RawJSON(Data(contentsOf: url))).reading).flatMap { try? $0.get() }
        }.value
        guard current == generation else { return }
        if let saved {
            repositories = saved
            loaded = true
            Perf.launch("tasks_saved")
        } else {
            repositories = []
            loaded = false
        }
        let observation = client.observeTasks(expression: filter.expression(wave: wave))
        self.observation = observation
        reader = Task { [weak self] in
            do {
                for try await update in observation.readings {
                    guard let self, current == self.generation else { return }
                    switch update.reading {
                    case .success(let repositories):
                        let changed = repositories != self.repositories || !self.loaded
                        if changed { self.repositories = repositories }
                        self.loaded = true
                        self.error = nil
                        Perf.launch("tasks_current")
                        if let waiting = self.waiting { Perf.end(waiting, self.filter.rawValue) }
                        self.waiting = nil
                        if changed { self.save(update.wire) }
                        if self.waves.isEmpty, let found = try? await self.client.waves() { self.waves = found }
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
        let saved = (try? FileManager.default.contentsOfDirectory(at: .cachesDirectory, includingPropertiesForKeys: nil)) ?? []
        for file in saved where file.lastPathComponent.hasPrefix("tasks-\(client.savedMachine.machineID)-") {
            try? FileManager.default.removeItem(at: file)
        }
    }

    private var savedURL: URL {
        URL.cachesDirectory.appending(path: "tasks-\(client.savedMachine.machineID)-\(filter.rawValue)\(wave.map { "-\($0)" } ?? "").json")
    }

    private func save(_ wire: Data) {
        let url = savedURL
        Task.detached(priority: .utility) {
            try? wire.write(to: url, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
        }
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
    /// The chosen repository, or all of them once a reading shows it is gone.
    /// While a new reading loads the choice holds.
    private var shown: String {
        !feed.loaded || feed.repositories.contains { $0.repository == repository } ? repository : ""
    }

    private var title: String {
        guard !shown.isEmpty else { return "All repositories" }
        guard let wave = waves.first(where: { $0.id == feed.wave }) else { return name(shown) }
        return "\(name(shown)) · \(wave.displayName)"
    }

    /// The shown repository's Waves; a Wave is chosen within a repository, as on Desktop.
    private var waves: [Wave] {
        guard let path = feed.repositories.first(where: { $0.repository == shown })?.path else { return [] }
        return feed.waves.filter { $0.repo == path }.sorted { $0.displayName < $1.displayName }
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
                            Text(name(section.repository))
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
                            ForEach(TaskList.allCases) { Text($0.title).tag($0) }
                        }
                        if !waves.isEmpty {
                            Picker("Wave", selection: $feed.wave) {
                                Text("Every Wave").tag(String?.none)
                                ForEach(waves) { Text($0.displayName).tag(String?.some($0.id)) }
                            }
                        }
                    } label: {
                        Image(systemName: feed.filter == .started && feed.wave == nil
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
        .onChange(of: repository) { feed.wave = nil }
        .task(id: [feed.filter.rawValue, feed.wave ?? "", String(scenePhase == .active)]) {
            if scenePhase == .active { await feed.start() } else { feed.stop() }
        }
    }

    /// A repository's folder name; one the Machine cannot place keeps its short ID,
    /// so two of them stay told apart.
    private func name(_ repository: String) -> String {
        feed.repositories.first { $0.repository == repository }?.name ?? "Repository"
    }

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

    private func row(_ task: WorkViewPlan.Content) -> some View {
        HStack(alignment: .top, spacing: 12) {
            RoundedRectangle(cornerRadius: 2)
                .fill(task.status.tone.ink(in: theme.palette))
                .frame(width: 3)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 6) {
                Text(task.title)
                    .font(Typography.strong(17))
                    .foregroundStyle(theme.palette.text)
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 6) {
                    Text(task.status.label)
                        .foregroundStyle(task.status.tone.ink(in: theme.palette))
                    if let identifier = task.identifier {
                        Text(identifier).foregroundStyle(theme.palette.textSecondary)
                    }
                }
                .font(Typography.caption(13))
            }
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }
}
