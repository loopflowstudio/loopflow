import AppKit
import Loopflow
import SwiftUI

/// Working positions belong to navigation; observation supplies current facts.
struct WorkNavigator: View {
    @Bindable var model: WorkModel
    let onOpenSession: (SessionRecord) -> Void
    var onNewTaskSession: ((String) -> Void)?
    @Environment(\.palette) private var palette
    @State private var actionTaskID: String?
    @FocusState private var focusedWork: WorkReference?
    @State private var focusMemory = SidebarResponderMemory()
    /// The clicked Wave row moves to the top; a Dashboard's label into the list header.
    @Namespace private var scopeTitle
    @State private var wavesHeight: CGFloat = 130
    /// The Wave row just clicked, shown before its selection moves it.
    @State private var pressedScope: WorkNavigation.SidebarScope?
    /// Pointing at the Waves section reveals New Wave; only that row reads it.
    @State private var wavesHover = HoverFlag()
    @State private var taskDrag = TaskOrderDrag.State()
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var listTitle: String {
        let filter = model.navigation.sidebarFilter
        let scope: String
        switch model.navigation.sidebarScope {
        case .repository: return filter.rawValue
        case .wave: scope = model.sidebarWave?.wave.displayName ?? "Wave unavailable"
        case .dashboard(let id): scope = model.dashboard(id).map { $0.name ?? id } ?? "Dashboard unavailable"
        }
        return filter == .working ? scope : "\(scope) · \(filter.rawValue)"
    }

    private var emptyListText: String {
        switch model.navigation.sidebarScope {
        case .repository: "No working Tasks in this list"
        case .wave: "No working Tasks in this Wave"
        case .dashboard(let id): model.dashboard(id) == nil ? "This Dashboard is no longer available." : "No Tasks in this Dashboard"
        }
    }

    private var rowOrder: [WorkReference] {
        (model.sidebarTaskIDs + model.sidebarAutonomousTaskIDs).map { .task(id: $0) }
            + model.visibleRoadmaps.map { .wave(id: $0.wave.id) }
    }

    private func moveFocus(_ direction: MoveCommandDirection) {
        guard direction == .up || direction == .down, !rowOrder.isEmpty else { return }
        let offset = direction == .down ? 1 : -1
        let current = focusedWork.flatMap { rowOrder.firstIndex(of: $0) }
        let index = current.map { min(rowOrder.count - 1, max(0, $0 + offset)) } ?? (offset > 0 ? 0 : rowOrder.count - 1)
        focusedWork = rowOrder[index]
    }

    var body: some View {
        VStack(spacing: 0) {
            if let status = model.workStatus.message {
                Text(status).font(Typography.body(11)).foregroundStyle(palette.textSecondary).padding(10)
                    .accessibilityIdentifier("work-status")
            }
            if let error = model.dashboardError ?? model.selectionPlans.errorMessage {
                Text(error).font(Typography.body(11)).foregroundStyle(palette.warning).padding(10)
            }
            // The repository tops the sidebar; a selected Wave moves up under it.
            VStack(alignment: .leading, spacing: 5) {
                repositoryRow
                if let wave = model.sidebarWave { waveRow(wave) }
            }.padding([.horizontal, .top], 8)
            WorkTaskList(navigation: model.navigation) {
                // Read once: each read re-filters the plan and every Task's Sessions.
                let taskIDs = model.sidebarTaskIDs
                let scope = model.navigation.sidebarScope
                VStack(alignment: .leading, spacing: 5) {
                    if case .dashboard = scope {
                        HStack {
                            Text(listTitle.uppercased()).lineLimit(1)
                                .matchedGeometryEffect(id: scope, in: scopeTitle)
                            Spacer()
                            Text("\(taskIDs.count)")
                        }.font(Typography.code(10)).foregroundStyle(palette.textSecondary).padding(10)
                    }
                    // ⌘N counts the rows on screen; sidebar Task IDs are already unique.
                    ForEach(Array(taskIDs.enumerated()), id: \.element) { index, id in
                        taskRow(id, slotHint: index < 9 ? "⌘\(index + 1)" : nil)
                    }
                    if taskIDs.isEmpty {
                        Text(emptyListText)
                            .font(Typography.body(12))
                            .foregroundStyle(palette.textSecondary).padding(10)
                    }
                    autonomousSection
                    if let selection = model.selection, selection.kind == .task,
                       !(taskIDs + model.sidebarAutonomousTaskIDs).contains(where: { model.isSelected(.task(id: $0)) }) {
                        VStack(alignment: .leading, spacing: 5) {
                            Text("Open Task is outside this list")
                            Button("Show working tasks") {
                                model.navigation.sidebarFilter = .working
                                model.navigation.sidebarWaveID = nil
                            }
                        }.font(Typography.body(11)).padding(10)
                    }
                }.padding(8)
            }
            WaveSectionDivider(navigation: model.navigation)
            waveSection
        }
        .animation(reduceMotion ? nil : .snappy(duration: 0.25), value: model.navigation.sidebarScope)
        .foregroundStyle(palette.text).background(palette.surfaceMuted)
        .accessibilityIdentifier("work-navigator")
        .background(SidebarResponderCapture(memory: focusMemory))
        .onMoveCommand(perform: moveFocus)
        .onKeyPress(.home) { focusedWork = rowOrder.first; return .handled }
        .onKeyPress(.end) { focusedWork = rowOrder.last; return .handled }
        .onKeyPress("f", phases: .down) { key in
            guard key.modifiers.intersection([.command, .shift, .control, .option]) == .command,
                  focusedWork != nil else { return .ignored }
            model.navigation.palette = .sidebarSearch
            return .handled
        }
        .onKeyPress(KeyEquivalent("\u{F70D}"), phases: .down) { key in
            guard key.modifiers.intersection([.command, .shift, .control, .option]) == .shift,
                  let focusedWork, focusedWork.kind == .task,
                  model.task(id: focusedWork.id) != nil else { return .ignored }
            actionTaskID = focusedWork.id
            return .handled
        }
        .onChange(of: model.navigation.sidebarFocusRequested, initial: true) { _, requested in
            guard requested else { return }
            focusMemory.window = NSApp.keyWindow
            focusMemory.previous = NSApp.keyWindow?.firstResponder
            focusedWork = rowOrder.first { model.isSelected($0) } ?? rowOrder.first
            model.navigation.sidebarFocusRequested = false
        }
        .onKeyPress(.return) {
            guard let focusedWork else { return .ignored }
            model.select(focusedWork)
            return .handled
        }
        .renderFlash()
        .onExitCommand { focusedWork = nil; focusMemory.restore() }
        .onDisappear { if focusedWork != nil { focusMemory.restore() } }
    }

    /// Pinned below the Task list so any Wave is a click away however far the
    /// list scrolls; it starts about three rows tall and its divider drags.
    private var waveSection: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 5) {
                ForEach(model.visibleRoadmaps.filter { $0.wave.id != model.sidebarWave?.wave.id }, id: \.wave.id) { waveRow($0) }
                NewWaveRow(model: model, hover: wavesHover)
            }.padding(8)
        }
        .onScrollGeometryChange(for: CGFloat.self) { $0.contentSize.height } action: { _, height in
            wavesHeight = height
        }
        .frame(height: min(wavesHeight, model.navigation.sidebarWavesHeight))
        .hoverSource(wavesHover)
    }

    /// The repository is the default Wave; it lists Working Tasks and can't be edited.
    private var repositoryRow: some View {
        waveRow(scope: .repository, name: model.repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Repository",
                icon: nil, count: model.workingTaskMembers.count, unavailableReason: nil)
    }

    private func waveRow(_ wave: WaveRoadmap) -> some View {
        waveRow(scope: .wave(wave.wave.id), name: wave.wave.displayName, icon: "water.waves",
                count: model.startedTaskCount(wave), unavailableReason: wave.tasks.unavailableReason)
    }

    private func waveRow(scope: WorkNavigation.SidebarScope, name: String, icon: String?,
                         count: Int, unavailableReason: String?) -> some View {
        let reference: WorkReference? = if case .wave(let id) = scope { .wave(id: id) } else { nil }
        let state: SidebarSelection = pressedScope == scope || model.navigation.destination == .work(reference) ? .shown
            : model.navigation.sidebarScope == scope ? .containing : .none
        return Button {
            // The click darkens the row first; it moves once that has landed.
            guard !reduceMotion else { model.select(reference); return }
            withAnimation(.easeOut(duration: 0.12)) { pressedScope = scope }
            Task {
                try? await Task.sleep(for: .milliseconds(120))
                model.select(reference)
                pressedScope = nil
            }
        } label: {
            HStack {
                if let icon { Image(systemName: icon) }
                Text(name).lineLimit(1)
                Spacer()
                if let unavailableReason {
                    Image(systemName: "exclamationmark.triangle").help(unavailableReason)
                } else {
                    Text("\(count)").font(Typography.code(10)).help("\(count) working or autonomous Tasks")
                }
            }.font(Typography.body(13)).padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .foregroundStyle(state == .shown ? palette.workSelectedText : palette.text)
                .background(SidebarSelectionShape(state: state))
                // The whole row clicks, as a Task row does, not only its text.
                .contentShape(RoundedRectangle(cornerRadius: 6))
        }.buttonStyle(.plain)
            .modifier(FocusedWork(focus: $focusedWork, reference: reference))
            .accessibilityLabel("\(name), \(count) working or autonomous Tasks")
            .accessibilityIdentifier("work-wave-\(reference?.id ?? "repository")")
            .matchedGeometryEffect(id: scope, in: scopeTitle)
    }

    /// Shipping Tasks run to merge without anyone; they sit below, quieter, under a heading.
    @ViewBuilder private var autonomousSection: some View {
        let ids = model.sidebarAutonomousTaskIDs
        if !ids.isEmpty {
            HStack {
                Text("Autonomous")
                Spacer()
                Text("\(ids.count)").font(Typography.code(10))
            }
            .font(Typography.strong(12)).foregroundStyle(palette.textSecondary)
            .padding(.horizontal, 10).padding(.top, 14).padding(.bottom, 2)
            .help("Shipping: running to merge with nothing left for a person to do")
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("work-autonomous-heading")
            ForEach(ids, id: \.self) { id in
                taskRow(id, slotHint: nil, quiet: true)
            }
        }
    }

    private func taskRow(_ id: String, slotHint: String?, quiet: Bool = false) -> some View {
        let found = model.task(id: id)
        let selected = model.isSelected(.task(id: id))
        let muted = quiet || model.selection?.kind == .wave && found?.wave?.wave.id != model.selection?.id
        let sessions = model.taskSessions(id)
        let status = SessionProgramStatus(sessions: sessions, unavailable: model.sessions.errorMessage)
        let title = found?.task.task.name ?? "Task unavailable · \(id)"
        // Rows slide under the pointer while scrolling. Only the ⌘N hint reads the
        // row's hover, so a crossing re-renders that hint, not rows or the sidebar.
        return RowHover { hover in VStack(alignment: .leading, spacing: 6) {
            Text(title).font(Typography.strong(13)).lineLimit(2).help(title)
                .allowsHitTesting(false)
            // Only a live excerpt ages; a row showing its summary needs no clock.
            let fallback = found?.task.task.description ?? "Planning unavailable"
            let current = status.preview(now: .now, fallback: fallback, sessions: sessions)
            if current.age == nil {
                rowDetail(current, slotHint: slotHint, found: found, status: status, selected: selected, hover: hover)
            } else {
                TimelineView(.periodic(from: .now, by: 5)) { clock in
                    rowDetail(status.preview(now: clock.date, fallback: fallback, sessions: sessions),
                              slotHint: slotHint, found: found, status: status, selected: selected, hover: hover)
                }
            }
        }
        .padding(12).frame(maxWidth: .infinity, alignment: .leading)
        .foregroundStyle(selected ? palette.workSelectedText : muted ? palette.textSecondary : palette.text)
        .background {
            Button { model.select(.task(id: id)) } label: {
                SidebarSelectionShape(state: selected ? .shown : .none)
                    .contentShape(Rectangle())
            }.buttonStyle(.plain).focused($focusedWork, equals: .task(id: id)).accessibilityLabel(title)
                .accessibilityAddTraits(selected ? .isSelected : [])
                .accessibilityIdentifier("work-task-\(id)")
        }
        .overlay(alignment: .leading) {
            RoundedRectangle(cornerRadius: 1)
                .fill(status.tone?.ink(in: palette) ?? palette.borderActive)
                .frame(width: 2).padding(.vertical, 12).padding(.leading, 4)
                .allowsHitTesting(false)
        }
        .renderFlash()
        }
        .modifier(TaskOrderDragging(model: model, workID: model.navigation.sidebarScope == .repository ? model.workingTaskWorkID(row: id) : nil,
                                    drag: taskDrag))
        .contextMenu {
            actions(for: id)
        }
        .popover(isPresented: Binding(get: { actionTaskID == id }, set: { if !$0 { actionTaskID = nil } })) {
            VStack(alignment: .leading, spacing: 10) { actions(for: id) }
                .padding(12)
                .onExitCommand { actionTaskID = nil }
        }
    }

    /// The preview line and metadata beneath a row's title.
    private func rowDetail(_ preview: (text: String, help: String, age: String?), slotHint: String?, found: (wave: WaveRoadmap?, task: RoadmapTask)?,
                           status: SessionProgramStatus, selected: Bool, hover: HoverFlag) -> some View {
        let shipping = found?.task.shipping.map { "\($0.label) · \($0.reason)" }
        return VStack(alignment: .leading, spacing: 6) {
            Text(shipping ?? preview.text).font(Typography.code(11)).lineLimit(status.session?.availability == .available ? 2 : 1)
                .foregroundStyle(selected ? palette.workSelectedSecondaryText : palette.textSecondary)
                .help(preview.help).allowsHitTesting(false)
            if let task = found?.task {
                TaskRowMetadata(task: task, status: status, selected: selected,
                    slotHint: slotHint,
                    showsSlotHint: model.navigation.showsSlotHints, hover: hover,
                    sourceAge: preview.age)
            } else {
                Text("Planning unavailable").font(Typography.code(10))
                    .foregroundStyle(palette.textSecondary).allowsHitTesting(false)
            }
        }
        .renderFlash()
    }

    @ViewBuilder private func actions(for id: String) -> some View {
        if let found = model.task(id: id) {
            TaskActionItems(model: model, task: found.task, wave: found.wave?.wave,
                newConversation: onNewTaskSession.map { action in { action(id) } },
                openSession: onOpenSession)
        }
    }
}

/// Compared by model alone: its callbacks are closures over references, and
/// every row reads what it needs from the model itself.
extension WorkNavigator: Equatable {
    nonisolated static func == (lhs: Self, rhs: Self) -> Bool {
        MainActor.assumeIsolated { lhs.model === rhs.model }
    }
}

struct WorkBreadcrumbs: View {
    @Bindable var model: WorkModel
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 8) {
            ViewThatFits(in: .horizontal) {
                trail(showWave: true).fixedSize(horizontal: true, vertical: false)
                trail(showWave: false)
            }
            if let copy = model.previewCopy {
                WorkChip(text: "Preview", tone: .warning)
                    .fixedSize()
                    .help("Preview copy: \((copy as NSString).abbreviatingWithTildeInPath) — work here is not in the shared store")
                    .accessibilityIdentifier("preview-copy")
            }
        }
        .buttonStyle(.plain)
        .font(Typography.strong(13))
        .foregroundStyle(palette.titlebarText, palette.titlebarSecondaryText)
        .accessibilityIdentifier("work-breadcrumbs")
    }

    private func trail(showWave: Bool) -> some View {
        HStack(alignment: .center, spacing: 6) {
            Button { model.navigation.toggleNavigator() } label: { Image(systemName: "sidebar.left") }
                .frame(width: 28, height: 28).contentShape(Rectangle())
                .help("Toggle sidebar")
            Button { model.goBack() } label: { Image(systemName: "chevron.left") }
                .frame(width: 24, height: 28).contentShape(Rectangle())
                .disabled(model.navigation.backStack.isEmpty).help("Back")
            Button { model.goForward() } label: { Image(systemName: "chevron.right") }
                .frame(width: 24, height: 28).contentShape(Rectangle())
                .disabled(model.navigation.forwardStack.isEmpty).help("Forward")
            Button(model.repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Work") {
                // The parent crumb of a zoomed part zooms out.
                if model.navigation.waveView == .repository { model.setZoomed(nil, in: .repository) }
                model.select(nil)
                model.navigation.content = .terminals
            }.lineLimit(1).fixedSize()
            WorkJumpSeparator(model: model, scope: .repository, label: "Waves")
            // A Wave view in the trail, or the Wave of Work outside any board, heads
            // the crumbs after the repository; the repository page adds none.
            let board = model.navigation.trailBoard
            if case .wave(let id) = board {
                waveCrumb(id, name: model.wave(id: id)?.wave.displayName ?? id)
            } else if showWave, board == nil || board == .repository, let wave = model.breadcrumb?.wave {
                waveCrumb(wave.roadmap.wave.id, name: wave.roadmap.wave.displayName)
            }
            switch board {
            // A Dashboard opened through `lf desktop` names itself; Back returns to it.
            case .saved(let id):
                Text(model.dashboard(id)?.name ?? id).lineLimit(1).fixedSize()
                if model.breadcrumb?.task != nil { Text("/").foregroundStyle(.secondary) }
            case .workingTasks:
                Text(WorkNavigation.workingTasksTitle).lineLimit(1).fixedSize()
                if model.breadcrumb?.task != nil { Text("/").foregroundStyle(.secondary) }
            case .wave, .repository, nil: EmptyView()
            }
            if let view = model.navigation.waveView, let zoomed = model.zoomedPart(in: view) {
                ZoomCrumb(title: zoomed.title) { model.setZoomed(nil, in: view) }
            }
            if let task = model.breadcrumb?.task {
                TaskHeading(task: task.task)
                if let workspace = model.shownWorkspace, workspace.key.kind == .task, workspace.key.id == task.task.id {
                    ShowHiddenMenu(workspace: workspace).font(Typography.caption())
                    let store = workspace.multiplexer
                    if let zoomed = store.zoomedPaneId, let pane = store.layout.pane(for: zoomed) {
                        ZoomCrumb(title: pane.kind.rawValue.capitalized) { store.setZoom(zoomed, enabled: false) }
                    }
                }
            }
            if let target = model.machineTarget {
                WorkMachineControl(model: model, target: target).padding(.leading, 6)
            }
        }
    }

    private func waveCrumb(_ id: String, name: String) -> some View {
        HStack(alignment: .center, spacing: 6) {
            Button(name) {
                if model.navigation.waveView == .wave(id) { model.setZoomed(nil, in: .wave(id)) }
                model.select(.wave(id: id))
                model.navigation.content = .terminals
            }.lineLimit(1).fixedSize()
            WorkJumpSeparator(model: model, scope: .wave(id), label: "Tasks in \(name)")
        }
    }
}

/// Where the current Task or Wave runs, and the Machines it can run on instead.
/// Planning supplies the reading; `lf task move --machine` and `lf set machine` write. A Task move keeps
/// its person, so each choice reads “Run on”.
struct WorkMachineControl: View {
    let model: WorkModel
    let target: WorkMachineTarget
    @State private var machines: [Machine] = []
    @State private var error: String?

    var body: some View {
        let current = model.machine(of: target)
        Menu {
            ForEach(choices, id: \.self) { id in
                Button("Run on \(name(id))") { Task { await use(id) } }
                    .disabled(id == current.id)
                    .accessibilityIdentifier("work-machine-" + id)
            }
            if let error { Text(error) }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: error == nil ? "desktopcomputer" : "exclamationmark.triangle")
                Text(current.id.map(name) ?? current.label ?? "Machine unknown").lineLimit(1)
            }
            .font(Typography.caption())
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .help("Machine this Work runs on")
        .accessibilityIdentifier("work-machine")
        .task(id: target) { await load() }
    }

    /// This Machine first, then your registered Machines.
    private var choices: [String] {
        var ids = model.localMachineId.map { [$0] } ?? []
        for machine in machines where !ids.contains(machine.id) { ids.append(machine.id) }
        return ids
    }

    private func name(_ id: String) -> String {
        if id == model.localMachineId { return "This Mac" }
        let current = model.machine(of: target)
        return machines.first { $0.id == id }?.label ?? (current.id == id ? current.label : nil) ?? id
    }

    private func load() async {
        error = nil
        // A newer load or a navigation cancels this one; that is not a failure to show.
        do { machines = try await model.machines(for: target) }
        catch is CancellationError {}
        catch { self.error = error.localizedDescription }
    }

    private func use(_ id: String) async {
        error = nil
        do { try await model.useMachine(id, for: target) }
        catch is CancellationError {}
        catch { self.error = error.localizedDescription }
    }
}

/// A jump-bar separator: the › after a crumb lists what is inside it, and
/// typing searches that scope. Find Work stays on ⌘K.
struct WorkJumpSeparator: View {
    let model: WorkModel
    let scope: WorkJumpScope
    let label: String
    @State private var open = false

    var body: some View {
        Button { open.toggle() } label: {
            Image(systemName: "chevron.right")
                .font(.system(size: 10, weight: .semibold))
                .rotationEffect(.degrees(open ? 90 : 0))
                .frame(width: 18, height: 28).contentShape(Rectangle())
        }
        .help(label).accessibilityLabel(label)
        .accessibilityIdentifier("work-jump-" + scope.identifier)
        .popover(isPresented: $open, arrowEdge: .bottom) {
            WorkJumpMenu(model: model, scope: scope, title: label) { open = false }
        }
    }
}

struct WorkJumpMenu: View {
    let model: WorkModel
    let scope: WorkJumpScope
    let title: String
    let dismiss: () -> Void
    @State private var filter = ""
    @Environment(\.palette) private var palette

    var body: some View {
        let rows = model.jumpRows(scope, filter: filter)
        VStack(alignment: .leading, spacing: 6) {
            Text(title.uppercased()).font(Typography.code(10)).foregroundStyle(palette.textSecondary)
            TextField("Search \(title.lowercased())", text: $filter)
                .textFieldStyle(.roundedBorder)
                .onSubmit { if let first = rows.first { choose(first.id) } }
                .accessibilityIdentifier("work-jump-filter")
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(rows) { row in
                        Button { choose(row.id) } label: {
                            HStack {
                                Text(row.title).lineLimit(1)
                                Spacer()
                                Text(row.detail).font(Typography.caption()).foregroundStyle(palette.textSecondary).lineLimit(1)
                            }.padding(6).frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
                        }.buttonStyle(.plain)
                    }
                    if rows.isEmpty { Text("No matches").font(Typography.caption()).foregroundStyle(palette.textSecondary).padding(6) }
                }
            }.frame(maxHeight: 320)
        }
        .padding(10).frame(width: 380)
        .onExitCommand(perform: dismiss)
        .accessibilityIdentifier("work-jump-menu")
    }

    private func choose(_ destination: WorkDestination) {
        dismiss()
        model.jump(to: destination, source: "jump_bar")
    }
}

extension WorkJumpScope {
    fileprivate var identifier: String {
        switch self {
        case .repository: "repository"
        case .wave(let id): "wave-" + id
        }
    }
}

/// The last row of the Waves list while the pointer is over it; names and authors a Wave.
private struct NewWaveRow: View {
    let model: WorkModel
    let hover: HoverFlag
    @Environment(\.palette) private var palette
    @State private var naming = false
    @State private var name = ""
    @State private var error: String?

    var body: some View {
        HoverRevealed(flag: hover) {
            Button { naming = true } label: {
                HStack {
                    Image(systemName: "plus")
                    Text("New Wave").lineLimit(1)
                    Spacer()
                }.font(Typography.body(13)).foregroundStyle(palette.textSecondary).padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(RoundedRectangle(cornerRadius: 6))
            }.buttonStyle(.plain)
                .accessibilityIdentifier("work-new-wave")
                .popover(isPresented: $naming, arrowEdge: .trailing) { form }
        }
    }

    private var form: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField("Wave name", text: $name).onSubmit(create).frame(width: 200)
            if let error { Text(error).font(Typography.body(11)).foregroundStyle(palette.warning) }
            HStack { Spacer(); Button("Create", action: create).keyboardShortcut(.defaultAction) }
        }.padding(12)
    }

    private func create() {
        guard let repoPath = model.repoPath else { return }
        Task {
            do {
                try await PortfolioRepoState.createWave(name: name, repoPath: repoPath)
                naming = false
                name = ""
                error = nil
                await model.refreshPlanning()
            } catch let failure {
                error = failure.localizedDescription
            }
        }
    }
}

/// Arrow keys walk Wave rows; the repository row has no WorkReference to focus.
private struct FocusedWork: ViewModifier {
    let focus: FocusState<WorkReference?>.Binding
    let reference: WorkReference?

    func body(content: Content) -> some View {
        if let reference { content.focused(focus, equals: reference) } else { content }
    }
}

/// One rule for every sidebar row: the row whose Work fills the center pane is
/// shown; the Wave or Dashboard containing it, when that isn't itself shown, is
/// containing. Nothing else is marked.
enum SidebarSelection {
    case none, containing, shown
}

struct SidebarSelectionShape: View {
    let state: SidebarSelection
    @Environment(\.palette) private var palette

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 6)
        switch state {
        case .none: shape.fill(Color.clear)
        case .containing: shape.fill(palette.surface)
        case .shown: shape.fill(palette.workSelectedFill).overlay(shape.strokeBorder(palette.selectionEdge))
        }
    }
}

/// Drags the pinned Waves section taller or shorter.
private struct WaveSectionDivider: View {
    let navigation: WorkNavigation
    @Environment(\.palette) private var palette
    @State private var start: CGFloat?

    var body: some View {
        Rectangle().fill(palette.border).frame(height: 1)
            .frame(maxWidth: .infinity).padding(.vertical, 3)
            .contentShape(Rectangle())
            .onHover { if $0 { NSCursor.resizeUpDown.push() } else { NSCursor.pop() } }
            .gesture(DragGesture(minimumDistance: 0).onChanged { value in
                if start == nil { start = navigation.sidebarWavesHeight }
                navigation.sidebarWavesHeight = max(44, (start ?? navigation.sidebarWavesHeight) - value.translation.height)
            }.onEnded { _ in start = nil })
            .accessibilityLabel("Waves height")
            .accessibilityIdentifier("work-waves-divider")
    }
}

/// Navigation already belongs to one repository, including its sidebar viewport.
private struct WorkTaskList<Content: View>: View {
    let navigation: WorkNavigation
    @ViewBuilder let content: Content
    @State private var position: ScrollPosition

    init(navigation: WorkNavigation, @ViewBuilder content: () -> Content) {
        self.navigation = navigation
        self.content = content()
        _position = State(initialValue: ScrollPosition(y: navigation.sidebarScrollOffset))
    }

    var body: some View {
        ScrollView { content }
            .scrollPosition($position)
            // A new scope's header is at the top, even after clicking a row far down.
            // Deferred: scrolling in the update that swaps the rows kept the old offset.
            .onChange(of: navigation.sidebarScope) { Task { position.scrollTo(edge: .top) } }
            .onScrollGeometryChange(for: CGFloat.self) { geometry in
                max(0, geometry.contentOffset.y + geometry.contentInsets.top)
            } action: { _, offset in navigation.sidebarScrollOffset = offset }
    }
}

@MainActor
private final class SidebarResponderMemory {
    weak var previous: NSResponder?
    weak var window: NSWindow?
    func restore() {
        guard let previous = previous as? NSView, previous.window === window else { return }
        window?.makeFirstResponder(previous)
    }
}

private struct SidebarResponderCapture: NSViewRepresentable {
    let memory: SidebarResponderMemory
    func makeCoordinator() -> Coordinator { Coordinator(memory: memory) }
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        context.coordinator.view = view
        context.coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .keyDown]) { [weak coordinator = context.coordinator] event in
            guard let coordinator, let view = coordinator.view, let window = view.window,
                  window.isKeyWindow, event.window === window, window.attachedSheet == nil else { return event }
            let entering = event.type == .leftMouseDown && view.bounds.contains(view.convert(event.locationInWindow, from: nil))
                || event.type == .keyDown && event.keyCode == 48
            if entering, let responder = window.firstResponder as? NSView,
               responder is NSTextInputClient {
                coordinator.memory.previous = responder
                coordinator.memory.window = window
            }
            return event
        }
        return view
    }
    func updateNSView(_ view: NSView, context: Context) {}
    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        if let monitor = coordinator.monitor { NSEvent.removeMonitor(monitor) }
    }
    @MainActor final class Coordinator {
        let memory: SidebarResponderMemory
        weak var view: NSView?
        var monitor: Any?
        init(memory: SidebarResponderMemory) { self.memory = memory }
    }
}

/// Owns one row's hover. Its body never reads the flag, so hovering does not
/// re-run the row; leaves given the flag read it themselves.
private struct RowHover<Content: View>: View {
    @ViewBuilder let content: (HoverFlag) -> Content
    @State private var hover = HoverFlag()

    var body: some View {
        content(hover).hoverSource(hover)
    }
}

/// Working Tasks rows reorder the repository's one Task order by drag; Wave lists don't.
/// A drag is a move, not a copy: no + badge, and a line marks where the row lands.
private struct TaskOrderDragging: ViewModifier {
    let model: WorkModel
    let workID: String?
    let drag: TaskOrderDrag.State
    @Environment(\.palette) private var palette

    func body(content: Content) -> some View {
        if let workID {
            content
                .onDrag {
                    drag.source = workID
                    return NSItemProvider(object: TaskOrderDrag.token(workID) as NSString)
                }
                .onDrop(of: [.plainText], delegate: TaskOrderDrop(model: model, target: workID, drag: drag))
                .overlay(alignment: drag.landing(on: workID) ?? .top) {
                    if drag.landing(on: workID) != nil {
                        Rectangle().fill(palette.accent).frame(height: 2)
                            .allowsHitTesting(false).accessibilityIdentifier("task-order-insertion")
                    }
                }
        } else {
            content
        }
    }
}

extension TaskOrderDrag {
    /// The sidebar row being dragged and the row it would take the place of.
    @MainActor @Observable final class State {
        var source: String?
        var target: String?
        /// The dragged row lands after the target when it comes from above.
        var below = false

        func landing(on row: String) -> Alignment? {
            target == row ? (below ? .bottom : .top) : nil
        }
    }
}

private struct TaskOrderDrop: DropDelegate {
    let model: WorkModel
    let target: String
    let drag: TaskOrderDrag.State

    func dropEntered(info: DropInfo) {
        drag.target = drag.source == target ? nil : target
        let ids = model.workingTaskMembers.map(\.work.id)
        drag.below = (ids.firstIndex(of: drag.source ?? "") ?? 0) < (ids.firstIndex(of: target) ?? 0)
    }

    func dropExited(info: DropInfo) {
        if drag.target == target { drag.target = nil }
    }

    func dropUpdated(info: DropInfo) -> DropProposal? { DropProposal(operation: .move) }

    func performDrop(info: DropInfo) -> Bool {
        defer { drag.source = nil; drag.target = nil }
        guard let source = drag.source, source != target else { return false }
        Task { await model.reorderWorkingTask(source, to: target) }
        return true
    }
}
