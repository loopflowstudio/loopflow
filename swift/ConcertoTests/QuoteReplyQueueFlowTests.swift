import Testing
@testable import Concerto

@Suite("Quote reply queue flow")
struct QuoteReplyQueueFlowTests {
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

    @Test("Mixed queue produces expected blockquote format")
    func mixedQueueFormat() {
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
