import Foundation

/// Two versions of a Markdown document compared block by block, the way the
/// Diff compares lines: blocks of the same kind and text anchor the two in
/// order; between anchors the old and new blocks pair in turn, and a pair that
/// reads as one edit is one changed block with its words marked. Anything
/// else stands alone, removed or added.
struct MarkdownRedline: Equatable {
    typealias Block = MarkdownBlocks.Block

    enum Entry: Equatable {
        case same(Block)
        case changed(old: Block, new: Block, words: [ReviewWords])
        case removed(Block)
        case added(Block)
    }

    let entries: [Entry]

    init(old: String, new: String) {
        entries = Self.compare(MarkdownBlocks.blocks(old), MarkdownBlocks.blocks(new))
    }

    static func compare(_ old: [Block], _ new: [Block]) -> [Entry] {
        let a = old.map(Key.init), b = new.map(Key.init)
        var entries: [Entry] = []
        var i = 0, j = 0
        // Shared blocks at either end anchor without the quadratic table.
        while i < a.count, j < b.count, a[i] == b[j] { entries.append(.same(new[j])); i += 1; j += 1 }
        var endA = a.count, endB = b.count
        while endA > i, endB > j, a[endA - 1] == b[endB - 1] { endA -= 1; endB -= 1 }
        let middleA = Array(a[i..<endA]), middleB = Array(b[j..<endB])
        // A middle too long to compare pairs in order, as a run between anchors does.
        let table = middleA.count * middleB.count <= 250_000 ? LongestCommon.table(middleA, middleB) : nil
        var runA: [Block] = [], runB: [Block] = []
        func flush() {
            entries += pair(runA, runB)
            runA = []; runB = []
        }
        var x = 0, y = 0
        while x < middleA.count || y < middleB.count {
            if let table, x < middleA.count, y < middleB.count, middleA[x] == middleB[y] {
                flush()
                entries.append(.same(new[j + y])); x += 1; y += 1
            } else if y < middleB.count, x == middleA.count || (table.map { $0[x][y + 1] >= $0[x + 1][y] } ?? false) {
                runB.append(new[j + y]); y += 1
            } else {
                runA.append(old[i + x]); x += 1
            }
        }
        flush()
        entries += new[endB...].map(Entry.same)
        return entries
    }

    /// The blocks between two anchors, paired in order as removed and added
    /// lines are.
    private static func pair(_ old: [Block], _ new: [Block]) -> [Entry] {
        (0..<max(old.count, new.count)).flatMap { index -> [Entry] in
            let before = old.indices.contains(index) ? old[index] : nil
            let after = new.indices.contains(index) ? new[index] : nil
            if let before, let after, before.kind == after.kind, before.kind.readsAsWords,
               let words = ReviewWords.edit(String(before.text.characters), String(after.text.characters)) {
                return [.changed(old: before, new: after, words: words)]
            }
            return [before.map(Entry.removed), after.map(Entry.added)].compactMap { $0 }
        }
    }

    /// The pieces of a changed block's text in order, each keeping its own
    /// side's inline attributes (emphasis, code, links). `kinds` picks what
    /// to show: all three for a redline, `.same` and one side for a column.
    static func pieces(_ words: [ReviewWords], old: AttributedString, new: AttributedString,
                       showing kinds: Set<ReviewWords.Kind>) -> [(text: AttributedString, kind: ReviewWords.Kind)] {
        var oldAt = old.startIndex, newAt = new.startIndex
        func take(_ count: Int, from text: AttributedString, at index: inout AttributedString.Index) -> AttributedString {
            let end = text.characters.index(index, offsetBy: count, limitedBy: text.endIndex) ?? text.endIndex
            defer { index = end }
            return AttributedString(text[index..<end])
        }
        var pieces: [(text: AttributedString, kind: ReviewWords.Kind)] = []
        for word in words {
            let count = word.text.count
            let piece: AttributedString
            switch word.kind {
            case .same:
                _ = take(count, from: old, at: &oldAt)
                piece = take(count, from: new, at: &newAt)
            case .removed: piece = take(count, from: old, at: &oldAt)
            case .added: piece = take(count, from: new, at: &newAt)
            }
            if kinds.contains(word.kind) { pieces.append((piece, word.kind)) }
        }
        return pieces
    }

    /// What anchors a block: its kind, nesting and text. A list marker is
    /// left out, since inserting an item renumbers the ones after it.
    private struct Key: Equatable {
        let kind: Block.Kind
        let depth: Int
        let cells: [String]
        init(_ block: Block) {
            kind = block.kind
            depth = block.depth
            cells = block.cells.map { String($0.characters) }
        }
    }
}

extension MarkdownBlocks.Block.Kind {
    /// Prose and code mark their changed words; a table row or rule that
    /// changed reads as the old one removed and the new one added.
    var readsAsWords: Bool {
        switch self {
        case .heading, .paragraph, .quote, .code: true
        case .rule, .row: false
        }
    }
}
