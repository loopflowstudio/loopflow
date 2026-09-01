import Foundation
import Testing
@testable import Loopflow

// The split tree is the multiplexer's spine: split, close-and-collapse, replace,
// resize, and round-trip must hold, because focus/color/persistence all read it.
@Suite("Multiplexer layout")
struct MultiplexerLayoutTests {
    @Test("default layout is a single empty pane")
    func defaultIsOneLeaf() {
        let layout = LayoutNode.defaultLayout()
        #expect(layout.allPanes.count == 1)
        #expect(layout.allPanes.first?.content == .empty)
    }

    @Test("splitting a leaf yields two panes under a split")
    func splittingAddsAPane() {
        let base = PaneState(content: .empty)
        let layout = LayoutNode.leaf(base)
        let shell = PaneState(content: .shell)
        let split = layout.splitting(base.id, axis: .vertical, newPane: shell)

        #expect(split.allPanes.count == 2)
        #expect(split.pane(for: shell.id)?.content == .shell)
        if case .split(let axis, _, _, let ratio) = split {
            #expect(axis == .vertical)
            #expect(ratio == 0.5)
        } else {
            Issue.record("expected a split node")
        }
    }

    @Test("removing a pane collapses its split back to the sibling")
    func removingCollapses() {
        let a = PaneState(content: .empty)
        let b = PaneState(content: .shell)
        let split = LayoutNode.leaf(a).splitting(a.id, axis: .horizontal, newPane: b)

        let collapsed = split.removing(b.id)
        #expect(collapsed == .leaf(a))          // back to a bare leaf
        #expect(collapsed?.allPanes.count == 1)
    }

    @Test("removing the only pane returns nil")
    func removingLastReturnsNil() {
        let a = PaneState(content: .empty)
        #expect(LayoutNode.leaf(a).removing(a.id) == nil)
    }

    @Test("replacing pane content preserves identity and tree shape")
    func replacePreservesShape() {
        let a = PaneState(content: .empty)
        let b = PaneState(content: .shell)
        let split = LayoutNode.leaf(a).splitting(a.id, axis: .vertical, newPane: b)

        let replaced = split.replacingContent(of: b.id, with: .session(id: "session-1"))

        #expect(replaced.allPanes.count == 2)
        #expect(replaced.pane(for: b.id)?.content == .session(id: "session-1"))
    }

    @Test("resizing updates the split between two panes")
    func updatingRatioChangesContainingSplit() {
        let a = PaneState(content: .empty)
        let b = PaneState(content: .shell)
        let split = LayoutNode.leaf(a).splitting(a.id, axis: .vertical, newPane: b)

        let resized = split.updatingRatio(between: a.id, and: b.id, ratio: 0.3)
        if case .split(_, _, _, let ratio) = resized {
            #expect(ratio == 0.3)
        } else {
            Issue.record("expected a split node")
        }
    }

    @Test("a nested tree round-trips through Codable")
    func codableRoundTrip() throws {
        let a = PaneState(content: .session(id: "session-1"))
        let b = PaneState(content: .shell)
        let c = PaneState(content: .empty)
        let tree = LayoutNode
            .leaf(a)
            .splitting(a.id, axis: .vertical, newPane: b)
            .splitting(b.id, axis: .horizontal, newPane: c)

        let data = try JSONEncoder().encode(tree)
        let decoded = try JSONDecoder().decode(LayoutNode.self, from: data)
        #expect(decoded == tree)
        #expect(decoded.allPanes.count == 3)
    }
}
