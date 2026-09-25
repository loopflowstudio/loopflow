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
        VStack(spacing: 0) {
            header
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    forReadErrors
                    ForEach(rows) { row in outlineRow(row) }
                    if model.repoPath != nil, rows.isEmpty, !model.roadmap.isLoading, !model.sessions.isLoading,
                       model.roadmap.errorMessage == nil, model.sessions.errorMessage == nil {
                        Text(navigation.presentation == .sessions ? "No open Sessions." : "No matching Work.")
                            .foregroundStyle(palette.textSecondary).padding(8)
                    }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 8)
            }
            .scrollPosition($scrollPosition)
            .onScrollGeometryChange(for: CGFloat.self) { geometry in
                max(0, geometry.contentOffset.y + geometry.contentInsets.top)
            } action: { _, offset in navigation.listScrollOffset = offset }
            // Search sits below the data so the repository heads its outline.
            HStack(spacing: Spacing.xs) {
                Image(systemName: "magnifyingglass").foregroundStyle(palette.textSecondary)
                TextField("Find work or Sessions", text: $navigation.search)
                    .textFieldStyle(.plain)
                    .accessibilityIdentifier("workspace-search")
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .overlay(alignment: .top) { Rectangle().fill(palette.border).frame(height: 1) }
        }
        .font(Typography.body(12))
        .foregroundStyle(palette.text)
        .tint(palette.text)
        .buttonStyle(.plain)
        .background(palette.surfaceMuted)
        .contextMenu { repositoryActions }
        .accessibilityIdentifier("workspace-navigator")
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
                if !repositoryActionsEmpty {
                    Divider()
                    repositoryActions
                }
            } label: {
                HStack(spacing: Spacing.xs) {
                    Text(model.repoPath.map { URL(fileURLWithPath: model.repoIdentity($0)).lastPathComponent }
                        ?? "Choose repository")
                        .font(Typography.sectionTitle(20))
                        .lineLimit(1)
                    Image(systemName: "chevron.down").font(.system(size: 10, weight: .semibold))
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
        .foregroundStyle(palette.text)
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
        .background(palette.surfaceMuted)
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

    private func outlineRow(_ row: WorkspaceOutlineRow) -> some View {
        HStack(spacing: 4) {
            if case .work(let subject, true, _) = row.content {
                let key = subject.key
                Button { model.navigation.toggle(key) } label: {
                    Image(systemName: model.navigation.isExpanded(key) ? "chevron.down" : "chevron.right")
                        .frame(width: 16, height: 26)
                }
                .accessibilityLabel("Toggle \(row.title)")
                .accessibilityIdentifier("workspace-disclose-\(key.work.kind.rawValue)-\(key.work.id)")
            } else {
                if row.session != nil {
                    Image(systemName: "terminal")
                        .foregroundStyle(palette.textSecondary).frame(width: 16)
                } else {
                    Color.clear.frame(width: 16, height: 1)
                }
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
                HStack(spacing: Spacing.sm) {
                    if let work = row.workKey?.work, work.kind == .wave,
                       let wave = model.rosterWave(id: work.id) {
                        WaveLensView(lens: wave.lens)
                    }
                    VStack(alignment: .leading, spacing: 2) {
                        Text(row.title)
                            .font(row.workKey?.work.kind == .wave ? Typography.sectionTitle(18) : Typography.body(12))
                            .lineLimit(row.workKey?.work.kind == .task ? 1 : 2)
                            .truncationMode(.tail)
                        if let detail = row.detail {
                            Text(detail).font(Typography.caption(11))
                                .foregroundStyle(palette.textSecondary).lineLimit(2)
                        }
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
        .padding(.vertical, 4)
        .padding(.horizontal, 6)
        .background(isSelected(row) ? palette.accent.opacity(0.10) : Color.clear,
                    in: RoundedRectangle(cornerRadius: CornerRadius.md))
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

    /// Exact open conversations on a Task, by name. Clicking the count inspects
    /// the Task overview without entering any one of them.
    private func sessionCount(_ sessions: [SessionRecord], work: WorkReference) -> some View {
        let names = sessions.map(\.title).joined(separator: ", ")
        return Button { model.select(work) } label: {
            HStack(spacing: 2) {
                Image(systemName: "bubble.left")
                Text("\(sessions.count)")
            }
            .font(Typography.caption(10))
            .foregroundStyle(palette.textSecondary)
        }
        .help(names)
        .accessibilityLabel("\(sessions.count) open \(sessions.count == 1 ? "Session" : "Sessions"): \(names). Inspect Task")
        .accessibilityIdentifier("workspace-session-count-\(work.id)")
    }

    @ViewBuilder private func subjectActions(_ work: WorkReference, title: String) -> some View {
        Button("Inspect \(title)") { model.select(work) }
        if let onConversation { Button("New conversation · \(title)") { onConversation(work) } }
    }

    private var repositoryActionsEmpty: Bool {
        onConversation == nil && onNewShell == nil && onShowTerminals == nil
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
        // A Task row carrying the open Session inline keeps its location visible.
        if let selected = model.navigation.selectedSessionId {
            return row.inlineSessions.contains { $0.id == selected }
        }
        return row.workKey?.work == model.selection
    }

    private func warning(_ text: String) -> some View {
        Label(text, systemImage: "exclamationmark.triangle")
            .foregroundStyle(Color.statusWarning).textSelection(.enabled).padding(4)
    }
}
