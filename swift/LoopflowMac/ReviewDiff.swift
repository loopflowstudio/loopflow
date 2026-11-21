#if os(macOS)
import Foundation

/// A Task's whole comparison read as one review: every file, its hunks, and the
/// changes a reader steps through. Parsed from the patch `lf task diff` returns.
struct ReviewDiff: Equatable, Sendable {
    var files: [ReviewFile]

    /// Every change and moved block in reading order, across files.
    var stops: [ReviewStop] {
        files.flatMap { file in file.stops.map { ReviewStop(id: $0, path: file.path) } }
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
                let span = Self.hunkSpan(line)
                old = span.old; new = span.new
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

    /// Where a hunk starts on each side and how many lines it holds there:
    /// `@@ -12,7 +12,9 @@` → (12, 7, 12, 9).
    static func hunkSpan(_ header: String) -> (old: Int, oldCount: Int, new: Int, newCount: Int) {
        let parts = header.split(separator: " ")
        func side(_ prefix: Character) -> (Int, Int) {
            guard let part = parts.dropFirst().first(where: { $0.first == prefix }) else { return (0, 0) }
            let numbers = part.dropFirst().split(separator: ",")
            return (Int(numbers.first ?? "") ?? 0, numbers.count > 1 ? Int(numbers[1]) ?? 1 : 1)
        }
        let (old, oldCount) = side("-"), (new, newCount) = side("+")
        return (old, oldCount, new, newCount)
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
                guard let first = removed.first ?? added.first else { return }
                blocks.append(.change(ReviewChange(id: first.id(in: path), removed: removed, added: added)))
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
    let additions: Int
    let deletions: Int
    var id: String { path }

    init(path: String, oldPath: String?, status: Status, blocks: [ReviewBlock]) {
        self.path = path
        self.oldPath = oldPath
        self.status = status
        self.blocks = blocks
        var additions = 0, deletions = 0
        for line in Self.lines(blocks) {
            if line.kind == .added { additions += 1 } else if line.kind == .removed { deletions += 1 }
        }
        self.additions = additions
        self.deletions = deletions
    }

    /// The id of the file's first row under its header.
    var topID: String { blocks.first?.id(in: path) ?? "\(path)#empty" }

    /// The ids of the file's changes and moved blocks, in reading order.
    var stops: [String] { blocks.filter(\.isStop).map { $0.id(in: path) } }

    /// Every line the patch shows for this file, context included.
    var lines: [ReviewLine] { Self.lines(blocks) }

    private static func lines(_ blocks: [ReviewBlock]) -> [ReviewLine] {
        blocks.flatMap { block -> [ReviewLine] in
            switch block {
            case .context(let line): [line]
            case .change(let change): change.removed + change.added
            case .moved(let move): move.lines
            case .hunk: []
            }
        }
    }

    /// What each hunk header skips, by block index: the unchanged lines between
    /// the last line shown above it and its first, and the declaration git names.
    var gaps: [Int: ReviewGap] {
        var gaps: [Int: ReviewGap] = [:]
        var lastNew = 0
        for (index, block) in blocks.enumerated() {
            switch block {
            case .hunk(let header): gaps[index] = ReviewGap(header, after: lastNew)
            case .context(let line): lastNew = max(lastNew, line.new ?? 0)
            case .change(let change): lastNew = max(lastNew, change.added.last?.new ?? 0)
            case .moved(let move): lastNew = max(lastNew, move.lines.compactMap(\.new).max() ?? 0)
            }
        }
        return gaps
    }

    /// The file with skipped lines shown: each hunk header that has lines here
    /// gives way to them, so the hunks on either side read as one run.
    func opening(_ opened: [String: [ReviewLine]]) -> ReviewFile {
        guard !opened.isEmpty else { return self }
        return ReviewFile(path: path, oldPath: oldPath, status: status, blocks: blocks.flatMap { block -> [ReviewBlock] in
            if case .hunk(let header) = block, let lines = opened[header] { return lines.map { .context($0) } }
            return [block]
        })
    }
}

/// The unchanged lines a hunk header stands for.
struct ReviewGap: Equatable, Sendable {
    /// The skipped lines' numbers after the change; empty when nothing is skipped.
    let new: Range<Int>
    /// The first skipped line's number before the change.
    let old: Int
    /// The declaration git names for the hunk, when it names one.
    let declaration: String

    /// `@@ -86,5 +90,5 @@ final class Store {` after new line 40 skips new lines 41–89.
    init(_ header: String, after lastNew: Int) {
        let span = ReviewDiff.hunkSpan(header)
        // A side with no lines names the line before it.
        let start = max(span.newCount == 0 ? span.new + 1 : span.new, lastNew + 1)
        new = (lastNew + 1)..<start
        old = (span.oldCount == 0 ? span.old + 1 : span.old) - new.count
        let parts = header.split(separator: " ", maxSplits: 4, omittingEmptySubsequences: true)
        declaration = parts.count == 5 ? String(parts[4]) : ""
    }

    /// "49 unchanged lines · final class Store {".
    var label: String {
        var pieces: [String] = []
        if !new.isEmpty { pieces.append(new.count == 1 ? "1 unchanged line" : "\(new.count) unchanged lines") }
        if !declaration.isEmpty { pieces.append(declaration) }
        return pieces.joined(separator: " · ")
    }

    /// The skipped lines, read from the file as it stands; nil when the file is
    /// too short to hold them, so it is no longer the file the hunk describes.
    func lines(in content: String) -> [ReviewLine]? {
        let text = content.split(separator: "\n", omittingEmptySubsequences: false)
        guard !new.isEmpty, new.upperBound - 1 <= text.count else { return nil }
        return new.map { number in
            let line = text[number - 1]
            return ReviewLine(kind: .context, text: String(line.hasSuffix("\r") ? line.dropLast() : line),
                              old: old + number - new.lowerBound, new: number)
        }
    }
}

enum ReviewBlock: Equatable, Sendable {
    case hunk(String)
    case context(ReviewLine)
    case change(ReviewChange)
    case moved(ReviewMove)

    /// The block's row in a stream; a change's and a move's are their stops.
    func id(in path: String) -> String {
        switch self {
        case .hunk(let header): "\(path)#\(header)"
        case .context(let line): "\(path)#c\(line.old ?? 0)-\(line.new ?? 0)"
        case .change(let change): change.id
        case .moved(let move): move.id
        }
    }

    var isStop: Bool {
        switch self {
        case .change, .moved: true
        case .hunk, .context: false
        }
    }
}

struct ReviewLine: Equatable, Sendable {
    enum Kind: Sendable { case context, added, removed }
    let kind: Kind
    let text: String
    let old: Int?
    let new: Int?

    /// Past this many UTF-16 units a line is cut until you ask for the rest.
    static let readable = 1_000

    /// The line within its file: its old number when removed, else its new one.
    /// A change takes its first line's.
    func id(in path: String) -> String {
        "\(path)#" + (kind == .removed ? "o\(old ?? 0)" : "n\(new ?? 0)")
    }

    /// The start of a line past `readable`, and how many UTF-16 units follow.
    var cut: (shown: String, more: Int)? {
        let units = text.utf16
        guard units.count > Self.readable else { return nil }
        let end = text.rangeOfComposedCharacterSequence(at: units.index(units.startIndex, offsetBy: Self.readable)).lowerBound
        let shown = String(text[..<end])
        return (shown, units.count - shown.utf16.count)
    }
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

    /// The line's number on this side; nil when the line is not on it.
    func number(of line: ReviewLine) -> Int? { side == .old ? line.old : line.new }

    func contains(_ line: ReviewLine, path: String) -> Bool {
        guard path == self.path, let number = number(of: line) else { return false }
        return (first...last).contains(number)
    }

    var range: String { first == last ? "L\(first)" : "L\(first)-\(last)" }

    /// One line for the conversation's unsent input. Desktop types literal text
    /// only, and a newline there would send, so the code stays in the file and
    /// the line names exactly where: file, side, lines and the revision compared.
    func reference(revision: String) -> String {
        "`\(path):\(range)` (\(side.rawValue), \(revision)) "
    }

    /// The selected lines as plain text, without patch markers or line numbers.
    func text(in file: ReviewFile) -> String {
        let lines = numbered(in: file)
        return (first...last).compactMap { lines[$0] }.joined(separator: "\n")
    }

    /// This side's lines of a file by number.
    func numbered(in file: ReviewFile) -> [Int: String] {
        var lines: [Int: String] = [:]
        for line in file.lines {
            if let number = number(of: line) { lines[number] = line.text }
        }
        return lines
    }
}

/// A comment you left on lines of the Diff. It waits under its lines until you
/// send every comment to the conversation as one message.
struct ReviewComment: Equatable, Sendable, Identifiable {
    let id: UUID
    var selection: ReviewSelection
    let text: String
    /// The commented lines as they read when you wrote it, to find them again.
    let anchor: [String]
    /// The lines are no longer in the Diff as they were; the comment keeps its
    /// last place and is still sent.
    var detached = false

    init(id: UUID = UUID(), selection: ReviewSelection, text: String, in file: ReviewFile) {
        self.id = id
        self.selection = selection
        self.text = text
        let lines = selection.numbered(in: file)
        anchor = (selection.first...selection.last).map { lines[$0] ?? "" }
    }

    /// The comment after the file changed: on the nearest lines that still read
    /// as they did, or detached when none do. Nil is a file that left the Diff.
    func anchored(in file: ReviewFile?) -> ReviewComment {
        var next = self
        guard let file else { next.detached = true; return next }
        let lines = selection.numbered(in: file)
        let starts = lines.keys.filter { start in
            anchor.enumerated().allSatisfy { lines[start + $0.offset] == $0.element }
        }
        guard let start = starts.min(by: { (abs($0 - selection.first), $0) < (abs($1 - selection.first), $1) }) else {
            next.detached = true
            return next
        }
        next.detached = false
        next.selection = ReviewSelection(path: selection.path, side: selection.side,
                                         first: start, last: start + selection.last - selection.first)
        return next
    }

    /// Every comment as one line for the conversation's unsent input, each
    /// naming its file, lines and side.
    static func message(_ comments: [ReviewComment], revision: String) -> String {
        func words(_ comment: ReviewComment) -> String {
            comment.text.split(whereSeparator: \.isWhitespace).joined(separator: " ")
        }
        func place(_ comment: ReviewComment) -> String {
            let selection = comment.selection
            return "`\(selection.path):\(selection.range)` (\(selection.side.rawValue)\(comment.detached ? ", lines since changed" : ""))"
        }
        if comments.count == 1, let comment = comments.first {
            return "\(place(comment)): \(words(comment)) (\(revision)) "
        }
        let parts = comments.enumerated().map { "(\($0.offset + 1)) \(place($0.element)): \(words($0.element))" }
        return "\(comments.count) comments on the diff, \(revision): \(parts.joined(separator: " ")) "
    }
}
#endif
