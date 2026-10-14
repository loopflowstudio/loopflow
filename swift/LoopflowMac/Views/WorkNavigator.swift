import Loopflow
import SwiftUI

struct WorkNavigator: View {
    @Bindable var model: WorkModel
    let onOpenSession: (SessionRecord) -> Void
    let onOpenRepository: (() -> Void)?
    let onOpenTask: ((WorkReference) -> Void)?
    let onConversation: ((WorkReference?) -> Void)?
    let onNewShell: (() -> Void)?
    let onShowTerminals: (() -> Void)?
    @Environment(\.palette) private var palette
    @State private var scrollPosition: ScrollPosition
    @State private var hoveringPresentation = false

    init(model: WorkModel, onOpenSession: @escaping (SessionRecord) -> Void,
         onConversation: ((WorkReference?) -> Void)? = nil,
         onNewShell: (() -> Void)? = nil, onShowTerminals: (() -> Void)? = nil,
         onOpenTask: ((WorkReference) -> Void)? = nil,
         onOpenRepository: (() -> Void)? = nil) {
        self.model = model
        self.onOpenSession = onOpenSession
        self.onOpenRepository = onOpenRepository
        self.onOpenTask = onOpenTask
        self.onConversation = onConversation
        self.onNewShell = onNewShell
        self.onShowTerminals = onShowTerminals
        _scrollPosition = State(initialValue: ScrollPosition(y: model.navigation.listScrollOffset))
    }

    private func rows(_ workspace: WorkProjection) -> [WorkOutlineRow] {
        workspace.outline(
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
        // One projection per render: rows and every row's menu share it.
        let workspace = model.visibleWork
        let rows = rows(workspace)
        VStack(spacing: 0) {
            header()
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    forReadErrors
                    ForEach(rows) { row in outlineRow(row) }
                    if model.repoPath != nil, rows.isEmpty, model.workStatus == .current {
                        Text("No matching Work.")
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
                TextField("Find Work", text: Binding(
                    get: { navigation.search },
                    set: { Perf.begin(Perf.hierarchyInteraction, "filter", id: "outline"); navigation.search = $0 }))
                    .textFieldStyle(.plain)
                    .font(Typography.body(13))
                    .accessibilityIdentifier("work-search")
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
        .buttonStyle(WorkPlainButtonStyle())
        .background(palette.surfaceMuted)
        .contextMenu { repositoryActions() }
        .accessibilityIdentifier("work-navigator")
        .onChange(of: rows.isEmpty, initial: true) { _, empty in
            guard !empty else { return }
            Perf.endAfterCommit(Perf.coldStart, id: "launch")
            let source = model.showsSavedWork ? "saved" : "fresh"
            DispatchQueue.main.async { LaunchJournal.home.mark(.usable, ["source": source]) }
        }
        .onChange(of: rows.map(\.id)) { _, _ in
            Perf.endAfterCommit(Perf.hierarchyInteraction, id: "outline")
        }
    }

    /// The repository heads its own outline: one scope, switched in place.
    private func header() -> some View {
        @Bindable var navigation = model.navigation
        return HStack(spacing: Spacing.sm) {
            Button {
                onOpenRepository?()
            } label: {
                Text(model.repoPath.map { URL(fileURLWithPath: model.repoIdentity($0)).lastPathComponent }
                    ?? "Choose repository")
                    .font(Typography.sectionTitle(25))
                    .foregroundStyle(palette.emphasisInk)
                    .lineLimit(1)
            }
            .disabled(model.repoPath == nil)
            .accessibilityIdentifier("work-open-repository")
            Menu {
                ForEach(repositories, id: \.self) { repo in
                    Button(URL(fileURLWithPath: repo).lastPathComponent) { selectRepository(repo) }
                        .accessibilityIdentifier("work-repository-\(repo)")
                }
                Divider()
                repositoryActions()
            } label: {
                Image(systemName: "chevron.down").font(.system(size: 10, weight: .semibold))
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .accessibilityLabel("Choose repository")
            .accessibilityIdentifier("work-repository")
            Spacer(minLength: 0)
            Menu {
                Picker("Presentation", selection: Binding(
                    get: { navigation.presentation },
                    set: { Perf.begin(Perf.hierarchyInteraction, "presentation", id: "outline"); navigation.presentation = $0 })) {
                    ForEach(WorkPresentation.allCases, id: \.self) { presentation in
                        Text(presentation.rawValue).tag(presentation)
                    }
                }
                if let onShowTerminals {
                    Divider()
                    Button("Show retained terminals", action: onShowTerminals)
                        .accessibilityIdentifier("work-return-terminals")
                }
            } label: {
                Image(systemName: "line.3.horizontal.decrease")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(hoveringPresentation ? palette.text : palette.textSecondary)
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .frame(width: 28, height: 28)
            .background(hoveringPresentation ? palette.actionHover : Color.clear,
                        in: RoundedRectangle(cornerRadius: 6))
            .onHover { hoveringPresentation = $0 }
            .help("Outline presentation")
            .accessibilityLabel("Outline presentation")
            .accessibilityIdentifier("work-presentation")
        }
        .foregroundStyle(palette.text)
        .padding(.leading, 18)
        .padding(.trailing, 12)
        .padding(.top, 14)
        .padding(.bottom, 10)
        .background(palette.surfaceMuted)
    }

    @ViewBuilder private var forReadErrors: some View {
        let status = model.workStatus
        if let message = status.message {
            Group {
                if case .failed = status { warning(message) }
                else { Text(message).foregroundStyle(palette.textSecondary).padding(4) }
            }
            .accessibilityIdentifier("work-status")
        }
        ForEach(model.projection.waves) { wave in
            if let reason = wave.roadmap.tasks.unavailableReason { warning(reason) }
            if case .available(_, let truncated) = wave.roadmap.tasks, truncated {
                warning("Planning is partial; more Tasks exist.")
            }
        }
    }

    /// Waves head the outline; Tasks keep title navigation separate from metadata links.
    private func outlineRow(_ row: WorkOutlineRow) -> some View {
        let isWave = row.id.work.kind == .wave
        let status = row.task.map { _ in
            TaskRowProgramStatus(sessions: row.inlineSessions, unavailable: model.sessions.errorMessage)
        }
        return HStack(spacing: 6) {
            if row.hasChildren {
                let key = row.id
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
                .accessibilityIdentifier("work-disclose-\(key.work.kind.rawValue)-\(key.work.id)")
            } else {
                // The slot stays so titles align whether or not a Task has a dot.
                Circle().fill(status?.tone?.ink(in: palette) ?? .clear)
                    .frame(width: 6, height: 6).frame(width: 14)
                    .accessibilityHidden(true)
            }
            VStack(alignment: .leading, spacing: 3) {
                Button {
                    if row.id.work.kind == .task, let onOpenTask {
                        onOpenTask(row.id.work)
                    } else {
                        model.select(row.id.work)
                    }
                } label: {
                    Text(row.title)
                        .font(isWave ? Typography.textStrong : Typography.text)
                        .foregroundStyle(isWave ? palette.textSecondary : palette.text)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                }
                .help(row.title)
                .accessibilityIdentifier("work-\(row.id.work.kind.rawValue)-\(row.id.work.id)")
                if let task = row.task, let status {
                    TaskRowMetadata(task: task, status: status, localMachineId: model.observingMachineId)
                }
            }
        }
        .padding(.horizontal, 6)
        .frame(minHeight: isWave ? 28 : 44)
        .workSelection(isSelected(row))
        .accessibilityAddTraits(isSelected(row) ? [.isSelected] : [])
        .padding(.top, isWave ? 8 : 0)
        .padding(.leading, CGFloat(row.depth) * 14)
        .contextMenu {
            ForEach(row.inlineSessions) { session in
                Button("Open \(session.title)") { onOpenSession(session) }
            }
            subjectActions(row.id.work, title: row.title)
            ForEach(row.ancestors, id: \.key) { ancestor in
                subjectActions(ancestor.key.work, title: ancestor.title)
            }
            Divider()
            repositoryActions()
        }
    }

    @ViewBuilder private func subjectActions(_ work: WorkReference, title: String) -> some View {
        Button("Inspect \(title)") { model.select(work) }
        if let onConversation { Button("New conversation · \(title)") { onConversation(work) } }
    }

    @ViewBuilder private func repositoryActions() -> some View {
        if let onOpenRepository { Button("Repository conversation", action: onOpenRepository) }
        if let onNewShell { Button("New shell", action: onNewShell).accessibilityIdentifier("sessions-new-shell") }
        if let onShowTerminals { Button("Show retained terminals", action: onShowTerminals) }
    }

    private func selectRepository(_ path: String) {
        Perf.begin(Perf.hierarchyInteraction, "repository", id: "outline")
        model.setRepoPath(path)
        Task.detached { try? saveLoopflowState(LoopflowState(selectedRepoPath: path.normalizedFilePath)) }
    }

    private func isSelected(_ row: WorkOutlineRow) -> Bool {
        // A Task row carrying the open Session inline keeps its location visible.
        if let selected = model.navigation.selectedSessionId {
            return row.inlineSessions.contains { $0.id == selected }
        }
        return row.id.work == model.selection
    }

    private func warning(_ text: String) -> some View {
        Label(text, systemImage: "exclamationmark.triangle")
            .foregroundStyle(WorkTone.human.ink).textSelection(.enabled).padding(4)
    }
}
