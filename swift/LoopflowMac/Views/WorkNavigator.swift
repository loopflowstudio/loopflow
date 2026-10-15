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
        let displayedWave = waves.first { $0.id == selectedWaveID }
            ?? waves.first { $0.roadmap.wave.id == navigation.navigatorWaveID }
            ?? waves.first
        let attention = workspace.attention(unavailable: model.sessions.errorMessage)
        VStack(spacing: 0) {
            header
            attentionQueue(attention)
            forReadErrors
            HStack(spacing: 0) {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 4) {
                        Text("WAVES").font(Typography.code(10)).foregroundStyle(palette.textTertiary)
                            .padding(.horizontal, 9).padding(.vertical, 6)
                        ForEach(waves) { wave in
                            waveRow(wave,
                                    count: rows.filter { $0.ancestors.contains { $0.key == wave.id } }.count,
                                    waiting: attention.filter { $0.waveID == wave.roadmap.wave.id }.count)
                        }
                    }.padding(6)
                }
                .scrollPosition($scrollPosition)
                .onScrollGeometryChange(for: CGFloat.self) { geometry in
                    max(0, geometry.contentOffset.y + geometry.contentInsets.top)
                } action: { _, offset in navigation.listScrollOffset = offset }
                .frame(width: 144)
                Rectangle().fill(palette.border).frame(width: 1)
                if let wave = displayedWave {
                    taskColumn(wave, rows: rows.filter { $0.ancestors.contains { $0.key == wave.id } })
                        .id(wave.id)
                } else {
                    Text(navigation.search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                         ? "No Waves yet" : "No matching Work").foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("work-search-empty")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
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

    private var header: some View {
        HStack(spacing: 6) {
            Button {
                if let onOpenRepository { onOpenRepository() } else { model.select(nil) }
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "folder")
                    VStack(alignment: .leading, spacing: 2) {
                        Text(model.repoPath.map { URL(fileURLWithPath: model.repoIdentity($0)).lastPathComponent }
                            ?? "Choose repository").font(Typography.sectionTitle(22)).lineLimit(1)
                        Text("Repository").font(Typography.body(10)).foregroundStyle(palette.textSecondary)
                    }
                    Spacer(minLength: 0)
                }.padding(9).contentShape(Rectangle())
            }
            .disabled(model.repoPath == nil)
            .workSelection(model.selection == nil)
            .accessibilityAddTraits(model.selection == nil ? [.isSelected] : [])
            .accessibilityIdentifier("work-open-repository")
            Menu {
                ForEach(repositories, id: \.self) { repo in
                    Button(URL(fileURLWithPath: repo).lastPathComponent) { selectRepository(repo) }
                        .accessibilityIdentifier("work-repository-\(repo)")
                }
                Divider()
                repositoryActions()
            } label: { Image(systemName: "ellipsis").frame(width: 24, height: 28) }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .accessibilityLabel("Choose repository").accessibilityIdentifier("work-repository")
        }.padding(8)
    }

    private func waveRow(_ wave: WaveProjection, count: Int, waiting: Int) -> some View {
        Button { model.selectWaveContext(wave) } label: {
            VStack(alignment: .leading, spacing: 6) {
                Text(wave.roadmap.wave.displayName).font(Typography.textStrong)
                    .foregroundStyle(palette.emphasisInk).lineLimit(2)
                HStack(spacing: 4) {
                    Text("\(count) Tasks").foregroundStyle(palette.textSecondary)
                    if waiting > 0 {
                        Spacer(minLength: 0)
                        Label("\(waiting)", systemImage: "tray.fill").foregroundStyle(palette.waiting)
                    }
                }.font(Typography.body(10))
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(9).frame(minHeight: 64).contentShape(Rectangle())
        }
        .workSelection(model.selection == wave.id.work)
        .accessibilityAddTraits(model.selection == wave.id.work ? [.isSelected] : [])
        .accessibilityLabel("\(wave.roadmap.wave.displayName), \(count) Tasks, \(waiting) need attention")
        .accessibilityIdentifier("work-wave-\(wave.roadmap.wave.id)")
        .help("Return to your last destination in \(wave.roadmap.wave.displayName)")
        .contextMenu {
            ForEach(wave.sessions) { session in Button("Open \(session.title)") { onOpenSession(session) } }
            subjectActions(wave.id.work, title: wave.roadmap.wave.displayName)
        }
    }

    private func taskColumn(_ wave: WaveProjection, rows: [WorkOutlineRow]) -> some View {
        VStack(spacing: 0) {
            Button {
                if let onOpenWave { onOpenWave(wave.id.work) } else { model.select(wave.id.work) }
            } label: {
                VStack(alignment: .leading, spacing: 4) {
                    Text(wave.roadmap.wave.displayName).font(Typography.textStrong).foregroundStyle(palette.emphasisInk)
                    if !wave.roadmap.wave.goal.isEmpty {
                        Text(wave.roadmap.wave.goal).font(Typography.body(11)).lineLimit(2)
                            .foregroundStyle(palette.textSecondary)
                    }
                    Label("Wave conversation", systemImage: "bubble.left").font(Typography.body(11))
                        .foregroundStyle(palette.textSecondary)
                }.frame(maxWidth: .infinity, alignment: .leading).padding(12).contentShape(Rectangle())
            }
            .accessibilityIdentifier("work-wave-conversation-\(wave.roadmap.wave.id)")
            WorkTaskColumn(navigation: model.navigation, waveID: wave.roadmap.wave.id) {
                LazyVStack(alignment: .leading, spacing: 5) {
                    ForEach(rows) { taskRow($0) }
                    if rows.isEmpty {
                        Text(model.navigation.search.isEmpty ? "No active Tasks" : "No matching Tasks")
                            .foregroundStyle(palette.textSecondary).padding(12)
                    }
                }.padding(6)
            }
        }.frame(maxWidth: .infinity)
    }

    private func taskRow(_ row: WorkOutlineRow) -> some View {
        let runtimeWork = row.task?.runtime.map { WorkReference.task(id: $0.workId) }
        let selected = model.selection.map { $0 == row.id.work || $0 == runtimeWork } ?? false
        return VStack(alignment: .leading, spacing: 7) {
            Text(row.title).font(Typography.textStrong).lineLimit(2)
                .frame(maxWidth: .infinity, alignment: .leading).allowsHitTesting(false)
            if let task = row.task {
                TaskRowMetadata(task: task,
                    status: SessionProgramStatus(sessions: row.inlineSessions, unavailable: model.sessions.errorMessage),
                    localMachineId: model.observingMachineId)
            }
        }
        .padding(10).frame(minHeight: 66, alignment: .leading)
        .background {
            Button { open(row.id.work) } label: { Color.clear.contentShape(Rectangle()) }
                .buttonStyle(.plain).help(row.title).accessibilityLabel(row.title)
                .accessibilityIdentifier("work-task-\(row.id.work.id)")
        }
        .workSelection(selected)
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
                ForEach(Array(items.prefix(3))) { item in
                    Button { openAttention(item) } label: {
                        HStack(spacing: 8) {
                            Text(item.waveName).foregroundStyle(palette.emphasisInk).frame(width: 88, alignment: .leading)
                            Text(item.title).frame(maxWidth: .infinity, alignment: .leading)
                            Text(item.reason).foregroundStyle(palette.waiting)
                        }.font(Typography.body(11)).lineLimit(1).padding(.vertical, 6).contentShape(Rectangle())
                    }
                    .help(item.detail)
                    .accessibilityIdentifier("work-attention-\(item.work.kind.rawValue)-\(item.work.id)")
                }
            }.padding(.horizontal, 15).padding(.vertical, 10)
            Divider()
        }
    }

    private func openAttention(_ item: WorkAttentionItem) {
        // Session opening owns selection too: an already retained surface may
        // belong to another workspace. One opening means one Back entry.
        onOpenSession(item.session)
    }

    private func open(_ work: WorkReference) {
        if work.kind == .task, let onOpenTask { onOpenTask(work) } else { model.select(work) }
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

/// Each Wave owns its viewport; recreating a column never resets another Wave.
private struct WorkTaskColumn<Content: View>: View {
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
