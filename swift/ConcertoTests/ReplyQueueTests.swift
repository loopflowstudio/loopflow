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

    @Test("remove deletes entry by id")
    func removeDeletesById() {
        let queue = ReplyQueue()
        queue.addFreeText("first")
        queue.addFreeText("second")
        #expect(queue.count == 2)

        let idToRemove = queue.entries[0].id
        queue.remove(id: idToRemove)

        #expect(queue.count == 1)
        #expect(queue.entries[0].responseText == "second")
    }

    @Test("clear removes all entries")
    func clearRemovesAll() {
        let queue = ReplyQueue()
        queue.addFreeText("one")
        queue.addFreeText("two")
        #expect(!queue.isEmpty)

        queue.clear()

        #expect(queue.isEmpty)
        #expect(queue.count == 0)
    }

    @Test("assembleMessage with empty queue but extra text returns the extra text")
    func assembleEmptyQueueWithExtraText() {
        let queue = ReplyQueue()
        let assembled = queue.assembleMessage(extraFreeText: "Just a thought.")
        #expect(assembled == "Just a thought.")
    }

    @Test("CRLF in input is normalized to LF")
    func crlfNormalized() {
        let queue = ReplyQueue()
        queue.addFreeText("line one\r\nline two")
        #expect(queue.entries[0].responseText == "line one\nline two")
    }
}
