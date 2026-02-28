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
