import Loopflow
import SwiftUI

struct WorkNavigator: View {
    @Bindable var model: WorkModel
    let onOpenSession: (SessionRecord) -> Void
    let onOpenRepository: (() -> Void)?
    let onOpenWave: ((WorkReference) -> Void)?
    let onOpenTask: ((WorkReference) -> Void)?
    let onConversation: ((WorkReference?) -> Void)?
    let onNewShell: (() -> Void)?
    let onShowTerminals: (() -> Void)?
    @Environment(\.palette) private var palette
    @FocusState private var searchFocused: Bool
    @State private var scrollPosition: ScrollPosition

    init(model: WorkModel, onOpenSession: @escaping (SessionRecord) -> Void,
         onConversation: ((WorkReference?) -> Void)? = nil,
         onNewShell: (() -> Void)? = nil, onShowTerminals: (() -> Void)? = nil,
         onOpenTask: ((WorkReference) -> Void)? = nil,
         onOpenRepository: (() -> Void)? = nil,
         onOpenWave: ((WorkReference) -> Void)? = nil) {
        self.model = model
        self.onOpenSession = onOpenSession
        self.onOpenRepository = onOpenRepository
        self.onOpenWave = onOpenWave
        self.onOpenTask = onOpenTask
        self.onConversation = onConversation
        self.onNewShell = onNewShell
        self.onShowTerminals = onShowTerminals
        _scrollPosition = State(initialValue: ScrollPosition(y: model.navigation.listScrollOffset))
    }

    private var repositories: [String] {
        Set((model.allRepos.map(\.path) + model.visibleRoadmaps.map { $0.wave.repo }
            + [model.repoPath].compactMap { $0 }).map { model.repoIdentity($0) }).sorted()
    }

    var body: some View {
        @Bindable var navigation = model.navigation
        let workspace = model.visibleWork
        let presentation = workspace.navigator(search: navigation.search,
            planningReadable: model.roadmap.value != nil && model.roadmap.errorMessage == nil,
            selection: model.selection, unavailable: model.sessions.errorMessage)
        let selectedWork = model.navigatorSelection
        let searching = !navigation.search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        VStack(spacing: 0) {
            header(selectedWork: selectedWork)
            forReadErrors
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 5) {
                    if !presentation.available.isEmpty {
                        sectionTitle("AVAILABLE · \(presentation.available.count)")
                        ForEach(presentation.available) { item in
                            taskRow(item.row, selectedWork: selectedWork, attention: item.attention)
                        }
                        Divider().padding(.vertical, 10)
                    }
                    if !presentation.standalone.isEmpty {
                        sectionTitle("TASKS")
                        ForEach(presentation.standalone) { taskRow($0, selectedWork: selectedWork) }
                        Divider().padding(.vertical, 10)
                    }
                    sectionTitle(searching ? "MATCHING WAVES" : "WAVES")
                    ForEach(presentation.waves) { branch in
                        waveRow(branch.wave, count: branch.tasks.count, selectedWork: selectedWork)
                        if searching || navigation.expandedWaves.contains(branch.id) {
                            ForEach(branch.tasks) { taskRow($0, selectedWork: selectedWork).padding(.leading, 12) }
                        }
                    }
                    if presentation.waves.isEmpty && presentation.available.isEmpty && presentation.standalone.isEmpty {
                        Text(searching ? "No matching Work" : "No Waves yet")
                            .foregroundStyle(palette.textSecondary).padding(12)
                            .accessibilityIdentifier("work-search-empty")
                    }
                }.padding(10)
            }
            .scrollPosition($scrollPosition)
            .onScrollGeometryChange(for: CGFloat.self) { geometry in
                max(0, geometry.contentOffset.y + geometry.contentInsets.top)
            } action: { _, offset in navigation.listScrollOffset = offset }
            HStack(spacing: Spacing.sm) {
                Image(systemName: "magnifyingglass").foregroundStyle(palette.textTertiary)
                TextField("Find Work", text: Binding(
                    get: { navigation.search },
                    set: {
                        Perf.begin(Perf.hierarchyInteraction, "filter", id: "outline")
                        navigation.search = $0
                    }))
                    .textFieldStyle(.plain).font(Typography.body(13))
                    .accessibilityIdentifier("work-search")
                    .focused($searchFocused)
            }
            .padding(10)
            .background(palette.surface.opacity(0.55), in: RoundedRectangle(cornerRadius: 7))
            .padding(10)
        }
        .font(Typography.body(12))
        .foregroundStyle(palette.text)
        .tint(palette.text)
        .buttonStyle(WorkPlainButtonStyle())
        .background(palette.surfaceMuted)
        .contextMenu { repositoryActions() }
        .accessibilityIdentifier("work-navigator")
        .onAppear { if navigation.palette == .search { searchFocused = true } }
        .onChange(of: navigation.palette) { _, presentation in
            if presentation == .search { searchFocused = true }
        }
        .task {
            await model.loadFlowCatalog()
            await model.loadWorkflowCatalog()
        }
        .onExitCommand {
            if navigation.palette == .search { navigation.palette = nil }
            else { navigation.showsNavigator = false }
        }
        .onChange(of: navigation.search) { _, _ in
            Perf.endAfterCommit(Perf.hierarchyInteraction, id: "outline")
        }
        .onChange(of: model.repoPath) { _, _ in
            Perf.endAfterCommit(Perf.hierarchyInteraction, id: "outline")
        }
        .onChange(of: workspace.waves.isEmpty, initial: true) { _, empty in
            guard !empty else { return }
            Perf.endAfterCommit(Perf.coldStart, id: "launch")
            let source = model.showsSavedWork ? "saved" : "fresh"
            DispatchQueue.main.async { LaunchJournal.home.mark(.usable, ["source": source]) }
        }
    }

    private func sectionTitle(_ title: String) -> some View {
        Text(title).font(Typography.code(10)).foregroundStyle(palette.textTertiary)
            .padding(.horizontal, 12).padding(.vertical, 10)
    }

    private func header(selectedWork: WorkReference?) -> some View {
        HStack(spacing: 4) {
            Button {
                if let onOpenRepository { onOpenRepository() } else { model.select(nil) }
            } label: {
                Text(model.repoPath.map { URL(fileURLWithPath: model.repoIdentity($0)).lastPathComponent }
                     ?? "Choose repository")
                    .font(Typography.strong(21))
                    .foregroundStyle(selectedWork == nil ? palette.workSelectedText : palette.text)
                    .lineLimit(1).padding(9)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(Rectangle())
            }
            .disabled(model.repoPath == nil)
            .workDestinationSelection(selectedWork == nil)
            .accessibilityIdentifier("work-open-repository")
            Menu {
                ForEach(repositories, id: \.self) { repo in
                    Button(URL(fileURLWithPath: repo).lastPathComponent) { selectRepository(repo) }
                        .accessibilityIdentifier("work-repository-\(repo)")
                }
                Divider()
                repositoryActions()
            } label: { Image(systemName: "ellipsis").frame(width: 20, height: 28) }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .accessibilityLabel("Choose repository").accessibilityIdentifier("work-repository")
            WorkGlyphButton("sidebar.left", label: model.navigation.showsNavigator ? "Hide sidebar" : "Pin sidebar",
                identifier: "work-toggle-sidebar") {
                if model.navigation.showsNavigator { model.navigation.toggleNavigator() }
                else { model.navigation.pinNavigator() }
            }
        }.padding(12)
    }

    private func waveRow(_ wave: WaveProjection, count: Int, selectedWork: WorkReference?) -> some View {
        let selected = selectedWork == wave.id.work
        let expanded = model.navigation.expandedWaves.contains(wave.id)
        return VStack(alignment: .leading, spacing: 0) {
            Button { open(wave.id.work) } label: {
                VStack(alignment: .leading, spacing: 6) {
                    Text(wave.roadmap.wave.displayName).font(Typography.strong(17))
                        .foregroundStyle(selected ? palette.workSelectedText : palette.emphasisInk).lineLimit(2)
                    if !wave.roadmap.wave.goal.isEmpty {
                        Text(wave.roadmap.wave.goal).font(Typography.body(12)).lineLimit(2)
                            .foregroundStyle(selected ? palette.workSelectedText : palette.textSecondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(14).contentShape(Rectangle())
            }
            .accessibilityIdentifier("work-wave-\(wave.roadmap.wave.id)")
            .help("Open \(wave.roadmap.wave.displayName) conversation · Command-click to open beside current Work")
            Button {
                if expanded { model.navigation.expandedWaves.remove(wave.id) }
                else { model.navigation.expandedWaves.insert(wave.id) }
            } label: {
                Text("\(count) \(count == 1 ? "Task" : "Tasks") · \(expanded ? "Hide" : "Show")")
                    .font(Typography.body(10))
                    .foregroundStyle(selected ? palette.workSelectedText : palette.textSecondary)
                    .padding(.horizontal, 14).padding(.bottom, 12)
                    .contentShape(Rectangle())
            }
            .accessibilityLabel("\(expanded ? "Hide" : "Show") \(wave.roadmap.wave.displayName) Tasks")
            .accessibilityIdentifier("work-fold-\(wave.roadmap.wave.id)")
        }
        .workDestinationSelection(selected)
        .accessibilityAddTraits(selected ? [.isSelected] : [])
        .contextMenu {
            ForEach(wave.sessions) { session in Button("Open \(session.title)") { onOpenSession(session) } }
            subjectActions(wave.id.work, title: wave.roadmap.wave.displayName)
        }
    }

    private func taskRow(_ row: WorkOutlineRow, selectedWork: WorkReference?, attention: WorkAttentionItem? = nil) -> some View {
        let selected = selectedWork == row.id.work
        return VStack(alignment: .leading, spacing: 7) {
            Text(row.title).font(Typography.strong(14)).lineLimit(2)
                .foregroundStyle(selected ? palette.workSelectedText : palette.text)
                .frame(maxWidth: .infinity, alignment: .leading).allowsHitTesting(false)
            if let attention {
                Text(attention.waveName ?? "No Wave").font(Typography.body(11))
                    .foregroundStyle(selected ? palette.workSelectedText : palette.emphasisInk)
                    .allowsHitTesting(false)
            }
            if let task = row.task {
                Text(task.task.description).font(Typography.body(12)).lineLimit(2)
                    .foregroundStyle(selected ? palette.workSelectedSecondaryText : palette.textSecondary)
                    .allowsHitTesting(false)
                TaskRowMetadata(task: task,
                    status: SessionProgramStatus(sessions: attention.map { [$0.session] } ?? row.inlineSessions, unavailable: model.sessions.errorMessage),
                    localMachineId: model.observingMachineId, selected: selected)
            }
        }
        .padding(14).frame(minHeight: 82, alignment: .leading)
        .background {
            Button {
                if let attention { openAttention(attention) } else { open(row.id.work) }
            } label: { Color.clear.contentShape(Rectangle()) }
                .buttonStyle(.plain).help(row.title).accessibilityLabel(row.title)
                .accessibilityIdentifier(attention.map { "work-attention-\($0.work.kind.rawValue)-\($0.work.id)" } ?? "work-task-\(row.id.work.id)")
        }
        .workDestinationSelection(selected)
        .accessibilityAddTraits(selected ? [.isSelected] : [])
        .contextMenu {
            ForEach(row.inlineSessions) { session in Button("Open \(session.title)") { onOpenSession(session) } }
            subjectActions(row.id.work, title: row.title)
        }
    }

    private func openAttention(_ item: WorkAttentionItem) {
        // Session opening owns selection too: an already retained surface may
        // belong to another workspace. One opening means one Back entry.
        onOpenSession(item.session)
    }

    private func open(_ work: WorkReference) {
        model.navigation.search = ""
        if work.kind == .task, let onOpenTask { onOpenTask(work) }
        else if work.kind == .wave, let onOpenWave { onOpenWave(work) }
        else { model.select(work) }
    }

    @ViewBuilder private var forReadErrors: some View {
        if let message = model.workStatus.message {
            warning(message).accessibilityIdentifier("work-status")
        }
        ForEach(model.projection.waves) { wave in
            if let reason = wave.roadmap.tasks.unavailableReason { warning(reason) }
            if case .available(_, let truncated) = wave.roadmap.tasks, truncated {
                warning("Planning is partial; more Tasks exist.")
            }
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
        if let flows = model.flowCatalog.value, !flows.isEmpty {
            Menu("Flow definitions") {
                ForEach(flows, id: \.name) { entry in
                    Button(entry.name) { model.navigation.palette = .flow(entry.name) }
                }
            }
        }
        if let workflows = model.workflowCatalog.value, !workflows.isEmpty {
            Menu("Workflow definitions") {
                ForEach(workflows, id: \.name) { entry in
                    Button(entry.name) { model.navigation.palette = .workflow(entry.name) }
                }
            }
        }
    }

    private func selectRepository(_ path: String) {
        Perf.begin(Perf.hierarchyInteraction, "repository", id: "outline")
        model.setRepoPath(path)
        Task.detached { try? saveLoopflowState(LoopflowState(selectedRepoPath: path.normalizedFilePath)) }
    }

    private func warning(_ text: String) -> some View {
        Label(text, systemImage: "exclamationmark.triangle")
            .foregroundStyle(palette.waiting).textSelection(.enabled).padding(4)
    }
}
