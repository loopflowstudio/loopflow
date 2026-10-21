import AppKit
import Loopflow
import SwiftUI

/// Working positions belong to navigation; observation supplies current facts.
struct WorkNavigator: View {
    @Bindable var model: WorkModel
    let onOpenSession: (SessionRecord) -> Void
    var onNewTaskSession: ((String) -> Void)?
    @Environment(\.palette) private var palette
    @State private var hoveredTask: String?
    @State private var actionTaskID: String?
    @FocusState private var focusedWork: WorkReference?
    @State private var focusMemory = SidebarResponderMemory()

    private var listTitle: String {
        let filter = model.navigation.sidebarFilter
        guard model.navigation.sidebarWaveID != nil else { return filter.rawValue }
        let wave = model.sidebarWave?.wave.displayName ?? "Wave unavailable"
        return filter == .working ? wave : "\(wave) · \(filter.rawValue)"
    }

    private var rowOrder: [WorkReference] {
        model.sidebarTaskIDs.map { .task(id: $0) } + model.sidebarWaves.map { .wave(id: $0.wave.id) }
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
            WorkTaskList(navigation: model.navigation) {
                VStack(alignment: .leading, spacing: 5) {
                    if model.navigation.sidebarWaveID != nil {
                        Button { model.select(nil) } label: {
                            Label(model.repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Repository",
                                  systemImage: "chevron.left")
                        }.buttonStyle(.plain).font(Typography.body(12)).foregroundStyle(palette.textSecondary)
                            .padding(.horizontal, 10).padding(.top, 6)
                            .help("Back to Working Tasks")
                            .accessibilityIdentifier("work-sidebar-back")
                    }
                    HStack {
                        Text(listTitle.uppercased()).lineLimit(1)
                        Spacer()
                        Text("\(model.sidebarTaskIDs.count)")
                    }.font(Typography.code(10)).foregroundStyle(palette.textSecondary).padding(10)
                    ForEach(model.sidebarTaskIDs, id: \.self) { id in taskRow(id) }
                    if model.sidebarTaskIDs.isEmpty {
                        Text(model.navigation.sidebarWaveID == nil ? "No working Tasks in this list" : "No open Tasks in this Wave")
                            .font(Typography.body(12))
                            .foregroundStyle(palette.textSecondary).padding(10)
                    }
                    if let selection = model.selection, selection.kind == .task,
                       !model.sidebarTaskIDs.contains(where: { model.isSelected(.task(id: $0)) }) {
                        VStack(alignment: .leading, spacing: 5) {
                            Text("Open Task is outside this list")
                            Button("Show working tasks") {
                                model.navigation.sidebarFilter = .working
                                model.navigation.sidebarWaveID = nil
                            }
                        }.font(Typography.body(11)).padding(10)
                    }
                    if !model.sidebarWaves.isEmpty {
                        Text(model.navigation.sidebarWaveID == nil ? "WAVES" : "SUBWAVES").font(Typography.code(10))
                            .foregroundStyle(palette.textSecondary).padding(10).padding(.top, 8)
                    }
                    ForEach(model.sidebarWaves, id: \.wave.id) { wave in
                        Button { model.select(.wave(id: wave.wave.id)) } label: {
                            HStack {
                                Image(systemName: "water.waves")
                                Text(wave.wave.displayName).lineLimit(1)
                                Spacer()
                                if let reason = wave.tasks.unavailableReason {
                                    Image(systemName: "exclamationmark.triangle").help(reason)
                                } else {
                                    Text("\(wave.tasks.items.count)")
                                        .font(Typography.code(10)).help("\(wave.tasks.items.count) Tasks")
                                }
                            }.font(Typography.body(13)).padding(10)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(model.isSelected(.wave(id: wave.wave.id)) ? palette.surface : .clear,
                                            in: RoundedRectangle(cornerRadius: 6))
                        }.buttonStyle(.plain)
                            .focused($focusedWork, equals: .wave(id: wave.wave.id))
                            .accessibilityLabel("\(wave.wave.displayName), \(wave.tasks.items.count) Tasks")
                            .accessibilityIdentifier("work-wave-\(wave.wave.id)")
                    }
                    if model.navigation.sidebarWaveID == nil { DashboardSidebar(model: model) }
                }.padding(8)
            }
        }
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
        .onExitCommand { focusedWork = nil; focusMemory.restore() }
        .onDisappear { if focusedWork != nil { focusMemory.restore() } }
    }

    private func taskRow(_ id: String) -> some View {
        let found = model.task(id: id)
        let selected = model.isSelected(.task(id: id))
        let otherWave = model.selection?.kind == .wave && found?.wave?.wave.id != model.selection?.id
        let sessions = model.taskSessions(id)
        let status = SessionProgramStatus(sessions: sessions, unavailable: model.sessions.errorMessage)
        let title = found?.task.task.name ?? "Task unavailable · \(id)"
        return VStack(alignment: .leading, spacing: 6) {
            Text(title).font(Typography.strong(13)).lineLimit(2).help(title)
                .allowsHitTesting(false)
            TimelineView(.periodic(from: .now, by: 5)) { clock in
                let preview = status.preview(now: clock.date, fallback: found?.task.task.description ?? "Planning unavailable", sessions: sessions)
                VStack(alignment: .leading, spacing: 6) {
                    Text(preview.text).font(Typography.code(11)).lineLimit(status.session?.availability == .available ? 2 : 1)
                        .foregroundStyle(selected ? palette.workSelectedSecondaryText : palette.textSecondary)
                        .help(preview.help).allowsHitTesting(false)
                    if let task = found?.task {
                        TaskRowMetadata(task: task, status: status,
                            localMachineId: model.observingMachineId, selected: selected,
                            slotHint: (model.navigation.showsSlotHints || hoveredTask == id)
                                ? model.workingTaskIDs.firstIndex(of: id).flatMap { $0 < 9 ? "⌘\($0 + 1)" : nil } : nil,
                            sourceAge: preview.age)
                    } else {
                        Text("Planning unavailable").font(Typography.code(10))
                            .foregroundStyle(palette.textSecondary).allowsHitTesting(false)
                    }
                }
            }
        }
        .padding(12).frame(maxWidth: .infinity, alignment: .leading)
        .foregroundStyle(selected ? palette.workSelectedText : otherWave ? palette.textSecondary : palette.text)
        .background {
            Button { model.select(.task(id: id)) } label: {
                RoundedRectangle(cornerRadius: 6)
                    .fill(selected ? palette.workSelectedFill : Color.clear)
                    .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(selected ? palette.selectionEdge : .clear))
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
        .onHover { hoveredTask = $0 ? id : nil }
        .modifier(TaskOrderDragging(model: model, workID: model.navigation.sidebarWaveID == nil ? model.workingTaskWorkID(row: id) : nil))
        .contextMenu {
            actions(for: id)
        }
        .popover(isPresented: Binding(get: { actionTaskID == id }, set: { if !$0 { actionTaskID = nil } })) {
            VStack(alignment: .leading, spacing: 10) { actions(for: id) }
                .padding(12)
                .onExitCommand { actionTaskID = nil }
        }
    }

    @ViewBuilder private func actions(for id: String) -> some View {
        if let found = model.task(id: id) {
            TaskActionItems(model: model, task: found.task, wave: found.wave?.wave,
                newConversation: onNewTaskSession.map { action in { action(id) } },
                openSession: onOpenSession)
        }
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
                model.select(nil)
                model.navigation.content = .terminals
            }.lineLimit(1).fixedSize()
            WorkJumpSeparator(model: model, scope: .repository, label: "Waves and Dashboards")
            if showWave, model.navigation.trailDashboardID == nil, let wave = model.breadcrumb?.wave {
                Button(wave.roadmap.wave.displayName) {
                    model.select(.wave(id: wave.roadmap.wave.id))
                    model.navigation.content = .terminals
                }.lineLimit(1).fixedSize()
                WorkJumpSeparator(model: model, scope: .wave(wave.roadmap.wave.id), label: "Tasks in \(wave.roadmap.wave.displayName)")
            }
            if model.navigation.destination == .workingTasks {
                Text(WorkNavigation.workingTasksTitle).lineLimit(1).fixedSize()
            }
            if let id = model.navigation.trailDashboardID {
                Button(model.dashboard(id)?.name ?? id) { model.selectDashboard(id) }
                    .lineLimit(1).fixedSize().disabled(model.navigation.dashboardID == id)
                if model.breadcrumb?.task != nil { Text("/").foregroundStyle(.secondary) }
            }
            if let task = model.breadcrumb?.task {
                TaskHeading(task: task.task)
            }
            if let target = model.machineTarget {
                WorkMachineControl(model: model, target: target).padding(.leading, 6)
            }
        }
    }
}

/// The execution Machine of the current Wave or Task and where it comes from.
/// Reads and writes go through `lf wave place` / `lf task place`.
struct WorkMachineControl: View {
    let model: WorkModel
    let target: WorkMachineTarget
    @State private var reading: WorkMachine?
    @State private var error: String?

    var body: some View {
        WorkMachineMenu(reading: reading, error: error, parent: model.breadcrumb?.wave?.roadmap.wave.displayName,
                        waveName: { model.wave(id: $0)?.wave.displayName }) { placement in
            Task { await place(placement) }
        }
        .task(id: target) { await load() }
    }

    private func load() async {
        reading = nil
        error = nil
        do { reading = try await model.workMachine(target) } catch { self.error = error.localizedDescription }
    }

    private func place(_ placement: WorkMachinePlacement) async {
        error = nil
        do { reading = try await model.placeWork(target, placement) } catch { self.error = error.localizedDescription }
    }
}

struct WorkMachineMenu: View {
    let reading: WorkMachine?
    let error: String?
    /// The Task's Wave, which it inherits from once its own Machine is cleared.
    let parent: String?
    /// A Wave's display name by ID, for inherited placement.
    let waveName: (String) -> String?
    let place: (WorkMachinePlacement) -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        Menu {
            if let reading { choices(reading) }
            if let error { Text(error) }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: error == nil ? "desktopcomputer" : "exclamationmark.triangle")
                Text(title).lineLimit(1)
                Text(origin).foregroundStyle(palette.textSecondary).lineLimit(1)
            }
            .font(Typography.caption())
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .help("Machine this Work runs on")
        .accessibilityIdentifier("work-machine")
    }

    var title: String {
        guard let reading else { return error == nil ? "Machine…" : "Machine unavailable" }
        return reading.machineID.map(reading.name(of:)) ?? "Unrecorded Machine"
    }

    var origin: String {
        guard let reading else { return "" }
        switch reading.execution {
        case .recordedCheckout: return "· started there"
        case .admission: return "· starting there"
        case .effectiveDelegation, nil: break
        }
        if reading.placement.provenance == .localDefault { return "· default" }
        if reading.placedHere { return reading.work.kind == "task" ? "· set on this Task" : "· set on this Wave" }
        return "· from " + sourceName(reading.placement.source)
    }

    private func sourceName(_ source: WorkMachine.Ref) -> String {
        switch source.kind {
        case "wave": waveName(source.id) ?? "its Wave"
        case "project": "its Project"
        default: "its Task"
        }
    }

    @ViewBuilder private func choices(_ reading: WorkMachine) -> some View {
        if reading.followsPlacement {
            if reading.work.kind == "task" {
                Button { place(.inherit) } label: {
                    choice("Inherit from \(parent ?? "its Wave")", selected: !reading.placedHere)
                }
                .accessibilityIdentifier("work-machine-inherit")
                Divider()
            }
            ForEach(reading.machines) { machine in
                Button { place(.machine(machine.id)) } label: {
                    choice(reading.name(of: machine.id), selected: reading.placedHere && reading.placement.machineID == machine.id)
                }
                .accessibilityIdentifier("work-machine-" + machine.id)
            }
        } else {
            Text("Started on \(title); moving it is explicit takeover")
        }
    }

    @ViewBuilder private func choice(_ name: String, selected: Bool) -> some View {
        if selected { Label(name, systemImage: "checkmark") } else { Text(name) }
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

/// Working Tasks rows reorder the repository's one Task order by drag; Wave lists don't.
private struct TaskOrderDragging: ViewModifier {
    let model: WorkModel
    let workID: String?

    func body(content: Content) -> some View {
        if let workID {
            content
                .draggable(TaskOrderDrag.token(workID))
                .dropDestination(for: String.self) { values, _ in
                    guard let task = values.first.flatMap(TaskOrderDrag.taskID), task != workID else { return false }
                    Task { await model.reorderWorkingTask(task, to: workID) }
                    return true
                }
        } else {
            content
        }
    }
}
