// Flow diagrams: a definition's composition and one Flow process's launched graph
// with where it stands. Topology, occurrence keys and return counts come
// from Rust; these views draw them.

#if os(macOS)
import Loopflow
import SwiftUI

/// How one occurrence reads in the diagram. Completion is scoped to the current
/// pass; a repeated occurrence is pending again after its loop returns.
enum FlowNodeState: Equatable {
    case completed
    case running
    case blocked
    case stalled
    case stopped
    case unknown
    case pendingHuman
    case pending
}

/// Classify every drawn occurrence from the shared projection.
func flowNodeStates(_ graph: FlowGraph, latest: FlowProcessDetail?) -> [UInt32: FlowNodeState] {
    var states: [UInt32: FlowNodeState] = [:]
    func visit(_ nodes: [FlowNode]) {
        for node in nodes {
            states[node.key] = state(of: node, latest: latest)
            for path in node.paths { visit(path.steps) }
        }
    }
    visit(graph.steps)
    return states
}

private func state(of node: FlowNode, latest: FlowProcessDetail?) -> FlowNodeState {
    guard let latest else { return node.human ? .pendingHuman : .pending }
    let isCurrent = latest.current.map { node.contains($0) } ?? false
    if isCurrent {
        switch latest.presentation.execution {
        case .running, .starting: return .running
        case .blocked: return .blocked
        case .stalled: return .stalled
        case .idle: return .stopped
        case .unknown: return .unknown
        }
    }
    if latest.completed.contains(node.key) { return .completed }
    return node.human ? .pendingHuman : .pending
}

// MARK: - Diagram

struct FlowCompositionView: View {
    let graph: FlowGraph
    let composition: FlowComposition
    @Bindable var navigation: WorkNavigation

    private var expanded: Binding<Set<String>> {
        Binding(get: { navigation.expandedCompositionGroups[composition.revision] ?? [] },
                set: { navigation.expandedCompositionGroups[composition.revision] = $0 })
    }

    static func spans(_ original: FlowGraph, projection: FlowCompositionProjection) -> [LoopSpan] {
        FlowDiagram.spans(original, latest: nil).compactMap { edge in
            guard let fromKey = projection.visibleKeys[original.steps[edge.from].key],
                  let toKey = projection.visibleKeys[original.steps[edge.to].key],
                  let from = projection.graph.steps.firstIndex(where: { $0.key == fromKey }),
                  let to = projection.graph.steps.firstIndex(where: { $0.key == toKey }) else { return nil }
            return LoopSpan(decider: edge.decider, number: edge.number, from: from, to: to, evidence: nil)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            CompositionDisclosures(items: composition.items, graph: graph, expanded: expanded)
            CompositionDiagram(graph: graph, items: composition.items, expanded: expanded)
        }
        .id(composition.revision)
        .accessibilityIdentifier("flow-composition-\(graph.name)")
    }
}

/// Disclosure controls keep composition identity even for an empty body.
private struct CompositionDisclosures: View {
    let items: [FlowCompositionItem]
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
                    AnyView(CompositionDisclosures(items: children, graph: graph, expanded: $expanded))
                } label: {
                    Text("\(name) · \(item.nodeKeys.count) steps").font(Typography.mono)
                }
                .disclosureGroupStyle(TemplateDisclosureStyle())
                .accessibilityIdentifier("composition-group-\(id)")
            case .node(let key, let paths):
                if !paths.isEmpty {
                    ForEach(paths.keys.sorted(), id: \.self) { name in
                        DisclosureGroup("\(graph.node(key)?.label ?? String(key)) · \(name)") {
                            AnyView(CompositionDisclosures(items: paths[name]!, graph: graph, expanded: $expanded))
                            if let node = graph.node(key), let path = node.paths.first(where: { $0.name == name }) {
                                let branch = FlowGraph(name: name, steps: path.steps, interactions: graph.interactions)
                                CompositionDiagram(graph: branch, items: paths[name]!, expanded: $expanded)
                            }
                        }
                        .disclosureGroupStyle(TemplateDisclosureStyle())
                        .font(Typography.mono)
                    }
                }
            }
        }
    }
}

private struct TemplateDisclosureStyle: DisclosureGroupStyle {
    func makeBody(configuration: Configuration) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Button { configuration.isExpanded.toggle() } label: {
                HStack(spacing: 4) {
                    Image(systemName: configuration.isExpanded ? "chevron.down" : "chevron.right")
                        .font(.system(size: 10, weight: .semibold))
                        .frame(width: 12)
                        .accessibilityHidden(true)
                    configuration.label
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .focusable(interactions: .edit)
            .onKeyPress(keys: [.space, .return, .rightArrow, .leftArrow]) { press in
                switch press.key {
                case .rightArrow: configuration.isExpanded = true
                case .leftArrow: configuration.isExpanded = false
                default: configuration.isExpanded.toggle()
                }
                return .handled
            }
            .accessibilityValue(configuration.isExpanded ? "Expanded" : "Collapsed")
            if configuration.isExpanded {
                configuration.content.padding(.leading, 16)
            }
        }
    }
}

private struct CompositionDiagram: View {
    let graph: FlowGraph
    let items: [FlowCompositionItem]
    @Binding var expanded: Set<String>
    @State private var inspected: UInt32?
    var body: some View {
        let projection = FlowCompositionProjection(graph: graph, items: items, expanded: expanded)
        FlowDiagram(graph: projection.graph, latest: nil, inspected: Binding(
            get: { inspected },
            set: { key in
                if let key, let group = projection.groups[key] {
                    expanded.insert(group)
                    inspected = nil
                } else { inspected = key }
            }
        ), compositionSpans: FlowCompositionView.spans(graph, projection: projection), detailGraph: graph)
    }
}

/// A Flow's top-level occurrences in rows: what runs once, the loops, then the
/// tail. Loop endpoints always share a row, so each authored backward edge draws
/// its tinted span and a return arrow on its own lane beneath. When the window
/// is wide enough the tail follows on one indented row; otherwise each section
/// gets its own labelled row. Only an overwide loop row scrolls.
struct FlowDiagram: View {
    let graph: FlowGraph
    let latest: FlowProcessDetail?
    @Binding var inspected: UInt32?
    var compositionSpans: [LoopSpan]? = nil
    /// Layout may fold nodes; detail reads the complete definition.
    var detailGraph: FlowGraph? = nil

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

    /// Top-level authored returns in authored order, numbered from 1. A launched
    /// Flow adds each edge's saved counts. Nested XOR returns appear in node detail.
    static func spans(_ graph: FlowGraph, latest: FlowProcessDetail?) -> [LoopSpan] {
        let steps = graph.steps
        return steps.enumerated().compactMap { to, node -> (Int, Int, FlowNode)? in
            guard let target = node.returnsTo,
                  let from = steps.firstIndex(where: { $0.key == target }), from <= to else { return nil }
            return (from, to, node)
        }
        .enumerated()
        .map { number, edge in
            LoopSpan(decider: edge.2.key, number: number + 1, from: edge.0, to: edge.1,
                     evidence: latest?.returns.first { $0.decider == edge.2.key })
        }
    }

    var body: some View {
        let states = flowNodeStates(graph, latest: latest)
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
                        $0.contains(key)
                    }) else { return }
                    reader.scrollTo(root.key, anchor: .center)
                }
            }
            if let key = inspected, let node = (detailGraph ?? graph).node(key) {
                FlowNodeDetail(node: node, state: states[key] ?? .pending, latest: latest, graph: detailGraph ?? graph)
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
        let spans = compositionSpans ?? Self.spans(graph, latest: latest)
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

    private func rowView(_ row: Row, states: [UInt32: FlowNodeState]) -> some View {
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
                .font(Typography.strong(11))
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
        let selected = inspected.map { node.contains($0) } ?? false
        let emphasized = state == .running || state == .blocked || state == .stalled
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
    let decider: UInt32
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
    let latest: FlowProcessDetail?
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
                let current = latest?.current.map { key in path.steps.contains { $0.contains(key) } } ?? false
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
            let label = graph.node(target)?.label ?? String(target)
            let taken = latest?.returns.first { $0.decider == node.key }?.traversals
            facts.append("Iterate returns to \(label)" + (taken.map { " · taken \($0)×" } ?? ""))
        }
        if node.sources.count > 1 { facts.append("from \(node.sources.joined(separator: " › "))") }
        return facts
    }
}

private extension String {
    func ifEmpty(_ fallback: String) -> String { isEmpty ? fallback : self }
}

enum FlowPalette {
    /// Loop tints in authored order; running shares the first.
    static let loops = [Color.adaptive(light: 0x3A74C4, dark: 0x86B0EA), Color.adaptive(light: 0x24508F, dark: 0xB4CDEF)]

    static func tone(_ state: FlowNodeState) -> WorkTone {
        switch state {
        case .completed: .done
        case .running: .running
        case .pendingHuman: .human
        case .blocked, .stalled: .blocked
        case .stopped, .unknown: .stopped
        case .pending: .neutral
        }
    }

    static func describe(_ state: FlowNodeState) -> String {
        switch state {
        case .completed: "completed this pass"
        case .running: "running"
        case .blocked: "blocked"
        case .stalled: "stalled"
        case .stopped: "stopped here"
        case .unknown: "current, worker state unknown"
        case .pendingHuman: "human review, pending"
        case .pending: "pending"
        }
    }
}
#endif
