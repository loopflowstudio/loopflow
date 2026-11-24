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
    @State private var removingWave: WaveSnapshot?
    @State private var renamingWave: WaveSnapshot?
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

    /// Nil until the list is read: the work status above says it is loading, and
    /// a failed read is the Notice's.
    private var emptyListText: String? {
        switch model.navigation.sidebarScope {
        case .dashboard(let id):
            return model.dashboard(id) == nil ? "This Dashboard is no longer available." : "No Tasks in this Dashboard."
        case .repository, .wave:
            if model.taskListReading.value == nil { return nil }
            if case .wave = model.navigation.sidebarScope { return "No working Tasks in this Wave." }
            return "No working Tasks in this list."
        }
    }

    private var rowOrder: [WorkReference] {
        model.sidebarTaskIDs.map { .task(id: $0) }
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
            if let status = model.workStatus.progress {
                WorkReadStatus(status: status)
            }
            // The repository tops the sidebar; a selected Wave moves up under it.
            VStack(alignment: .leading, spacing: Spacing.xs) {
                repositoryRow
                if let wave = model.sidebarWave { waveRow(wave) }
            }.padding([.horizontal, .top], Spacing.sm)
            WorkTaskList(navigation: model.navigation) {
                // Read once: each read re-filters the plan and every Task's Sessions.
                let rows = model.sidebarRows
                let scope = model.navigation.sidebarScope
                let dragOrder = scope == .repository && !model.navigation.sidebarSortByStatus ? rows.map(\.work.id) : []
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    HStack {
                        NewTaskRow(model: model)
                        if scope == .repository {
                            SidebarStatusToggle(isOn: model.navigation.sidebarSortByStatus, toggle: model.toggleSidebarSortByStatus)
                        }
                    }
                    if case .dashboard = scope {
                        HStack {
                            Text(listTitle).font(Typography.textStrong).lineLimit(1)
                                .matchedGeometryEffect(id: scope, in: scopeTitle)
                            Spacer()
                            Text("\(rows.count)").font(Typography.meta).monospacedDigit()
                        }.foregroundStyle(palette.textTertiary).padding(Spacing.sm)
                    }
                    // ⌘N counts the rows on screen; sidebar Task IDs are already unique.
                    ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
                        taskRow(row, dragOrder: dragOrder, slotHint: index < 9 ? "⌘\(index + 1)" : nil)
                    }
                    if rows.isEmpty, let emptyListText {
                        Text(emptyListText)
                            .font(Typography.text)
                            .foregroundStyle(palette.textSecondary).padding(Spacing.sm)
                    }
                    if let selection = model.selection, selection.kind == .task,
                       !rows.contains(where: { model.isSelected(.task(id: $0.id)) }) {
                        VStack(alignment: .leading, spacing: Spacing.xs) {
                            Text("Open Task is outside this list")
                            Button("Show working Tasks") {
                                Perf.userAction(Perf.hierarchyInteraction, scenario: "sidebar_working", id: "outline:\(WorkNavigation.SidebarFilter.working.rawValue)",
                                                repository: model.repoPath, liveSessions: model.liveSessionCount, owner: model.navigation.performanceOwner) {
                                    model.navigation.sidebarFilter = .working
                                    model.navigation.sidebarWaveID = nil
                                }
                            }
                        }.font(Typography.meta).padding(Spacing.sm)
                    }
                }.padding(Spacing.sm).coordinateSpace(.named(TaskOrderDrag.space))
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
        .onChange(of: rowOrder.isEmpty, initial: true) { _, empty in
            guard !empty else { return }
            let source = model.showsSavedWork ? "saved" : "fresh"
            DispatchQueue.main.async { LaunchJournal.home.mark(.usable, ["source": source]) }
        }
        .onChange(of: model.sidebarTaskIDs) { before, after in
            Perf.rowsChanged(from: before, to: after, owner: model.navigation.performanceOwner)
        }
        .onChange(of: model.navigation.sidebarFilter) { _, filter in
            Perf.stateObserved(Perf.hierarchyInteraction, id: "outline:\(filter.rawValue)", owner: model.navigation.performanceOwner)
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
            Perf.userAction(Perf.taskWorkspaceReady, scenario: focusedWork.kind == .task ? "task" : "wave",
                            id: focusedWork.id, repository: model.repoPath, liveSessions: model.liveSessionCount,
                            owner: model.navigation.performanceOwner) { model.select(focusedWork) }
            return .handled
        }
        .renderFlash()
        .onExitCommand { focusedWork = nil; focusMemory.restore() }
        .onDisappear { if focusedWork != nil { focusMemory.restore() } }
    }

    /// Pinned below the Task list so any Wave is a click away however far the
    /// list scrolls; it starts about three rows tall and its divider drags.
    private var waveSection: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(model.visibleRoadmaps.filter { $0.wave.id != model.sidebarWave?.wave.id }, id: \.wave.id) { waveRow($0) }
                }.padding(Spacing.sm)
            }
            .onScrollGeometryChange(for: CGFloat.self) { $0.contentSize.height } action: { _, height in
                wavesHeight = height
            }
            .frame(height: min(wavesHeight, model.navigation.sidebarWavesHeight))
            NewWaveRow(model: model).padding(.horizontal, Spacing.sm).padding(.bottom, Spacing.sm)
        }
        .waveLifecycleSheets(model: model, removing: $removingWave, renaming: $renamingWave)
    }

    /// The repository is the default Wave; it lists Working Tasks and can't be edited.
    private var repositoryRow: some View {
        waveRow(scope: .repository, name: model.repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Repository",
                icon: nil, count: model.startedTaskCount(.repository), unavailableReason: nil)
    }

    private func waveRow(_ wave: WaveRoadmap) -> some View {
        waveRow(scope: .wave(wave.wave.id), name: wave.wave.displayName, icon: "water.waves",
                count: model.startedTaskCount(.wave(wave.wave.id)), unavailableReason: wave.tasks.unavailableReason)
            .contextMenu {
                WaveLifecycleMenu(onRename: { renamingWave = wave.wave }, onRemove: { removingWave = wave.wave })
            }
    }

    private func waveRow(scope: WorkNavigation.SidebarScope, name: String, icon: String?,
                         count: Int, unavailableReason: String?) -> some View {
        let reference: WorkReference? = if case .wave(let id) = scope { .wave(id: id) } else { nil }
        let state: SidebarSelection = pressedScope == scope || model.navigation.destination == .work(reference) ? .shown
            : model.navigation.sidebarScope == scope ? .containing : .none
        return Button {
            Perf.userAction(Perf.taskWorkspaceReady, scenario: "wave", id: reference?.id ?? "repository",
                            repository: model.repoPath, liveSessions: model.liveSessionCount, owner: model.navigation.performanceOwner) {
                // Select in the click; the pressed highlight animates over the selected row.
                model.select(reference)
                guard !reduceMotion else { return }
                withAnimation(.easeOut(duration: 0.12)) { pressedScope = scope }
                withAnimation(.easeOut(duration: 0.12).delay(0.12)) { pressedScope = nil }
            }
        } label: {
            HStack {
                if let icon { Image(systemName: icon) }
                Text(name).lineLimit(1)
                Spacer()
                if let unavailableReason {
                    Image(systemName: "exclamationmark.triangle").help(unavailableReason)
                } else {
                    Text("\(count)").font(Typography.meta).monospacedDigit().help("\(count) working Tasks")
                }
            }.font(Typography.text).padding(Spacing.sm)
                .frame(maxWidth: .infinity, alignment: .leading)
                .foregroundStyle(state == .shown ? palette.workSelectedText : palette.text)
                .background(SidebarSelectionShape(state: state))
                // The whole row clicks, as a Task row does, not only its text.
                .contentShape(RoundedRectangle(cornerRadius: CornerRadius.row))
        }.buttonStyle(.plain)
            .modifier(FocusedWork(focus: $focusedWork, reference: reference))
            .accessibilityLabel("\(name), \(count) working Tasks")
            .accessibilityIdentifier("work-wave-\(reference?.id ?? "repository")")
            .matchedGeometryEffect(id: scope, in: scopeTitle)
    }

    private func taskRow(_ row: WorkModel.SidebarTaskRow, dragOrder: [String], slotHint: String?) -> some View {
        let id = row.id
        let found = model.task(id: id)
        let selected = model.isSelected(.task(id: id))
        let muted = model.selection?.kind == .wave && found?.wave?.wave.id != model.selection?.id
        let status = model.programStatus(task: id)
        let presentation = model.taskPresentation(id: id)
        let title = presentation.title
        // Rows slide under the pointer while scrolling. Only the ⌘N hint reads the
        // row's hover, so a crossing re-renders that hint, not rows or the sidebar.
        return RowHover { hover in TaskRow(
            id: id, presentation: presentation, task: found?.task, status: status,
            graphStep: row.graphStep, auto: row.states.contains(.auto), selected: selected, muted: muted,
            slotHint: slotHint, showsSlotHint: model.navigation.showsSlotHints, hover: hover)
        .background {
            Button {
                Perf.userAction(Perf.taskWorkspaceReady, scenario: "task", id: id,
                                repository: model.repoPath, liveSessions: model.liveSessionCount, owner: model.navigation.performanceOwner) {
                    model.select(.task(id: id))
                }
            } label: {
                SidebarSelectionShape(state: selected ? .shown : .none)
                    .contentShape(Rectangle())
            }.buttonStyle(.plain).focused($focusedWork, equals: .task(id: id)).accessibilityLabel(title)
                .accessibilityAddTraits(selected ? .isSelected : [])
                .accessibilityIdentifier("work-task-\(id)")
        }
        .renderFlash()
        }
        .modifier(TaskOrderDragging(model: model, work: row.work, order: dragOrder, drag: taskDrag))
        .contextMenu {
            actions(for: id)
        }
        .popover(isPresented: Binding(get: { actionTaskID == id }, set: { if !$0 { actionTaskID = nil } })) {
            VStack(alignment: .leading, spacing: Spacing.sm) { actions(for: id) }
                .padding(Spacing.md)
                .onExitCommand { actionTaskID = nil }
        }
    }

    @ViewBuilder private func actions(for id: String) -> some View {
        if let found = model.task(id: id) {
            TaskActionItems(model: model, task: found.task, wave: found.wave?.wave,
                work: model.taskWork[id].value,
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

/// Where the current Task or Wave runs, and the Machines it can run on instead.
/// Planning supplies the reading; `lf task move --machine` and `lf set machine` write. A Task move keeps
/// its person, so each choice reads “Run on”.
struct WorkMachineControl: View {
    let model: WorkModel
    let target: WorkMachineTarget
    private var machines: [Machine] { model.machines.value ?? [] }

    var body: some View {
        let current = model.machine(of: target)
        Menu {
            ForEach(choices, id: \.self) { id in
                Button("Run on \(name(id))") { Task { await use(id) } }
                    .disabled(id == current.id)
                    .accessibilityIdentifier("work-machine-" + id)
            }
        } label: {
            HStack(spacing: Spacing.xs) {
                Image(systemName: "desktopcomputer")
                Text(current.id.map(name) ?? current.label ?? "Machine unknown").lineLimit(1)
            }
            .font(Typography.meta)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .help("Machine this Work runs on")
        .accessibilityIdentifier("work-machine")
        .task(id: target) { await model.loadMachines() }
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

    /// A failure posts to the window that asked.
    private func use(_ id: String) async {
        let notices = model.navigation.notices
        do { try await model.useMachine(id, for: target) }
        catch is CancellationError {}
        catch { notices.post(Notice("Could not run on \(name(id)): \(error.localizedDescription)")) }
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
                .font(Typography.meta).fontWeight(.semibold)
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
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(title).font(Typography.textStrong).foregroundStyle(palette.textTertiary)
            TextField("Search \(title.lowercased())", text: $filter)
                .textFieldStyle(.roundedBorder)
                .onSubmit { if let first = rows.first { choose(first.id) } }
                .accessibilityIdentifier("work-jump-filter")
            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.xxs) {
                    ForEach(rows) { row in
                        Button { choose(row.id) } label: {
                            HStack {
                                Text(row.title).lineLimit(1)
                                Spacer()
                                Text(row.detail).font(Typography.meta).foregroundStyle(palette.textSecondary).lineLimit(1)
                            }.padding(Spacing.xs).frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
                        }.buttonStyle(.plain)
                    }
                    if rows.isEmpty { Text("No matches.").emptyLine().padding(Spacing.xs) }
                }
            }.frame(maxHeight: 320)
        }
        .padding(Spacing.sm).frame(width: 380)
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

/// Always first in the Task list: the main way to start work. It opens the
/// New Task sheet, as ⌘N does.
private struct NewTaskRow: View {
    let model: WorkModel
    @Environment(\.palette) private var palette

    var body: some View {
        Button { model.navigation.palette = .newTask } label: {
            HStack {
                Image(systemName: "plus")
                Text("New Task").lineLimit(1)
                Spacer()
                Text("⌘N").font(Typography.meta)
            }.font(Typography.text).foregroundStyle(palette.textSecondary).padding(Spacing.sm)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(RoundedRectangle(cornerRadius: CornerRadius.row))
        }.buttonStyle(.plain)
            .disabled(model.repoPath == nil)
            .accessibilityIdentifier("work-new-task")
    }
}

/// Pinned below the scrolling Waves list; opens the New Wave form for this
/// window's repository and selects the Wave it adds.
private struct NewWaveRow: View {
    let model: WorkModel
    @Environment(\.palette) private var palette
    @State private var naming = false

    var body: some View {
        Button { naming = true } label: {
            HStack {
                Image(systemName: "plus")
                Text("New Wave").lineLimit(1)
                Spacer()
            }.font(Typography.text).foregroundStyle(palette.textSecondary).padding(Spacing.sm)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(RoundedRectangle(cornerRadius: CornerRadius.row))
        }.buttonStyle(.plain)
            .accessibilityIdentifier("work-new-wave")
            .popover(isPresented: $naming, arrowEdge: .trailing) {
                CreateWaveSheet(
                    repos: model.repoPath.map { [PortfolioRepo(path: $0, error: nil)] } ?? [],
                    initialRepoPath: model.repoPath,
                    onCreate: { repo, name, objective in
                        let added = try await model.addWave(name: name, objective: objective, repo: repo)
                        model.select(.wave(id: added.id))
                        return added
                    },
                    onCancel: { naming = false })
                    .environment(\.palette, palette)
            }
    }
}

/// An add receipt with lines a person still has to act on, and a Done button.
struct WaveAddedLeftovers: View {
    let added: WaveAdded
    let onDone: () -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(added.created ? "Added \(added.name)" : "\(added.name) already exists")
                .font(Typography.text).foregroundStyle(palette.text)
            VStack(alignment: .leading, spacing: Spacing.xs) {
                ForEach(Array(added.summary.enumerated()), id: \.offset) { _, line in
                    Text(line).font(Typography.meta).foregroundStyle(palette.textSecondary)
                }
            }.accessibilityIdentifier("new-wave-summary")
            HStack {
                Spacer()
                Button("Done", action: onDone)
                    .keyboardShortcut(.defaultAction)
                    .accessibilityIdentifier("new-wave-done")
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
        let shape = RoundedRectangle(cornerRadius: CornerRadius.row)
        switch state {
        case .none: shape.fill(Color.clear)
        case .containing: shape.fill(palette.surface)
        case .shown: shape.fill(palette.workSelectedFill)
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
            .frame(maxWidth: .infinity).padding(.vertical, Spacing.xxs)
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

struct SidebarStatusToggle: View {
    let isOn: Bool
    let toggle: () -> Void
    @Environment(\.palette) private var palette
    @State private var hovering = false

    private var fill: Color {
        if isOn { return palette.accent.opacity(hovering ? 0.28 : 0.16) }
        return hovering ? palette.actionHover : .clear
    }

    var body: some View {
        Button(action: toggle) {
            Label("Sort by status", systemImage: "arrow.up.arrow.down")
                .labelStyle(.iconOnly)
                .padding(Spacing.sm)
                .foregroundStyle(isOn ? palette.accent : palette.textSecondary)
                .background(fill, in: RoundedRectangle(cornerRadius: CornerRadius.control))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help(isOn ? "Sorted by status. Click to restore saved order." : "Sort by status: Blocked, Error, Available, Idle, Busy.")
        .accessibilityValue(isOn ? "On" : "Off")
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
/// The row itself follows the pointer, a line marks where it lands, and release
/// places it at once. A system drag would hold its image for the drop animation.
private struct TaskOrderDragging: ViewModifier {
    let model: WorkModel
    let work: WorkWorkspaceKey
    let order: [String]
    let drag: TaskOrderDrag.State
    @Environment(\.palette) private var palette

    func body(content: Content) -> some View {
        if !order.isEmpty {
            let dragging = drag.source == work.id
            let landing = drag.landing(on: work.id)
            content
                .background(dragging ? palette.surfaceMuted : .clear)
                .shadow(color: .black.opacity(dragging ? 0.18 : 0), radius: 6, y: 2)
                .offset(y: dragging ? drag.offset : 0)
                .zIndex(dragging ? 1 : 0)
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(TaskOrderDrag.space)) } action: {
                    drag.frames[work.id] = $0
                }
                .highPriorityGesture(DragGesture(minimumDistance: 4, coordinateSpace: .named(TaskOrderDrag.space))
                    .onChanged { drag.move(work.id, by: $0.translation.height, at: $0.location.y,
                                           order: order) }
                    .onEnded { _ in
                        guard let (target, side) = drag.end() else { return }
                        model.reorderWorkingTask(work, to: target, side: side)
                    })
                .overlay(alignment: landing ?? .top) {
                    if landing != nil {
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
    static let space = "task-order"

    /// The sidebar row being dragged and the row it would take the place of.
    @MainActor @Observable final class State {
        var source: String?
        private var drop: (target: String, side: DropSide)?
        var offset: CGFloat = 0
        /// Each row's frame in the list, read only while a drag picks its target.
        @ObservationIgnored var frames: [String: CGRect] = [:]

        func landing(on row: String) -> Alignment? {
            guard let drop, drop.target == row else { return nil }
            return drop.side == .after ? .bottom : .top
        }

        func move(_ row: String, by translation: CGFloat, at y: CGFloat, order: [String]) {
            source = row
            offset = translation
            let under = { (id: String) in self.frames[id].map { $0.minY <= y && y < $0.maxY } == true }
            // Keep the insertion line between rows. Returning to the source cancels the drop.
            let hovered = order.first { $0 != row && under($0) }
            guard let target = hovered ?? (under(row) ? nil : drop?.target),
                  let side = TaskOrderDrag.side(moving: row, onto: target, in: order) else {
                drop = nil
                return
            }
            drop = (target, side)
        }

        /// Clears the drag in the same frame the caller moves the row.
        func end() -> (target: String, side: DropSide)? {
            defer { source = nil; drop = nil; offset = 0 }
            return drop
        }
    }
}

/// The name remains useful before the full Task and its metadata arrive.
struct WorkTaskIdentity: View {
    let presentation: WorkModel.TaskPresentation
    @Environment(\.palette) private var palette
    var body: some View {
        HStack(spacing: Spacing.xs) {
            if let identifier = presentation.identifier {
                Text(identifier).font(Typography.mono).fixedSize()
            }
            Text(presentation.title).font(Typography.textStrong).lineLimit(2).help(presentation.title)
            if let context = presentation.context {
                Text(context).font(Typography.text).foregroundStyle(palette.titlebarSecondaryText)
            }
        }
    }
}

struct WorkReadStatus: View {
    let status: String
    var body: some View {
        Text(status).font(Typography.meta).padding(Spacing.sm).accessibilityIdentifier("work-status")
    }
}
