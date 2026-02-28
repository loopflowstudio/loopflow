#if canImport(UIKit)
import SwiftUI
import UIKit
import LoopflowCore

enum QuoteAction {
    case quoteReply(String)
    case emoji(String, String)
}

struct SelectableAssistantTextView: UIViewRepresentable {
    let text: String
    let onQuoteAction: (QuoteAction) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onQuoteAction: onQuoteAction)
    }

    func makeUIView(context: Context) -> SelectableTextView {
        let textView = SelectableTextView()
        textView.coordinator = context.coordinator
        textView.applyStyledContent(text)
        return textView
    }

    func updateUIView(_ textView: SelectableTextView, context: Context) {
        context.coordinator.onQuoteAction = onQuoteAction
        if textView.lastText != text {
            textView.applyStyledContent(text)
        }
    }
}

final class SelectableTextView: UITextView {
    weak var coordinator: SelectableAssistantTextView.Coordinator?
    var lastText: String = ""

    private static let emojis = ["👍", "👎", "✏️", "❓"]

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

    func applyStyledContent(_ markdown: String) {
        lastText = markdown
        attributedText = Self.styledAttributedString(from: markdown)
        invalidateIntrinsicContentSize()
    }

    override var intrinsicContentSize: CGSize {
        let width = bounds.width > 0 ? bounds.width : UIScreen.main.bounds.width - 32
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
        coordinator?.onQuoteAction(.emoji(selected, emoji))
    }

    private var selectedText: String? {
        guard selectedRange.length > 0,
              let text = text,
              let range = Range(selectedRange, in: text) else { return nil }
        let cleaned = String(text[range]).trimmingCharacters(in: .whitespacesAndNewlines)
        return cleaned.isEmpty ? nil : cleaned
    }

    static func styledAttributedString(from markdown: String) -> NSAttributedString {
        let baseFont = UIFont(name: Typography.sansFamily, size: 14) ?? .systemFont(ofSize: 14)
        let baseColor = UIColor.label

        guard let foundation = try? NSAttributedString(
            markdown: markdown,
            options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        ) else {
            return NSAttributedString(string: markdown, attributes: [
                .font: baseFont,
                .foregroundColor: baseColor,
            ])
        }

        let mutable = NSMutableAttributedString(attributedString: foundation)
        let fullRange = NSRange(location: 0, length: mutable.length)

        mutable.enumerateAttributes(in: fullRange) { attrs, range, _ in
            var updated: [NSAttributedString.Key: Any] = [
                .foregroundColor: baseColor,
            ]

            if let existingFont = attrs[.font] as? UIFont {
                let traits = existingFont.fontDescriptor.symbolicTraits
                let isBold = traits.contains(.traitBold)
                let isItalic = traits.contains(.traitItalic)
                let isMonospace = traits.contains(.traitMonoSpace)

                if isMonospace {
                    let monoFont = UIFont(name: Typography.monoFamily, size: 13) ?? .monospacedSystemFont(ofSize: 13, weight: .regular)
                    updated[.font] = monoFont
                } else if isBold && isItalic {
                    var descriptor = baseFont.fontDescriptor
                    descriptor = descriptor.withSymbolicTraits([.traitBold, .traitItalic]) ?? descriptor
                    updated[.font] = UIFont(descriptor: descriptor, size: 14)
                } else if isBold {
                    updated[.font] = UIFont(name: "\(Typography.sansFamily)-Bold", size: 14) ?? baseFont.bold()
                } else if isItalic {
                    updated[.font] = UIFont(name: "\(Typography.sansFamily)-Italic", size: 14) ?? baseFont.italic()
                } else {
                    updated[.font] = baseFont
                }
            } else {
                updated[.font] = baseFont
            }

            if let link = attrs[.link] {
                updated[.link] = link
            }
            if let strikethrough = attrs[.strikethroughStyle] {
                updated[.strikethroughStyle] = strikethrough
            }

            mutable.setAttributes(updated, range: range)
        }

        return mutable
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

private extension UIFont {
    func bold() -> UIFont {
        guard let descriptor = fontDescriptor.withSymbolicTraits(.traitBold) else { return self }
        return UIFont(descriptor: descriptor, size: pointSize)
    }

    func italic() -> UIFont {
        guard let descriptor = fontDescriptor.withSymbolicTraits(.traitItalic) else { return self }
        return UIFont(descriptor: descriptor, size: pointSize)
    }
}

#endif
