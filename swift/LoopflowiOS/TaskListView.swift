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
    /// The last reading; while reads fail it keeps what the list showed and the Notice says why.
    private(set) var tasks: Reading<[RepositoryTasks]> = .loading {
        didSet {
            notices.read("tasks", "Tasks", from: oldValue.failure, to: tasks.failure, link: pendingChanges.link,
                         action: Notice.Action("Try again") { [weak self] in Task { await self?.start() } })
        }
    }
    private(set) var waves: [Wave] = []
    /// This Machine's changes shown before the reading that shows them.
    let pendingChanges: PendingChanges
    /// The app's one Notice.
    let notices: NoticeCenter
    var filter = TaskList.started {
        didSet {
            guard filter != oldValue else { return }
            requested?.finish(.superseded)
            requested = Perf.begin("find_work", scenario: filter.rawValue)
            if let requested { Perf.acknowledged(requested) }
        }
    }
    private(set) var reading = 0
    @ObservationIgnored private var requested: Interaction?
    /// The Wave whose Tasks are listed; nil lists every Wave's.
    var wave: String?
    @ObservationIgnored private var waiting: Interaction?
    @ObservationIgnored private let client: RemoteMobileClient
    @ObservationIgnored private let saved: SavedReadings
    @ObservationIgnored private var observation: RemoteTaskObservation?
    @ObservationIgnored private var reader: Task<Void, Never>?
    @ObservationIgnored private var generation = 0

    init(client: RemoteMobileClient, notices: NoticeCenter) {
        self.client = client
        self.notices = notices
        saved = SavedReadings(machine: client.savedMachine.machineID)
        pendingChanges = PendingChanges(send: { try await client.change($0, requestID: $1) },
                                        describe: machineConnectionMessage)
        pendingChanges.report(to: notices)
    }

    var repositories: [RepositoryTasks] { tasks.value ?? [] }
    var loaded: Bool { tasks.value != nil }
    var failed: Bool { tasks.failure != nil }

    /// The Machine's Tasks with this phone's pending changes made.
    var shownRepositories: [RepositoryTasks] {
        guard !pendingChanges.changes.isEmpty else { return repositories }
        return repositories.map { entry in
            RepositoryTasks(repository: entry.repository, path: entry.path,
                            tasks: pendingChanges.apply(to: entry.tasks, readAt: tasks.readAt, wave: wave))
        }
    }

    /// The row leaves the list in the swipe; a refusal brings it back and the Notice
    /// says why, its Show running `show` to open the Task.
    func complete(_ task: WorkViewPlan.Content, show: @escaping @MainActor () -> Void) {
        guard let directory = directory(of: task) else { return }
        let issue = task.identifier ?? task.work.id
        pendingChanges.make(.complete(task: issue), subject: issue, verb: "completed",
                            write: .taskComplete(directory: directory, issue: issue), notices: notices,
                            show: Notice.Action("Show", perform: show)) { [weak self] in
            try? await self?.observation?.refresh()
        }
    }

    /// Start, Run, Auto-Ship and Stop Auto-Ship: the Notice says the change was requested.
    func request(_ name: String, _ task: WorkViewPlan.Content, _ write: (String, String) -> ServedWrite) {
        guard let directory = directory(of: task) else { return }
        let issue = task.identifier ?? task.work.id
        pendingChanges.request(name, subject: issue, write: write(directory, issue), notices: notices) { [weak self] in
            try? await self?.observation?.refresh()
        }
    }

    /// The repository on the Machine whose reading listed `task`.
    private func directory(of task: WorkViewPlan.Content) -> String? {
        repositories.first { $0.tasks.contains { $0.work == task.work } }?.path
    }

    /// A read failed: the list keeps what it showed.
    private func failed(_ message: String) {
        tasks = tasks.failed(message)
    }

    private var subscription: String { "tasks-\(filter.rawValue)\(wave.map { "-\($0)" } ?? "")" }

    func start() async {
        stop()
        let current = generation
        waiting = requested ?? Perf.begin("find_work", scenario: filter.rawValue)
        requested = nil
        // The last reading under each filter, shown at once while the Machine answers.
        let (subscription, saved) = (subscription, saved)
        let restored = await Task.detached(priority: .userInitiated) {
            saved.load(subscription).flatMap { try? RemoteTaskObservation.decode(frame: RawJSON($0)).reading }
        }.value
        guard current == generation else { return }
        if let restored, restored.failure == nil {
            tasks = restored
            Perf.launch("tasks_saved")
        } else {
            tasks = .loading
        }
        let observation = InteractionTrace.$current.withValue(waiting) {
            client.observeTasks(expression: filter.expression(wave: wave), saved: saved, subscription: subscription)
        }
        self.observation = observation
        reader = InteractionTrace.$current.withValue(waiting) { Task { [weak self] in
            do {
                for try await update in observation.readings {
                    guard let self, current == self.generation else { return }
                    self.pendingChanges.linkReturned()
                    self.pendingChanges.read("tasks", update)
                    if let reason = update.failure {
                        self.waiting?.finish(.failed)
                        self.waiting = nil
                        self.failed(reason)
                        continue
                    }
                    guard self.tasks.yields(to: update) else { continue }
                    if update != self.tasks { self.tasks = update }
                    Perf.launch("tasks_current")
                    self.reading += 1
                    if self.waves.isEmpty, let found = try? await self.client.waves() { self.waves = found }
                }
                throw RegistryQueryError("Task updates ended.")
            } catch {
                guard let self, current == self.generation else { return }
                self.waiting?.finish(.failed)
                self.stop()
                if ServerLink.lostLink(error) { self.pendingChanges.linkLost() }
                self.failed(machineConnectionMessage(error))
            }
        } }
    }

    func project() { (requested ?? waiting)?.work.project() }

    func observed() {
        guard let waiting else { return }
        Perf.end(waiting)
        Task {
            await waiting.waitForObservation()
            if self.waiting === waiting { self.waiting = nil }
        }
    }

    func refresh() async {
        guard let observation else { await start(); return }
        do { try await observation.refresh() } catch {
            stop()
            failed(machineConnectionMessage(error))
        }
    }

    func stop() {
        waiting?.finish(.cancelled)
        waiting = nil
        generation += 1
        reader?.cancel()
        reader = nil
        observation?.close()
        observation = nil
        pendingChanges.ended("tasks")
    }

    /// Removes what this Machine's readings left on the phone.
    func forget() {
        stop()
        saved.forget()
    }
}

struct TaskListView: View {
    let client: RemoteMobileClient
    let onForget: () -> Void
    let onChangeAddress: () -> Void
    @State private var feed: TaskFeed
    @State private var search = ""
    @State private var drafts: [String: String] = [:]
    @State private var creation: MobileTaskCreation?
    @State private var created: MobileTask?
    @State private var showingCreated = false
    @State private var captureError: String?
    @State private var selectedTask: MobileTask?
    @State private var notices: NoticeCenter
    /// The repository shown, by durable ID; empty shows every repository.
    @AppStorage("repository") private var repository = ""
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.scenePhase) private var scenePhase
    private var theme: ThemeDefinition { colorScheme == .dark ? .dark : .light }

    init(client: RemoteMobileClient, onForget: @escaping () -> Void, onChangeAddress: @escaping () -> Void) {
        self.client = client
        self.onForget = onForget
        self.onChangeAddress = onChangeAddress
        let notices = NoticeCenter()
        _notices = State(initialValue: notices)
        _feed = State(initialValue: TaskFeed(client: client, notices: notices))
    }

    /// Repositories with matching Tasks, each split into Tasks with a conversation and those without.
    private var sections: [(repository: String, open: [WorkViewPlan.Content], quiet: [WorkViewPlan.Content])] {
        feed.project()
        let term = search.trimmingCharacters(in: .whitespacesAndNewlines)
        return feed.shownRepositories.compactMap { entry in
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
                ForEach(sections, id: \.repository) { section in
                    Section {
                        ForEach(section.open, id: \.work) { link($0) }
                        if !section.quiet.isEmpty {
                            // Started Tasks nobody has opened a conversation for stay one tap away.
                            DisclosureGroup("\(section.quiet.count) without a conversation") {
                                ForEach(section.quiet, id: \.work) { link($0) }
                            }
                            .font(Typography.meta)
                            .foregroundStyle(theme.palette.textSecondary)
                            .listRowBackground(theme.palette.background)
                        }
                    } header: {
                        if shown.isEmpty {
                            Text(name(section.repository))
                                .font(Typography.meta)
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
                if !feed.loaded && !feed.failed {
                    ProgressView()
                } else if sections.isEmpty && feed.loaded {
                    if search.isEmpty {
                        StateView("No \(feed.filter.title.lowercased()) Tasks") {
                            Button("New Task", action: newTask).buttonStyle(LoopflowButtonStyle(.accent))
                        }
                    } else {
                        StateView("No results for “\(search)”") {
                            Button("Clear search") { search = "" }.buttonStyle(LoopflowButtonStyle(.accent))
                        }
                    }
                }
            }
            .overlay { NoticeView(center: notices) }
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
            .onChange(of: feed.reading) { _, _ in feed.observed() }
            .navigationDestination(isPresented: Binding(get: { selectedTask != nil }, set: { if !$0 { selectedTask = nil } })) {
                if let selectedTask { workspace(selectedTask) }
            }
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
                    Button(action: newTask) { Image(systemName: "plus") }
                        .accessibilityLabel("New Task")
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Text(client.savedMachine.endpoint.host ?? "Machine")
                        Button(InteractionFrame.isEnabled ? "Stop visual timing" : "Start visual timing…") {
                            Task {
                                do {
                                    if InteractionFrame.isEnabled { await InteractionFrame.disableCapture() }
                                    else { try await InteractionFrame.enableCapture() }
                                } catch { captureError = error.localizedDescription }
                            }
                        }
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
        .alert("Visual timing unavailable", isPresented: Binding(get: { captureError != nil }, set: { if !$0 { captureError = nil } })) {
            Button("OK") { captureError = nil }
        } message: { Text(captureError ?? "") }
        .tint(theme.palette.accentInk)
        .onChange(of: repository) { feed.wave = nil }
        .task(id: [feed.filter.rawValue, feed.wave ?? "", String(scenePhase == .active)]) {
            if scenePhase == .active { await feed.start() } else { feed.stop() }
        }
    }

    private func newTask() {
        created = nil
        creation = MobileTaskCreation(client: client)
    }

    /// A repository's folder name; one the Machine cannot place keeps its short ID,
    /// so two of them stay told apart.
    private func name(_ repository: String) -> String {
        feed.repositories.first { $0.repository == repository }?.name ?? "Repository"
    }

    private func link(_ task: WorkViewPlan.Content) -> some View {
        Button {
            if task.primarySession != nil { Perf.opening(task.work.id) }
            selectedTask = MobileTask(task)
        } label: {
            // The list reading carries no issue link: the whole row opens the Task.
            TaskRow(id: task.work.id, title: task.title, identifier: task.identifier, status: task.status,
                    auto: task.states.contains(.auto))
                .contentShape(Rectangle())
                .accessibilityElement(children: .combine)
        }
            .buttonStyle(.plain)
            .swipeActions(edge: .trailing) {
                Button("Complete", systemImage: "checkmark") { feed.complete(task) { selectedTask = MobileTask(task) } }
                    .tint(theme.palette.success)
                    .disabled(!feed.pendingChanges.acceptsChanges)
            }
            .listRowInsets(EdgeInsets(top: Spacing.xs, leading: Spacing.sm, bottom: Spacing.xs, trailing: Spacing.sm))
            .listRowBackground(theme.palette.background)
    }

    private func workspace(_ task: MobileTask) -> some View {
        TaskWorkspaceView(client: client, feed: feed, task: task, theme: theme, draft: Binding(
            get: { drafts[task.id] ?? "" },
            set: { drafts[task.id] = $0 }))
    }
}
