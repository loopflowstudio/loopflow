import AppKit
import Loopflow
import SwiftUI

struct WorkspacePaletteRow: Identifiable, Equatable {
    let id: WorkspaceDestination
    let title: String
    let detail: String
    let key: String
}

extension PodiumModel {
    var paletteIsStale: Bool {
        roadmap.errorMessage != nil || sessions.errorMessage != nil || flowCatalog.errorMessage != nil
            || visibleRoadmaps.contains { $0.tasks.unavailableReason != nil || !$0.unavailableTasks.isEmpty }
    }

    var paletteRows: [WorkspacePaletteRow] {
        var rows: [WorkspacePaletteRow] = []
        for wave in visibleRoadmaps {
            rows.append(.init(id: .wave(wave.wave.id), title: wave.wave.name, detail: "Wave", key: wave.wave.name))
            for task in wave.tasks.items {
                rows.append(.init(id: .task(task.id), title: task.task.name,
                                  detail: "Task · \(wave.wave.name) · \(task.task.identifier)", key: task.task.identifier))
            }
        }
        if let linked = navigation.selectedTaskEvidence,
           !rows.contains(where: { $0.id == .task(linked.task.id) }) {
            rows.append(.init(id: .task(linked.task.id), title: linked.task.task.name,
                              detail: "Task · \(linked.wave.wave.name) · \(linked.task.task.identifier)",
                              key: linked.task.task.identifier))
        }
        // Current-plan absence says nothing about retained historical Tasks.
        for recent in navigation.recentDestinations {
            if case .task = recent.id, !rows.contains(where: { $0.id == recent.id }) {
                rows.append(recent)
            }
        }
        for session in visibleSessions {
            rows.append(.init(id: .session(session.id), title: session.title,
                              detail: ["Session", session.workPath, session.provider].compactMap { $0 }.joined(separator: " · "),
                              key: session.title))
        }
        for flow in flowCatalog.value ?? [] {
            rows.append(.init(id: .flow(flow.name), title: flow.name,
                              detail: flow.kind == .workflow ? "Workflow" : "Flow template", key: flow.name))
        }
        if let selection, selection.kind == .task, let found = task(id: selection.id) {
            rows.append(.init(id: .flowLog(selection.id), title: "Flow execs of \(found.task.task.identifier)",
                              detail: "Action · Open the log", key: "Flow execs"))
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

    func searchDestinations(_ text: String) -> [WorkspacePaletteRow] {
        let query = text.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let rows = paletteRows
        func rank(_ row: WorkspacePaletteRow) -> Int {
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

struct WorkspacePalette: View {
    @Bindable var model: PodiumModel
    let activate: (WorkspaceDestination) -> Void
    @State private var search = ""
    @State private var highlighted: WorkspaceDestination?
    @Environment(\.palette) private var palette

    private var rows: [WorkspacePaletteRow] { model.searchDestinations(search) }

    private var effectiveSelection: WorkspaceDestination? {
        rows.first(where: { $0.id == highlighted })?.id ?? rows.first?.id
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PaletteSearchField(text: $search, move: move, submit: {
                if let id = effectiveSelection { activate(id) }
            }, cancel: { model.navigation.palette = nil })
            .frame(height: 24)
            Divider()
            if model.paletteIsStale {
                Text("May be out of date · showing last available readings")
                    .font(Typography.caption()).foregroundStyle(palette.textSecondary)
            }
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 2) {
                        ForEach(rows) { row in
                            Button { activate(row.id) } label: {
                                VStack(alignment: .leading, spacing: 3) {
                                    Text(row.title).font(Typography.body())
                                    Text(row.detail).font(Typography.caption()).foregroundStyle(palette.textSecondary)
                                }
                                .frame(maxWidth: .infinity, alignment: .leading).padding(8)
                                .background(effectiveSelection == row.id ? palette.surfaceMuted : .clear)
                            }.buttonStyle(.plain).id(row.id)
                        }
                        if rows.isEmpty { Text("No matching destinations").font(Typography.caption()).padding() }
                    }
                }
                .onChange(of: effectiveSelection) { _, id in if let id { proxy.scrollTo(id) } }
            }
            Text("↑↓ Select    ↩ Open    Esc Cancel").font(Typography.caption()).foregroundStyle(palette.textSecondary)
        }
        .padding(20).frame(width: 560, height: 420).background(palette.background)
        .onChange(of: search) { _, _ in highlighted = rows.first?.id }
        .onExitCommand { model.navigation.palette = nil }
        .task {
            await model.loadFlowCatalog()
            if !model.paletteIsStale && !model.roadmap.isLoading && !model.sessions.isLoading {
                let available = Set(model.paletteRows.map(\.id))
                model.navigation.recentDestinations.removeAll { !available.contains($0.id) }
            }
        }
        .accessibilityIdentifier("workspace-palette")
    }

    private func move(_ offset: Int) {
        guard !rows.isEmpty else { return }
        let index = rows.firstIndex { $0.id == effectiveSelection } ?? 0
        highlighted = rows[min(max(index + offset, 0), rows.count - 1)].id
    }

}

struct TaskLinkView: View {
    @Bindable var model: PodiumModel
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Open Task").font(Typography.sectionTitle(26))
            if model.taskLinkReading.isLoading { Text("Finding Task…") }
            if let error = model.taskLinkReading.errorMessage { Text(error) }
            if let result = model.taskLinkReading.value {
                let matches = result.waves.flatMap { wave in wave.tasks.items.map { (wave, $0) } }
                if matches.isEmpty { Text("No exact Task found in available planning.") }
                ForEach(result.waves, id: \.wave.id) { wave in
                    if let reason = wave.tasks.unavailableReason { Text("\(wave.wave.repo): \(reason)") }
                    ForEach(wave.tasks.items) { task in
                        Button("\(task.task.identifier) · \(task.task.name)\n\(wave.wave.repo) / \(wave.wave.name)") {
                            Task { await model.chooseLinkedTask(wave: wave, task: task) }
                        }.buttonStyle(.plain)
                    }
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

/// Cmd-K is intercepted before a retained terminal can consume it.
struct WorkspacePaletteShortcut: NSViewRepresentable {
    let presented: Bool
    let restoreFocus: Bool
    let open: () -> Void
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        context.coordinator.view = view
        context.coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak coordinator = context.coordinator] event in
            guard let coordinator, let window = coordinator.view?.window, event.window === window,
                  event.modifierFlags.intersection([.command, .shift, .control, .option]) == [.command],
                  event.charactersIgnoringModifiers?.lowercased() == "k" else { return event }
            if !coordinator.presented {
                coordinator.previous = window.firstResponder
                coordinator.open()
            }
            return nil
        }
        return view
    }
    func makeCoordinator() -> Coordinator { Coordinator(open: open) }
    func updateNSView(_ view: NSView, context: Context) {
        let coordinator = context.coordinator
        coordinator.open = open
        if coordinator.presented && !presented && restoreFocus,
           let previous = coordinator.previous as? NSView, previous.window === view.window {
            view.window?.makeFirstResponder(previous)
        }
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
        init(open: @escaping () -> Void) { self.open = open }
    }
}

struct FlowCatalogInspector: View {
    let entry: FlowCatalogEntry?
    @Bindable var navigation: WorkspaceNavigation
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(entry?.name ?? "Flow unavailable").font(Typography.sectionTitle(26))
            if let graph = entry?.graph, let template = entry?.template {
                FlowTemplateView(graph: graph, template: template, navigation: navigation)
            } else if let workflow = entry?.workflow {
                WorkflowGraph(nodes: workflow.nodes, edges: workflow.edges)
            } else { Text(entry?.unavailable ?? "Refresh the Flow catalog and try again.") }
            Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
        }.padding(24).frame(width: 780)
    }
}

/// The field editor owns arrows while typing; handle its navigation commands
/// directly rather than relying on SwiftUI's unfocused move-command responder.
private struct PaletteSearchField: NSViewRepresentable {
    @Binding var text: String
    let move: (Int) -> Void
    let submit: () -> Void
    let cancel: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(parent: self) }
    func makeNSView(context: Context) -> SearchField {
        let field = SearchField()
        field.isBordered = false
        field.drawsBackground = false
        field.font = NSFont(name: "Lato-Regular", size: 13) ?? .systemFont(ofSize: 13)
        field.placeholderString = "Go to a Wave, Task, Session or Flow"
        field.setAccessibilityIdentifier("workspace-palette-search")
        field.delegate = context.coordinator
        return field
    }
    func updateNSView(_ field: SearchField, context: Context) {
        context.coordinator.parent = self
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
