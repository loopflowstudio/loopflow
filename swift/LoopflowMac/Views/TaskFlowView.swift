// A Task's Flow: the definition a Start would pin, or the pinned invocation and
// where its saved cursor stands. Topology, occurrence keys, return counts, and
// control legality come from Rust (`TaskFlowSnapshot`); this view draws them.

#if os(macOS)
import Loopflow
import SwiftUI

/// How one occurrence reads in the diagram. Completion is scoped to the current
/// pass; a repeated occurrence is pending again after its loop returns.
enum FlowNodeState: Equatable {
    case completed
    case running
    case waitingForHuman
    case blocked
    case stopped
    case unknown
    case pendingHuman
    case pending
}

/// Classify every drawn occurrence from the shared projection.
func flowNodeStates(_ graph: FlowGraph, pinned: PinnedTaskFlow?) -> [String: FlowNodeState] {
    var states: [String: FlowNodeState] = [:]
    func visit(_ nodes: [FlowNode]) {
        for node in nodes {
            states[node.key] = state(of: node, pinned: pinned)
            for path in node.paths { visit(path.steps) }
        }
    }
    visit(graph.steps)
    return states
}

private func state(of node: FlowNode, pinned: PinnedTaskFlow?) -> FlowNodeState {
    guard let pinned else { return node.human ? .pendingHuman : .pending }
    let isCurrent = pinned.current == node.key
        || (node.kind == .xor && pinned.current?.hasPrefix(node.key + "/") == true)
    if isCurrent {
        switch pinned.execution {
        case .running, .starting: return .running
        case .human: return .waitingForHuman
        case .blocked: return .blocked
        case .idle: return .stopped
        case .unknown: return .unknown
        }
    }
    if pinned.completed.contains(node.key) { return .completed }
    return node.human ? .pendingHuman : .pending
}

struct TaskFlowView: View {
    @Bindable var model: PodiumModel
    let task: RoadmapTask
    let wave: WaveSnapshot

    @Environment(\.palette) private var palette
    @State private var hovering = false
    /// Clock for the running status line's elapsed time; ticks while running.
    @State private var now = Date()
    @FocusState private var focus: HeaderFocus?

    private enum HeaderFocus: Hashable { case name, restart, search }

    private var flow: TaskFlowSnapshot { task.flow }
    private var catalog: PodiumReading<[FlowCatalogEntry]> { model.flowCatalog }

    private var draft: TaskFlowDraft {
        get { model.navigation.flowDrafts[task.id] ?? TaskFlowDraft() }
        nonmutating set { model.navigation.flowDrafts[task.id] = newValue }
    }

    private var inspected: Binding<String?> {
        Binding(get: {
            guard let selection = draft.selectedNode,
                  selection.invocationId == pinned?.invocationId else { return nil }
            return selection.node
        }, set: { node in
            var next = draft
            next.selectedNode = node.map { FlowNodeSelection(invocationId: pinned?.invocationId, node: $0) }
            draft = next
        })
    }

    private var search: Binding<String> {
        Binding(get: { draft.search }, set: { draft.search = $0 })
    }

    private var pinned: PinnedTaskFlow? {
        if case .pinned(let pinned) = flow.record { return pinned }
        return nil
    }

    private var previewName: String { draft.preview ?? flow.recommended }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                header
                if draft.picker != nil { pickerList }
                if let pendingRestart = draft.pendingRestart { restartConfirmation(pendingRestart) }
                if let error = draft.error {
                    Text(error)
                        .font(Typography.body(12))
                        .foregroundStyle(WorkspaceTone.blocked.ink)
                        .textSelection(.enabled)
                        .accessibilityIdentifier("task-flow-error")
                }
                diagram
            }
            .workspacePanel(padding: 13)
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Circle().fill(statusTone.ink).frame(width: 7, height: 7)
                    .alignmentGuide(.firstTextBaseline) { $0[VerticalAlignment.center] + 4 }
                    .accessibilityHidden(true)
                statusText
                    .font(Typography.body(13))
                    .foregroundStyle(pinned?.execution == .blocked ? WorkspaceTone.blocked.ink : palette.textSecondary)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-flow-status")
            }
            .padding(.leading, 2)
        }
        .id("task-flow-anchor")
        .task { await model.loadFlowCatalog() }
        .task(id: runningStep != nil) {
            // Elapsed time only moves while the worker runs; unchanged readings
            // do not re-render, so the line keeps its own coarse clock.
            guard runningStep != nil else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(30))
                now = Date()
            }
        }
        .onExitCommand { dismissTransient() }
    }

    /// One line under the Flow. Running work reads `● review-slice · 12m · claude`:
    /// the current occurrence, how long the shared Task record has been in this
    /// state, and the harness of the exact active Run (else the Task's provider).
    private var statusText: Text {
        guard let step = runningStep else { return Text(statusLine) }
        var parts: [String] = []
        if let elapsed = task.runtime.flatMap({ Self.elapsed(since: $0.updatedAt, now: now) }) { parts.append(elapsed) }
        if let provider = runningProvider { parts.append(provider) }
        return Text(step).font(Typography.mono) + Text(parts.map { " · \($0)" }.joined())
    }

    /// The label of the occurrence the worker is running, when it is.
    private var runningStep: String? {
        guard let pinned, pinned.execution == .running || pinned.execution == .starting,
              let current = pinned.current else { return nil }
        return pinned.graph.node(current)?.label ?? current
    }

    private var runningProvider: String? {
        guard let runtime = task.runtime else { return nil }
        let work = WorkReference.task(id: runtime.workId)
        return model.activeRuns.value?.runs.first { $0.work == work }?.harness ?? runtime.provider
    }

    /// `12s`, `12m`, `3h 05m`, `2d 03h`; nil when the timestamp does not parse.
    static func elapsed(since iso: String, now: Date) -> String? {
        let parser = ISO8601DateFormatter()
        parser.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let since = parser.date(from: iso) ?? {
            parser.formatOptions = [.withInternetDateTime]
            return parser.date(from: iso)
        }()
        guard let since else { return nil }
        let seconds = max(0, Int(now.timeIntervalSince(since)))
        switch seconds {
        case ..<60: return "\(seconds)s"
        case ..<3600: return "\(seconds / 60)m"
        case ..<86400: return String(format: "%dh %02dm", seconds / 3600, seconds % 3600 / 60)
        default: return String(format: "%dd %02dh", seconds / 86400, seconds % 86400 / 3600)
        }
    }

    private var statusTone: WorkspaceTone {
        switch flow.record {
        case .pinned(let pinned):
            switch pinned.execution {
            case .running, .starting: .running
            case .human: .human
            case .blocked: .blocked
            case .idle, .unknown: .stopped
            }
        case .finished: .done
        case .none: .neutral
        }
    }

    /// Returns per loop, in authored order: the Flow's iteration tuple.
    private var iterationTuple: String? {
        guard let pinned else { return nil }
        return flowIterationLabel(pinned.iterations).map { "Iteration \($0)" }
    }

    // MARK: Header

    private var header: some View {
        HStack(alignment: .center, spacing: Spacing.sm) {
            Button {
                open(pinned == nil ? .preview : .restart)
            } label: {
                HStack(spacing: 4) {
                    Text(pinned?.graph.name ?? previewName)
                        .font(Typography.code(15))
                    Image(systemName: "chevron.down").font(.system(size: 9, weight: .semibold))
                        .foregroundStyle(palette.textTertiary)
                }
                .foregroundStyle(palette.text)
            }
            .buttonStyle(.plain)
            .focused($focus, equals: .name)
            .help(pinned == nil ? "Choose the Flow to start" : "Pinned Flow · choose a replacement to stop and restart")
            .accessibilityLabel(pinned == nil ? "Flow \(previewName), choose another" : "Pinned Flow \(pinned!.graph.name)")
            .accessibilityIdentifier("task-flow-name")

            if let iterationTuple {
                Text(iterationTuple)
                    .font(Typography.code(11.5))
                    .monospacedDigit()
                    .foregroundStyle(FlowPalette.loops[1])
                    .padding(.horizontal, 7)
                    .padding(.vertical, 2)
                    .background(FlowPalette.loops[0].opacity(0.10), in: RoundedRectangle(cornerRadius: 4))
                    .help("Returns per loop, in authored order")
                    .accessibilityIdentifier("task-flow-iteration")
            } else if pinned == nil {
                Text("Preview").font(Typography.caption(12)).foregroundStyle(palette.textTertiary)
            }

            if pinned != nil, revealRestart {
                let restart = flow.control(.restart)
                Button("Stop & restart…") { open(.restart) }
                    .buttonStyle(.plain)
                    .font(Typography.body(12.5))
                    .foregroundStyle(palette.accentInk)
                    .focused($focus, equals: .restart)
                    .disabled(restart?.unavailable != nil || draft.acting)
                    .help(restart?.unavailable ?? "Replace this Flow after confirming")
                    .accessibilityIdentifier("task-flow-restart")
            }
            Spacer(minLength: Spacing.sm)
            if draft.acting { ProgressView().controlSize(.small) }
            controls
        }
        .frame(minHeight: 28)
        .onHover { hovering = $0 }
    }

    private var revealRestart: Bool {
        hovering || focus == .name || focus == .restart || draft.picker == .restart || draft.pendingRestart != nil
    }

    @ViewBuilder
    private var controls: some View {
        if pinned != nil {
            let resume = flow.control(.resume)
            Button("Resume") { run(.resume) }
                .buttonStyle(WorkspaceOutlineButtonStyle())
                .disabled(resume?.unavailable != nil || draft.acting)
                .help(resume?.unavailable ?? "Continue from the saved boundary")
                .accessibilityIdentifier("task-flow-resume")
        } else {
            let start = flow.control(.start)
            Button("Start") { run(.start(flow: previewName)) }
                .buttonStyle(WorkspaceOutlineButtonStyle())
                .disabled(start?.unavailable != nil || draft.acting || previewEntry?.graph == nil)
                .help(start?.unavailable ?? "Pin \(previewName) and start its first step")
                .accessibilityIdentifier("task-flow-start")
        }
    }

    // MARK: Picker

    private var matches: [FlowCatalogEntry] {
        let entries = catalog.value ?? []
        let needle = draft.search.trimmingCharacters(in: .whitespaces).lowercased()
        return needle.isEmpty ? entries : entries.filter { $0.name.lowercased().contains(needle) }
    }

    private var pickerList: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                TextField(draft.picker == .restart ? "Replacement Flow" : "Search Flows", text: search)
                    .textFieldStyle(.plain)
                    .font(Typography.code(12))
                    .focused($focus, equals: .search)
                    .onSubmit { if let first = matches.first(where: { $0.graph != nil }) { choose(first.name) } }
                    .accessibilityIdentifier("task-flow-search")
                Button("Cancel") { dismissTransient() }
                    .buttonStyle(.link)
                    .font(Typography.body(12))
                    .accessibilityIdentifier("task-flow-search-cancel")
            }
            .padding(Spacing.xs)
            .background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: CornerRadius.md))
            if let reason = catalog.errorMessage {
                Text("Flows unavailable: \(reason)").font(Typography.caption(11)).foregroundStyle(Color.statusWarning)
            } else if catalog.isLoading {
                Text("Reading Flows…").font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
            } else if matches.isEmpty {
                Text("No Flow matches").font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
            }
            ForEach(matches.prefix(8)) { entry in
                Button { choose(entry.name) } label: {
                    HStack {
                        Text(entry.name).font(Typography.code(12))
                        if entry.name == flow.recommended {
                            Text("recommended").font(Typography.caption(10)).foregroundStyle(palette.textSecondary)
                        }
                        Spacer()
                        if let reason = entry.unavailable {
                            Text("unavailable").font(Typography.caption(10)).foregroundStyle(Color.statusWarning).help(reason)
                        }
                    }
                    .padding(.vertical, 3)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .disabled(entry.graph == nil)
                .accessibilityIdentifier("task-flow-option-\(entry.name)")
            }
        }
    }

    private func restartConfirmation(_ replacement: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text("Stop \(pinned?.graph.name ?? "the Flow") and restart with \(replacement)?")
                .font(Typography.body(13).weight(.bold))
            Text("Loopflow refreshes this Task from Linear, commits and pushes every change in its worktree as a checkpoint, stops the current worker, and starts \(replacement) from its first step. The Task, worktree, and PR history stay.")
                .font(Typography.body(12))
                .foregroundStyle(palette.textSecondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack {
                Button("Cancel") { dismissTransient() }
                    .accessibilityIdentifier("task-flow-restart-cancel")
                Button("Stop & restart") { run(.restart(flow: replacement)) }
                    .buttonStyle(WorkspaceOutlineButtonStyle())
                    .disabled(draft.acting)
                    .accessibilityIdentifier("task-flow-restart-confirm")
            }
            .controlSize(.small)
        }
        .padding(Spacing.sm)
        .background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: CornerRadius.md))
        .accessibilityIdentifier("task-flow-restart-confirmation")
    }

    // MARK: Diagram

    private var previewEntry: FlowCatalogEntry? {
        catalog.value?.first { $0.name == previewName }
    }

    @ViewBuilder
    private var diagram: some View {
        if let pinned {
            FlowDiagram(graph: pinned.graph, pinned: pinned, delivery: deliveryState, inspected: inspected)
        } else if let entry = previewEntry, let graph = entry.graph, let template = entry.template {
            FlowTemplateView(graph: graph, template: template, navigation: model.navigation)
        } else if let entry = catalog.value?.first(where: { $0.name == previewName }), let reason = entry.unavailable {
            Text("\(previewName) cannot be previewed: \(reason)")
                .font(Typography.caption(11)).foregroundStyle(Color.statusWarning)
        } else if let reason = catalog.errorMessage {
            Text("Flow preview unavailable: \(reason)")
                .font(Typography.caption(11)).foregroundStyle(Color.statusWarning)
        } else if catalog.value != nil {
            Text("\(previewName) is not an available Flow here")
                .font(Typography.caption(11)).foregroundStyle(Color.statusWarning)
        } else {
            Text("Reading Flows…").font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
        }
    }

    /// The delivery operation's real state, from the Task's active PR record.
    /// Queue admission is never presented as a merge.
    private var deliveryState: String? {
        guard let pr = task.activePr else { return nil }
        let number = pr.publication?.github.map { "PR #\($0.number)" }
        switch pr.phase {
        case .working: return nil
        case .publishing: return "publishing"
        case .open:
            if pr.publication?.merge?.mode == .auto { return [number, "auto-merge requested"].compactMap { $0 }.joined(separator: " ") }
            return [number, "open"].compactMap { $0 }.joined(separator: " ")
        case .merged: return [number, "merged"].compactMap { $0 }.joined(separator: " ")
        case .abandoned: return [number, "abandoned"].compactMap { $0 }.joined(separator: " ")
        }
    }

    private var statusLine: String {
        switch flow.record {
        case .pinned(let pinned):
            // Running, starting, review and unknown reasons already name their
            // state; a failure reason and an idle boundary need the label.
            switch pinned.execution {
            case .blocked: return "Blocked · \(pinned.reason)"
            case .idle: return "Stopped · \(pinned.reason)"
            case .running, .starting, .human, .unknown: return pinned.reason
            }
        case .finished(let name):
            return "\(name) finished · its pinned definition is not retained. Preview: \(previewName)"
        case .none:
            if task.runtime?.started == true {
                return "Flow not started · independent Runs exist"
            }
            return "Not started · No runs yet."
        }
    }

    // MARK: Actions

    private func open(_ mode: TaskFlowDraft.Picker) {
        guard !draft.acting else { return }
        if mode == .restart, flow.control(.restart)?.unavailable != nil { return }
        var next = draft
        next.picker = mode
        next.pendingRestart = nil
        next.search = ""
        draft = next
        focus = .search
        Task { await model.loadFlowCatalog(force: true) }
    }

    private func choose(_ name: String) {
        var next = draft
        switch next.picker {
        case .preview:
            next.preview = name
            next.selectedNode = nil
        case .restart: next.pendingRestart = name
        case nil: break
        }
        next.picker = nil
        next.search = ""
        draft = next
    }

    private func dismissTransient() {
        var next = draft
        next.picker = nil
        next.pendingRestart = nil
        next.search = ""
        draft = next
        inspected.wrappedValue = nil
    }

    private func run(_ request: TaskFlowControlRequest) {
        Task { await model.performFlowControl(request, task: task, wave: wave) }
    }
}

// MARK: - Diagram

struct FlowTemplateView: View {
    let graph: FlowGraph
    let template: FlowTemplate
    @Bindable var navigation: WorkspaceNavigation
    @State private var inspected: String?

    private var expanded: Binding<Set<String>> {
        Binding(get: { navigation.expandedTemplateGroups[template.revision] ?? [] },
                set: { navigation.expandedTemplateGroups[template.revision] = $0 })
    }

    static func spans(_ original: FlowGraph, projection: FlowTemplateProjection) -> [LoopSpan] {
        FlowDiagram.spans(original, pinned: nil).compactMap { edge in
            guard let fromKey = projection.visibleKeys[original.steps[edge.from].key],
                  let toKey = projection.visibleKeys[original.steps[edge.to].key],
                  let from = projection.graph.steps.firstIndex(where: { $0.key == fromKey }),
                  let to = projection.graph.steps.firstIndex(where: { $0.key == toKey }) else { return nil }
            return LoopSpan(decider: edge.decider, number: edge.number, from: from, to: to, evidence: nil)
        }
    }

    var body: some View {
        let projection = template.project(graph, expanded: expanded.wrappedValue)
        VStack(alignment: .leading, spacing: Spacing.sm) {
            TemplateDisclosures(items: template.items, graph: graph, expanded: expanded)
            FlowDiagram(graph: projection.graph, pinned: nil, inspected: Binding(
                get: { inspected },
                set: { key in
                    if let key, template.items.contains(where: { $0.groupIDs.contains(key) }) {
                        expanded.wrappedValue.insert(key)
                        inspected = nil
                    } else { inspected = key }
                }
            ), templateSpans: Self.spans(graph, projection: projection))
        }
        .id(template.revision)
        .onChange(of: template.revision) { _, _ in inspected = nil }
        .accessibilityIdentifier("flow-template-\(graph.name)")
    }
}

/// Disclosure controls keep composition identity even for an empty body.
private struct TemplateDisclosures: View {
    let items: [FlowTemplateItem]
    let graph: FlowGraph
    @Binding var expanded: Set<String>

    var body: some View {
        ForEach(items) { item in
            switch item {
            case .group(let id, let name, let children):
                DisclosureGroup(isExpanded: Binding(
                    get: { expanded.contains(id) },
                    set: { if $0 { expanded.insert(id) } else { expanded.remove(id) } }
                )) {
                    if children.isEmpty { Text("No steps").font(Typography.caption()) }
                    AnyView(TemplateDisclosures(items: children, graph: graph, expanded: $expanded))
                } label: {
                    Text("\(name) · \(item.nodeKeys.count) steps").font(Typography.mono)
                }
                .accessibilityIdentifier("template-group-\(id)")
            case .node(let key, let paths):
                if !paths.isEmpty {
                    ForEach(paths.keys.sorted(), id: \.self) { name in
                        DisclosureGroup("\(graph.node(key)?.label ?? key) · \(name)") {
                            AnyView(TemplateDisclosures(items: paths[name]!, graph: graph, expanded: $expanded))
                            if let node = graph.node(key), let path = node.paths.first(where: { $0.name == name }) {
                                let branch = FlowGraph(name: name, steps: path.steps)
                                TemplatePathDiagram(graph: branch, items: paths[name]!, expanded: $expanded)
                            }
                        }.font(Typography.mono)
                    }
                }
            }
        }
    }
}

private struct TemplatePathDiagram: View {
    let graph: FlowGraph
    let items: [FlowTemplateItem]
    @Binding var expanded: Set<String>
    @State private var inspected: String?
    var body: some View {
        let projection = FlowTemplate(revision: "", items: items).project(graph, expanded: expanded)
        FlowDiagram(graph: projection.graph, pinned: nil, inspected: Binding(
            get: { inspected },
            set: { key in
                if let key, items.contains(where: { $0.groupIDs.contains(key) }) {
                    expanded.insert(key)
                    inspected = nil
                } else { inspected = key }
            }
        ), templateSpans: FlowTemplateView.spans(graph, projection: projection))
    }
}

/// A Flow's top-level occurrences in rows: what runs once, the loops, then the
/// tail. Loop endpoints always share a row, so each authored backward edge draws
/// its tinted span and a return arrow on its own lane beneath. When the window
/// is wide enough the tail follows on one indented row; otherwise each section
/// gets its own labelled row. Only an overwide loop row scrolls.
struct FlowDiagram: View {
    let graph: FlowGraph
    let pinned: PinnedTaskFlow?
    /// Real state of the delivery operation (the `pr land` op), shown on its chip.
    var delivery: String? = nil
    @Binding var inspected: String?
    var templateSpans: [LoopSpan]? = nil

    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var available: CGFloat = 0

    static let nodeHeight: CGFloat = 24
    static let gap: CGFloat = 9
    static let compactGap: CGFloat = 5
    /// JetBrains Mono advances 0.6 em; node labels are 11 pt.
    static let charWidth: CGFloat = 6.6
    /// Room for the widest row label, `↳ delivery`.
    static let labelSlot: CGFloat = 74
    static let rowGap: CGFloat = 6
    static let loopLabelHeight: CGFloat = 12

    /// Top-level authored returns in authored order, numbered from 1. A pinned
    /// Flow adds each edge's saved counts. Nested XOR returns appear in node detail.
    static func spans(_ graph: FlowGraph, pinned: PinnedTaskFlow?) -> [LoopSpan] {
        let steps = graph.steps
        return steps.enumerated().compactMap { to, node -> (Int, Int, FlowNode)? in
            guard let target = node.returnsTo,
                  let from = steps.firstIndex(where: { $0.key == target }), from <= to else { return nil }
            return (from, to, node)
        }
        .enumerated()
        .map { number, edge in
            LoopSpan(decider: edge.2.key, number: number + 1, from: edge.0, to: edge.1,
                     evidence: pinned?.returns.first { $0.decider == edge.2.key })
        }
    }

    var body: some View {
        let states = flowNodeStates(graph, pinned: pinned)
        let rows = layout(width: available > 0 ? available : 1000)
        let contentWidth = rows.map(\.width).max() ?? 0
        VStack(alignment: .leading, spacing: Spacing.sm) {
            ScrollViewReader { reader in
                ScrollView(.horizontal, showsIndicators: contentWidth > available + 1) {
                    VStack(alignment: .leading, spacing: Self.rowGap) {
                        ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                            rowView(row, states: states)
                        }
                    }
                    .frame(width: max(contentWidth, 0), alignment: .leading)
                }
                .scrollDisabled(contentWidth <= available + 1)
                .frame(maxWidth: .infinity, alignment: .leading)
                .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { available = $0 }
                .accessibilityIdentifier("task-flow-diagram")
                .onChange(of: inspected, initial: true) { _, key in
                    guard let key, let root = graph.steps.first(where: {
                        $0.key == key || key.hasPrefix($0.key + "/")
                    }) else { return }
                    reader.scrollTo(root.key, anchor: .center)
                }
            }
            if let key = inspected, let node = graph.node(key) {
                FlowNodeDetail(node: node, state: states[key] ?? .pending, pinned: pinned, graph: graph)
            }
        }
    }

    // MARK: Layout

    @MainActor
    struct Row {
        var indices: [Int]
        var label: String?
        var indent: CGFloat
        var loops: [LoopSpan]
        var widths: [CGFloat]
        var gap: CGFloat = FlowDiagram.gap

        var start: CGFloat {
            (label == nil ? 0 : FlowDiagram.labelSlot) + indent + (loops.isEmpty ? 1 : FlowLoopGeometry.pad(loops.count - 1) + 1)
        }
        var width: CGFloat {
            start + widths.reduce(0, +) + CGFloat(max(indices.count - 1, 0)) * gap
                + (loops.isEmpty ? 1 : FlowLoopGeometry.pad(loops.count - 1) + 1)
        }
        var top: CGFloat {
            loops.indices.map { FlowLoopGeometry.above($0) }.max().map { $0 + 2 } ?? 3
        }
        var bottom: CGFloat {
            loops.indices.map { FlowLoopGeometry.below($0) }.max().map { $0 + 2 } ?? 3
        }
        var height: CGFloat { top + FlowDiagram.nodeHeight + bottom }

        func x(_ position: Int) -> CGFloat {
            start + widths.prefix(position).reduce(0, +) + CGFloat(position) * gap
        }
    }

    private func nodeWidth(_ index: Int) -> CGFloat {
        let digits = CGFloat(String(index + 1).count)
        return (6 + max(12, digits * 6) + 5 + CGFloat(nodeText(graph.steps[index]).count) * Self.charWidth + 8).rounded(.up)
    }

    private func nodeText(_ node: FlowNode) -> String {
        switch node.kind {
        case .xor: "\(node.label) · \(node.paths.count) paths"
        case .op where node.label.hasPrefix("pr ") && delivery != nil: "\(node.label) · \(delivery!)"
        case .op, .skill: node.label
        }
    }

    private func row(_ indices: [Int], label: String?, indent: CGFloat = 0, spans: [LoopSpan],
                     gap: CGFloat = FlowDiagram.gap) -> Row {
        let set = Set(indices)
        return Row(indices: indices, label: label, indent: indent,
                   loops: spans.filter { set.contains($0.from) && set.contains($0.to) },
                   widths: indices.map(nodeWidth), gap: gap)
    }

    /// Tighten the chip gap before an overwide loop row has to scroll.
    func layout(width available: CGFloat) -> [Row] {
        let rows = layout(width: available, gap: Self.gap)
        guard rows.contains(where: { $0.width > available }) else { return rows }
        return layout(width: available, gap: Self.compactGap)
    }

    private func layout(width available: CGFloat, gap: CGFloat) -> [Row] {
        let count = graph.steps.count
        let spans = templateSpans ?? Self.spans(graph, pinned: pinned)
        guard let first = spans.map(\.from).min(), let last = spans.map(\.to).max() else {
            // No loops: wrap the sequence greedily.
            var rows: [Row] = []
            var current: [Int] = []
            for index in 0..<count {
                if !current.isEmpty, row(current + [index], label: nil, spans: [], gap: gap).width > available {
                    rows.append(row(current, label: nil, spans: [], gap: gap))
                    current = []
                }
                current.append(index)
            }
            if !current.isEmpty { rows.append(row(current, label: nil, spans: [], gap: gap)) }
            return rows
        }
        let once = Array(0..<first)
        let loops = Array(first...last)
        let tail = Array((last + 1)..<count)
        // Wide: once and loops together, the tail indented beneath.
        let lead = row(once + loops, label: nil, spans: spans, gap: gap)
        if lead.width <= available {
            guard !tail.isEmpty else { return [lead] }
            let flat = row(tail, label: "↳ delivery", spans: spans, gap: gap)
            if flat.width <= available {
                let indent = min(160, available - flat.width)
                return [lead, row(tail, label: "↳ delivery", indent: indent, spans: spans, gap: gap)]
            }
        }
        // Narrow: each section on its own labelled row.
        let sections = [(once, "once"), (loops, "loops"), (tail, "delivery")].filter { !$0.0.isEmpty }
        let labelled = sections.map { row($0.0, label: $0.1, spans: spans, gap: gap) }
        if labelled.allSatisfy({ $0.width <= available }) { return labelled }
        return sections.map { row($0.0, label: nil, spans: spans, gap: gap) }
    }

    // MARK: Drawing

    private func rowView(_ row: Row, states: [String: FlowNodeState]) -> some View {
        let nodeTop = row.top
        let nodeBottom = nodeTop + Self.nodeHeight
        let frames: [Int: (x: CGFloat, width: CGFloat)] = Dictionary(uniqueKeysWithValues:
            row.indices.enumerated().map { position, index in (index, (row.x(position), row.widths[position])) })
        return ZStack(alignment: .topLeading) {
            if let label = row.label {
                Text(label)
                    .font(Typography.code(10.5))
                    .foregroundStyle(palette.textTertiary)
                    .frame(width: Self.labelSlot - 6, height: Self.nodeHeight, alignment: .leading)
                    .offset(x: 0, y: nodeTop)
            }
            ForEach(Array(row.loops.enumerated().reversed()), id: \.offset) { level, span in
                if let from = frames[span.from], let to = frames[span.to] {
                    loopRegion(span, level: level, left: from.x, right: to.x + to.width,
                               nodeTop: nodeTop, nodeBottom: nodeBottom)
                }
            }
            Canvas { context, _ in
                // Connectors between adjacent occurrences: hairline tone, 1.5 pt.
                for position in row.indices.indices.dropLast() {
                    let x = row.x(position) + row.widths[position]
                    var line = Path()
                    line.move(to: CGPoint(x: x, y: nodeTop + Self.nodeHeight / 2))
                    line.addLine(to: CGPoint(x: x + row.gap, y: nodeTop + Self.nodeHeight / 2))
                    context.stroke(line, with: .color(palette.border), lineWidth: 1.5)
                }
                // Each return owns a lane and its own port on the shared target:
                // Loop 1 arrives at the top-left, Loop 2 at the bottom-left.
                for (level, span) in row.loops.enumerated() {
                    guard let from = frames[span.from], let to = frames[span.to] else { continue }
                    let color = FlowPalette.loops[level % FlowPalette.loops.count]
                    let above = FlowLoopGeometry.returnsAbove(level)
                    let lane = above ? nodeTop - FlowLoopGeometry.laneOffset(level) : nodeBottom + FlowLoopGeometry.laneOffset(level)
                    let edge = above ? nodeTop : nodeBottom
                    let sign: CGFloat = above ? -1 : 1
                    let startX = to.x + to.width / 2
                    let endX = from.x + FlowLoopGeometry.landing(level)
                    var path = Path()
                    path.move(to: CGPoint(x: startX, y: edge))
                    path.addLine(to: CGPoint(x: startX, y: lane))
                    path.addLine(to: CGPoint(x: endX, y: lane))
                    path.addLine(to: CGPoint(x: endX, y: edge + sign * 7))
                    context.stroke(path, with: .color(color), lineWidth: 1.5)
                    var head = Path()
                    head.move(to: CGPoint(x: endX, y: edge + sign * 1.5))
                    head.addLine(to: CGPoint(x: endX - 3.5, y: edge + sign * 8))
                    head.addLine(to: CGPoint(x: endX + 3.5, y: edge + sign * 8))
                    head.closeSubpath()
                    context.fill(head, with: .color(color))
                }
            }
            .frame(width: row.width, height: row.height)
            .allowsHitTesting(false)
            .accessibilityHidden(true)
            ForEach(Array(row.indices.enumerated()), id: \.element) { position, index in
                let node = graph.steps[index]
                nodeButton(node, number: index + 1, width: row.widths[position],
                           state: states[node.key] ?? .pending)
                    .offset(x: row.x(position), y: nodeTop)
            }
        }
        .frame(width: row.width, height: row.height, alignment: .topLeading)
    }

    private func loopRegion(_ span: LoopSpan, level: Int, left: CGFloat, right: CGFloat,
                            nodeTop: CGFloat, nodeBottom: CGFloat) -> some View {
        let pad = FlowLoopGeometry.pad(level)
        let top = nodeTop - FlowLoopGeometry.above(level)
        let bottom = nodeBottom + FlowLoopGeometry.below(level)
        let width = right - left + pad * 2
        let color = FlowPalette.loops[level % FlowPalette.loops.count]
        let title = span.evidence.map { "Loop \(span.number) · \($0.traversals) \($0.traversals == 1 ? "return" : "returns")" }
            ?? "Loop \(span.number)"
        let shape = RoundedRectangle(cornerRadius: 12)
        // Every label sits in its region's top-left corner; nested regions stack.
        return ZStack(alignment: .topLeading) {
            shape.fill(color.opacity(level.isMultiple(of: 2) ? 0.10 : 0.045))
            if level.isMultiple(of: 2) {
                shape.strokeBorder(color.opacity(0.35), lineWidth: 1)
            } else {
                shape.strokeBorder(color.opacity(0.30), style: StrokeStyle(lineWidth: 1, dash: [4, 3]))
            }
            Text(title)
                .font(Typography.caption(11).weight(.bold))
                .foregroundStyle(color)
                .padding(.horizontal, 10)
                .padding(.top, 1)
                .help(span.evidence.map { "\($0.traversals) returns along this arrow" } ?? "Loop")
                .accessibilityIdentifier("task-flow-loop-\(span.decider)")
        }
        .frame(width: width, height: bottom - top)
        .offset(x: left - pad, y: top)
    }

    private func nodeButton(_ node: FlowNode, number: Int, width: CGFloat, state: FlowNodeState) -> some View {
        let tone = FlowPalette.tone(state)
        let shape = RoundedRectangle(cornerRadius: 7)
        let selected = inspected == node.key || inspected?.hasPrefix(node.key + "/") == true
        let emphasized = state == .running || state == .blocked
        return Button {
            inspected = selected ? nil : node.key
        } label: {
            HStack(spacing: 5) {
                Text("\(number)")
                    .font(Typography.caption(9.5))
                    .monospacedDigit()
                    .foregroundStyle(palette.textTertiary)
                    .frame(minWidth: 12, alignment: .trailing)
                Text(nodeText(node))
                    .font(Typography.code(11))
                    .italic(node.kind == .op)
                    .lineLimit(1)
                    .foregroundStyle(tone == .neutral ? palette.text : tone.text)
            }
            .padding(.leading, 6)
            .padding(.trailing, 8)
            .frame(width: width, height: Self.nodeHeight, alignment: .leading)
            // An opaque base keeps state colors legible over the loop tint.
            .background(shape.fill(tone == .neutral ? palette.surface : tone.fill))
            .overlay {
                if state == .stopped || state == .unknown {
                    shape.strokeBorder(tone.stroke, style: StrokeStyle(lineWidth: 1, dash: [3, 2]))
                } else {
                    shape.strokeBorder(tone == .neutral ? palette.borderStrong : tone.stroke,
                                       lineWidth: emphasized ? 2 : 1)
                }
            }
            .overlay {
                // The running occurrence carries the diagram's only motion.
                if state == .running, !reduceMotion {
                    RunningShimmer(width: width).clipShape(shape).allowsHitTesting(false)
                }
            }
            .overlay {
                if selected {
                    RoundedRectangle(cornerRadius: 9).stroke(palette.accentInk, lineWidth: 2).padding(-3)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(node.label)
        .accessibilityLabel("\(node.label), \(FlowPalette.describe(state))")
        .accessibilityAddTraits(selected ? .isSelected : [])
        .accessibilityIdentifier("flow-node-\(node.key)")
        .id(node.key)
    }
}

/// Nested loops step outward. Every region carries its label in its top-left
/// corner, so labels stack down the left edge. Even levels return through a lane
/// above the row into the target's top-left port; odd levels return through a
/// lane below into its bottom-left port, so two arrows never share a side.
@MainActor
enum FlowLoopGeometry {
    static let laneGap: CGFloat = 10
    static func pad(_ level: Int) -> CGFloat { 4 + 4 * CGFloat(level) }
    static func returnsAbove(_ level: Int) -> Bool { level.isMultiple(of: 2) }
    private static func padStep(_ level: Int) -> CGFloat { level == 0 ? pad(0) : pad(level) - pad(level - 1) }
    /// Region top above the node row: the enclosed levels, this level's pad,
    /// its lane when it returns above, and its label row.
    static func above(_ level: Int) -> CGFloat {
        guard level >= 0 else { return 0 }
        return above(level - 1) + padStep(level) + (returnsAbove(level) ? laneGap : 0) + FlowDiagram.loopLabelHeight
    }
    static func below(_ level: Int) -> CGFloat {
        guard level >= 0 else { return 0 }
        return below(level - 1) + padStep(level) + (returnsAbove(level) ? 0 : laneGap)
    }
    /// Lane centre, measured outward from the node row's top (even) or bottom (odd).
    static func laneOffset(_ level: Int) -> CGFloat {
        (returnsAbove(level) ? above(level - 1) : below(level - 1)) + padStep(level) + laneGap / 2
    }
    /// Port x from the target's left edge; each side spreads its own levels.
    static func landing(_ level: Int) -> CGFloat { 10 + 12 * CGFloat(level / 2) }
}

/// A 2 s highlight sweeping across the running chip: the one thing that moves
/// at rest. Callers skip it under reduce-motion, leaving the running tone still.
private struct RunningShimmer: View {
    let width: CGFloat
    @State private var sweeping = false

    var body: some View {
        LinearGradient(
            colors: [.clear, .white.opacity(0.45), .clear],
            startPoint: .leading, endPoint: .trailing
        )
        .frame(width: width * 0.7)
        .offset(x: sweeping ? width : -width)
        .onAppear {
            withAnimation(.linear(duration: 2).repeatForever(autoreverses: false)) { sweeping = true }
        }
    }
}

struct LoopSpan {
    let decider: String
    /// Authored order among the Flow's loops, from 1.
    let number: Int
    let from: Int
    let to: Int
    /// Saved counts; `nil` for a preview that has not run.
    let evidence: FlowReturn?
}

private struct FlowNodeDetail: View {
    let node: FlowNode
    let state: FlowNodeState
    let pinned: PinnedTaskFlow?
    let graph: FlowGraph

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: Spacing.sm) {
                Text(node.label).font(Typography.mono)
                Text(FlowPalette.describe(state)).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
            }
            .accessibilityIdentifier("flow-node-detail-\(node.key)")
            if !facts.isEmpty {
                Text(facts.joined(separator: " · "))
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
            ForEach(node.paths, id: \.name) { path in
                let current = pinned?.current.map { $0.hasPrefix("\(node.key)/\(path.name)/") } ?? false
                Text("\(path.name)\(current ? " (selected)" : "") — \(path.steps.map(\.label).joined(separator: " → ").ifEmpty("no steps")) · \(path.description)")
                    .font(Typography.caption(11))
                    .foregroundStyle(current ? palette.text : palette.textSecondary)
            }
        }
        .padding(Spacing.sm)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: CornerRadius.md))
        .accessibilityIdentifier("task-flow-detail")
    }

    private var facts: [String] {
        var facts: [String] = []
        switch node.kind {
        case .skill where node.human: facts.append("Human review")
        case .op: facts.append("Operation")
        case .xor: facts.append("Chooses one path")
        case .skill: break
        }
        if let id = node.id { facts.append("id \(id)") }
        if let target = node.returnsTo {
            let label = graph.steps.first { $0.key == target }?.label ?? target
            let taken = pinned?.returns.first { $0.decider == node.key }?.traversals
            facts.append("Iterate returns to \(label)" + (taken.map { " · taken \($0)×" } ?? ""))
        }
        if node.parents.count > 1 { facts.append("from \(node.parents.joined(separator: " › "))") }
        return facts
    }
}

private extension String {
    func ifEmpty(_ fallback: String) -> String { isEmpty ? fallback : self }
}

enum FlowPalette {
    /// Loop tints in authored order; running shares the first.
    static let loops = [Color.adaptive(light: 0x3A74C4, dark: 0x86B0EA), Color.adaptive(light: 0x24508F, dark: 0xB4CDEF)]

    static func tone(_ state: FlowNodeState) -> WorkspaceTone {
        switch state {
        case .completed: .done
        case .running: .running
        case .waitingForHuman, .pendingHuman: .human
        case .blocked: .blocked
        case .stopped, .unknown: .stopped
        case .pending: .neutral
        }
    }

    static func describe(_ state: FlowNodeState) -> String {
        switch state {
        case .completed: "completed this pass"
        case .running: "running"
        case .waitingForHuman: "waiting for your review"
        case .blocked: "blocked"
        case .stopped: "stopped here"
        case .unknown: "current, worker state unknown"
        case .pendingHuman: "human review, pending"
        case .pending: "pending"
        }
    }
}
#endif
