#if os(macOS)
import SwiftUI
import AppKit
import Loopflow

/// Selectable, autosizing prose for an assistant turn. Recovered from the
/// conversations UI, trimmed to plain selection — the quote-reply machinery it
/// used to drive lives in a subsystem that no longer exists. An `NSTextView`
/// gives real text selection (copy, drag) that SwiftUI's `Text` can't, while
/// sizing itself to its content so it flows in a `VStack`.
struct SelectableAssistantMessageTextView: NSViewRepresentable {
    let text: String

    func makeNSView(context: Context) -> AutosizingSelectableTextView {
        let textView = AutosizingSelectableTextView(frame: .zero, textContainer: nil)
        textView.string = text
        return textView
    }

    func updateNSView(_ nsView: AutosizingSelectableTextView, context: Context) {
        if nsView.string != text {
            nsView.string = text
            nsView.invalidateIntrinsicContentSize()
        }
    }
}

final class AutosizingSelectableTextView: NSTextView {
    override init(frame frameRect: NSRect, textContainer container: NSTextContainer?) {
        let textStorage = NSTextStorage()
        let layoutManager = NSLayoutManager()
        let textContainer = NSTextContainer(size: NSSize(width: frameRect.width, height: .greatestFiniteMagnitude))
        textContainer.widthTracksTextView = true
        textContainer.lineFragmentPadding = 0

        textStorage.addLayoutManager(layoutManager)
        layoutManager.addTextContainer(textContainer)

        super.init(frame: frameRect, textContainer: textContainer)

        isEditable = false
        isSelectable = true
        drawsBackground = false
        isRichText = false
        allowsUndo = false
        importsGraphics = false
        textContainerInset = NSSize(width: 0, height: 0)
        isVerticallyResizable = true
        isHorizontallyResizable = false
        autoresizingMask = [.width]

        font = NSFont(name: "Lato", size: 14) ?? .systemFont(ofSize: 14)
        textColor = .labelColor
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var intrinsicContentSize: NSSize {
        guard let layoutManager, let textContainer else {
            return super.intrinsicContentSize
        }
        layoutManager.ensureLayout(for: textContainer)
        let usedRect = layoutManager.usedRect(for: textContainer)
        let height = ceil(usedRect.height + textContainerInset.height * 2)
        return NSSize(width: NSView.noIntrinsicMetric, height: max(20, height))
    }

    override func layout() {
        super.layout()
        invalidateIntrinsicContentSize()
    }

    override var frame: NSRect {
        didSet {
            if oldValue.size.width != frame.size.width {
                invalidateIntrinsicContentSize()
            }
        }
    }
}

#endif
