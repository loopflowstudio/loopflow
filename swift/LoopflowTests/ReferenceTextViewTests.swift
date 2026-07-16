#if os(macOS)
import Testing
import AppKit
@testable import LoopflowMac
@testable import Loopflow

@Suite("Reference text rendering")
struct ReferenceTextViewTests {
    private let font = NSFont.systemFont(ofSize: 14)
    private let textColor = NSColor.labelColor
    private let accentColor = NSColor.systemPurple

    private func build(_ text: String) -> NSAttributedString {
        ReferenceTextView.attributedString(
            for: text,
            font: font,
            textColor: textColor,
            accentColor: accentColor,
            references: parseChatReferences(in: text)
        )
    }

    @Test("A reference range carries the reference link and accent color")
    func referenceRangeIsLinked() {
        let text = "closing W2-174 now"
        let attributed = build(text)
        let range = (text as NSString).range(of: "W2-174")
        let attrs = attributed.attributes(at: range.location, effectiveRange: nil)

        let link = attrs[.link] as? URL
        #expect(link != nil)
        let decoded = link.flatMap(ReferenceTextView.decodeReference)
        #expect(decoded?.kind == .task)
        #expect(decoded?.identifier == "W2-174")
        #expect(attrs[.foregroundColor] as? NSColor == accentColor)
    }

    @Test("Plain prose outside a reference is not linked")
    func plainRangeIsNotLinked() {
        let text = "closing W2-174 now"
        let attributed = build(text)
        let range = (text as NSString).range(of: "closing")
        let attrs = attributed.attributes(at: range.location, effectiveRange: nil)
        #expect(attrs[.link] == nil)
        #expect(attrs[.foregroundColor] as? NSColor == textColor)
    }

    @Test("A message with no references is entirely plain")
    func noReferencesEntirelyPlain() {
        let text = "Project Session is healthy and current"
        let attributed = build(text)
        var sawLink = false
        attributed.enumerateAttribute(.link, in: NSRange(location: 0, length: attributed.length)) { value, _, _ in
            if value != nil { sawLink = true }
        }
        #expect(!sawLink)
        #expect(attributed.string == text)
    }

    @Test("A PR reference round-trips through the reference URL scheme")
    func pullRequestURLRoundTrips() {
        let url = ReferenceTextView.referenceURL(kind: .pullRequest, identifier: "889")
        let decoded = url.flatMap(ReferenceTextView.decodeReference)
        #expect(decoded?.kind == .pullRequest)
        #expect(decoded?.identifier == "889")
    }
}
#endif
