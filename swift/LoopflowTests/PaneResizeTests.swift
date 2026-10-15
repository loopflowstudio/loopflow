#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Pane resizing")
@MainActor
struct PaneResizeTests {
    @Test("New shells use the golden split and retain the existing pane", arguments: [false, true])
    func newShellStartsAtGoldenRatio(explicitSplit: Bool) throws {
        let store = MultiplexerStore()
        store.load(sessionId: "conversation")
        let original = store.focusedPane
        let added = try #require(explicitSplit
            ? store.split(original.id, axis: .horizontal)
            : store.newShell())
        guard case .split(_, let first, let second, let ratio) = store.layout else {
            Issue.record("Expected a shell split"); return
        }
        #expect(first.firstPane == original)
        #expect(second.firstPane.id == added.id)
        #expect(ratio == 0.618)
        #expect(store.focusedPaneId == added.id)
    }

    @Test("Drag snaps near either golden proportion and moves freely elsewhere")
    func goldenSnapping() {
        let sizing = SplitSizing(extent: 1000, firstMinimum: 240, secondMinimum: 240)
        #expect(sizing.snapped(0.625) == LayoutNode.goldenRatio)
        #expect(sizing.snapped(0.375) == 1 - LayoutNode.goldenRatio)
        #expect(sizing.snapped(0.65) == 0.65)
        #expect(sizing.snapped(0.5) == 0.5)
    }

    @Test("Both sides keep their minimum size; snapping cannot override it")
    func boundedResizing() {
        let sizing = SplitSizing(extent: 1000, firstMinimum: 400, secondMinimum: 300)
        #expect(abs(sizing.constrain(-1) * 1000 - 3 - 400) < 0.001)
        #expect(abs((1 - sizing.constrain(2)) * 1000 - 3 - 300) < 0.001)
        #expect(sizing.snapped(0.382) == sizing.constrain(0.382))
        #expect(sizing.snapped(0.382) > 0.382)
    }

    @Test("Nested panes contribute minimum size and small windows shrink both sides to fit")
    func nestedAndSmallLayouts() {
        let leaf = LayoutNode.leaf(PaneState(content: .empty))
        let columns = LayoutNode.split(.vertical, first: leaf, second: leaf, ratio: 0.618)
        let rows = LayoutNode.split(.horizontal, first: columns, second: leaf, ratio: 0.618)
        #expect(rows.minimumPaneSize == CGSize(width: 486, height: 286))
        let sizing = SplitSizing(extent: 300, firstMinimum: columns.minimumPaneSize.width, secondMinimum: 240)
        let left = sizing.constrain(0.1), right = sizing.constrain(0.9)
        #expect(abs(left - right) < 0.001)
        let first = 300 * left - sizing.divider / 2
        let second = 300 * (1 - left) - sizing.divider / 2
        #expect(first > 0 && second > 0)
        #expect(abs(first + second + sizing.divider - 300) < 0.001)
        #expect(abs(first / second - 486.0 / 240.0) < 0.001)
        #expect(SplitSizing(extent: 0, firstMinimum: 240, secondMinimum: 240).constrain(0.618) == 0.5)
    }
}
#endif
