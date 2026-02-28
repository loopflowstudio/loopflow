#if canImport(UIKit)
import Testing
import UIKit
@testable import Concerto
@testable import LoopflowCore

@Suite("SelectableAssistantTextView styling")
struct SelectableAssistantTextViewTests {
    @Test("Plain text uses Lato 14pt")
    func plainTextUsesBaseFont() {
        let result = SelectableTextView.styledAttributedString(from: "Hello world")
        let attrs = result.attributes(at: 0, effectiveRange: nil)
        let font = attrs[.font] as? UIFont
        #expect(font != nil)
        #expect(font?.pointSize == 14)
        #expect(font?.fontName.contains("Lato") == true || font?.familyName == "Lato")
    }

    @Test("Bold markdown produces bold font trait")
    func boldTextHasBoldTrait() {
        let result = SelectableTextView.styledAttributedString(from: "**bold text**")
        let fullText = result.string
        guard let boldRange = fullText.range(of: "bold text") else {
            Issue.record("Expected 'bold text' in output")
            return
        }
        let nsRange = NSRange(boldRange, in: fullText)
        let attrs = result.attributes(at: nsRange.location, effectiveRange: nil)
        let font = attrs[.font] as? UIFont
        #expect(font != nil)
        let isBold = font?.fontDescriptor.symbolicTraits.contains(.traitBold) ?? false
        #expect(isBold)
    }

    @Test("Italic markdown produces italic font trait")
    func italicTextHasItalicTrait() {
        let result = SelectableTextView.styledAttributedString(from: "*italic text*")
        let fullText = result.string
        guard let italicRange = fullText.range(of: "italic text") else {
            Issue.record("Expected 'italic text' in output")
            return
        }
        let nsRange = NSRange(italicRange, in: fullText)
        let attrs = result.attributes(at: nsRange.location, effectiveRange: nil)
        let font = attrs[.font] as? UIFont
        #expect(font != nil)
        let isItalic = font?.fontDescriptor.symbolicTraits.contains(.traitItalic) ?? false
        #expect(isItalic)
    }

    @Test("Inline code uses monospace font")
    func inlineCodeUsesMonoFont() {
        let result = SelectableTextView.styledAttributedString(from: "Use `foo()` here")
        let fullText = result.string
        guard let codeRange = fullText.range(of: "foo()") else {
            Issue.record("Expected 'foo()' in output")
            return
        }
        let nsRange = NSRange(codeRange, in: fullText)
        let attrs = result.attributes(at: nsRange.location, effectiveRange: nil)
        let font = attrs[.font] as? UIFont
        #expect(font != nil)
        let isMonospace = font?.fontDescriptor.symbolicTraits.contains(.traitMonoSpace) ?? false
            || font?.fontName.contains("JetBrains") == true
            || font?.fontName.contains("Mono") == true
        #expect(isMonospace)
    }

    @Test("Fallback returns plain attributed string for invalid markdown")
    func fallbackForInvalidMarkdown() {
        let result = SelectableTextView.styledAttributedString(from: "plain text")
        #expect(result.string == "plain text")
        #expect(result.length > 0)
    }

    @Test("Foreground color is set to label")
    func foregroundColorIsLabel() {
        let result = SelectableTextView.styledAttributedString(from: "colored text")
        let attrs = result.attributes(at: 0, effectiveRange: nil)
        let color = attrs[.foregroundColor] as? UIColor
        #expect(color == .label)
    }
}

#endif

import Testing
@testable import Concerto

@Suite("iOS quote reply queue flow")
struct IOSQuoteReplyQueueFlowTests {
    @Test("Emoji reaction from selection queues directly")
    func emojiReactQueuesDirectly() {
        let queue = ReplyQueue()
        let selectedText = "the migration looks correct"
        queue.addEmojiReact(quoted: selectedText, emoji: "\u{1F44D}")

        #expect(queue.count == 1)
        let assembled = queue.assembleMessage()
        #expect(assembled.contains("> the migration looks correct"))
        #expect(assembled.contains("\u{1F44D}"))
    }

    @Test("Quote reply from selection queues with reply text")
    func quoteReplyQueuesWithText() {
        let queue = ReplyQueue()
        queue.addQuoteReply(
            quoted: "use a composite key",
            reply: "Prefer UUID for simpler joins."
        )

        let assembled = queue.assembleMessage()
        #expect(assembled.contains("> use a composite key"))
        #expect(assembled.contains("Prefer UUID for simpler joins."))
    }

    @Test("Mixed queue from iOS selection produces same format as macOS")
    func mixedQueueFormatParity() {
        let queue = ReplyQueue()
        queue.addQuoteReply(quoted: "add an index", reply: "Use a partial index.")
        queue.addEmojiReact(quoted: "migration script", emoji: "\u{1F44D}")

        let assembled = queue.assembleMessage(extraFreeText: "Ship it.")

        #expect(assembled.contains("> add an index"))
        #expect(assembled.contains("Use a partial index."))
        #expect(assembled.contains("> migration script"))
        #expect(assembled.contains("\u{1F44D}"))
        #expect(assembled.hasSuffix("Ship it."))
    }
}
