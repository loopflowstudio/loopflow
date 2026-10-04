import Loopflow
import SwiftUI

struct WorkspaceNavigator: View {
    @Bindable var model: PodiumModel
    let onOpenSession: (SessionRecord) -> Void
    let onOpenTask: ((WorkReference) -> Void)?
    let onConversation: ((WorkReference?) -> Void)?
    let onNewShell: (() -> Void)?
    let onShowTerminals: (() -> Void)?
    @Environment(\.palette) private var palette
    @State private var scrollPosition: ScrollPosition
    @State private var hoveringPresentation = false

    init(model: PodiumModel, onOpenSession: @escaping (SessionRecord) -> Void,
         onConversation: ((WorkReference?) -> Void)? = nil,
         onNewShell: (() -> Void)? = nil, onShowTerminals: (() -> Void)? = nil,
         onOpenTask: ((WorkReference) -> Void)? = nil) {
        self.model = model
        self.onOpenSession = onOpenSession
        self.onOpenTask = onOpenTask
        self.onConversation = onConversation
        self.onNewShell = onNewShell
        self.onShowTerminals = onShowTerminals
        _scrollPosition = State(initialValue: ScrollPosition(y: model.navigation.listScrollOffset))
    }

    private var rows: [WorkspaceOutlineRow] {
        model.visibleWorkspace.outline(
            presentation: model.navigation.presentation, collapsed: model.navigation.collapsed,
            search: model.navigation.search,
            planningReadable: model.roadmap.value != nil && model.roadmap.errorMessage == nil
        )
    }

    private var repositories: [String] {
        Set((model.allRepos.map(\.path) + model.visibleRoadmaps.map { $0.wave.repo }
            + [model.repoPath].compactMap { $0 }).map { model.repoIdentity($0) }).sorted()
    }

    var body: some View {
        @Bindable var navigation = model.navigation
        let rows = rows
        VStack(spacing: 0) {
            header
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    forReadErrors
                    ForEach(rows) { row in outlineRow(row) }
                    if model.repoPath != nil, rows.isEmpty, model.workspaceStatus == .current {
                        Text(navigation.presentation == .sessions ? "No open Sessions." : "No matching Work.")
                            .foregroundStyle(palette.textSecondary).padding(8)
                    }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 4)
            }
            .scrollPosition($scrollPosition)
            .onScrollGeometryChange(for: CGFloat.self) { geometry in
                max(0, geometry.contentOffset.y + geometry.contentInsets.top)
            } action: { _, offset in navigation.listScrollOffset = offset }
            // Search sits below the data so the repository heads its outline.
            HStack(spacing: Spacing.sm) {
                Image(systemName: "magnifyingglass").foregroundStyle(palette.textTertiary)
                TextField("Find work or Sessions", text: Binding(
                    get: { navigation.search },
                    set: { Perf.begin(Perf.hierarchyInteraction, "filter", id: "outline"); navigation.search = $0 }))
                    .textFieldStyle(.plain)
                    .font(Typography.body(13))
                    .accessibilityIdentifier("workspace-search")
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(palette.surface.opacity(0.55), in: RoundedRectangle(cornerRadius: 7))
            .padding(.horizontal, 10)
            .padding(.top, 8)
            .padding(.bottom, 12)
        }
        .font(Typography.body(12))
        .foregroundStyle(palette.text)
        .tint(palette.text)
        .buttonStyle(.plain)
        .background(palette.surfaceMuted)
        .contextMenu { repositoryActions }
        .accessibilityIdentifier("workspace-navigator")
        .onChange(of: rows.isEmpty, initial: true) { _, empty in
            if !empty { Perf.endAfterCommit(Perf.coldStart, id: "launch") }
        }
        .onChange(of: rows.map(\.id)) { _, _ in
            Perf.endAfterCommit(Perf.hierarchyInteraction, id: "outline")
        }
    }

    /// The repository heads its own outline: one scope, switched in place.
    private var header: some View {
        @Bindable var navigation = model.navigation
        return HStack(spacing: Spacing.sm) {
            Menu {
                ForEach(repositories, id: \.self) { repo in
                    Button(URL(fileURLWithPath: repo).lastPathComponent) { selectRepository(repo) }
                        .accessibilityIdentifier("workspace-repository-\(repo)")
                }
                Divider()
                repositoryActions
            } label: {
                HStack(spacing: Spacing.xs) {
                    Text(model.repoPath.map { URL(fileURLWithPath: model.repoIdentity($0)).lastPathComponent }
                        ?? "Choose repository")
                        .font(Typography.sectionTitle(25))
                        .foregroundStyle(palette.accentInk)
                        .lineLimit(1)
                    Image(systemName: "chevron.down").font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(palette.textTertiary)
                }
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .help(model.repoPath ?? "Choose a repository")
            .accessibilityLabel("Repository")
            .accessibilityIdentifier("workspace-repository")
            Spacer(minLength: 0)
            Menu {
                Picker("Presentation", selection: Binding(
                    get: { navigation.presentation },
                    set: { Perf.begin(Perf.hierarchyInteraction, "presentation", id: "outline"); navigation.presentation = $0 })) {
                    ForEach(WorkspacePresentation.allCases, id: \.self) { presentation in
                        Text(presentation.rawValue).tag(presentation)
                    }
                }
                Toggle("Show headless Sessions", isOn: Binding(
                    get: { navigation.showsHeadlessSessions },
                    set: {
                        navigation.showsHeadlessSessions = $0
                        Task { await model.refreshSessions() }
                    }))
                    .accessibilityIdentifier("workspace-headless-sessions")
                if let onShowTerminals {
                    Divider()
                    Button("Show retained terminals", action: onShowTerminals)
                        .accessibilityIdentifier("workspace-return-terminals")
                }
            } label: {
                Image(systemName: "line.3.horizontal.decrease")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(hoveringPresentation ? palette.accentInk : palette.textSecondary)
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .frame(width: 28, height: 28)
            .background(hoveringPresentation ? palette.accentInk.opacity(0.08) : Color.clear,
                        in: RoundedRectangle(cornerRadius: 6))
            .onHover { hoveringPresentation = $0 }
            .help("Outline presentation")
            .accessibilityLabel("Outline presentation")
            .accessibilityIdentifier("workspace-presentation")
        }
        .foregroundStyle(palette.text)
        .padding(.leading, 18)
        .padding(.trailing, 12)
        .padding(.top, 14)
        .padding(.bottom, 10)
        .background(palette.surfaceMuted)
    }

    @ViewBuilder private var forReadErrors: some View {
        let status = model.workspaceStatus
        if let message = status.message {
            Group {
                if case .failed = status { warning(message) }
                else { Text(message).foregroundStyle(palette.textSecondary).padding(4) }
            }
            .accessibilityIdentifier("workspace-status")
        }
        ForEach(model.workspace.waves) { wave in
            if let reason = wave.roadmap.tasks.unavailableReason { warning(reason) }
            if case .available(_, let truncated) = wave.roadmap.tasks, truncated {
                warning("Planning is partial; more Tasks exist.")
            }
        }
    }

    /// One row component at two levels. A Wave is a sans section head with a
    /// chevron and nothing else; a Task carries a dot only while it needs eyes
    /// (running, blocked, waiting on you) and a count only while Sessions exist.
    private func outlineRow(_ row: WorkspaceOutlineRow) -> some View {
        let isWave = row.workKey?.work.kind == .wave
        return HStack(spacing: 6) {
            if case .work(let subject, true, _) = row.content {
                let key = subject.key
                Button {
                    Perf.begin(Perf.hierarchyInteraction, "fold", id: "outline")
                    model.navigation.toggle(key)
                } label: {
                    Image(systemName: model.navigation.isExpanded(key) ? "chevron.down" : "chevron.right")
                        .font(.system(size: 9, weight: .bold))
                        .foregroundStyle(palette.textTertiary)
                        .frame(width: 14, height: 24)
                }
                .accessibilityLabel("Toggle \(row.title)")
                .accessibilityIdentifier("workspace-disclose-\(key.work.kind.rawValue)-\(key.work.id)")
            } else if row.session != nil {
                Image(systemName: "terminal").font(.system(size: 11))
                    .foregroundStyle(palette.textTertiary).frame(width: 14)
            } else {
                // The slot stays so titles align whether or not a Task has a dot.
                Circle().fill(taskTone(row)?.ink ?? .clear)
                    .frame(width: 6, height: 6).frame(width: 14)
                    .accessibilityHidden(true)
            }
            Button {
                switch row.content {
                case .session(let session): onOpenSession(session)
                case .work(let subject, _, _):
                    if subject.key.work.kind == .task, let onOpenTask {
                        onOpenTask(subject.key.work)
                    } else {
                        model.select(subject.key.work)
                    }
                }
            } label: {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(row.title)
                        .font(isWave ? Typography.textStrong : Typography.text)
                        .foregroundStyle(isWave
                            ? (isSelected(row) ? palette.accentInk : palette.textSecondary)
                            : palette.text)
                        .lineLimit(1)
                        .truncationMode(.tail)
                    if let detail = row.detail {
                        Text(detail).font(Typography.meta)
                            .foregroundStyle(palette.textTertiary).lineLimit(1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .help(row.title)
            .accessibilityIdentifier(identifier(row))
            if let key = row.workKey, !row.inlineSessions.isEmpty {
                sessionCount(row.inlineSessions, work: key.work)
            }
        }
        .padding(.horizontal, 6)
        .frame(minHeight: 28)
        .background(isSelected(row) ? palette.selectionTint : Color.clear,
                    in: RoundedRectangle(cornerRadius: 6))
        .overlay(alignment: .leading) {
            if isSelected(row) {
                RoundedRectangle(cornerRadius: 1).fill(palette.accentInk)
                    .frame(width: 2).padding(.vertical, 7).offset(x: -10)
            }
        }
        .padding(.top, isWave ? 8 : 0)
        .padding(.leading, CGFloat(row.depth) * 14)
        .contextMenu {
            ForEach(row.inlineSessions) { session in
                Button("Open \(session.title)") { onOpenSession(session) }
            }
            if let key = row.workKey { subjectActions(key.work, title: row.title) }
            ForEach(row.ancestors, id: \.key) { ancestor in
                subjectActions(ancestor.key.work, title: ancestor.title)
            }
            Divider()
            repositoryActions
        }
    }

    /// Open an exact available conversation from its Task or collapsed Wave.
    private func sessionCount(_ sessions: [SessionRecord], work: WorkReference) -> some View {
        let names = sessions.map(\.title).joined(separator: ", ")
        return Menu {
            ForEach(sessions) { session in
                Button(session.title) { onOpenSession(session) }
            }
        } label: {
            Label("\(sessions.count)", systemImage: "bubble.left")
                .font(Typography.meta).foregroundStyle(palette.accentInk)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .help(names)
        .accessibilityLabel("\(sessions.count) Task Sessions: \(names)")
        .accessibilityIdentifier("workspace-session-count-\(work.id)")
    }

    // MARK: - Orphan Sessions

    private var orphans: [SessionRecord] { model.visibleWorkspace.orphanSessions(search: model.navigation.search) }

    @ViewBuilder private func subjectActions(_ work: WorkReference, title: String) -> some View {
        Button("Inspect \(title)") { model.select(work) }
        if let onConversation { Button("New conversation · \(title)") { onConversation(work) } }
    }

    @ViewBuilder private var repositoryActions: some View {
        Menu("Debug") {
            Menu("Sessions") {
                Menu("Orphan Sessions") {
                    ForEach(orphans) { session in
                        Button(session.title) { onOpenSession(session) }
                            .accessibilityIdentifier("debug-orphan-session-\(session.id)")
                    }
                    if orphans.isEmpty { Text("No Sessions without a Task association") }
                }
            }
        }
        if let onConversation { Button("New repository conversation") { onConversation(nil) } }
        if let onNewShell { Button("New shell", action: onNewShell).accessibilityIdentifier("sessions-new-shell") }
        if let onShowTerminals { Button("Show retained terminals", action: onShowTerminals) }
    }

    private func selectRepository(_ path: String) {
        Perf.begin(Perf.hierarchyInteraction, "repository", id: "outline")
        model.setRepoPath(path)
        Task.detached { try? saveLoopflowState(LoopflowState(selectedRepoPath: path.normalizedFilePath)) }
    }

    private func identifier(_ row: WorkspaceOutlineRow) -> String {
        switch row.id {
        case .session(let id): "session-row-\(id)"
        case .work(let key): "workspace-\(key.work.kind.rawValue)-\(key.work.id)"
        }
    }

    private func isSelected(_ row: WorkspaceOutlineRow) -> Bool {
        if let session = row.session { return model.navigation.selectedSessionId == session.id }
        // A Task row carrying the open Session inline keeps its location visible.
        if let selected = model.navigation.selectedSessionId {
            return row.inlineSessions.contains { $0.id == selected }
        }
        return row.workKey?.work == model.selection
    }

    /// The dot a Task row carries while it needs eyes, from its shared Flow
    /// projection: running, blocked, or waiting on you. Stopped and idle rows
    /// carry nothing.
    private func taskTone(_ row: WorkspaceOutlineRow) -> WorkspaceTone? {
        guard let work = row.workKey?.work, work.kind == .task, let found = model.task(id: work.id),
              case .pinned(let pinned) = found.task.flow.record else { return nil }
        let state = pinned.execution.presentation
        return state.label == nil ? nil : state.tone
    }

    private func warning(_ text: String) -> some View {
        Label(text, systemImage: "exclamationmark.triangle")
            .foregroundStyle(Color.statusWarning).textSelection(.enabled).padding(4)
    }
}
