#if os(macOS)
import AppKit

extension NSWindow {
    /// One action and the layout and drawing it causes, in milliseconds.
    @MainActor func measureFrame(_ action: () -> Void) -> Double {
        let start = DispatchTime.now().uptimeNanoseconds
        action()
        for _ in 0..<3 {
            contentView?.layoutSubtreeIfNeeded()
            layoutIfNeeded()
            displayIfNeeded()
            RunLoop.main.run(until: .now)
        }
        return Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
    }
}

extension NSView {
    /// The scroll view with the tallest document: a reader or stream, not a sidebar list.
    @MainActor var tallestScrollView: NSScrollView? {
        var found: [NSScrollView] = []
        func visit(_ view: NSView) {
            if let scroll = view as? NSScrollView { found.append(scroll) }
            view.subviews.forEach(visit)
        }
        visit(self)
        return found.max { ($0.documentView?.frame.height ?? 0) < ($1.documentView?.frame.height ?? 0) }
    }
}
#endif
