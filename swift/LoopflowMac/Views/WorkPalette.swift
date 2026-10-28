import AppKit
import Loopflow
import SwiftUI

struct WorkPaletteRow: Identifiable, Equatable {
    let id: WorkDestination
    let title: String
    let detail: String
    let key: String
}

extension WorkModel {
    var paletteIsStale: Bool {
        roadmap.errorMessage != nil || sessions.errorMessage != nil || graphCatalog.errorMessage != nil
            || visibleRoadmaps.contains { $0.tasks.unavailableReason != nil || !$0.unavailableTasks.isEmpty }
    }

    var paletteRows: [WorkPaletteRow] {
        var rows: [WorkPaletteRow] = []
        if let repoPath {
            let name = URL(fileURLWithPath: repoIdentity(repoPath)).lastPathComponent
            rows.append(.init(id: .repository, title: name, detail: "Repository", key: name))
        }
        for repo in repos where repo.path.normalizedFilePath != repoPath?.normalizedFilePath {
            rows.append(.init(id: .openRepository(repo.path), title: repo.displayName,
                detail: "Repository · \(repo.path)", key: repo.displayName))
        }
        for wave in roadmap.value?.waves ?? [] {
            rows.append(.init(id: .wave(wave.wave.id), title: wave.wave.name, detail: "Wave · \(wave.wave.repo)", key: wave.wave.name))
        }
        for task in roadmap.value?.tasks.items ?? [] {
            let home = roadmap.value?.waves.first { $0.wave.id == task.task.waveId }?.wave.name ?? "No Wave"
            rows.append(.init(id: .task(task.id), title: task.task.name,
                              detail: "Task · \(home) · \(task.task.identifier) · \(task.task.repo)", key: task.task.identifier))
        }
        if let linked = navigation.selectedTaskEvidence,
           !rows.contains(where: { $0.id == .task(linked.task.id) }) {
            rows.append(.init(id: .task(linked.task.id), title: linked.task.task.name,
                              detail: "Task · \(linked.wave?.wave.name ?? "No Wave") · \(linked.task.task.identifier)",
                              key: linked.task.task.identifier))
        }
        // Current-plan absence says nothing about retained historical Tasks.
        for recent in navigation.recentDestinations {
            if case .task = recent.id, !rows.contains(where: { $0.id == recent.id }) {
                rows.append(recent)
            }
        }
        for graph in graphCatalog.value ?? [] {
            rows.append(.init(id: .graph(graph.name), title: graph.name,
                              detail: "Graph", key: graph.name))
        }
        if let selection, selection.kind == .task, let found = task(id: selection.id) {
            rows.append(.init(id: .graphLog(selection.id), title: "Graph processes of \(found.task.task.identifier)",
                              detail: "Action · Open the log", key: "Graph processes"))
        }
        if let id = navigation.selectedSessionId, let session = sessions.value?.first(where: { $0.id == id }) {
            if session.titleSource != .unavailable {
                rows.append(.init(id: .rename(id), title: "Rename \(session.title)", detail: "Action · Edit name", key: "Rename"))
            }
            if session.work?.kind != .task {
                rows.append(.init(id: .bind(id), title: "Bind to Task…",
                                  detail: "Action · \(session.title) · Review permanent assignment", key: "Bind"))
            }
        }
        return rows
    }

    func searchDestinations(_ text: String, sidebarOnly: Bool = false) -> [WorkPaletteRow] {
        let query = text.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        var rows = paletteRows
        if sidebarOnly {
            let sidebar = Set(sidebarTaskIDs.map { WorkDestination.task($0) }
                + sidebarWaves.map { WorkDestination.wave($0.wave.id) })
            rows.removeAll { !sidebar.contains($0.id) }
        }
        let working = Set(workingTaskIDs)
        func rank(_ row: WorkPaletteRow) -> Int {
            if query.isEmpty {
                let recent = navigation.recentDestinations.firstIndex { $0.id == row.id } ?? 999
                switch row.id {
                case .task(let id): return (working.contains(id) ? 0 : 2000) + recent
                case .wave: return 4000 + recent
                case .repository, .openRepository: return 6000 + recent
                default: return 8000 + recent
                }
            }
            let labels = [row.key.lowercased(), row.title.lowercased()]
            if labels.contains(query) { return 0 }
            if labels.contains(where: { $0.hasPrefix(query) }) { return 1 }
            return 2
        }
        return rows.enumerated().filter {
            query.isEmpty || [$0.element.title, $0.element.detail, $0.element.key].contains { $0.localizedCaseInsensitiveContains(query) }
        }.sorted { lhs, rhs in
            let l = rank(lhs.element), r = rank(rhs.element)
            return l == r ? lhs.offset < rhs.offset : l < r
        }.map(\.element)
    }
}

/// What a breadcrumb's › lists: the level inside that crumb.
enum WorkJumpScope: Hashable {
    case repository
    case wave(String)
}

extension WorkModel {
    /// The jump bar's menu rows. Without text a level lists its direct contents;
    /// typing searches everything inside it. Repository scope stays implicit.
    func jumpRows(_ scope: WorkJumpScope, filter text: String) -> [WorkPaletteRow] {
        let query = text.trimmingCharacters(in: .whitespacesAndNewlines)
        let rows: [WorkPaletteRow]
        switch scope {
        case .repository:
            let waves = visibleRoadmaps.map { WorkPaletteRow(id: .wave($0.wave.id), title: $0.wave.displayName, detail: "Wave", key: $0.wave.name) }
            guard !query.isEmpty else { return waves }
            let tasks = visibleTasks.map { task in
                let home = visibleRoadmaps.first { $0.wave.id == task.task.waveId }?.wave.displayName ?? "No Wave"
                return WorkPaletteRow(id: .task(task.id), title: task.task.name,
                    detail: "Task · \(home) · \(task.task.identifier)", key: task.task.identifier)
            }
            rows = waves + tasks
        case .wave(let id):
            rows = (wave(id: id)?.tasks.items ?? []).sorted { $0.task.rank < $1.task.rank }.map { task in
                WorkPaletteRow(id: .task(task.id), title: task.task.name,
                    detail: task.task.identifier + (task.runtime?.owner != nil ? "" : " · Not started"), key: task.task.identifier)
            }
        }
        guard !query.isEmpty else { return rows }
        return rows.filter { [$0.title, $0.detail, $0.key].contains { $0.localizedCaseInsensitiveContains(query) } }
    }

    /// Go to a Work destination from search or the jump bar. Another
    /// repository's Work opens in that repository's own window.
    func jump(to destination: WorkDestination, source: String) {
        navigation.palette = nil
        switch destination {
        case .repository:
            select(nil)
            navigation.content = .terminals
        case .openRepository(let path): openRepository?(path, nil)
        case .wave(let id): openPaletteWave(id)
        case .task(let id):
            Perf.$inputTrace.withValue(UUID().uuidString) {
                Perf.agentInput("task_click", task: id, detail: "source=" + source)
                Task { await openPaletteTask(id) }
            }
            // Opening the Task records it as recent once it resolves.
            return
        default: return
        }
        remember(destination)
    }
}

struct WorkPalette: View {
    @Bindable var model: WorkModel
    var sidebarOnly = false
    let activate: (WorkDestination) -> Void
    @State private var search = ""
    @State private var highlighted: WorkDestination?
    @Environment(\.palette) private var palette

    private var rows: [WorkPaletteRow] { model.searchDestinations(search, sidebarOnly: sidebarOnly) }

    private func group(_ row: WorkPaletteRow, working: Set<String>) -> String {
        switch row.id {
        case .task(let id): working.contains(id) ? "Working Tasks" : "Other Tasks"
        case .wave: "Waves"
        case .repository, .openRepository: "Repositories"
        default: "Definitions and Session actions"
        }
    }

    var body: some View {
        let rows = rows
        let working = Set(model.workingTaskIDs)
        let selection = rows.first(where: { $0.id == highlighted })?.id ?? rows.first?.id
        VStack(alignment: .leading, spacing: 12) {
            if sidebarOnly {
                HStack {
                    Text("Search sidebar").font(Typography.caption())
                    Spacer()
                    Button("Search all Work") { model.navigation.palette = .search }
                }
            }
            PaletteSearchField(text: $search, move: move, submit: {
                if let id = selection { activate(id) }
            }, cancel: { model.navigation.palette = nil })
            .frame(height: 24)
            Divider()
            Picker("Sidebar shows", selection: Binding(
                get: { model.navigation.sidebarFilter },
                set: { model.navigation.sidebarFilter = $0 }
            )) {
                ForEach(WorkNavigation.SidebarFilter.allCases, id: \.self) { filter in
                    Text(filter.rawValue).tag(filter)
                }
            }
            .onChange(of: model.navigation.sidebarFilter) { _, _ in model.navigation.palette = nil }
            if model.paletteIsStale {
                Text("May be out of date · showing last available readings")
                    .font(Typography.caption()).foregroundStyle(palette.textSecondary)
            }
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 2) {
                        ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
                            if search.isEmpty, index == 0 || group(rows[index - 1], working: working) != group(row, working: working) {
                                Text(group(row, working: working)).font(.caption).foregroundStyle(palette.textSecondary)
                                    .frame(maxWidth: .infinity, alignment: .leading).padding(8)
                            }
                            Button { activate(row.id) } label: {
                                VStack(alignment: .leading, spacing: 3) {
                                    Text(row.title).font(Typography.body())
                                    Text(row.detail).font(Typography.caption()).foregroundStyle(palette.textSecondary)
                                }
                                .frame(maxWidth: .infinity, alignment: .leading).padding(8)
                                .background(selection == row.id ? palette.surfaceMuted : .clear)
                            }.buttonStyle(.plain).id(row.id)
                        }
                        if rows.isEmpty { Text("No matching destinations").font(Typography.caption()).padding() }
                    }
                }
                .onChange(of: selection) { _, id in if let id { proxy.scrollTo(id) } }
            }
            Text("↑↓ Select    ↩ Open    Esc Cancel").font(Typography.caption()).foregroundStyle(palette.textSecondary)
        }
        .padding(20).frame(width: 560, height: 420).background(palette.background)
        .onChange(of: search) { _, _ in highlighted = rows.first?.id }
        .onExitCommand { model.navigation.palette = nil }
        .task {
            await model.loadGraphCatalog()
            if !model.paletteIsStale && !model.roadmap.isLoading && !model.sessions.isLoading {
                let available = Set(model.paletteRows.map(\.id))
                model.navigation.recentDestinations.removeAll { !available.contains($0.id) }
            }
        }
        .accessibilityIdentifier("work-palette")
    }

    private func move(_ offset: Int) {
        let rows = rows
        guard !rows.isEmpty else { return }
        let index = rows.firstIndex { $0.id == highlighted } ?? 0
        highlighted = rows[min(max(index + offset, 0), rows.count - 1)].id
    }

}

struct TaskLinkView: View {
    @Bindable var model: WorkModel
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Open Task").font(Typography.sectionTitle(26))
            if let error = model.taskLinkReading.errorMessage { Text(error) }
            if let result = model.taskLinkReading.value {
                if result.tasks.items.isEmpty { Text("No exact Task found in available planning.") }
                if let reason = result.tasks.unavailableReason { Text(reason) }
                ForEach(result.tasks.items) { task in
                    let wave = result.waves.first { $0.wave.id == task.task.waveId }
                    Button("\(task.task.identifier) · \(task.task.name)\n\(task.task.repo)") {
                        Task { await model.chooseLinkedTask(wave: wave, task: task) }
                    }.buttonStyle(.plain)
                }
            }
            HStack {
                Button("Cancel") { model.dismissTaskLink() }.keyboardShortcut(.cancelAction)
                Button("Retry") {
                    Task { await model.retryTaskLink() }
                }
            }
        }.padding(24).frame(width: 560).font(Typography.body())
    }
}

/// Window chrome keys; Work navigation keys are registry actions.
enum WorkNavigationShortcut: Equatable {
    case search
    case toggleSidebar

    init?(characters: String?, modifiers: NSEvent.ModifierFlags) {
        let keys = modifiers.intersection([.command, .shift, .control, .option])
        switch (characters?.lowercased(), keys) {
        case ("k", [.command]): self = .search
        case ("\\", [.command]): self = .toggleSidebar
        default: return nil
        }
    }
}

/// Chrome shortcuts are intercepted before a retained terminal can consume them.
struct WorkPaletteShortcut: NSViewRepresentable {
    let presented: Bool
    let restoreFocus: Bool
    let toggleSidebar: () -> Void
    var modifiersChanged: (Bool) -> Void = { _ in }
    let open: () -> Void
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        context.coordinator.view = view
        context.coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged]) { [weak coordinator = context.coordinator] event in
            guard let coordinator, let window = coordinator.view?.window,
                  event.window === window, window.isKeyWindow, window.attachedSheet == nil,
                  (window.firstResponder as? NSTextView)?.hasMarkedText() != true
            else { return event }
            if event.type == .flagsChanged {
                coordinator.modifiersChanged(event.modifierFlags.intersection([.command, .control, .option, .shift]) == [.command])
                return event
            }
            guard let shortcut = WorkNavigationShortcut(characters: event.charactersIgnoringModifiers,
                modifiers: event.modifierFlags) else { return event }
            switch shortcut {
            case .toggleSidebar: coordinator.toggleSidebar()
            case .search:
                if !coordinator.presented {
                    coordinator.previous = window.firstResponder
                    coordinator.open()
                }
            }
            return nil
        }
        return view
    }
    func makeCoordinator() -> Coordinator { Coordinator(open: open, toggleSidebar: toggleSidebar) }
    func updateNSView(_ view: NSView, context: Context) {
        let coordinator = context.coordinator
        coordinator.modifiersChanged = modifiersChanged
        coordinator.open = open
        coordinator.toggleSidebar = toggleSidebar
        if !coordinator.presented && presented && coordinator.previous == nil {
            coordinator.previous = view.window?.firstResponder
        }
        if coordinator.presented && !presented && restoreFocus,
           let previous = coordinator.previous as? NSView, previous.window === view.window {
            view.window?.makeFirstResponder(previous)
        }
        if coordinator.presented && !presented { coordinator.previous = nil }
        coordinator.presented = presented
    }
    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        if let monitor = coordinator.monitor { NSEvent.removeMonitor(monitor) }
        coordinator.modifiersChanged(false)
    }
    @MainActor final class Coordinator {
        weak var view: NSView?
        weak var previous: NSResponder?
        var monitor: Any?
        var modifiersChanged: (Bool) -> Void = { _ in }
        var presented = false
        var open: () -> Void
        var toggleSidebar: () -> Void
        init(open: @escaping () -> Void, toggleSidebar: @escaping () -> Void) {
            self.open = open
            self.toggleSidebar = toggleSidebar
        }
    }
}

struct GraphCatalogInspector: View {
    @Environment(\.palette) private var palette
    let entry: GraphCatalogEntry?
    @Bindable var navigation: WorkNavigation
    let model: WorkModel
    @State private var sourceError: String?
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(entry?.name ?? "Graph unavailable").font(Typography.sectionTitle(26))
            if let entry {
                Text(entry.source ?? "builtin Graph").font(Typography.code(11))
                Button(entry.source == nil ? "Customize" : "Edit") {
                    Task {
                        do {
                            let url = try await model.definitionFile(entry.name)
                            sourceError = nil
                            NSWorkspace.shared.open(url)
                        } catch { sourceError = error.localizedDescription }
                    }
                }
            }
            if let error = sourceError ?? model.graphCatalog.errorMessage {
                Text(error).foregroundStyle(palette.warning)
            }
            Button("Refresh") { Task { await model.loadGraphCatalog(force: true) } }
            if let compiled = entry?.compiled, let composition = entry?.composition {
                GraphCompositionView(compiled: compiled, composition: composition, navigation: navigation)

            } else { Text(entry?.unavailable ?? "Refresh the Graph catalog and try again.") }
            Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
        }.padding(24).frame(width: 780)
    }
}

struct WorkflowCatalogInspector: View {
    @Environment(\.palette) private var palette
    let entry: WorkflowCatalogEntry?
    let model: WorkModel
    @State private var sourceError: String?
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(entry?.name ?? "Workflow unavailable").font(Typography.sectionTitle(26))
            if let entry {
                Text(entry.source ?? "builtin Workflow").font(Typography.code(11))
                Button(entry.source == nil ? "Customize" : "Edit") {
                    Task {
                        do {
                            let url = try await model.definitionFile(entry.name)
                            sourceError = nil
                            NSWorkspace.shared.open(url)
                        } catch { sourceError = error.localizedDescription }
                    }
                }
            }
            if let error = sourceError ?? model.workflowCatalog.errorMessage {
                Text(error).foregroundStyle(palette.warning)
            }
            Button("Refresh") { Task { await model.loadGraphCatalog(force: true) } }
            if let workflow = entry?.workflow {
                WorkflowDiagram(nodes: workflow.steps, edges: workflow.runs)
            } else { Text(entry?.unavailable ?? "Refresh the Workflow catalog and try again.") }
            Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
        }.padding(24).frame(width: 780)
    }
}

/// The field editor owns arrows while typing; handle its navigation commands
/// directly rather than relying on SwiftUI's unfocused move-command responder.
private struct PaletteSearchField: NSViewRepresentable {
    @Environment(\.palette) private var palette
    @Binding var text: String
    let move: (Int) -> Void
    let submit: () -> Void
    let cancel: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(parent: self) }
    func makeNSView(context: Context) -> SearchField {
        let field = SearchField()
        field.isBordered = false
        field.focusRingType = .none
        field.drawsBackground = false
        field.font = NSFont(name: "Lato-Regular", size: 13) ?? .systemFont(ofSize: 13)
        field.placeholderString = "Go to a Task, Wave or repository"
        field.setAccessibilityIdentifier("work-palette-search")
        field.delegate = context.coordinator
        return field
    }
    func updateNSView(_ field: SearchField, context: Context) {
        context.coordinator.parent = self
        field.textColor = NSColor(palette.text)
        if let editor = field.currentEditor() as? NSTextView { editor.applyPalette(palette) }
        if field.stringValue != text { field.stringValue = text }
    }
    final class SearchField: NSTextField {
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            window?.makeFirstResponder(self)
        }
    }
    @MainActor final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: PaletteSearchField
        init(parent: PaletteSearchField) { self.parent = parent }
        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSTextField else { return }
            parent.text = field.stringValue
        }
        func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
            switch selector {
            case #selector(NSResponder.moveDown(_:)): parent.move(1)
            case #selector(NSResponder.moveUp(_:)): parent.move(-1)
            case #selector(NSResponder.insertNewline(_:)): parent.submit()
            case #selector(NSResponder.cancelOperation(_:)): parent.cancel()
            default: return false
            }
            return true
        }
    }
}
