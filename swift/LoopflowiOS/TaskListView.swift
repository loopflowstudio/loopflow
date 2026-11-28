import Loopflow
import Observation
import SwiftUI

extension TaskList {
    var title: String {
        switch self {
        case .started: "Working"
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
                            tasks: pendingChanges.apply(to: entry.tasks, readAt: tasks.readAt, wave: wave)
                                .filter { $0.states.contains(filter.state) })
        }
    }

    /// The parent a swipe is asking about: what happens to its open children.
    var completing: WorkViewPlan.Content?

    /// The row leaves the list at once and the Notice offers Undo; a refusal brings
    /// it back and says why, its Show running `show` to open the Task. A swipe on a
    /// parent sends no `children`: the row stays and `completing` asks what happens to them.
    func complete(_ task: WorkViewPlan.Content, children: TaskChildPolicy? = nil, show: @escaping @MainActor () -> Void) {
        completing = nil
        if children == nil, (task.openChildren ?? 0) > 0 {
            completing = task
            return
        }
        guard let directory = directory(of: task) else { return }
        complete(issue: task.identifier ?? task.work.id, directory: directory, children: children, show: Notice.Action("Show", perform: show))
    }

    func complete(issue: String, directory: String, children: TaskChildPolicy? = nil, show: Notice.Action? = nil) {
        if children == nil, let parent = content(for: issue), (parent.openChildren ?? 0) > 0 {
            completing = parent
            return
        }
        notices.post(Notice("\(issue) completed", key: "complete-\(issue)", action: Notice.Action("Undo") { [weak self] in
            self?.change(.reopen(task: issue), issue: issue, verb: "reopened",
                         write: .taskReopen(directory: directory, issue: issue))
        }))
        pendingChanges.make(.complete(task: issue), subject: issue, verb: "completed",
                            write: .taskComplete(directory: directory, issue: issue, children: children), notices: notices,
                            show: show) { [weak self] in
            try? await self?.observation?.refresh()
        }
    }

    private func content(for issue: String) -> WorkViewPlan.Content? {
        repositories.lazy.flatMap(\.tasks).first { $0.identifier == issue || $0.work.id == issue }
    }

    /// Show `command` on the Task now and send `write`; a refusal undoes it and
    /// the Notice says "LOO-4 couldn't be `verb`: …".
    func change(_ command: PendingCommand, issue: String, verb: String, write: ServedWrite) {
        pendingChanges.make(command, subject: issue, verb: verb, write: write, notices: notices, show: nil) { [weak self] in
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
    /// Read again on connect and whenever the app returns to the foreground.
    let machineTheme: MachineTheme
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
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.theme) private var theme

    init(client: RemoteMobileClient, machineTheme: MachineTheme, onForget: @escaping () -> Void, onChangeAddress: @escaping () -> Void) {
        self.client = client
        self.machineTheme = machineTheme
        self.onForget = onForget
        self.onChangeAddress = onChangeAddress
        let notices = NoticeCenter()
        _notices = State(initialValue: notices)
        _feed = State(initialValue: TaskFeed(client: client, notices: notices))
    }

    private struct TaskSectionRows: Identifiable {
        let repository: String
        let group: WorkingGroup?
        let tasks: [WorkViewPlan.Content]
        var id: String { repository + "/" + (group?.rawValue ?? "") }
    }

    private var sections: [TaskSectionRows] {
        feed.project()
        let term = search.trimmingCharacters(in: .whitespacesAndNewlines)
        return feed.shownRepositories.flatMap { entry -> [TaskSectionRows] in
            guard shown.isEmpty || entry.repository == shown else { return [] }
            let tasks = term.isEmpty ? entry.tasks : entry.tasks.filter {
                $0.title.localizedStandardContains(term) || $0.location?.cwd.localizedStandardContains(term) == true
            }
            guard !tasks.isEmpty else { return [] }
            guard feed.filter == .started else {
                return [TaskSectionRows(repository: entry.repository, group: nil, tasks: tasks)]
            }
            let groups = Dictionary(grouping: tasks, by: \.workingGroup)
            return WorkingGroup.allCases.compactMap { group in
                groups[group].map { TaskSectionRows(repository: entry.repository, group: group, tasks: $0) }
            }
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
                ForEach(sections) { section in
                    let quiet = section.tasks.filter { $0.primarySession == nil }
                    Section {
                        ForEach(section.tasks.filter { $0.primarySession != nil }, id: \.work) { link($0) }
                        if !quiet.isEmpty {
                            // Started Tasks nobody has opened a conversation for stay one tap away.
                            DisclosureGroup("\(quiet.count) without a conversation") {
                                ForEach(quiet, id: \.work) { link($0) }
                            }
                            .font(Typography.meta)
                            .foregroundStyle(theme.palette.textSecondary)
                            .listRowBackground(theme.palette.background)
                        }
                    } header: {
                        let title = [shown.isEmpty ? name(section.repository) : nil, section.group?.title]
                            .compactMap { $0 }.joined(separator: " · ")
                        if !title.isEmpty {
                            Text(title)
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
            .textInputAutocapitalization(.never)
            .onChange(of: feed.reading) { _, _ in feed.observed() }
            .navigationDestination(isPresented: Binding(get: { selectedTask != nil }, set: { if !$0 { selectedTask = nil } })) {
                if let selectedTask { workspace(selectedTask) }
            }
            .navigationDestination(isPresented: $showingCreated) {
                if let created { workspace(created) }
            }
            .sheet(item: $creation, onDismiss: { showingCreated = created != nil }) { model in
                MobileNewTaskView(model: model) { opening in
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
        .confirmationDialog(
            feed.completing.map { TaskChildPolicy.question(issue: $0.identifier ?? $0.work.id, openChildren: $0.openChildren ?? 0) }
                ?? "Complete Task?",
            isPresented: Binding(get: { feed.completing != nil }, set: { if !$0 { feed.completing = nil } }),
            titleVisibility: .visible,
            presenting: feed.completing
        ) { task in
            Button("Move Children Up") { feed.complete(task, children: .rehome) { selectedTask = MobileTask(task) } }
            Button("Abandon Children", role: .destructive) {
                feed.complete(task, children: .abandon) { selectedTask = MobileTask(task) }
            }
            Button("Cancel", role: .cancel) {}
        } message: { _ in
            Text("Move children up to keep them open, or abandon open children (merged ones are marked done).")
        }
        .alert("Visual timing unavailable", isPresented: Binding(get: { captureError != nil }, set: { if !$0 { captureError = nil } })) {
            Button("OK") { captureError = nil }
        } message: { Text(captureError ?? "") }
        .tint(theme.palette.accentInk)
        .onChange(of: repository) { feed.wave = nil }
        .task(id: scenePhase == .active) {
            if scenePhase == .active { await machineTheme.refresh(client) }
        }
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

    func link(_ task: WorkViewPlan.Content) -> some View {
        Button {
            if task.primarySession != nil { Perf.opening(task.work.id) }
            selectedTask = MobileTask(task)
        } label: {
            // The list reading carries no issue link: the whole row opens the Task.
            TaskRow(id: task.work.id, title: task.title, identifier: task.identifier, status: task.status,
                    auto: task.states.contains(.auto), context: task.graphStep)
                .contentShape(Rectangle())
                .accessibilityElement(children: .combine)
        }
            .buttonStyle(.plain)
            .swipeActions(edge: .trailing) {
                Button("Complete", systemImage: "checkmark") { feed.complete(task) { selectedTask = MobileTask(task) } }
                    .tint(theme.palette.success)
                    .disabled(!feed.pendingChanges.acceptsChanges)
            }
            // With the row's own padding the title starts where the section header does.
            .listRowInsets(EdgeInsets(top: Spacing.xs, leading: Spacing.xs, bottom: Spacing.xs, trailing: Spacing.xs))
            .listRowBackground(theme.palette.background)
    }

    private func workspace(_ task: MobileTask) -> some View {
        TaskWorkspaceView(client: client, feed: feed, task: task, draft: Binding(
            get: { drafts[task.id] ?? "" },
            set: { drafts[task.id] = $0 }))
    }
}
