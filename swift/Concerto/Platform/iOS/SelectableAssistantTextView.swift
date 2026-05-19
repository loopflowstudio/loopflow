#if canImport(UIKit)
import SwiftUI
import UIKit

enum QuoteAction {
    case quoteReply(String)
    case emojiReact(selected: String, emoji: String)
}

struct SelectableAssistantTextView: UIViewRepresentable {
    let attributedText: AttributedString
    let onQuoteAction: (QuoteAction) -> Void

    init(attributedText: AttributedString, onQuoteAction: @escaping (QuoteAction) -> Void) {
        self.attributedText = attributedText
        self.onQuoteAction = onQuoteAction
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(onQuoteAction: onQuoteAction)
    }

    func makeUIView(context: Context) -> SelectableTextView {
        let textView = SelectableTextView()
        textView.coordinator = context.coordinator
        textView.applyAttributedContent(attributedText)
        return textView
    }

    func updateUIView(_ textView: SelectableTextView, context: Context) {
        context.coordinator.onQuoteAction = onQuoteAction
        if textView.lastText != attributedText {
            textView.applyAttributedContent(attributedText)
        }
    }
}

final class SelectableTextView: UITextView {
    weak var coordinator: SelectableAssistantTextView.Coordinator?
    var lastText = AttributedString()

    private static let emojis = reactionEmojis.map(\.emoji)

    init() {
        super.init(frame: .zero, textContainer: nil)
        isEditable = false
        isSelectable = true
        isScrollEnabled = false
        backgroundColor = .clear
        textContainerInset = .zero
        textContainer.lineFragmentPadding = 0
        adjustsFontForContentSizeCategory = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func applyAttributedContent(_ content: AttributedString) {
        lastText = content
        attributedText = NSAttributedString(content)
        invalidateIntrinsicContentSize()
    }

    override var intrinsicContentSize: CGSize {
        let width = bounds.width > 0 ? bounds.width : (window?.screen?.bounds.width ?? 320) - 32
        let size = sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude))
        return CGSize(width: UIView.noIntrinsicMetric, height: ceil(size.height))
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        invalidateIntrinsicContentSize()
    }

    override func buildMenu(with builder: any UIMenuBuilder) {
        super.buildMenu(with: builder)

        let quoteReplyAction = UIAction(title: "Quote Reply", image: UIImage(systemName: "quote.bubble")) { [weak self] _ in
            self?.handleQuoteReply()
        }
        let quoteMenu = UIMenu(title: "", options: .displayInline, children: [quoteReplyAction])
        builder.insertChild(quoteMenu, atStartOfMenu: .standardEdit)

        let emojiChildren = Self.emojis.map { emoji in
            UIAction(title: emoji) { [weak self] _ in
                self?.handleEmoji(emoji)
            }
        }
        let emojiMenu = UIMenu(title: "", options: .displayInline, children: emojiChildren)
        builder.insertChild(emojiMenu, atEndOfMenu: .standardEdit)
    }

    private func handleQuoteReply() {
        guard let selected = selectedText else { return }
        coordinator?.onQuoteAction(.quoteReply(selected))
    }

    private func handleEmoji(_ emoji: String) {
        guard let selected = selectedText else { return }
        coordinator?.onQuoteAction(.emojiReact(selected: selected, emoji: emoji))
    }

    private var selectedText: String? {
        guard selectedRange.length > 0,
              let text = text,
              let range = Range(selectedRange, in: text) else { return nil }
        let cleaned = String(text[range]).trimmingCharacters(in: .whitespacesAndNewlines)
        return cleaned.isEmpty ? nil : cleaned
    }

}

extension SelectableAssistantTextView {
    final class Coordinator: NSObject {
        var onQuoteAction: (QuoteAction) -> Void

        init(onQuoteAction: @escaping (QuoteAction) -> Void) {
            self.onQuoteAction = onQuoteAction
        }
    }
}

#endif
