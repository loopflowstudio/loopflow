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
        roadmap.errorMessage != nil || sessions.errorMessage != nil || flowCatalog.errorMessage != nil || workflowCatalog.errorMessage != nil
            || visibleRoadmaps.contains { $0.tasks.unavailableReason != nil || !$0.unavailableTasks.isEmpty }
    }

    var paletteRows: [WorkPaletteRow] {
        var rows: [WorkPaletteRow] = []
        if let repoPath {
            let name = URL(fileURLWithPath: repoIdentity(repoPath)).lastPathComponent
            rows.append(.init(id: .repository, title: name, detail: "Repository", key: name))
        }
        for wave in visibleRoadmaps {
            rows.append(.init(id: .wave(wave.wave.id), title: wave.wave.name, detail: "Wave", key: wave.wave.name))
        }
        for task in visibleTasks {
            let home = visibleRoadmaps.first { $0.wave.id == task.task.waveId }?.wave.name ?? "No Wave"
            rows.append(.init(id: .task(task.id), title: task.task.name,
                              detail: "Task · \(home) · \(task.task.identifier)", key: task.task.identifier))
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
        for flow in flowCatalog.value ?? [] {
            rows.append(.init(id: .flow(flow.name), title: flow.name,
                              detail: "Flow definition", key: flow.name))
        }
        for workflow in workflowCatalog.value ?? [] {
            rows.append(.init(id: .workflow(workflow.name), title: workflow.name,
                              detail: "Workflow", key: workflow.name))
        }
        if let selection, selection.kind == .task, let found = task(id: selection.id) {
            rows.append(.init(id: .flowLog(selection.id), title: "Flow processes of \(found.task.task.identifier)",
                              detail: "Action · Open the log", key: "Flow processes"))
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

    func searchDestinations(_ text: String) -> [WorkPaletteRow] {
        let query = text.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let rows = paletteRows
        func rank(_ row: WorkPaletteRow) -> Int {
            if query.isEmpty {
                return navigation.recentDestinations.firstIndex { $0.id == row.id } ?? Int.max
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

enum WorkNavigationShortcut {
    case search
    case toggleSidebar

    init?(characters: String?, modifiers: NSEvent.ModifierFlags) {
        let keys = modifiers.intersection([.command, .shift, .control, .option])
        switch (characters?.lowercased(), keys) {
        case ("k", [.command]): self = .search
        case ("s", [.command, .control]): self = .toggleSidebar
        default: return nil
        }
    }
}

/// Navigation shortcuts are intercepted before a retained terminal can consume them.
struct WorkPaletteShortcut: NSViewRepresentable {
    let presented: Bool
    let restoreFocus: Bool
    let toggleSidebar: () -> Void
    let open: () -> Void
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        context.coordinator.view = view
        context.coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak coordinator = context.coordinator] event in
            guard let coordinator, let window = coordinator.view?.window,
                  event.window === window || window.childWindows?.contains(where: { $0 === event.window }) == true
            else { return event }
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
    }
    @MainActor final class Coordinator {
        weak var view: NSView?
        weak var previous: NSResponder?
        var monitor: Any?
        var presented = false
        var open: () -> Void
        var toggleSidebar: () -> Void
        init(open: @escaping () -> Void, toggleSidebar: @escaping () -> Void) {
            self.open = open
            self.toggleSidebar = toggleSidebar
        }
    }
}

struct FlowCatalogInspector: View {
    @Environment(\.palette) private var palette
    let entry: FlowCatalogEntry?
    @Bindable var navigation: WorkNavigation
    let model: WorkModel
    @State private var sourceError: String?
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(entry?.name ?? "Flow unavailable").font(Typography.sectionTitle(26))
            if let entry {
                Text(entry.source ?? "builtin Flow").font(Typography.code(11))
                Button(entry.source == nil ? "Customize" : "Edit") {
                    Task {
                        do {
                            let url = try await model.definitionSource(entry)
                            sourceError = nil
                            NSWorkspace.shared.open(url)
                        } catch { sourceError = error.localizedDescription }
                    }
                }
            }
            if let error = sourceError ?? model.flowCatalog.errorMessage {
                Text(error).foregroundStyle(palette.waiting)
            }
            Button("Refresh") { Task { await model.loadFlowCatalog(force: true) } }
            if let graph = entry?.graph, let composition = entry?.composition {
                FlowCompositionView(graph: graph, composition: composition, navigation: navigation)

            } else { Text(entry?.unavailable ?? "Refresh the Flow catalog and try again.") }
            Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
        }.padding(24).frame(width: 780)
    }
}

struct WorkflowCatalogInspector: View {
    @Environment(\.palette) private var palette
    let entry: WorkflowCatalogEntry?
    let model: WorkModel
    @State private var editingWorkflow = false
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(entry?.name ?? "Workflow unavailable").font(Typography.sectionTitle(26))
            if let entry {
                Text(entry.source == "stored" ? "Stored in this Wave" : (entry.source ?? "builtin Workflow")).font(Typography.code(11))
                Button(entry.source == nil ? "Customize" : "Edit") {
                    editingWorkflow = true
                }
                .disabled(model.breadcrumb?.wave?.roadmap.wave == nil)
            }
            if let error = model.workflowCatalog.errorMessage {
                Text(error).foregroundStyle(palette.waiting)
            }
            Button("Refresh") { Task { await model.loadWorkflowCatalog(force: true) } }
            if let workflow = entry?.workflow {
                WorkflowGraph(nodes: workflow.nodes, edges: workflow.edges)
            } else { Text(entry?.unavailable ?? "Refresh the Workflow catalog and try again.") }
            Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
        }.padding(24).frame(width: 780)
            .sheet(isPresented: $editingWorkflow) {
                if let entry, let wave = model.breadcrumb?.wave?.roadmap.wave {
                    WorkflowEditor(model: model, wave: wave, name: entry.name)
                }
            }
    }
}
