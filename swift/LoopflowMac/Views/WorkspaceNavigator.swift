import Loopflow
import SwiftUI

struct WorkspaceNavigator: View {
    @Bindable var model: PodiumModel
    let onOpenSession: (SessionRecord) -> Void
    let onOpenTask: ((WorkReference) -> Void)?
    let sessionStatus: (SessionRecord) -> String
    let onConversation: ((WorkReference?) -> Void)?
    let onNewShell: (() -> Void)?
    let onShowTerminals: (() -> Void)?
    private let palette = LoopflowPalette.deepWine
    @State private var scrollPosition: ScrollPosition

    init(model: PodiumModel, onOpenSession: @escaping (SessionRecord) -> Void,
         sessionStatus: @escaping (SessionRecord) -> String = { $0.state.rawValue.capitalized },
         onConversation: ((WorkReference?) -> Void)? = nil,
         onNewShell: (() -> Void)? = nil, onShowTerminals: (() -> Void)? = nil,
         onOpenTask: ((WorkReference) -> Void)? = nil) {
        self.model = model
        self.onOpenSession = onOpenSession
        self.onOpenTask = onOpenTask
        self.sessionStatus = sessionStatus
        self.onConversation = onConversation
        self.onNewShell = onNewShell
        self.onShowTerminals = onShowTerminals
        _scrollPosition = State(initialValue: ScrollPosition(y: model.navigation.listScrollOffset))
    }

    private var rows: [WorkspaceOutlineRow] {
        model.workspace.outline(
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
        let repositories = repositories
        VStack(spacing: 0) {
            HStack {
                TextField("Find work or Sessions", text: $navigation.search)
                    .textFieldStyle(.plain)
                    .accessibilityIdentifier("workspace-search")
                Menu {
                    Picker("Presentation", selection: $navigation.presentation) {
                        ForEach(WorkspacePresentation.allCases, id: \.self) { presentation in
                            Text(presentation.rawValue).tag(presentation)
                        }
                    }
                    if let onShowTerminals {
                        Divider()
                        Button("Show retained terminals", action: onShowTerminals)
                            .accessibilityIdentifier("workspace-return-terminals")
                    }
                } label: { Image(systemName: "line.3.horizontal.decrease") }
                .menuStyle(.borderlessButton)
                .fixedSize()
                .accessibilityLabel("Outline presentation")
                .accessibilityIdentifier("workspace-presentation")
            }
            .padding(12)
            .background(Color.loopflowBurgundy)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    forReadErrors
                    ForEach(repositories, id: \.self) { repo in
                        let selected = model.repoPath.map { model.repoIdentity($0) == model.repoIdentity(repo) } ?? false
                        let omitRoot = repositories.count == 1 && selected && navigation.presentation != .full
                            && !navigation.repositoryCollapsed && model.roadmap.value != nil
                            && model.roadmap.errorMessage == nil
                        if !omitRoot { repositoryRow(repo, selected: selected) }
                        if selected && (!navigation.repositoryCollapsed || !navigation.search.isEmpty) {
                            ForEach(rows) { row in
                                outlineRow(row)
                                    .padding(.leading, omitRoot ? 0 : 14)
                            }
                            if rows.isEmpty, !model.roadmap.isLoading, !model.sessions.isLoading,
                               model.roadmap.errorMessage == nil, model.sessions.errorMessage == nil {
                                Text(navigation.presentation == .sessions ? "No open Sessions." : "No matching Work.")
                                    .foregroundStyle(palette.textSecondary).padding(8)
                            }
                        }
                    }
                    // Before a repository is selected, keep mandatory boundaries
                    // visible even if planning has not supplied its roots yet.
                    if model.repoPath == nil {
                        ForEach(rows.filter { $0.session != nil }) { row in outlineRow(row) }
                    }
                }
                .padding(.horizontal, 8)
                .padding(.bottom, 8)
            }
            .scrollPosition($scrollPosition)
            .onScrollGeometryChange(for: CGFloat.self) { geometry in
                max(0, geometry.contentOffset.y + geometry.contentInsets.top)
            } action: { _, offset in navigation.listScrollOffset = offset }
        }
        .font(Typography.body(12))
        .foregroundStyle(palette.text)
        .tint(palette.text)
        .buttonStyle(.plain)
        .background(palette.surface)
        .environment(\.colorScheme, .dark)
        .environment(\.palette, palette)
        .contextMenu { repositoryActions }
        .accessibilityIdentifier("workspace-navigator")
    }

    @ViewBuilder private var forReadErrors: some View {
        if model.roadmap.isLoading {
            Text("Reading planning…").foregroundStyle(palette.textSecondary)
                .accessibilityIdentifier("workspace-planning-loading")
        }
        if let error = model.roadmap.errorMessage {
            warning("Planning unavailable: \(error)").accessibilityIdentifier("workspace-planning-unavailable")
        }
        if model.sessions.isLoading { Text("Reading Sessions…").foregroundStyle(palette.textSecondary) }
        if let error = model.sessions.errorMessage { warning("Sessions unavailable: \(error)") }
        ForEach(model.workspace.waves) { wave in
            if let reason = wave.roadmap.tasks.unavailableReason { warning(reason) }
            if case .available(_, let truncated) = wave.roadmap.tasks, truncated {
                warning("Planning is partial; more Tasks exist.")
            }
        }
    }

    private func repositoryRow(_ repo: String, selected: Bool) -> some View {
        HStack(spacing: 4) {
            Button {
                if selected { model.navigation.repositoryCollapsed.toggle() }
                else { selectRepository(repo) }
            } label: {
                Image(systemName: selected && !model.navigation.repositoryCollapsed ? "chevron.down" : "chevron.right")
                    .frame(width: 16, height: 26)
            }
            .accessibilityLabel("Toggle repository \(repo)")
            Button { selectRepository(repo) } label: {
                Text(URL(fileURLWithPath: repo).lastPathComponent)
                    .font(Typography.caption(12).weight(.semibold))
                    .frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
            }
            .help(repo)
            .accessibilityIdentifier("workspace-repository-\(repo)")
        }
        .contextMenu {
            Button("Open repository") { selectRepository(repo) }
            if selected { repositoryActions }
        }
    }

    private func outlineRow(_ row: WorkspaceOutlineRow) -> some View {
        HStack(spacing: 4) {
            if case .work(let subject, true) = row.content {
                let key = subject.key
                Button { model.navigation.toggle(key) } label: {
                    Image(systemName: model.navigation.isExpanded(key) ? "chevron.down" : "chevron.right")
                        .frame(width: 16, height: 26)
                }
                .accessibilityLabel("Toggle \(row.title)")
                .accessibilityIdentifier("workspace-disclose-\(key.work.kind.rawValue)-\(key.work.id)")
            } else {
                Image(systemName: row.session == nil ? "minus" : "terminal")
                    .foregroundStyle(palette.textSecondary).frame(width: 16)
            }
            Button {
                switch row.content {
                case .session(let session): onOpenSession(session)
                case .work(let subject, _):
                    if subject.key.work.kind == .task, let onOpenTask {
                        onOpenTask(subject.key.work)
                    } else {
                        model.select(subject.key.work)
                    }
                }
            } label: {
                HStack(spacing: Spacing.sm) {
                    if let work = row.workKey?.work, work.kind == .wave,
                       let wave = model.rosterWave(id: work.id) {
                        WaveLensView(lens: wave.lens)
                    }
                    VStack(alignment: .leading, spacing: 2) {
                        Text(row.title)
                            .font(row.workKey?.work.kind == .wave ? Typography.sectionTitle(18) : Typography.body(12))
                            .lineLimit(2)
                        if let detail = row.detail {
                            Text(detail).font(Typography.caption(11))
                                .foregroundStyle(palette.textSecondary).lineLimit(2)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .accessibilityIdentifier(identifier(row))
            if let session = row.session {
                Text(sessionStatus(session)).font(Typography.caption(10)).foregroundStyle(palette.textSecondary)
            }
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 6)
        .background(isSelected(row) ? Color.loopflowBurgundy : Color.clear,
                    in: RoundedRectangle(cornerRadius: CornerRadius.md))
        .padding(.leading, CGFloat(row.depth) * 14)
        .contextMenu {
            if let key = row.workKey { subjectActions(key.work, title: row.title) }
            ForEach(row.ancestors, id: \.key) { ancestor in
                subjectActions(ancestor.key.work, title: ancestor.title)
            }
            Divider()
            repositoryActions
        }
    }

    @ViewBuilder private func subjectActions(_ work: WorkReference, title: String) -> some View {
        Button("Inspect \(title)") { model.select(work) }
        if let onConversation { Button("New conversation · \(title)") { onConversation(work) } }
    }

    @ViewBuilder private var repositoryActions: some View {
        if let onConversation { Button("New repository conversation") { onConversation(nil) } }
        if let onNewShell { Button("New terminal", action: onNewShell).accessibilityIdentifier("sessions-new-shell") }
        if let onShowTerminals { Button("Show retained terminals", action: onShowTerminals) }
    }

    private func selectRepository(_ path: String) {
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
        return model.navigation.selectedSessionId == nil && row.workKey?.work == model.selection
    }

    private func warning(_ text: String) -> some View {
        Label(text, systemImage: "exclamationmark.triangle")
            .foregroundStyle(Color.statusWarning).textSelection(.enabled).padding(4)
    }
}
