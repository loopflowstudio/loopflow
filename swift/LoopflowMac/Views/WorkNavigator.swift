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
        let outline = workspace.outline(collapsed: [], search: navigation.search,
            planningReadable: model.roadmap.value != nil && model.roadmap.errorMessage == nil,
            selection: model.selection)
        let rows = outline.filter { $0.task != nil }
        let matchingIDs = Set(outline.flatMap { [$0.id] + $0.ancestors.map(\.key) })
        let waves = workspace.waves.filter { matchingIDs.contains($0.id) }
        let selectedWaveID = workspace.breadcrumb(selection: model.selection, sessionId: nil)?.wave?.id
        let selectedWave = workspace.waves.first { $0.id == selectedWaveID }
        let searching = !navigation.search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        let attention = workspace.attention(unavailable: model.sessions.errorMessage)
        VStack(spacing: 0) {
            header(wave: searching ? nil : selectedWave)
            attentionQueue(attention)
            forReadErrors
            if let wave = selectedWave, !searching {
                WorkTaskList(navigation: navigation, waveID: wave.roadmap.wave.id) {
                    LazyVStack(alignment: .leading, spacing: 5) {
                        sectionTitle("TASKS IN \(wave.roadmap.wave.displayName.uppercased())")
                        let tasks = rows.filter { $0.ancestors.contains { $0.key == wave.id } }
                        ForEach(tasks) { taskRow($0) }
                        if tasks.isEmpty { Text("No active Tasks").foregroundStyle(palette.textSecondary).padding(12) }
                    }.padding(10)
                }.id(wave.id)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 5) {
                        sectionTitle(searching ? "MATCHING WORK" : "WAVES")
                        ForEach(waves) { wave in
                            let tasks = rows.filter { $0.ancestors.contains { $0.key == wave.id } }
                            waveRow(wave, count: tasks.count,
                                    waiting: attention.filter { $0.waveID == wave.roadmap.wave.id }.count)
                            if searching { ForEach(tasks) { taskRow($0) } }
                        }
                        if waves.isEmpty {
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
            }
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

    private func header(wave: WaveProjection?) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 4) {
                Button {
                    if let onOpenRepository { onOpenRepository() } else { model.select(nil) }
                } label: {
                    Text(model.repoPath.map { URL(fileURLWithPath: model.repoIdentity($0)).lastPathComponent }
                         ?? "Choose repository")
                        .font(wave == nil ? Typography.strong(21) : Typography.body(14))
                        .foregroundStyle(model.selection == nil ? palette.workSelectedText : palette.textSecondary)
                        .lineLimit(1).padding(9)
                        .frame(maxWidth: wave == nil ? .infinity : nil, alignment: .leading)
                        .contentShape(Rectangle())
                }
                .disabled(model.repoPath == nil)
                .workDestinationSelection(model.selection == nil)
                .accessibilityIdentifier("work-open-repository")
                if let wave {
                    Text("/").foregroundStyle(palette.textTertiary)
                    Button { open(wave.id.work) } label: {
                        Text(wave.roadmap.wave.displayName).font(Typography.strong(14)).lineLimit(1)
                            .foregroundStyle(model.isSelected(wave.id.work) ? palette.workSelectedText : palette.emphasisInk)
                            .padding(9).contentShape(Rectangle())
                    }
                    .workDestinationSelection(model.isSelected(wave.id.work))
                    .accessibilityIdentifier("work-wave-conversation-\(wave.roadmap.wave.id)")
                    Spacer(minLength: 0)
                }
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
            }
            if let wave, !wave.roadmap.wave.goal.isEmpty {
                Text(wave.roadmap.wave.goal).font(Typography.body(11)).foregroundStyle(palette.textSecondary)
                    .lineLimit(2).padding(.horizontal, 9)
            }
        }.padding(12)
    }

    private func waveRow(_ wave: WaveProjection, count: Int, waiting: Int) -> some View {
        Button { open(wave.id.work) } label: {
            VStack(alignment: .leading, spacing: 6) {
                Text(wave.roadmap.wave.displayName).font(Typography.strong(17))
                    .foregroundStyle(model.isSelected(wave.id.work) ? palette.workSelectedText : palette.emphasisInk).lineLimit(2)
                if !wave.roadmap.wave.goal.isEmpty {
                    Text(wave.roadmap.wave.goal).font(Typography.body(12)).lineLimit(2)
                        .foregroundStyle(model.isSelected(wave.id.work) ? palette.workSelectedText : palette.textSecondary)
                }
                HStack(spacing: 4) {
                    Text("\(count) Tasks · Open Wave conversation")
                    if waiting > 0 {
                        Spacer(minLength: 0)
                        Label("\(waiting)", systemImage: "tray.fill")
                    }
                }.font(Typography.body(10))
                    .foregroundStyle(model.isSelected(wave.id.work) ? palette.workSelectedText : palette.textSecondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(14).frame(minHeight: 100).contentShape(Rectangle())
        }
        .workDestinationSelection(model.isSelected(wave.id.work))
        .accessibilityAddTraits(model.selection == wave.id.work ? [.isSelected] : [])
        .accessibilityLabel("\(wave.roadmap.wave.displayName), \(count) Tasks, \(waiting) need attention")
        .accessibilityIdentifier("work-wave-\(wave.roadmap.wave.id)")
        .help("Open \(wave.roadmap.wave.displayName) conversation · Command-click to open beside current Work")
        .contextMenu {
            ForEach(wave.sessions) { session in Button("Open \(session.title)") { onOpenSession(session) } }
            subjectActions(wave.id.work, title: wave.roadmap.wave.displayName)
        }
    }

    private func taskRow(_ row: WorkOutlineRow) -> some View {
        let selected = model.isSelected(row.id.work)
        return VStack(alignment: .leading, spacing: 7) {
            Text(row.title).font(Typography.strong(14)).lineLimit(2)
                .foregroundStyle(selected ? palette.workSelectedText : palette.text)
                .frame(maxWidth: .infinity, alignment: .leading).allowsHitTesting(false)
            if let task = row.task {
                Text(task.task.description).font(Typography.body(12)).lineLimit(2)
                    .foregroundStyle(selected ? palette.workSelectedSecondaryText : palette.textSecondary)
                    .allowsHitTesting(false)
                TaskRowMetadata(task: task,
                    status: SessionProgramStatus(sessions: row.inlineSessions, unavailable: model.sessions.errorMessage),
                    localMachineId: model.observingMachineId, selected: selected)
            }
        }
        .padding(14).frame(minHeight: 82, alignment: .leading)
        .background {
            Button { open(row.id.work) } label: { Color.clear.contentShape(Rectangle()) }
                .buttonStyle(.plain).help(row.title).accessibilityLabel(row.title)
                .accessibilityIdentifier("work-task-\(row.id.work.id)")
        }
        .workDestinationSelection(selected)
        .accessibilityAddTraits(selected ? [.isSelected] : [])
        .contextMenu {
            ForEach(row.inlineSessions) { session in Button("Open \(session.title)") { onOpenSession(session) } }
            subjectActions(row.id.work, title: row.title)
        }
    }

    @ViewBuilder private func attentionQueue(_ items: [WorkAttentionItem]) -> some View {
        if !items.isEmpty {
            VStack(alignment: .leading, spacing: 3) {
                HStack {
                    Label("NEEDS YOU · \(items.count)", systemImage: "tray")
                        .font(Typography.code(10)).foregroundStyle(palette.waiting)
                    Spacer()
                    if items.count > 3 {
                        Menu("All \(items.count)") {
                            ForEach(items) { item in
                                Button("\(item.waveName) · \(item.title) · \(item.reason)") { openAttention(item) }
                            }
                        }.menuStyle(.borderlessButton).fixedSize()
                        .accessibilityIdentifier("work-attention-overflow")
                    }
                }.padding(.bottom, 4)
                ForEach(visibleAttention(items)) { item in
                    let selected = model.isSelected(item)
                    Button { openAttention(item) } label: {
                        HStack(spacing: 8) {
                            Text(item.waveName).foregroundStyle(selected ? palette.workSelectedText : palette.emphasisInk).frame(width: 88, alignment: .leading)
                            Text(item.title).foregroundStyle(selected ? palette.workSelectedText : palette.text).frame(maxWidth: .infinity, alignment: .leading)
                            Text(item.reason).foregroundStyle(selected ? palette.workSelectedText : palette.waiting)
                        }.font(Typography.body(11)).lineLimit(1).padding(6).contentShape(Rectangle())
                    }
                    .workDestinationSelection(selected)
                    .help(item.detail)
                    .accessibilityIdentifier("work-attention-\(item.work.kind.rawValue)-\(item.work.id)")
                }
            }.padding(.horizontal, 15).padding(.vertical, 10)
            Divider()
        }
    }

    private func visibleAttention(_ items: [WorkAttentionItem]) -> [WorkAttentionItem] {
        var visible = Array(items.prefix(3))
        if let selected = items.first(where: { model.isSelected($0) }), !visible.contains(where: { $0.id == selected.id }) {
            visible[visible.count - 1] = selected
        }
        return visible
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

/// Each Wave owns its viewport through drill-in and keyboard history.
private struct WorkTaskList<Content: View>: View {
    let navigation: WorkNavigation
    let waveID: String
    @ViewBuilder let content: Content
    @State private var position: ScrollPosition

    init(navigation: WorkNavigation, waveID: String, @ViewBuilder content: () -> Content) {
        self.navigation = navigation
        self.waveID = waveID
        self.content = content()
        _position = State(initialValue: ScrollPosition(y: navigation.taskListScrollOffsets[waveID] ?? 0))
    }

    var body: some View {
        ScrollView { content }
            .scrollPosition($position)
            .onScrollGeometryChange(for: CGFloat.self) { geometry in
                max(0, geometry.contentOffset.y + geometry.contentInsets.top)
            } action: { _, offset in navigation.taskListScrollOffsets[waveID] = offset }
    }
}
