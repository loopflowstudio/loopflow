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

/// A Dashboard page's fixed slots as data. Tasks fill slots in order; they never
/// add, remove or resize slots.
struct DashboardSlotLayout: Equatable {
    indirect enum Node: Equatable {
        case slot(Int)
        /// `.vertical` places children side by side; `.horizontal` stacks them.
        case split(SplitAxis, [Double], [Node])
    }

    let root: Node

    /// Jack Heart's default, October 20: (1 | (2 | 3)) over (1 | 2 | 3 | 4), golden
    /// proportions — the top band takes 61.8% of the height, slot 1 61.8% of its width.
    static let golden = DashboardSlotLayout(root: .split(.horizontal, [FocusSizing.golden, 1 - FocusSizing.golden], [
        .split(.vertical, [FocusSizing.golden, 1 - FocusSizing.golden], [
            .slot(0), .split(.vertical, [1, 1], [.slot(1), .slot(2)]),
        ]),
        .split(.vertical, [1, 1, 1, 1], [.slot(3), .slot(4), .slot(5), .slot(6)]),
    ]))

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
                    if axis == .vertical {
                        let width = rect.width * share
                        lay(child, CGRect(x: rect.minX + offset, y: rect.minY, width: width, height: rect.height))
                        offset += width
                    } else {
                        let height = rect.height * share
                        lay(child, CGRect(x: rect.minX, y: rect.minY + offset, width: rect.width, height: height))
                        offset += height
                    }
                }
            }
        }
        lay(root, bounds)
        return frames
    }

    private static func slots(_ node: Node) -> [Int] {
        switch node {
        case .slot(let index): [index]
        case .split(_, _, let children): children.flatMap(slots)
        }
    }
}

/// Members fill the layout's slots in order; beyond one page they page side to side.
struct DashboardSlotGrid<Tile: View>: View {
    let members: [WorkMemberRef]
    let focused: WorkWorkspaceKey?
    let layout: DashboardSlotLayout
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
        .onChange(of: members.count) { _, _ in page = min(page, layout.pages(members).count - 1) }
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
                    Group {
                        if index < slice.count {
                            tile(slice[index]).id(slice[index].work)
                        } else {
                            RoundedRectangle(cornerRadius: PaneGeometry.cornerRadius)
                                .strokeBorder(palette.border, style: StrokeStyle(lineWidth: 1, dash: [4, 4]))
                                .accessibilityIdentifier("dashboard-empty-slot")
                        }
                    }
                    .frame(width: max(0, frame.width), height: max(0, frame.height))
                    .offset(x: frame.minX, y: frame.minY)
                }
            }
            .animation(FocusSizing.animation, value: focus)
        }
    }
}
