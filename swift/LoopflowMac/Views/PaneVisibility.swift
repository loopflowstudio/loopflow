import Loopflow
import SwiftUI

/// A view header's way back to its hidden panes. Absent when nothing is hidden.
struct ShowHiddenMenu: View {
    struct Item: Identifiable, Equatable {
        let id: String
        let title: String
    }
    let items: [Item]
    let show: (String) -> Void

    var body: some View {
        if !items.isEmpty {
            Menu("Show…") {
                ForEach(items) { item in
                    Button(item.title) { show(item.id) }
                        .accessibilityIdentifier("show-pane-\(item.id)")
                }
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .help("Show a hidden pane")
            .accessibilityIdentifier("show-hidden-panes")
        }
    }
}

extension ShowHiddenMenu {
    /// A Task workspace's hidden panes, as `lf desktop state` reports them.
    init(workspace: SessionsWorkspace) {
        let store = workspace.multiplexer
        items = store.layout.allPanes.filter { store.collapsedPaneIds.contains($0.id) }
            .map { Item(id: $0.id, title: $0.kind.rawValue.capitalized) }
        show = { id in
            workspace.setCollapsed(paneId: id, collapsed: false)
            workspace.presentation.content = .terminals
        }
    }

    /// A Wave view's hidden board, conversation and strip.
    init(model: WorkModel, view: WorkNavigation.Board) {
        items = WaveViewPart.allCases.filter { model.isHidden($0, in: view) }
            .map { Item(id: $0.rawValue, title: $0.title) }
        show = { id in WaveViewPart(rawValue: id).map { model.setHidden($0, false, in: view) } }
    }
}

/// A Wave view part with zoom and hide buttons revealed on hover, as every
/// workspace pane has. The view's conversation workspace keeps the hover flag, as
/// it does for pane strips.
struct HideablePart<Content: View>: View {
    let part: WaveViewPart
    let view: WorkNavigation.Board
    let model: WorkModel
    let workspace: SessionsWorkspace
    @ViewBuilder let content: Content
    @Environment(\.palette) private var palette

    private var hover: HoverFlag { workspace.stripHover("wave-\(part.rawValue)") }

    var body: some View {
        let zoomed = model.zoomedPart(in: view) == part
        content
            .overlay(alignment: .topTrailing) {
                HoverRevealed(flag: hover) {
                    HStack(spacing: 2) {
                        button(zoomed ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right",
                               label: zoomed ? "Zoom out" : "Zoom \(part.title)", id: "wave-zoom-\(part.rawValue)",
                               help: zoomed ? "Return to the view (Esc)" : "Fill the view with the \(part.title.lowercased())") {
                            model.setZoomed(zoomed ? nil : part, in: view)
                        }
                        button("eye.slash", label: "Hide \(part.title)", id: "wave-hide-\(part.rawValue)",
                               help: "Hide the \(part.title.lowercased()); Show… in the header brings it back") {
                            model.setHidden(part, true, in: view)
                        }
                    }
                    // Clear the tile's rounded corner, not just its edge.
                    .padding(Spacing.sm)
                    .accessibilityIdentifier("wave-part-actions-\(part.rawValue)")
                }
            }
            .background(PaneHoverRegion(identifier: "wave-hover-\(part.rawValue)") { [hover] inside in
                if hover.isHovered != inside { hover.isHovered = inside }
            })
    }

    private func button(_ systemImage: String, label: String, id: String, help: String,
                        action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 11))
                .foregroundStyle(palette.textSecondary)
                .frame(width: 22, height: 20)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(help)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }
}

/// Two views split by a divider that drags, as Task pane dividers do.
/// `vertical` places them side by side; `ratio` is the first one's share.
struct RatioSplit<First: View, Second: View>: View {
    let axis: SplitAxis
    let ratio: Double
    let update: (Double) -> Void
    @ViewBuilder let first: First
    @ViewBuilder let second: Second
    @Environment(\.palette) private var palette

    var body: some View {
        GeometryReader { geometry in
            let vertical = axis == .vertical
            let extent = vertical ? geometry.size.width : geometry.size.height
            let minimum: CGFloat = vertical ? 240 : 140
            let sizing = SplitSizing(extent: extent, firstMinimum: minimum, secondMinimum: minimum)
            let shown = sizing.constrain(ratio)
            let firstExtent = max(0, extent * shown - sizing.divider / 2)
            let secondExtent = max(0, extent - firstExtent - sizing.divider)
            let stack = vertical ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))
            stack {
                first.frame(width: vertical ? firstExtent : nil, height: vertical ? nil : firstExtent).clipped()
                SplitDivider(axis: axis, ratio: shown, sizing: sizing, update: update)
                    .background(palette.border.frame(width: vertical ? 1 : nil, height: vertical ? nil : 1))
                    .accessibilityIdentifier("wave-divider-\(vertical ? "band" : "strip")")
                second.frame(width: vertical ? secondExtent : nil, height: vertical ? nil : secondExtent).clipped()
            }
        }
    }
}

/// A Wave view: its conversation and board side by side over its Task strip, each
/// divider dragging. A hidden part gives its room to the rest; a zoomed part fills it.
struct WaveViewLayout<Session: View, Board: View, Tiles: View>: View {
    let model: WorkModel
    let view: WorkNavigation.Board
    let workspace: SessionsWorkspace
    @ViewBuilder let session: Session
    @ViewBuilder let board: Board
    @ViewBuilder let tiles: Tiles

    var body: some View {
        VStack(spacing: 0) {
            if let zoomed = model.zoomedPart(in: view) {
                part(zoomed)
                    .background {
                        // Esc zooms out where no terminal reads it.
                        if zoomed != .session {
                            Button("Zoom out") { model.setZoomed(nil, in: view) }
                                .keyboardShortcut(.cancelAction).opacity(0).allowsHitTesting(false)
                        }
                    }
            } else {
                if !shown(.board) { WaveViewHeader(model: model, view: view) }
                let top = shown(.board) || shown(.session)
                if top, shown(.tiles) {
                    RatioSplit(axis: .horizontal, ratio: ratios.band,
                               update: { model.navigation.waveRatios[view, default: WaveViewRatios()].band = $0 }) {
                        band
                    } second: { part(.tiles) }
                } else if top {
                    band
                } else if shown(.tiles) {
                    part(.tiles)
                } else {
                    Spacer()
                }
            }
        }
    }

    private var ratios: WaveViewRatios { model.navigation.waveRatios[view] ?? WaveViewRatios() }

    private func shown(_ part: WaveViewPart) -> Bool { !model.isHidden(part, in: view) }

    @ViewBuilder private var band: some View {
        if shown(.session), shown(.board) {
            RatioSplit(axis: .vertical, ratio: ratios.session,
                       update: { model.navigation.waveRatios[view, default: WaveViewRatios()].session = $0 }) {
                part(.session)
            } second: { part(.board) }
        } else if shown(.session) {
            part(.session)
        } else {
            part(.board)
        }
    }

    private func part(_ part: WaveViewPart) -> some View {
        HideablePart(part: part, view: view, model: model, workspace: workspace) {
            switch part {
            case .session: session
            case .board: board
            case .tiles: tiles
            }
        }
    }
}

/// The breadcrumb's last segment while a pane fills its view; the button, the
/// parent crumb or Esc returns to the layout.
struct ZoomCrumb: View {
    let title: String
    let zoomOut: () -> Void

    var body: some View {
        HStack(spacing: 6) {
            Text("›").foregroundStyle(.secondary)
            Text(title).lineLimit(1).fixedSize()
            Button(action: zoomOut) { Image(systemName: "arrow.down.right.and.arrow.up.left") }
                .frame(width: 24, height: 28).contentShape(Rectangle())
                .help("Zoom out")
                .accessibilityIdentifier("zoom-out")
        }
        .accessibilityIdentifier("zoom-crumb")
    }
}
