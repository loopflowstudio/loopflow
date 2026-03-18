import Foundation
import Observation
import SwiftUI

// MARK: - Reaction Emojis

let reactionEmojis: [(emoji: String, label: String)] = [
    ("👍", "Thumbs up"),
    ("👎", "Thumbs down"),
    ("✏️", "Edit"),
    ("❓", "Question"),
]

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

    var responseText: String {
        switch self {
        case .quoteReply(_, _, let reply):
            return reply
        case .emojiReact(_, _, let emoji):
            return emoji
        case .freeText(_, let text):
            return text
        }
    }

    var isEditable: Bool {
        switch self {
        case .quoteReply, .freeText:
            return true
        case .emojiReact:
            return false
        }
    }

    var assembledBlock: String {
        guard let quotedText else { return responseText }
        return "\(formattedQuote(quotedText))\n\n\(responseText)"
    }

    func withID(_ id: UUID) -> ReplyEntry {
        switch self {
        case .quoteReply(_, let quoted, let reply):
            return .quoteReply(id: id, quoted: quoted, reply: reply)
        case .emojiReact(_, let quoted, let emoji):
            return .emojiReact(id: id, quoted: quoted, emoji: emoji)
        case .freeText(_, let text):
            return .freeText(id: id, text: text)
        }
    }

    private func formattedQuote(_ text: String) -> String {
        text
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map { "> \($0)" }
            .joined(separator: "\n")
    }
}

// MARK: - Reply Queue

@Observable
final class ReplyQueue {
    private(set) var entries: [ReplyEntry] = []

    var isEmpty: Bool {
        entries.isEmpty
    }

    var count: Int {
        entries.count
    }

    func add(_ entry: ReplyEntry) {
        guard let normalizedEntry = normalized(entry) else { return }
        entries.append(normalizedEntry)
    }

    func addQuoteReply(quoted rawQuoted: String, reply rawReply: String) {
        add(.quoteReply(quoted: rawQuoted, reply: rawReply))
    }

    func addEmojiReact(quoted rawQuoted: String, emoji: String) {
        add(.emojiReact(quoted: rawQuoted, emoji: emoji))
    }

    func addFreeText(_ rawText: String) {
        add(.freeText(text: rawText))
    }

    func remove(id: UUID) {
        entries.removeAll { $0.id == id }
    }

    func move(fromOffsets source: IndexSet, toOffset destination: Int) {
        entries.move(fromOffsets: source, toOffset: destination)
    }

    func update(id: UUID, newEntry: ReplyEntry) {
        guard let index = entries.firstIndex(where: { $0.id == id }) else { return }
        guard let normalizedEntry = normalized(newEntry.withID(id)) else { return }
        entries[index] = normalizedEntry
    }

    func clear() {
        entries.removeAll()
    }

    func assembleMessage(extraFreeText rawExtraText: String = "") -> String {
        let entryBlocks = entries.map(\.assembledBlock)
        let extraText = sanitize(rawExtraText)
        guard !extraText.isEmpty else {
            return entryBlocks.joined(separator: "\n\n")
        }
        return (entryBlocks + [extraText]).joined(separator: "\n\n")
    }

    private func sanitize(_ text: String) -> String {
        text
            .replacingOccurrences(of: "\r\n", with: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func normalized(_ entry: ReplyEntry) -> ReplyEntry? {
        switch entry {
        case .quoteReply(let id, let rawQuoted, let rawReply):
            let quoted = sanitize(rawQuoted)
            let reply = sanitize(rawReply)
            guard !quoted.isEmpty, !reply.isEmpty else { return nil }
            return .quoteReply(id: id, quoted: quoted, reply: reply)
        case .emojiReact(let id, let rawQuoted, let rawEmoji):
            let quoted = sanitize(rawQuoted)
            let emoji = sanitize(rawEmoji)
            guard !quoted.isEmpty, !emoji.isEmpty else { return nil }
            return .emojiReact(id: id, quoted: quoted, emoji: emoji)
        case .freeText(let id, let rawText):
            let text = sanitize(rawText)
            guard !text.isEmpty else { return nil }
            return .freeText(id: id, text: text)
        }
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
