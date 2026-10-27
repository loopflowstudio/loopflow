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

/// A Wave view part with a hide button revealed on hover, as every workspace pane has.
/// The view's conversation workspace keeps the hover flag, as it does for pane strips.
struct HideablePart<Content: View>: View {
    let part: WaveViewPart
    let view: WorkNavigation.Board
    let model: WorkModel
    let workspace: SessionsWorkspace
    @ViewBuilder let content: Content
    @Environment(\.palette) private var palette

    private var hover: HoverFlag { workspace.stripHover("wave-\(part.rawValue)") }

    var body: some View {
        content
            .overlay(alignment: .topTrailing) {
                HoverRevealed(flag: hover) {
                    Button { model.setHidden(part, true, in: view) } label: {
                        Image(systemName: "eye.slash")
                            .font(.system(size: 11))
                            .foregroundStyle(palette.textSecondary)
                            .frame(width: 22, height: 20)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help("Hide the \(part.title.lowercased()); Show… in the header brings it back")
                    .accessibilityLabel("Hide \(part.title)")
                    .accessibilityIdentifier("wave-hide-\(part.rawValue)")
                    .padding(4)
                }
            }
            .background(PaneHoverRegion(identifier: "wave-hover-\(part.rawValue)") { [hover] inside in
                if hover.isHovered != inside { hover.isHovered = inside }
            })
    }
}
