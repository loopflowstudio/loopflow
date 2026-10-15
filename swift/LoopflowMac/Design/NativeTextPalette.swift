import AppKit
import Loopflow

extension NSTextView {
    func applyPalette(_ palette: LoopflowPalette) {
        backgroundColor = NSColor(palette.background)
        textColor = NSColor(palette.text)
        insertionPointColor = NSColor(palette.keyboardFocus)
        selectedTextAttributes = [
            .backgroundColor: NSColor(palette.workSelectedFill),
            .foregroundColor: NSColor(palette.workSelectedText),
        ]
        linkTextAttributes = [.foregroundColor: NSColor(palette.linkInk)]
    }
}
