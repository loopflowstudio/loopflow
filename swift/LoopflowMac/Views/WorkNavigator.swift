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

    private var rowOrder: [WorkReference] {
        model.sidebarTaskIDs.map { .task(id: $0) } + model.visibleRoadmaps.map { .wave(id: $0.wave.id) }
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
            if let error = model.selectionPlans.errorMessage {
                Text(error).font(Typography.body(11)).foregroundStyle(palette.waiting).padding(10)
            }
            WorkTaskList(navigation: model.navigation) {
                VStack(alignment: .leading, spacing: 5) {
                    HStack {
                        Text(model.navigation.sidebarFilter.rawValue.uppercased())
                        Spacer()
                        Text("\(model.sidebarTaskIDs.count)")
                    }.font(Typography.code(10)).foregroundStyle(palette.textSecondary).padding(10)
                    ForEach(model.sidebarTaskIDs, id: \.self) { id in taskRow(id) }
                    if model.sidebarTaskIDs.isEmpty {
                        Text("No working Tasks in this list").font(Typography.body(12))
                            .foregroundStyle(palette.textSecondary).padding(10)
                    }
                    if let selection = model.selection, selection.kind == .task,
                       !model.sidebarTaskIDs.contains(where: { model.isSelected(.task(id: $0)) }) {
                        VStack(alignment: .leading, spacing: 5) {
                            Text("Open Task is outside this list")
                            Button("Show working tasks") { model.navigation.sidebarFilter = .working }
                        }.font(Typography.body(11)).padding(10)
                    }
                    if !model.visibleRoadmaps.isEmpty {
                        Text("WAVES").font(Typography.code(10)).foregroundStyle(palette.textSecondary)
                            .padding(10).padding(.top, 8)
                    }
                    ForEach(model.visibleRoadmaps, id: \.wave.id) { wave in
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
                    Text(preview.text).font(Typography.code(11)).lineLimit(status.session?.attention == .available ? 2 : 1)
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

    var body: some View {
        ViewThatFits(in: .horizontal) {
            trail(showWave: true).fixedSize(horizontal: true, vertical: false)
            trail(showWave: false)
        }
        .buttonStyle(.plain)
        .font(Typography.strong(13))
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
            HStack(spacing: 2) {
                Button(model.repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Work") {
                    model.select(nil)
                    model.navigation.content = .terminals
                }.lineLimit(1)
                Button { model.navigation.palette = .search } label: {
                    Image(systemName: "chevron.down").frame(width: 24, height: 28).contentShape(Rectangle())
                }
                    .help("Navigate (⌘K)").accessibilityLabel("Navigate Tasks, Waves and repositories")
            }.fixedSize()
            if showWave, let wave = model.breadcrumb?.wave {
                Text("/")
                Button(wave.roadmap.wave.displayName) {
                    model.select(.wave(id: wave.roadmap.wave.id))
                    model.navigation.content = .terminals
                }.lineLimit(1).fixedSize()
            }
            if let task = model.breadcrumb?.task {
                Text("/")
                Text(task.task.task.identifier).font(.system(.body, design: .monospaced)).fixedSize()
                Text(task.task.task.name).lineLimit(1).truncationMode(.tail)
                    .frame(minWidth: 60).help(task.task.task.name)
            }
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
