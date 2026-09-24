import Loopflow
import SwiftUI

struct WorkspaceNavigator: View {
    @Bindable var model: PodiumModel
    let onOpenSession: (SessionRecord) -> Void
    let sessionStatus: (SessionRecord) -> String
    @Environment(\.palette) private var palette
    @State private var scrollPosition: ScrollPosition

    init(model: PodiumModel, onOpenSession: @escaping (SessionRecord) -> Void,
         sessionStatus: @escaping (SessionRecord) -> String = { $0.state.rawValue.uppercased() }) {
        self.model = model
        self.onOpenSession = onOpenSession
        self.sessionStatus = sessionStatus
        _scrollPosition = State(initialValue: ScrollPosition(y: model.navigation.listScrollOffset))
    }

    private var projection: WorkspaceProjection { model.workspace }
    private var taskQuery: TaskQuery { model.navigation.taskQuery }
    private var hasUnknownWorktrees: Bool {
        projection.waves.contains { wave in
            wave.projects.contains { project in
                project.project.tasks.contains { task in
                    task.reference.workspace.map { $0.localExists == nil } ?? false
                }
            }
        }
    }
    private var search: String { model.navigation.search.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        @Bindable var navigation = model.navigation
        VStack(spacing: 0) {
            HStack {
                Text("Tasks").font(Typography.sectionTitle(20))
                Spacer()
                if let error = model.roadmap.errorMessage {
                    Image(systemName: "exclamationmark.triangle").help(error)
                }
            }
            .padding(.horizontal, Spacing.md)
            .padding(.top, Spacing.md)
            Picker("Task view", selection: $navigation.taskQuery) {
                ForEach(TaskQuery.allCases, id: \.self) { query in
                    Text(query.rawValue).tag(query)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, Spacing.md)
            .padding(.top, Spacing.sm)
            .accessibilityIdentifier("workspace-task-view")
            TextField("Find work or Sessions", text: $navigation.search,
                      prompt: Text("Find work or Sessions").foregroundStyle(palette.textSecondary))
                .textFieldStyle(.plain)
                .padding(Spacing.sm)
                .background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: 5))
                .padding(Spacing.md)
                .accessibilityIdentifier("workspace-search")
            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.xs) {
                    if model.roadmap.isLoading {
                        Text("Reading planning…").foregroundStyle(palette.textSecondary)
                            .accessibilityIdentifier("workspace-planning-loading")
                    }
                    if let error = model.roadmap.errorMessage {
                        warning("Planning unavailable: \(error)")
                            .accessibilityIdentifier("workspace-planning-unavailable")
                    }
                    if let error = model.sessions.errorMessage { warning("Sessions unavailable: \(error)") }
                    if model.sessions.isLoading, model.repoPath != nil {
                        Text("Reading Sessions…").foregroundStyle(palette.textSecondary)
                    }
                    ForEach(projection.waves.filter { includes($0) && matches($0) }) { wave in
                        waveGroup(wave)
                    }
                    if taskQuery == .active {
                        if hasUnknownWorktrees {
                            warning("Some local worktrees could not be checked; Active may be incomplete.")
                        }
                        if model.processActivity.isLoading {
                            Text("Reading activity…").foregroundStyle(palette.textSecondary)
                        }
                        if let error = model.processActivity.errorMessage {
                            warning("Activity unavailable: \(error)")
                        }
                        if !model.roadmap.isLoading, !model.sessions.isLoading,
                           !model.processActivity.isLoading, model.roadmap.errorMessage == nil,
                           model.sessions.errorMessage == nil, model.processActivity.errorMessage == nil,
                           !hasUnknownWorktrees, projection.waves.allSatisfy(planningIsComplete),
                           !projection.waves.contains(where: { $0.projects.contains(where: { $0.tasks.contains(where: taskQuery.matches) }) }) {
                            Text("No active Tasks in this repository.")
                                .foregroundStyle(palette.textSecondary)
                        }
                    }
                    if !projection.unmatchedSessions.isEmpty {
                        Text("Other open Sessions")
                            .font(Typography.caption(10).weight(.semibold))
                            .foregroundStyle(palette.textSecondary)
                            .padding(.top, Spacing.md)
                        Text("Repository conversations or Work absent from this planning read.")
                            .font(Typography.caption(9))
                            .foregroundStyle(palette.textSecondary)
                        ForEach(projection.unmatchedSessions.filter { matches($0.title) || matches($0.detail) }) { session in
                            Button { onOpenSession(session) } label: {
                                HStack {
                                    VStack(alignment: .leading, spacing: Spacing.xs) {
                                        Label(session.title, systemImage: "terminal")
                                        if let path = session.workPath {
                                            Text(path).font(Typography.caption(9)).foregroundStyle(palette.textSecondary)
                                        }
                                    }
                                    Spacer()
                                    Text(sessionStatus(session)).font(Typography.caption(8))
                                }
                                .padding(.vertical, Spacing.xs)
                            }
                            .accessibilityIdentifier("session-row-\(session.id)")
                        }
                    }
                    if projection.waves.isEmpty, model.roadmap.errorMessage == nil, !model.roadmap.isLoading {
                        Text("No planned Work in this scope.").foregroundStyle(palette.textSecondary)
                            .accessibilityIdentifier("workspace-planning-empty")
                    }
                }
                .font(Typography.body(12))
                .buttonStyle(.plain)
                .padding(Spacing.md)
            }
            .scrollPosition($scrollPosition)
            .onScrollGeometryChange(for: CGFloat.self) { geometry in
                max(0, geometry.contentOffset.y + geometry.contentInsets.top)
            } action: { _, offset in
                navigation.listScrollOffset = offset
            }
        }
        .background(palette.surface)
        .accessibilityIdentifier("workspace-navigator")
    }

    private func waveGroup(_ wave: WorkspaceWave) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            heading(wave.roadmap.wave.name, key: wave.id,
                    count: taskCount(wave),
                    sessions: wave.sessions,
                    containedSessions: wave.sessions + wave.projects.flatMap { $0.sessions + $0.tasks.flatMap(\.sessions) })
            if model.navigation.isExpanded(wave.id) {
                if let reason = wave.roadmap.projects.unavailableReason { warning(reason) }
                if case .available(_, let truncated) = wave.roadmap.projects, truncated {
                    warning("Planning is partial; more Projects exist.")
                }
                ForEach(wave.projects.filter { includes($0) && (matches(wave.roadmap.wave.name) || matches($0)) }) { project in
                    projectGroup(project, waveMatches: matches(wave.roadmap.wave.name))
                }
            }
        }
        .padding(.bottom, Spacing.sm)
    }

    private func projectGroup(_ project: WorkspaceProject, waveMatches: Bool) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            heading(project.project.project.name, key: project.id, count: project.tasks.filter(taskQuery.matches).count,
                    sessions: project.sessions,
                    containedSessions: project.sessions + project.tasks.flatMap(\.sessions))
            if model.navigation.isExpanded(project.id) {
                ForEach(project.tasks.filter { taskQuery.matches($0) && (waveMatches || matches(project.project.project.name) || matches($0)) }) { task in
                    HStack(spacing: Spacing.sm) {
                        Button { select(task.id) } label: {
                            VStack(alignment: .leading, spacing: Spacing.xxs) {
                                Text(task.task.task.name).lineLimit(2)
                                HStack(spacing: Spacing.sm) {
                                    Text(task.task.task.identifier)
                                    if task.hasProviderInCheckout { Text("Provider in checkout") }
                                    else if task.task.reference.workspace?.localExists == true { Text("Local worktree") }
                                    Text(task.task.task.completed ? "Completed" : task.task.condition.state.rawValue.capitalized)
                                }
                                .font(Typography.caption(9))
                                .foregroundStyle(palette.textSecondary)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                        }
                        .accessibilityIdentifier("workspace-task-\(task.task.id)")
                        sessionAccess(task.sessions)
                    }
                    .padding(Spacing.sm)
                    .background(model.selection == task.id.work ? palette.surfaceMuted : Color.clear,
                                in: RoundedRectangle(cornerRadius: 5))
                }
            }
        }
    }

    private func heading(
        _ title: String, key: WorkspaceNodeKey, count: Int?,
        sessions: [SessionRecord], containedSessions: [SessionRecord]
    ) -> some View {
        HStack(spacing: Spacing.xs) {
            Button { model.navigation.toggle(key) } label: {
                Image(systemName: model.navigation.isExpanded(key) ? "chevron.down" : "chevron.right")
                    .font(.system(size: 9, weight: .semibold))
                    .frame(width: 20, height: 26)
            }
            .accessibilityLabel("Toggle \(title)")
            .accessibilityIdentifier("workspace-disclose-\(key.work.kind.rawValue)-\(key.work.id)")
            Button { select(key) } label: {
                Text(title).font(Typography.caption(key.work.kind == .wave ? 12 : 10).weight(.semibold))
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .accessibilityIdentifier("workspace-\(key.work.kind.rawValue)-\(key.work.id)")
            if !model.navigation.isExpanded(key) {
                Text(count.map(String.init) ?? "?")
                    .font(Typography.caption(9)).foregroundStyle(palette.textSecondary)
            }
            sessionAccess(model.navigation.isExpanded(key) ? sessions : containedSessions)
        }
        .foregroundStyle(key.work.kind == .wave ? palette.text : palette.textSecondary)
        .padding(.top, Spacing.xs)
    }

    @ViewBuilder
    private func sessionAccess(_ sessions: [SessionRecord]) -> some View {
        if sessions.count == 1, let session = sessions.first {
            Button { onOpenSession(session) } label: {
                Label(sessionStatus(session), systemImage: "terminal")
                    .font(Typography.caption(8))
            }
                .help([session.workPath, session.title].compactMap { $0 }.joined(separator: " · "))
                .accessibilityLabel("Open Session: \(session.title)")
                .accessibilityIdentifier("session-row-\(session.id)")
        } else if !sessions.isEmpty {
            Menu {
                ForEach(sessions) { session in
                    Button("\(session.title) · \(sessionStatus(session))") { onOpenSession(session) }
                        .accessibilityIdentifier("session-row-\(session.id)")
                }
            } label: { Label("\(sessions.count)", systemImage: "terminal") }
            .menuStyle(.borderlessButton)
            .fixedSize()
        }
    }

    private func taskCount(_ wave: WorkspaceWave) -> Int? {
        guard planningIsComplete(wave) else { return nil }
        return wave.projects.reduce(0) { $0 + $1.tasks.filter(taskQuery.matches).count }
    }

    private func select(_ key: WorkspaceNodeKey) {
        if model.repoPath == nil { model.setRepoPath(key.repo) }
        model.select(key.work)
    }

    private func includes(_ project: WorkspaceProject) -> Bool {
        taskQuery == .all || !project.sessions.isEmpty || project.tasks.contains(where: taskQuery.matches)
    }

    private func includes(_ wave: WorkspaceWave) -> Bool {
        taskQuery == .all || !wave.sessions.isEmpty || wave.projects.contains(where: includes)
            || !planningIsComplete(wave)
    }

    private func planningIsComplete(_ wave: WorkspaceWave) -> Bool {
        guard case .available(_, false) = wave.roadmap.projects else { return false }
        return wave.roadmap.unavailableProjects.isEmpty
    }

    private func matches(_ value: String) -> Bool { search.isEmpty || value.localizedCaseInsensitiveContains(search) }
    private func matches(_ task: WorkspaceTask) -> Bool {
        matches(task.task.task.name) || matches(task.task.task.identifier)
            || task.sessions.contains { matches($0.title) }
    }
    private func matches(_ project: WorkspaceProject) -> Bool {
        matches(project.project.project.name) || project.tasks.contains(where: matches)
            || project.sessions.contains { matches($0.title) }
    }
    private func matches(_ wave: WorkspaceWave) -> Bool {
        matches(wave.roadmap.wave.name) || wave.projects.contains(where: matches)
            || wave.sessions.contains { matches($0.title) }
    }
    private func warning(_ text: String) -> some View {
        Label(text, systemImage: "exclamationmark.triangle")
            .font(Typography.caption(10)).foregroundStyle(Color.statusWarning)
            .textSelection(.enabled)
    }
}
