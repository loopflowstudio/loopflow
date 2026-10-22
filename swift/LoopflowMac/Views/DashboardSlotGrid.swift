import Loopflow
import SwiftUI

/// Focus grows a pane in place in any split tree. At each split on the path to
/// the focused pane, its branch takes the golden share and the other branches
/// divide the rest by their base weights. Without focus the base weights apply.
/// Focus never rewrites the base weights.
enum FocusSizing {
    static let golden = 0.618
    static let animation = Animation.spring(duration: 0.25)

    static func shares(_ weights: [Double], focused: Int?) -> [Double] {
        let total = weights.reduce(0, +)
        guard total > 0 else { return weights.map { _ in 1 / Double(max(1, weights.count)) } }
        guard let focused, weights.indices.contains(focused), weights.count > 1 else {
            return weights.map { $0 / total }
        }
        let rest = total - weights[focused]
        return weights.indices.map { index in
            if index == focused { return golden }
            return rest > 0 ? (1 - golden) * weights[index] / rest : (1 - golden) / Double(weights.count - 1)
        }
    }
}

/// Page geometry and slot edits. Each edit keeps the slot count fixed until the
/// person adds or removes a slot; Tasks never change it.
extension DashboardSlotLayout {
    var count: Int { Self.slots(root).count }

    /// Members in order, one page per `count` slots; an empty list is one empty page.
    func pages(_ members: [WorkMemberRef]) -> [[WorkMemberRef]] {
        guard !members.isEmpty else { return [[]] }
        return stride(from: 0, to: members.count, by: count).map { Array(members[$0..<min($0 + count, members.count)]) }
    }

    /// Each slot's frame in `bounds`, grown around `focus` when set.
    func frames(in bounds: CGRect, focus: Int?) -> [Int: CGRect] {
        var frames: [Int: CGRect] = [:]
        func lay(_ node: Node, _ rect: CGRect) {
            switch node {
            case .slot(let index): frames[index] = rect
            case .split(let axis, let weights, let children):
                let hit = focus.flatMap { slot in children.firstIndex { Self.slots($0).contains(slot) } }
                var offset: CGFloat = 0
                for (child, share) in zip(children, FocusSizing.shares(weights, focused: hit)) {
                    let part = axis == .vertical
                        ? CGRect(x: rect.minX + offset, y: rect.minY, width: rect.width * share, height: rect.height)
                        : CGRect(x: rect.minX, y: rect.minY + offset, width: rect.width, height: rect.height * share)
                    lay(child, part)
                    offset += axis == .vertical ? part.width : part.height
                }
            }
        }
        lay(root, bounds)
        return frames
    }

    /// A new last slot beside `slot`. Along the parent's own axis it takes half of
    /// `slot`'s share; across it, the two divide `slot`'s place.
    func adding(beside slot: Int, axis: SplitAxis) -> Self {
        let new = count
        func add(_ node: Node) -> Node {
            switch node {
            case .slot(let index): return index == slot ? .split(axis, [1, 1], [node, .slot(new)]) : node
            case .split(let along, var weights, var children):
                if along == axis, let at = children.firstIndex(of: .slot(slot)) {
                    weights[at] /= 2
                    weights.insert(weights[at], at: at + 1)
                    children.insert(.slot(new), at: at + 1)
                    return .split(along, weights, children)
                }
                return .split(along, weights, children.map(add))
            }
        }
        return Self(root: add(root))
    }

    /// Nil for the last slot: a page always has one. Later slots move up a place.
    func removing(_ slot: Int) -> Self? {
        guard count > 1 else { return nil }
        func remove(_ node: Node) -> Node? {
            switch node {
            case .slot(let index): return index == slot ? nil : .slot(index > slot ? index - 1 : index)
            case .split(let axis, let weights, let children):
                let kept = zip(weights, children).compactMap { weight, child in remove(child).map { (weight, $0) } }
                return kept.count == 1 ? kept[0].1 : .split(axis, kept.map(\.0), kept.map(\.1))
            }
        }
        return remove(root).map(Self.init(root:))
    }

    /// Exchange two slots' places in fill order; geometry stays.
    func swapping(_ a: Int, _ b: Int) -> Self {
        func swap(_ node: Node) -> Node {
            switch node {
            case .slot(let index): return .slot(index == a ? b : index == b ? a : index)
            case .split(let axis, let weights, let children): return .split(axis, weights, children.map(swap))
            }
        }
        return Self(root: swap(root))
    }

    private static func slots(_ node: Node) -> [Int] {
        switch node {
        case .slot(let index): [index]
        case .split(_, _, let children): children.flatMap(slots)
        }
    }
}

/// Slot edits a saved Dashboard writes through its revisioned owner.
struct DashboardSlotEditing {
    let write: (DashboardSlotLayout) -> Void
}

/// Members fill the layout's slots in order; beyond one page they page side to side.
struct DashboardSlotGrid<Tile: View>: View {
    let members: [WorkMemberRef]
    let focused: WorkWorkspaceKey?
    let layout: DashboardSlotLayout
    var editing: DashboardSlotEditing?
    @ViewBuilder let tile: (WorkMemberRef) -> Tile
    @State private var page = 0
    @State private var forward = true
    @Environment(\.palette) private var palette

    var body: some View {
        let pages = layout.pages(members)
        let current = min(page, pages.count - 1)
        VStack(spacing: 8) {
            pageView(pages[current])
                .id(current)
                .transition(.push(from: forward ? .trailing : .leading))
                .clipped()
            if pages.count > 1 {
                HStack(spacing: 12) {
                    Button { turn(to: current - 1) } label: { Image(systemName: "chevron.left") }
                        .disabled(current == 0).help("Previous page")
                        .keyboardShortcut(.leftArrow, modifiers: [.command, .option])
                    Text("Page \(current + 1) of \(pages.count)").font(Typography.code(11))
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("dashboard-page")
                    Button { turn(to: current + 1) } label: { Image(systemName: "chevron.right") }
                        .disabled(current == pages.count - 1).help("Next page")
                        .keyboardShortcut(.rightArrow, modifiers: [.command, .option])
                }.buttonStyle(.plain)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onChange(of: layout.pages(members).count) { _, count in page = min(page, count - 1) }
    }

    private func turn(to target: Int) {
        forward = target > page
        withAnimation(FocusSizing.animation) { page = target }
    }

    private func pageView(_ slice: [WorkMemberRef]) -> some View {
        let focus = slice.firstIndex { $0.work == focused }
        return GeometryReader { geometry in
            let frames = layout.frames(in: CGRect(origin: .zero, size: geometry.size), focus: focus)
            ZStack(alignment: .topLeading) {
                ForEach(0..<layout.count, id: \.self) { index in
                    let frame = (frames[index] ?? .zero).insetBy(dx: 4, dy: 4)
                    // The layout owns each slot's frame; content never widens it
                    // and anything that cannot shrink is clipped at the slot edge.
                    Group {
                        if index < slice.count {
                            tile(slice[index]).id(slice[index].work)
                        } else {
                            RoundedRectangle(cornerRadius: PaneGeometry.cornerRadius)
                                .strokeBorder(palette.border, style: StrokeStyle(lineWidth: 1, dash: [4, 4]))
                                .accessibilityIdentifier("dashboard-empty-slot")
                        }
                    }
                    .frame(width: max(0, frame.width), height: max(0, frame.height), alignment: .topLeading)
                    .clipShape(RoundedRectangle(cornerRadius: PaneGeometry.cornerRadius))
                    .overlay { if let editing { slotControls(index, editing) } }
                    .offset(x: frame.minX, y: frame.minY)
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
            .environment(\.compactPanes, true)
            .animation(FocusSizing.animation, value: focus)
            .animation(FocusSizing.animation, value: layout)
        }
    }

    /// Drag a slot onto another to exchange their places; split or remove in place.
    private func slotControls(_ index: Int, _ editing: DashboardSlotEditing) -> some View {
        VStack(spacing: 8) {
            Text("Slot \(index + 1)").font(Typography.strong(13))
            HStack(spacing: 12) {
                Button { editing.write(layout.adding(beside: index, axis: .vertical)) } label: {
                    Image(systemName: "rectangle.split.2x1")
                }.help("Add a slot beside")
                Button { editing.write(layout.adding(beside: index, axis: .horizontal)) } label: {
                    Image(systemName: "rectangle.split.1x2")
                }.help("Add a slot below")
                Button { layout.removing(index).map(editing.write) } label: { Image(systemName: "minus.rectangle") }
                    .help("Remove this slot").disabled(layout.count == 1)
            }.buttonStyle(.plain)
        }
        .foregroundStyle(palette.text)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(palette.surface.opacity(0.85), in: RoundedRectangle(cornerRadius: PaneGeometry.cornerRadius))
        .contentShape(Rectangle())
        .draggable("slot:\(index)")
        .dropDestination(for: String.self) { values, _ in
            guard let value = values.first, value.hasPrefix("slot:"), let other = Int(value.dropFirst(5)), other != index else { return false }
            editing.write(layout.swapping(index, other)); return true
        }
        .accessibilityIdentifier("dashboard-slot-\(index)")
    }
}
