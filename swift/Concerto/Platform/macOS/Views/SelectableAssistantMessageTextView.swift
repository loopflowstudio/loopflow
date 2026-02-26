#if os(macOS)
import SwiftUI
import AppKit

struct SelectableAssistantMessageTextView: NSViewRepresentable {
    let text: String
    let selectionResetToken: Int
    let onSelectionChanged: (String?) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onSelectionChanged: onSelectionChanged)
    }

    func makeNSView(context: Context) -> AutosizingSelectableTextView {
        let textView = AutosizingSelectableTextView(frame: .zero, textContainer: nil)
        textView.delegate = context.coordinator
        textView.string = text
        textView.onIntrinsicSizeInvalidated = {
            context.coordinator.invalidateHostLayout()
        }
        return textView
    }

    func updateNSView(_ nsView: AutosizingSelectableTextView, context: Context) {
        context.coordinator.onSelectionChanged = onSelectionChanged
        context.coordinator.invalidateLayout = {
            nsView.invalidateIntrinsicContentSize()
        }

        if nsView.string != text {
            nsView.string = text
            nsView.invalidateIntrinsicContentSize()
        }

        if context.coordinator.selectionResetToken != selectionResetToken {
            context.coordinator.selectionResetToken = selectionResetToken
            nsView.setSelectedRange(NSRange(location: 0, length: 0))
            onSelectionChanged(nil)
        }
    }

    final class Coordinator: NSObject, NSTextViewDelegate {
        var onSelectionChanged: (String?) -> Void
        var selectionResetToken = 0
        var invalidateLayout: (() -> Void)?

        init(onSelectionChanged: @escaping (String?) -> Void) {
            self.onSelectionChanged = onSelectionChanged
        }

        func textViewDidChangeSelection(_ notification: Notification) {
            guard let textView = notification.object as? NSTextView else { return }
            let selection = textView.selectedRange()
            guard selection.length > 0 else {
                onSelectionChanged(nil)
                return
            }

            let fullText = textView.string as NSString
            guard selection.location + selection.length <= fullText.length else {
                onSelectionChanged(nil)
                return
            }

            let rawSelected = fullText.substring(with: selection)
            let cleaned = rawSelected.trimmingCharacters(in: .whitespacesAndNewlines)
            onSelectionChanged(cleaned.isEmpty ? nil : cleaned)
        }

        func invalidateHostLayout() {
            invalidateLayout?()
        }
    }
}

final class AutosizingSelectableTextView: NSTextView {
    var onIntrinsicSizeInvalidated: (() -> Void)?

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
        onIntrinsicSizeInvalidated?()
    }

    override var frame: NSRect {
        didSet {
            if oldValue.size.width != frame.size.width {
                invalidateIntrinsicContentSize()
                onIntrinsicSizeInvalidated?()
            }
        }
    }
}

#endif
