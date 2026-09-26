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
    @State private var inspected: String?
    @FocusState private var focus: HeaderFocus?

    private enum HeaderFocus: Hashable { case name, restart, search }

    private var flow: TaskFlowSnapshot { task.flow }
    private var catalog: PodiumReading<[FlowCatalogEntry]> { model.flowCatalog }

    private var draft: TaskFlowDraft {
        get { model.navigation.flowDrafts[task.id] ?? TaskFlowDraft() }
        nonmutating set { model.navigation.flowDrafts[task.id] = newValue }
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
                Text(statusLine)
                    .font(Typography.body(13))
                    .foregroundStyle(pinned?.execution == .blocked ? WorkspaceTone.blocked.ink : palette.textSecondary)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-flow-status")
            }
            .padding(.leading, 2)
        }
        .task { await model.loadFlowCatalog() }
        .onExitCommand { dismissTransient() }
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
                    .foregroundStyle(palette.accent)
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
                .disabled(start?.unavailable != nil || draft.acting || previewGraph == nil)
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
                .font(Typography.body(13).weight(.semibold))
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

    private var previewGraph: FlowGraph? {
        catalog.value?.first { $0.name == previewName }?.graph
    }

    @ViewBuilder
    private var diagram: some View {
        if let pinned {
            FlowDiagram(graph: pinned.graph, pinned: pinned, inspected: $inspected)
        } else if let graph = previewGraph {
            FlowDiagram(graph: graph, pinned: nil, inspected: $inspected)
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
                return "No Flow recorded · earlier Runs exist outside a managed Flow"
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
        case .preview: next.preview = name
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
        inspected = nil
    }

    private func run(_ request: TaskFlowControlRequest) {
        Task { await model.performFlowControl(request, task: task, wave: wave) }
    }
}

// MARK: - Diagram

/// A Flow's top-level occurrences in rows: what runs once, the loops, then the
/// tail. Loop endpoints always share a row, so each authored backward edge draws
/// its tinted span and a return arrow on its own lane beneath. When the window
/// is wide enough the tail follows on one indented row; otherwise each section
/// gets its own labelled row. Only an overwide loop row scrolls.
struct FlowDiagram: View {
    let graph: FlowGraph
    let pinned: PinnedTaskFlow?
    @Binding var inspected: String?

    @Environment(\.palette) private var palette
    @State private var available: CGFloat = 0

    static let nodeHeight: CGFloat = 24
    static let gap: CGFloat = 9
    /// JetBrains Mono advances 0.6 em; node labels are 11 pt.
    static let charWidth: CGFloat = 6.6
    static let labelSlot: CGFloat = 46
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
            if let key = inspected, let node = find(key, in: graph.steps) {
                FlowNodeDetail(node: node, state: states[key] ?? .pending, pinned: pinned, graph: graph)
            }
        }
    }

    // MARK: Layout

    struct Row {
        var indices: [Int]
        var label: String?
        var indent: CGFloat
        var loops: [LoopSpan]
        var widths: [CGFloat]

        var start: CGFloat {
            (label == nil ? 0 : FlowDiagram.labelSlot) + indent + (loops.isEmpty ? 1 : FlowLoopGeometry.pad(loops.count - 1) + 1)
        }
        var width: CGFloat {
            start + widths.reduce(0, +) + CGFloat(max(indices.count - 1, 0)) * FlowDiagram.gap
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
            start + widths.prefix(position).reduce(0, +) + CGFloat(position) * FlowDiagram.gap
        }
    }

    private func nodeWidth(_ index: Int) -> CGFloat {
        let digits = CGFloat(String(index + 1).count)
        return (6 + max(12, digits * 6) + 5 + CGFloat(nodeText(graph.steps[index]).count) * Self.charWidth + 8).rounded(.up)
    }

    private func nodeText(_ node: FlowNode) -> String {
        node.kind == .xor ? "\(node.label) · \(node.paths.count) paths" : node.label
    }

    private func row(_ indices: [Int], label: String?, indent: CGFloat = 0, spans: [LoopSpan]) -> Row {
        let set = Set(indices)
        return Row(indices: indices, label: label, indent: indent,
                   loops: spans.filter { set.contains($0.from) && set.contains($0.to) },
                   widths: indices.map(nodeWidth))
    }

    func layout(width available: CGFloat) -> [Row] {
        let count = graph.steps.count
        let spans = Self.spans(graph, pinned: pinned)
        guard let first = spans.map(\.from).min(), let last = spans.map(\.to).max() else {
            // No loops: wrap the sequence greedily.
            var rows: [Row] = []
            var current: [Int] = []
            for index in 0..<count {
                if !current.isEmpty, row(current + [index], label: nil, spans: []).width > available {
                    rows.append(row(current, label: nil, spans: []))
                    current = []
                }
                current.append(index)
            }
            if !current.isEmpty { rows.append(row(current, label: nil, spans: [])) }
            return rows
        }
        let once = Array(0..<first)
        let loops = Array(first...last)
        let tail = Array((last + 1)..<count)
        // Wide: once and loops together, the tail indented beneath.
        let lead = row(once + loops, label: nil, spans: spans)
        if lead.width <= available {
            guard !tail.isEmpty else { return [lead] }
            let flat = row(tail, label: "↳ then", spans: spans)
            if flat.width <= available {
                let indent = min(160, available - flat.width)
                return [lead, row(tail, label: "↳ then", indent: indent, spans: spans)]
            }
        }
        // Narrow: each section on its own labelled row.
        let sections = [(once, "once"), (loops, "loops"), (tail, "then")].filter { !$0.0.isEmpty }
        let labelled = sections.map { row($0.0, label: $0.1, spans: spans) }
        if labelled.allSatisfy({ $0.width <= available }) { return labelled }
        return sections.map { row($0.0, label: nil, spans: spans) }
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
                // Connectors between adjacent occurrences.
                for position in row.indices.indices.dropLast() {
                    let x = row.x(position) + row.widths[position]
                    var line = Path()
                    line.move(to: CGPoint(x: x, y: nodeTop + Self.nodeHeight / 2))
                    line.addLine(to: CGPoint(x: x + Self.gap, y: nodeTop + Self.nodeHeight / 2))
                    context.stroke(line, with: .color(palette.borderStrong), lineWidth: 1)
                }
                // Each return owns a lane and lands on its own side of the target.
                for (level, span) in row.loops.enumerated() {
                    guard let from = frames[span.from], let to = frames[span.to] else { continue }
                    let color = FlowPalette.loops[level % FlowPalette.loops.count]
                    let lane = nodeBottom + FlowLoopGeometry.lane(level)
                    let startX = to.x + to.width / 2
                    let endX = from.x + from.width / 2 + FlowLoopGeometry.landing(level)
                    var path = Path()
                    path.move(to: CGPoint(x: startX, y: nodeBottom))
                    path.addLine(to: CGPoint(x: startX, y: lane))
                    path.addLine(to: CGPoint(x: endX, y: lane))
                    path.addLine(to: CGPoint(x: endX, y: nodeBottom + 7))
                    context.stroke(path, with: .color(color), lineWidth: 1.4)
                    var head = Path()
                    head.move(to: CGPoint(x: endX, y: nodeBottom + 2))
                    head.addLine(to: CGPoint(x: endX - 3.5, y: nodeBottom + 8))
                    head.addLine(to: CGPoint(x: endX + 3.5, y: nodeBottom + 8))
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
        return ZStack(alignment: level.isMultiple(of: 2) ? .topLeading : .topTrailing) {
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
        let selected = inspected == node.key
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
                if selected {
                    RoundedRectangle(cornerRadius: 9).stroke(palette.accent, lineWidth: 2).padding(-3)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(node.label)
        .accessibilityLabel("\(node.label), \(FlowPalette.describe(state))")
        .accessibilityIdentifier("flow-node-\(node.key)")
    }

    private func find(_ key: String, in nodes: [FlowNode]) -> FlowNode? {
        for node in nodes {
            if node.key == key { return node }
            for path in node.paths {
                if let found = find(key, in: path.steps) { return found }
            }
        }
        return nil
    }
}

/// Nested loops step outward: each gets more padding, a lower return lane, a
/// higher label row, and a landing on the other side of the shared target.
enum FlowLoopGeometry {
    static func pad(_ level: Int) -> CGFloat { 4 + 4 * CGFloat(level) }
    static func lane(_ level: Int) -> CGFloat { 11 + 11 * CGFloat(level) }
    static func above(_ level: Int) -> CGFloat {
        pad(level) + FlowDiagram.loopLabelHeight + CGFloat(level) * (FlowDiagram.loopLabelHeight - 2)
    }
    static func below(_ level: Int) -> CGFloat { lane(level) + pad(level) + 4 }
    static func landing(_ level: Int) -> CGFloat {
        level.isMultiple(of: 2) ? 9 * CGFloat(level / 2 + 1) : -9 * CGFloat((level + 1) / 2)
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
                Text(node.label).font(Typography.code(12).weight(.semibold))
                Text(FlowPalette.describe(state)).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
            }
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
    static let loops = [Color(hex: 0x3A74C4), Color(hex: 0x24508F)]

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
