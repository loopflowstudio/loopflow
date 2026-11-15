#if os(macOS)
import Foundation

/// A Task's whole comparison read as one review: every file, its hunks, and the
/// changes a reader steps through. Parsed from the patch `lf task diff` returns.
struct ReviewDiff: Equatable, Sendable {
    var files: [ReviewFile]

    static let empty = ReviewDiff(files: [])

    /// Every change and moved block in reading order, across files.
    var stops: [ReviewStop] {
        files.flatMap { file in
            file.blocks.compactMap { block in
                switch block {
                case .change(let change): ReviewStop(id: change.id, path: file.path)
                case .moved(let move): ReviewStop(id: move.id, path: file.path)
                case .hunk, .context: nil
                }
            }
        }
    }

    /// The stop `offset` away from `current`; from nothing, the first or last.
    func stop(after current: String?, offset: Int) -> ReviewStop? {
        let stops = self.stops
        guard !stops.isEmpty else { return nil }
        guard let current, let index = stops.firstIndex(where: { $0.id == current }) else {
            return offset >= 0 ? stops.first : stops.last
        }
        let next = index + offset
        return stops.indices.contains(next) ? stops[next] : nil
    }

    func file(_ path: String) -> ReviewFile? { files.first { $0.path == path } }

    // MARK: Parsing

    static func parse(_ patch: String) -> ReviewDiff {
        var files: [ReviewFile] = []
        var current: FileBuilder?
        var old = 0, new = 0
        for raw in patch.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = String(raw.hasSuffix("\r") ? raw.dropLast() : raw)
            if line.hasPrefix("diff --git ") {
                if let current { files.append(current.build()) }
                current = FileBuilder(header: line)
                continue
            }
            guard var file = current else { continue }
            defer { current = file }
            if !file.inHunk {
                if line.hasPrefix("new file mode") { file.status = .added }
                else if line.hasPrefix("deleted file mode") { file.status = .deleted }
                else if line.hasPrefix("rename from ") { file.oldPath = String(line.dropFirst("rename from ".count)); file.status = .renamed }
                else if line.hasPrefix("rename to ") { file.path = String(line.dropFirst("rename to ".count)) }
                else if line.hasPrefix("Binary files ") || line.hasPrefix("GIT binary patch") { file.binary = true }
                else if line.hasPrefix("+++ "), line != "+++ /dev/null" { file.path = Self.strip(line.dropFirst(4)) }
                else if line.hasPrefix("--- "), line != "--- /dev/null", file.oldPath == nil {
                    let path = Self.strip(line.dropFirst(4))
                    if path != file.path { file.oldPath = path }
                }
            }
            if line.hasPrefix("@@") {
                file.inHunk = true
                let (o, n) = Self.hunkStarts(line)
                old = o; new = n
                file.lines.append(.hunk(line))
                continue
            }
            guard file.inHunk else { continue }
            if line.hasPrefix("+") {
                file.lines.append(.line(ReviewLine(kind: .added, text: String(line.dropFirst()), old: nil, new: new)))
                new += 1
            } else if line.hasPrefix("-") {
                file.lines.append(.line(ReviewLine(kind: .removed, text: String(line.dropFirst()), old: old, new: nil)))
                old += 1
            } else if line.hasPrefix(" ") {
                file.lines.append(.line(ReviewLine(kind: .context, text: String(line.dropFirst()), old: old, new: new)))
                old += 1; new += 1
            }
            // `\ No newline at end of file` and the final empty split carry no line.
        }
        if let current { files.append(current.build()) }
        return ReviewDiff(files: files)
    }

    private static func strip(_ path: Substring) -> String {
        let path = String(path)
        if path.hasPrefix("a/") || path.hasPrefix("b/") { return String(path.dropFirst(2)) }
        return path
    }

    /// `@@ -12,7 +12,9 @@` → (12, 12).
    private static func hunkStarts(_ header: String) -> (Int, Int) {
        let parts = header.split(separator: " ")
        func start(_ prefix: Character) -> Int {
            guard let part = parts.first(where: { $0.first == prefix }) else { return 0 }
            return Int(part.dropFirst().split(separator: ",").first ?? "") ?? 0
        }
        return (start("-"), start("+"))
    }

    private struct FileBuilder {
        enum Item { case hunk(String), line(ReviewLine) }
        var path: String
        var oldPath: String?
        var status = ReviewFile.Status.modified
        var binary = false
        var inHunk = false
        var lines: [Item] = []

        init(header: String) {
            // `diff --git a/x b/x`; `+++`/`rename to` refine it.
            let paths = header.dropFirst("diff --git ".count)
            if let split = paths.range(of: " b/") {
                path = String(paths[split.upperBound...])
            } else {
                path = String(paths)
            }
        }

        func build() -> ReviewFile {
            var blocks: [ReviewBlock] = []
            var removed: [ReviewLine] = [], added: [ReviewLine] = []
            func flush() {
                guard !removed.isEmpty || !added.isEmpty else { return }
                let anchor = removed.first.map { "o\($0.old ?? 0)" } ?? "n\(added.first?.new ?? 0)"
                blocks.append(.change(ReviewChange(id: "\(path)#\(anchor)", removed: removed, added: added)))
                removed = []; added = []
            }
            for item in lines {
                switch item {
                case .hunk(let header):
                    flush()
                    blocks.append(.hunk(header))
                case .line(let line):
                    switch line.kind {
                    case .removed:
                        if !added.isEmpty { flush() }
                        removed.append(line)
                    case .added: added.append(line)
                    case .context:
                        flush()
                        blocks.append(.context(line))
                    }
                }
            }
            flush()
            return ReviewFile(path: path, oldPath: oldPath, status: binary ? .binary : status,
                              blocks: ReviewMove.detect(in: blocks, path: path))
        }
    }
}

struct ReviewStop: Equatable, Sendable {
    let id: String
    let path: String
}

struct ReviewFile: Equatable, Sendable, Identifiable {
    enum Status: String, Sendable { case modified, added, deleted, renamed, binary }

    let path: String
    let oldPath: String?
    let status: Status
    let blocks: [ReviewBlock]
    var id: String { path }

    var additions: Int { lines.filter { $0.kind == .added }.count }
    var deletions: Int { lines.filter { $0.kind == .removed }.count }

    /// Every line the patch shows for this file, context included.
    var lines: [ReviewLine] {
        blocks.flatMap { block -> [ReviewLine] in
            switch block {
            case .context(let line): [line]
            case .change(let change): change.removed + change.added
            case .moved(let move): move.lines
            case .hunk: []
            }
        }
    }
}

enum ReviewBlock: Equatable, Sendable {
    case hunk(String)
    case context(ReviewLine)
    case change(ReviewChange)
    case moved(ReviewMove)
}

struct ReviewLine: Equatable, Sendable {
    enum Kind: Sendable { case context, added, removed }
    let kind: Kind
    let text: String
    let old: Int?
    let new: Int?
}

/// One run of removed lines followed by the lines that replaced them.
struct ReviewChange: Equatable, Sendable {
    let id: String
    let removed: [ReviewLine]
    let added: [ReviewLine]

    /// Removed and added lines side by side; a modified line pairs with its
    /// replacement and carries word-level emphasis, a pure addition or removal
    /// stands alone.
    var pairs: [ReviewPair] { ReviewPair.align(removed, added) }
}

struct ReviewPair: Equatable, Sendable {
    let removed: ReviewLine?
    let added: ReviewLine?
    /// Word-level segments for each side when the two lines are one edit.
    let removedSegments: [ReviewSegment]?
    let addedSegments: [ReviewSegment]?

    static func align(_ removed: [ReviewLine], _ added: [ReviewLine]) -> [ReviewPair] {
        (0..<max(removed.count, added.count)).map { index in
            let old = removed.indices.contains(index) ? removed[index] : nil
            let new = added.indices.contains(index) ? added[index] : nil
            guard let old, let new, let (left, right) = ReviewSegment.edit(old.text, new.text) else {
                return ReviewPair(removed: old, added: new, removedSegments: nil, addedSegments: nil)
            }
            return ReviewPair(removed: old, added: new, removedSegments: left, addedSegments: right)
        }
    }
}

/// A run of text and whether it is the part that changed.
struct ReviewSegment: Equatable, Sendable {
    let text: String
    let changed: Bool

    /// Word-level edits between two lines, or nil when they are not one edit:
    /// lines that share less than half their tokens read better as a removal
    /// and an addition.
    static func edit(_ old: String, _ new: String) -> ([ReviewSegment], [ReviewSegment])? {
        let a = tokens(old), b = tokens(new)
        // Bound the quadratic table; very long lines keep line-level color.
        guard a.count * b.count <= 160_000, let table = LongestCommon.table(a, b) else { return nil }
        var left: [ReviewSegment] = [], right: [ReviewSegment] = []
        var i = 0, j = 0, shared = 0
        func push(_ segments: inout [ReviewSegment], _ text: String, _ changed: Bool) {
            if let last = segments.last, last.changed == changed {
                segments[segments.count - 1] = ReviewSegment(text: last.text + text, changed: changed)
            } else {
                segments.append(ReviewSegment(text: text, changed: changed))
            }
        }
        while i < a.count || j < b.count {
            if i < a.count, j < b.count, a[i] == b[j] {
                if !a[i].allSatisfy(\.isWhitespace) { shared += 1 }
                push(&left, a[i], false); push(&right, b[j], false); i += 1; j += 1
            } else if j < b.count, i == a.count || table[i][j + 1] >= table[i + 1][j] {
                push(&right, b[j], true); j += 1
            } else {
                push(&left, a[i], true); i += 1
            }
        }
        let meaningful = { (tokens: [String]) in tokens.filter { !$0.allSatisfy(\.isWhitespace) }.count }
        guard shared > 0, shared * 2 >= max(meaningful(a), meaningful(b)) else { return nil }
        return (left, right)
    }

    /// Words, whitespace runs and single punctuation marks.
    static func tokens(_ line: String) -> [String] {
        var tokens: [String] = []
        var word = ""
        var space = ""
        for character in line {
            if character.isLetter || character.isNumber || character == "_" {
                if !space.isEmpty { tokens.append(space); space = "" }
                word.append(character)
            } else if character.isWhitespace {
                if !word.isEmpty { tokens.append(word); word = "" }
                space.append(character)
            } else {
                if !word.isEmpty { tokens.append(word); word = "" }
                if !space.isEmpty { tokens.append(space); space = "" }
                tokens.append(String(character))
            }
        }
        if !word.isEmpty { tokens.append(word) }
        if !space.isEmpty { tokens.append(space) }
        return tokens
    }
}

/// Lines removed in one place and added in another place of the same file.
/// The destination reads as one collapsed row; expanding it pairs the old and
/// new lines with their word-level edits. The source points at the destination.
struct ReviewMove: Equatable, Sendable {
    enum End: Sendable { case source, destination }

    let id: String
    let end: End
    /// The other end's id: the source jumps to its destination.
    let counterpart: String
    /// The block's own lines: removed at the source, added at the destination.
    let lines: [ReviewLine]
    /// The old lines, in order, for the destination's comparison.
    let original: [ReviewLine]
    /// First old line number and first new line number.
    let from: Int
    let to: Int
    /// Lines whose content changed beyond indentation.
    let changedLines: Int

    var pairs: [ReviewPair] { ReviewPair.align(original, lines) }

    /// Shorter runs match too easily: a closing brace or blank line is not a move.
    static let minimumLines = 3

    /// Pair a pure removal with a pure addition elsewhere in the file when most
    /// of their trimmed lines agree in order.
    static func detect(in blocks: [ReviewBlock], path: String) -> [ReviewBlock] {
        var blocks = blocks
        let changes: [(Int, ReviewChange)] = blocks.enumerated().compactMap { index, block in
            if case .change(let change) = block { return (index, change) }
            return nil
        }
        let removals = changes.filter { $0.1.added.isEmpty && significant($0.1.removed) >= minimumLines }
        var additions = changes.filter { $0.1.removed.isEmpty && significant($0.1.added) >= minimumLines }
        for (sourceIndex, source) in removals {
            let old = source.removed
            guard let match = additions.enumerated().max(by: { overlap(old, $0.element.1.added) < overlap(old, $1.element.1.added) }),
                  Double(overlap(old, match.element.1.added)) >= 0.6 * Double(max(old.count, match.element.1.added.count))
            else { continue }
            additions.remove(at: match.offset)
            let (destinationIndex, destination) = match.element
            let new = destination.added
            let changed = ReviewPair.align(old, new).filter {
                $0.removed?.text.trimmingCharacters(in: .whitespaces) != $0.added?.text.trimmingCharacters(in: .whitespaces)
            }.count
            let from = old.first?.old ?? 0, to = new.first?.new ?? 0
            blocks[sourceIndex] = .moved(ReviewMove(id: source.id, end: .source, counterpart: destination.id,
                                                    lines: old, original: old, from: from, to: to, changedLines: changed))
            blocks[destinationIndex] = .moved(ReviewMove(id: destination.id, end: .destination, counterpart: source.id,
                                                         lines: new, original: old, from: from, to: to, changedLines: changed))
        }
        return blocks
    }

    private static func significant(_ lines: [ReviewLine]) -> Int {
        lines.filter { !$0.text.trimmingCharacters(in: .whitespaces).isEmpty }.count
    }

    /// Trimmed lines the two runs share in order.
    private static func overlap(_ a: [ReviewLine], _ b: [ReviewLine]) -> Int {
        guard a.count * b.count <= 250_000 else { return 0 }
        let x = a.map { $0.text.trimmingCharacters(in: .whitespaces) }
        let y = b.map { $0.text.trimmingCharacters(in: .whitespaces) }
        return LongestCommon.table(x, y)?[0][0] ?? 0
    }
}

/// Longest common subsequence lengths: `table[i][j]` covers `a[i...]` and `b[j...]`.
private enum LongestCommon {
    static func table<T: Equatable>(_ a: [T], _ b: [T]) -> [[Int]]? {
        guard !a.isEmpty, !b.isEmpty else { return nil }
        var table = Array(repeating: Array(repeating: 0, count: b.count + 1), count: a.count + 1)
        for i in stride(from: a.count - 1, through: 0, by: -1) {
            for j in stride(from: b.count - 1, through: 0, by: -1) {
                table[i][j] = a[i] == b[j] ? table[i + 1][j + 1] + 1 : max(table[i + 1][j], table[i][j + 1])
            }
        }
        return table
    }
}

/// Lines you picked in the review, on one side of one file.
struct ReviewSelection: Equatable, Sendable {
    enum Side: String, Sendable { case old = "before", new = "after" }
    let path: String
    let side: Side
    let first: Int
    let last: Int

    func extended(to line: Int) -> ReviewSelection {
        ReviewSelection(path: path, side: side, first: min(first, line), last: max(last, line))
    }

    func contains(_ line: ReviewLine, path: String) -> Bool {
        guard path == self.path, let number = side == .old ? line.old : line.new else { return false }
        return (first...last).contains(number)
    }

    var range: String { first == last ? "L\(first)" : "L\(first)-\(last)" }

    /// One line for the conversation draft. Terminal input cannot carry a
    /// newline without submitting, so the code stays in the file and the draft
    /// names exactly where: file, side, lines and the revision compared.
    func reference(revision: String) -> String {
        "`\(path):\(range)` (\(side.rawValue), \(revision)) "
    }

    /// The selected lines as plain text, without patch markers or line numbers.
    func text(in diff: ReviewDiff) -> String {
        guard let file = diff.file(path) else { return "" }
        return file.lines.filter { contains($0, path: path) }
            .sorted { (side == .old ? $0.old : $0.new) ?? 0 < (side == .old ? $1.old : $1.new) ?? 0 }
            .map(\.text).joined(separator: "\n")
    }
}
#endif
