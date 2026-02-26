import Foundation
import Observation

// MARK: - Reply Entry

enum ReplyEntry: Identifiable, Equatable {
    case quoteReply(id: UUID = UUID(), quoted: String, reply: String)
    case emojiReact(id: UUID = UUID(), quoted: String, emoji: String)
    case freeText(id: UUID = UUID(), text: String)

    var id: UUID {
        switch self {
        case .quoteReply(let id, _, _):
            return id
        case .emojiReact(let id, _, _):
            return id
        case .freeText(let id, _):
            return id
        }
    }

    var quotedText: String? {
        switch self {
        case .quoteReply(_, let quoted, _), .emojiReact(_, let quoted, _):
            return quoted
        case .freeText:
            return nil
        }
    }
}

// MARK: - Reply Queue

@Observable
final class ReplyQueue {
    var entries: [ReplyEntry] = []

    var isEmpty: Bool {
        entries.isEmpty
    }

    func addQuoteReply(quoted rawQuoted: String, reply rawReply: String) {
        let quoted = sanitize(rawQuoted)
        let reply = sanitize(rawReply)
        guard !quoted.isEmpty, !reply.isEmpty else { return }
        entries.append(.quoteReply(quoted: quoted, reply: reply))
    }

    func addEmojiReact(quoted rawQuoted: String, emoji: String) {
        let quoted = sanitize(rawQuoted)
        let trimmedEmoji = sanitize(emoji)
        guard !quoted.isEmpty, !trimmedEmoji.isEmpty else { return }
        entries.append(.emojiReact(quoted: quoted, emoji: trimmedEmoji))
    }

    func addFreeText(_ rawText: String) {
        let text = sanitize(rawText)
        guard !text.isEmpty else { return }
        entries.append(.freeText(text: text))
    }

    func remove(id: UUID) {
        entries.removeAll { $0.id == id }
    }

    func clear() {
        entries.removeAll()
    }

    func assembleMessage(extraFreeText rawExtraText: String = "") -> String {
        let entryBlocks = entries.compactMap(assembleBlock)
        let extraText = sanitize(rawExtraText)

        if extraText.isEmpty {
            return entryBlocks.joined(separator: "\n\n")
        }

        if entryBlocks.isEmpty {
            return extraText
        }

        return (entryBlocks + [extraText]).joined(separator: "\n\n")
    }

    func previewLabel(for entry: ReplyEntry) -> String {
        switch entry {
        case .quoteReply(_, _, let reply):
            return reply
        case .emojiReact(_, _, let emoji):
            return emoji
        case .freeText(_, let text):
            return text
        }
    }

    private func assembleBlock(_ entry: ReplyEntry) -> String? {
        switch entry {
        case .quoteReply(_, let quoted, let reply):
            return quoteBlock(quoted: quoted, response: reply)
        case .emojiReact(_, let quoted, let emoji):
            return quoteBlock(quoted: quoted, response: emoji)
        case .freeText(_, let text):
            let normalized = sanitize(text)
            return normalized.isEmpty ? nil : normalized
        }
    }

    private func quoteBlock(quoted rawQuoted: String, response rawResponse: String) -> String? {
        let quoted = sanitize(rawQuoted)
        let response = sanitize(rawResponse)
        guard !quoted.isEmpty, !response.isEmpty else { return nil }
        return "> \(quoted)\n\n\(response)"
    }

    private func sanitize(_ text: String) -> String {
        text
            .replacingOccurrences(of: "\r\n", with: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

extension ReplyQueue {
    static func demoQueueEmpty() -> ReplyQueue {
        ReplyQueue()
    }

    static func demoQueueSingle() -> ReplyQueue {
        let queue = ReplyQueue()
        queue.addQuoteReply(
            quoted: "use a composite key for the junction table",
            reply: "No, use a UUID. Composite keys make joins harder to reason about."
        )
        return queue
    }

    static func demoQueueTriple() -> ReplyQueue {
        let queue = ReplyQueue()
        queue.addQuoteReply(
            quoted: "use a composite key for the junction table",
            reply: "No, use a UUID. Composite keys make joins harder to reason about."
        )
        queue.addEmojiReact(quoted: "here's the migration script", emoji: "👍")
        queue.addQuoteReply(
            quoted: "we should add an index on created_at",
            reply: "Use a partial index on active records only."
        )
        return queue
    }

    static func demoQueueFullMixed() -> ReplyQueue {
        let queue = demoQueueTriple()
        queue.addFreeText("Also, switch tests to fixtures instead of inline data.")
        queue.addEmojiReact(quoted: "these names feel too abstract", emoji: "✏️")
        return queue
    }
}
