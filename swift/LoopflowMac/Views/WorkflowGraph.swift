#if os(macOS)
import Loopflow
import SwiftUI

/// Where a workflow's nodes and edges are drawn. `start` and `end` are
/// circles and each node a box, in authored order on one row. An edge into
/// the next column runs straight; one that returns to its own node arcs over
/// it; any other takes its own lane below the row.
struct WorkflowGraphLayout {
    enum Route: Equatable {
        case straight
        case loop(level: Int)
        case lane(level: Int)
    }

    struct Box {
        let name: String
        /// What the person does here; `nil` on `start` and `end`.
        let description: String?
        let terminal: Bool
        let frame: CGRect
    }

    struct Arrow {
        let index: Int
        let edge: Workflow.Edge
        let route: Route
        let line: Path
        let head: Path
        /// The label's centre and width; it sits just above its line.
        let label: CGPoint
        let labelWidth: CGFloat

        /// An edge is named by what it runs.
        var text: String { Self.text(edge) }

        static func text(_ edge: Workflow.Edge) -> String { edge.flow ?? "finish" }
    }

    static let nodeHeight: CGFloat = 44
    static let labelHeight: CGFloat = 24
    static let laneStep: CGFloat = 30
    static let loopStep: CGFloat = 28
    private static let loopRise: CGFloat = 20

    let boxes: [Box]
    let arrows: [Arrow]
    let size: CGSize

    init(nodes: [Workflow.Node], edges: [Workflow.Edge]) {
        let columns = ["start"] + nodes.map(\.name) + ["end"]
        let column = Dictionary(columns.enumerated().map { ($1, $0) }, uniquingKeysWith: { first, _ in first })
        var routes: [Int: Route] = [:]
        var straight: [Int: Int] = [:]
        var loops: [Int: Int] = [:]
        var lanes = 0
        for (index, edge) in edges.enumerated() {
            guard let from = column[edge.from], let to = column[edge.to] else { continue }
            if from == to {
                routes[index] = .loop(level: loops[from, default: 0])
                loops[from, default: 0] += 1
            } else if to == from + 1, straight[from] == nil {
                routes[index] = .straight
                straight[from] = index
            } else {
                routes[index] = .lane(level: lanes)
                lanes += 1
            }
        }
        func labelWidth(_ index: Int) -> CGFloat { CGFloat(Arrow.text(edges[index]).count) * 7 + 26 }

        let top = (loops.values.max()).map { Self.loopRise + Self.loopStep * CGFloat($0 - 1) + Self.labelHeight + 4 } ?? 4
        var boxes: [Box] = []
        var x: CGFloat = 0
        for (position, name) in columns.enumerated() {
            let node = position == 0 || position == columns.count - 1 ? nil : nodes[position - 1]
            let description = node.map { $0.description ?? $0.skill }
            let looped = edges.indices.filter { edges[$0].from == name && edges[$0].to == name }
                .map(labelWidth).max() ?? 0
            let width = node == nil ? Self.nodeHeight : min(260, max(
                112, looped, CGFloat(name.count) * 7.4 + 24, CGFloat(description?.count ?? 0) * 5.8 + 24))
            boxes.append(Box(name: name, description: description, terminal: node == nil,
                             frame: CGRect(x: x, y: top, width: width, height: Self.nodeHeight)))
            x += width + max(56, straight[position].map { labelWidth($0) + 28 } ?? 0)
        }
        self.boxes = boxes

        let middle = top + Self.nodeHeight / 2
        let bottom = top + Self.nodeHeight
        arrows = edges.indices.compactMap { index in
            guard let route = routes[index], let from = column[edges[index].from],
                  let to = column[edges[index].to] else { return nil }
            let source = boxes[from].frame, target = boxes[to].frame
            let width = labelWidth(index)
            var line = Path()
            let tip: CGPoint, toward: CGVector, label: CGPoint
            switch route {
            case .straight:
                tip = CGPoint(x: target.minX, y: middle)
                toward = CGVector(dx: 1, dy: 0)
                line.move(to: CGPoint(x: source.maxX, y: middle))
                line.addLine(to: CGPoint(x: tip.x - 6, y: middle))
                label = CGPoint(x: (source.maxX + target.minX) / 2, y: middle - 3 - Self.labelHeight / 2)
            case .loop(let level):
                let rise = Self.loopRise + Self.loopStep * CGFloat(level)
                let reach = 18 + 8 * CGFloat(level)
                tip = CGPoint(x: source.midX + reach, y: top)
                toward = CGVector(dx: -0.35, dy: 1)
                line.move(to: CGPoint(x: source.midX - reach, y: top))
                // A cubic peaks at three quarters of its control height.
                line.addCurve(to: CGPoint(x: tip.x + 2, y: top - 6),
                              control1: CGPoint(x: source.midX - reach - 16, y: top - rise * 4 / 3),
                              control2: CGPoint(x: source.midX + reach + 16, y: top - rise * 4 / 3))
                label = CGPoint(x: source.midX, y: top - rise - 1 - Self.labelHeight / 2)
            case .lane(let level):
                let y = bottom + Self.labelHeight + 4 + Self.laneStep * CGFloat(level)
                let lean: CGFloat = to > from ? 12 : -12
                tip = CGPoint(x: target.midX - lean, y: bottom)
                toward = CGVector(dx: 0, dy: -1)
                line.move(to: CGPoint(x: source.midX + lean, y: bottom))
                line.addLine(to: CGPoint(x: source.midX + lean, y: y))
                line.addLine(to: CGPoint(x: tip.x, y: y))
                line.addLine(to: CGPoint(x: tip.x, y: bottom + 6))
                label = CGPoint(x: (source.midX + lean + tip.x) / 2, y: y - 1 - Self.labelHeight / 2)
            }
            return Arrow(index: index, edge: edges[index], route: route, line: line,
                         head: Self.head(tip: tip, toward: toward), label: label, labelWidth: width)
        }
        size = CGSize(width: boxes.last?.frame.maxX ?? 0,
                      height: bottom + (lanes == 0 ? 4 : Self.labelHeight + 8 + Self.laneStep * CGFloat(lanes - 1)))
    }

    private static func head(tip: CGPoint, toward: CGVector) -> Path {
        let length = (toward.dx * toward.dx + toward.dy * toward.dy).squareRoot()
        let (dx, dy) = (toward.dx / length, toward.dy / length)
        let base = CGPoint(x: tip.x - dx * 7, y: tip.y - dy * 7)
        var path = Path()
        path.move(to: tip)
        path.addLine(to: CGPoint(x: base.x - dy * 3.5, y: base.y + dx * 3.5))
        path.addLine(to: CGPoint(x: base.x + dy * 3.5, y: base.y - dx * 3.5))
        path.closeSubpath()
        return path
    }
}

/// A workflow as a graph. Given a Task's position it marks the node the Task
/// waits at or the edge it is on, running or stopped, and the edges that can
/// be chosen now are its buttons.
struct WorkflowGraph: View {
    let nodes: [Workflow.Node]
    let edges: [Workflow.Edge]
    var position: Workflow.Position?
    /// Edges that can be chosen now, by their place among `edges`.
    var choices: [Int] = []
    /// Why no edge can be chosen now.
    var unavailable: String?
    /// Shrink to the width offered instead of scrolling: the workspace header.
    var fits = false
    var hint: (Workflow.Edge) -> String = { _ in "" }
    var choose: (Workflow.Edge) -> Void = { _ in }
    @Environment(\.palette) private var palette
    @State private var offered: CGFloat?
    /// Below this the labels stop being readable; the header scrolls instead.
    private static let smallest: CGFloat = 0.6

    var body: some View {
        let layout = WorkflowGraphLayout(nodes: nodes, edges: edges)
        let scale = fits ? min(1, max(Self.smallest, (offered ?? layout.size.width) / max(layout.size.width, 1))) : 1
        ScrollView(.horizontal, showsIndicators: false) {
            drawing(layout)
                .scaleEffect(scale, anchor: .topLeading)
                .frame(width: layout.size.width * scale, height: layout.size.height * scale, alignment: .topLeading)
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { offered = $0 }
        .accessibilityIdentifier("task-workflow-graph")
    }

    private func drawing(_ layout: WorkflowGraphLayout) -> some View {
            ZStack(alignment: .topLeading) {
                Canvas { context, _ in
                    for arrow in layout.arrows {
                        let color = tone(arrow.index)?.ink ?? palette.borderStrong
                        context.stroke(arrow.line, with: .color(color), lineWidth: 1.5)
                        context.fill(arrow.head, with: .color(color))
                    }
                }
                .frame(width: layout.size.width, height: layout.size.height)
                .allowsHitTesting(false)
                .accessibilityHidden(true)
                ForEach(layout.boxes, id: \.name) { box in node(box) }
                ForEach(layout.arrows, id: \.index) { arrow in label(arrow) }
            }
            .frame(width: layout.size.width, height: layout.size.height, alignment: .topLeading)
    }

    /// Whether the Task is on `edge` with its Flow still running; `nil` when it is not on it.
    private func running(_ edge: Int) -> Bool? {
        if case .edge(edge, _, let running) = position { running } else { nil }
    }

    private func tone(_ edge: Int) -> WorkspaceTone? {
        running(edge).map { $0 ? .running : .blocked }
    }

    private func node(_ box: WorkflowGraphLayout.Box) -> some View {
        let marked = position == .node(box.name)
        let shape = box.terminal ? AnyShape(Circle()) : AnyShape(RoundedRectangle(cornerRadius: 7))
        return VStack(spacing: 1) {
            Text(box.name)
                .font(Typography.code(box.terminal ? 11 : 12))
                .foregroundStyle(marked ? WorkspaceTone.human.text : palette.text)
            if let description = box.description {
                Text(description)
                    .font(Typography.caption(10.5))
                    .foregroundStyle(marked ? WorkspaceTone.human.text : palette.textSecondary)
            }
        }
        .lineLimit(1)
        .padding(.horizontal, box.terminal ? 0 : 8)
        .frame(width: box.frame.width, height: box.frame.height)
        .background(shape.fill(marked ? WorkspaceTone.human.fill : palette.surface))
        .overlay(shape.stroke(marked ? WorkspaceTone.human.stroke : palette.borderStrong, lineWidth: marked ? 2 : 1))
        .offset(x: box.frame.minX, y: box.frame.minY)
        .accessibilityElement(children: .combine)
        .accessibilityValue(marked ? "Current" : "")
        .accessibilityIdentifier("task-workflow-node-\(box.name)")
    }

    @ViewBuilder
    private func label(_ arrow: WorkflowGraphLayout.Arrow) -> some View {
        let state = running(arrow.index).map { $0 ? "Running" : "Stopped" } ?? ""
        Group {
            if choices.contains(arrow.index) {
                Button(arrow.text) { choose(arrow.edge) }
                    .buttonStyle(WorkspaceOutlineButtonStyle())
                    .disabled(unavailable != nil)
                    .help(unavailable ?? hint(arrow.edge))
                    .accessibilityValue(state)
                    .accessibilityIdentifier("task-workflow-run-\(arrow.index)")
            } else {
                Text(arrow.text)
                    .font(Typography.code(11))
                    .foregroundStyle(tone(arrow.index)?.ink ?? palette.textTertiary)
                    .accessibilityValue(state)
                    .accessibilityIdentifier("task-workflow-edge-\(arrow.index)")
            }
        }
        .lineLimit(1)
        .frame(width: arrow.labelWidth, height: WorkflowGraphLayout.labelHeight, alignment: .bottom)
        .offset(x: arrow.label.x - arrow.labelWidth / 2, y: arrow.label.y - WorkflowGraphLayout.labelHeight / 2)
    }
}
#endif
