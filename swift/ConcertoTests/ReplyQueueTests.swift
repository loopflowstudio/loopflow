import Testing
@testable import Concerto

@Suite("ReplyQueue")
struct ReplyQueueTests {
    @Test("assembleMessage renders quote replies, emoji reacts, and free text")
    func assembleMessageMixedEntries() {
        let queue = ReplyQueue()

        queue.addQuoteReply(
            quoted: "use a composite key for the junction table",
            reply: "No, use a UUID. Composite keys make join queries ugly."
        )
        queue.addEmojiReact(quoted: "here's the migration script", emoji: "👍")
        queue.addFreeText("Also, switch tests to fixtures.")

        let assembled = queue.assembleMessage()

        #expect(assembled.contains("> use a composite key for the junction table"))
        #expect(assembled.contains("No, use a UUID. Composite keys make join queries ugly."))
        #expect(assembled.contains("> here's the migration script"))
        #expect(assembled.contains("👍"))
        #expect(assembled.hasSuffix("Also, switch tests to fixtures."))
    }

    @Test("assembleMessage appends composer text after queued replies")
    func assembleMessageAppendsExtraText() {
        let queue = ReplyQueue()
        queue.addQuoteReply(quoted: "add an index", reply: "Use a partial index instead.")

        let assembled = queue.assembleMessage(extraFreeText: "Please prioritize this in the next pass.")

        #expect(assembled.contains("Use a partial index instead."))
        #expect(assembled.hasSuffix("Please prioritize this in the next pass."))
    }

    @Test("empty and whitespace entries are ignored")
    func ignoresEmptyEntries() {
        let queue = ReplyQueue()
        queue.addQuoteReply(quoted: "", reply: "")
        queue.addEmojiReact(quoted: "   ", emoji: "👍")
        queue.addFreeText("\n\n")

        #expect(queue.entries.isEmpty)
        #expect(queue.assembleMessage().isEmpty)
    }

    @Test("multi-line quotes keep markdown quote markers on every line")
    func multilineQuotesArePrefixedPerLine() {
        let queue = ReplyQueue()
        queue.addQuoteReply(
            quoted: "first line\nsecond line",
            reply: "Looks good."
        )

        let assembled = queue.assembleMessage()

        #expect(assembled.contains("> first line\n> second line"))
    }
}
