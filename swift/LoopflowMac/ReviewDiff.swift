#if os(macOS)
import Foundation
import Observation
import Loopflow

/// A Task's whole comparison read as one review: every file, its hunks, and the
/// changes a reader steps through. Parsed from the patch `lf task diff` returns.
struct ReviewDiff: Equatable, Sendable {
    var files: [ReviewFile]

    /// Every change and moved block in the patch, in reading order, across files.
    var stops: [ReviewStop] {
        files.flatMap { file in file.stops.map { ReviewStop(id: $0, path: file.path) } }
    }

    func file(_ path: String) -> ReviewFile? { files.first { $0.path == path } }

    /// The index of the file a row belongs to: its header (`file:` and its
    /// path) or one of its blocks (its path, `#`, the block).
    func fileIndex(holding id: String) -> Int? {
        files.firstIndex { id == "file:\($0.path)" || id.hasPrefix($0.path + "#") }
    }

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

    /// Drops the `a/` or `b/` prefix and the tab Git writes after a name with a space.
    private static func strip(_ path: Substring) -> String {
        let path = String(path.hasSuffix("\t") ? path.dropLast() : path)
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

    /// A rendered file reads as one change, landing on its top.
    static func rendered(_ path: String) -> ReviewStop { ReviewStop(id: "\(path)#rendered", path: path) }
}

extension [ReviewStop] {
    /// The stop `offset` away from `current`; from nothing, the first or last.
    func stop(after current: String?, offset: Int) -> ReviewStop? {
        guard !isEmpty else { return nil }
        guard let current, let index = firstIndex(where: { $0.id == current }) else {
            return offset >= 0 ? first : last
        }
        let next = index + offset
        return indices.contains(next) ? self[next] : nil
    }
}

/// One Graph step's commits: a run of commits whose subjects start
/// `lf <step>:`. A commit without that prefix is a step of its own.
struct ReviewStep: Equatable, Sendable, Identifiable {
    /// The step's name, or the subject of a commit standing alone.
    let name: String
    /// The first parent of the step's first commit.
    let base: String
    let commits: [TaskCommit]
    /// A step's last commit, which no other step shares.
    let head: String
    var id: String { head }

    /// "implement · 3 commits".
    var title: String { commits.count == 1 ? name : "\(name) · \(commits.count) commits" }

    /// A commit joins the step before it only when it is the same step's and
    /// builds on that step's head; anything between them, a merge included,
    /// keeps them apart.
    static func steps(_ commits: [TaskCommit]) -> [ReviewStep] {
        var steps: [ReviewStep] = []
        for commit in commits {
            let name = Self.name(commit.subject)
            if let name, let last = steps.last, last.head == commit.parent, Self.name(last.commits[0].subject) == name {
                steps[steps.count - 1] = ReviewStep(name: name, base: last.base, commits: last.commits + [commit], head: commit.sha)
            } else {
                steps.append(ReviewStep(name: name ?? commit.subject, base: commit.parent, commits: [commit], head: commit.sha))
            }
        }
        return steps
    }

    /// `implement` from `lf implement: parse the old name`.
    private static func name(_ subject: String) -> String? {
        guard subject.hasPrefix("lf "), let colon = subject.firstIndex(of: ":") else { return nil }
        let name = subject[subject.index(subject.startIndex, offsetBy: 3)..<colon]
        guard !name.isEmpty, name.allSatisfy({ $0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" }) else { return nil }
        return String(name)
    }
}

struct ReviewFile: Equatable, Sendable, Identifiable {
    enum Status: String, Sendable { case modified, added, deleted, renamed, binary }

    let path: String
    let oldPath: String?
    let status: Status
    let blocks: [ReviewBlock]
    let additions: Int
    let deletions: Int
    /// The file's lines as this patch reads them; a file marked viewed stays
    /// viewed until this changes. Equal only within one run of the app.
    let fingerprint: Int
    /// The rows the stream draws for it at most: one per line and hunk.
    let rows: Int
    var id: String { path }

    init(path: String, oldPath: String?, status: Status, blocks: [ReviewBlock]) {
        self.path = path
        self.oldPath = oldPath
        self.status = status
        self.blocks = blocks
        var additions = 0, deletions = 0, rows = 1
        var hasher = Hasher()
        hasher.combine(status)
        for block in blocks { if case .hunk = block { rows += 1 } }
        for line in Self.lines(blocks) {
            if line.kind == .added { additions += 1 } else if line.kind == .removed { deletions += 1 }
            rows += 1
            hasher.combine(line.kind)
            hasher.combine(line.text)
        }
        self.additions = additions
        self.deletions = deletions
        self.rows = rows
        fingerprint = hasher.finalize()
    }

    /// An insertion marks its new lines. A pure deletion marks the surviving line above it.
    var gutterMarks: [Int: ReviewLine.Kind] {
        var result: [Int: ReviewLine.Kind] = [:]
        var preceding = 0
        for block in blocks {
            switch block {
            case .hunk(let header):
                let span = ReviewDiff.hunkSpan(header)
                preceding = span.new - (span.newCount == 0 ? 0 : 1)
            case .context(let line):
                if let number = line.new { preceding = number }
            case .change(let change):
                if change.added.isEmpty { result[max(1, preceding)] = .removed }
                for number in change.added.compactMap(\.new) {
                    result[number] = .added
                    preceding = number
                }
            case .moved(let move):
                if move.end == .source { result[max(1, preceding)] = .removed }
                for number in move.lines.compactMap(\.new) {
                    result[number] = .added
                    preceding = number
                }
            }
        }
        return result
    }

    /// The rendered block containing a working-file line.
    func row(containing number: Int) -> String? {
        for block in blocks {
            switch block {
            case .context(let line) where line.new == number: return block.id(in: path)
            case .change(let change):
                if let offset = change.added.firstIndex(where: { $0.new == number }) {
                    return status == .added ? change.rowID(offset) : change.id
                }
            case .moved(let move) where move.lines.contains(where: { $0.new == number }): return move.id
            default: continue
            }
        }
        return nil
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

    /// The declaration the row `id` sits in: the nearest line above it that
    /// opens a function, type or heading, else the one git names for its hunk.
    func enclosing(_ id: String) -> String? {
        let markdown = ["md", "markdown"].contains((path as NSString).pathExtension.lowercased())
        var found: String?
        for block in blocks {
            if case .hunk(let header) = block {
                let named = ReviewGap(header, after: 0).declaration
                if !named.isEmpty { found = named }
            }
            // Every line of a change is a row. A new file's rows are its lines
            // in order; another change's top row reads the declaration above it.
            if status == .added, case .change(let change) = block {
                for (offset, line) in change.added.enumerated() {
                    if change.rowID(offset) == id { return found }
                    if let declaration = line.declaration(markdown: markdown) { found = declaration }
                }
                continue
            }
            if case .change(let change) = block, change.hasRow(id) { return found }
            if block.id(in: path) == id { return found }
            let lines: [ReviewLine] = switch block {
            case .context(let line): [line]
            case .change(let change): change.added
            case .moved(let move): move.end == .destination ? move.lines : []
            case .hunk: []
            }
            for line in lines {
                if let declaration = line.declaration(markdown: markdown) { found = declaration }
            }
        }
        return nil
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

    /// The line itself when it opens a function or type, or in Markdown a
    /// heading: `pub(crate) async fn read(` or `## Remaining`.
    func declaration(markdown: Bool) -> String? {
        let trimmed = text.trimmingCharacters(in: .whitespaces)
        if markdown { return trimmed.hasPrefix("#") && trimmed.drop { $0 == "#" }.first == " " ? trimmed : nil }
        var words = trimmed.split(separator: " ", maxSplits: 8).map { $0.split(separator: "(").first.map(String.init) ?? "" }
        while let first = words.first, Self.modifiers.contains(first) || first.hasPrefix("@") { words.removeFirst() }
        guard let first = words.first, Self.openers.contains(first), words.count > 1 else { return nil }
        return trimmed
    }

    private static let modifiers: Set<String> = [
        "pub", "public", "private", "fileprivate", "internal", "open", "static", "final", "override", "async",
        "unsafe", "const", "export", "default", "abstract", "nonisolated", "mutating", "extern", "inline", "protected",
    ]
    private static let openers: Set<String> = [
        "func", "fn", "def", "class", "struct", "enum", "protocol", "extension", "actor", "impl", "trait", "mod",
        "interface", "function", "init", "macro_rules!",
    ]

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
    /// stands alone. Found once, where the patch is parsed, since the stream
    /// lists every change's rows each time it is drawn.
    let pairs: [ReviewPair]

    init(id: String, removed: [ReviewLine], added: [ReviewLine]) {
        self.id = id
        self.removed = removed
        self.added = added
        pairs = ReviewPair.align(removed, added)
    }

    /// Each of a change's lines is a row of its own; the first carries the change's id.
    func rowID(_ offset: Int) -> String { offset == 0 ? id : "\(id)+\(offset)" }

    /// Whether `id` names one of this change's rows.
    func hasRow(_ id: String) -> Bool { id == self.id || id.hasPrefix(self.id + "+") }
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
        guard let words = ReviewWords.edit(old, new) else { return nil }
        func side(_ dropped: ReviewWords.Kind) -> [ReviewSegment] {
            var segments: [ReviewSegment] = []
            for word in words where word.kind != dropped {
                let changed = word.kind != .same
                if let last = segments.last, last.changed == changed {
                    segments[segments.count - 1] = ReviewSegment(text: last.text + word.text, changed: changed)
                } else {
                    segments.append(ReviewSegment(text: word.text, changed: changed))
                }
            }
            return segments
        }
        return (side(.added), side(.removed))
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

/// One edit read in order: the words both sides share, the words removed
/// and the words added in their place, as a redline shows them.
struct ReviewWords: Equatable, Sendable {
    enum Kind: Equatable, Sendable { case same, removed, added }
    let text: String
    let kind: Kind

    /// Nil when the two texts are not one edit: they share less than half
    /// their tokens, or are too long to compare word by word.
    static func edit(_ old: String, _ new: String) -> [ReviewWords]? {
        let a = ReviewSegment.tokens(old), b = ReviewSegment.tokens(new)
        // Bound the quadratic table; very long lines keep line-level color.
        guard a.count * b.count <= 160_000, let table = LongestCommon.table(a, b) else { return nil }
        var words: [ReviewWords] = []
        // Between shared words, what was removed reads before what replaced it.
        var removed = "", added = ""
        func flush() {
            if !removed.isEmpty { words.append(ReviewWords(text: removed, kind: .removed)) }
            if !added.isEmpty { words.append(ReviewWords(text: added, kind: .added)) }
            removed = ""; added = ""
        }
        var i = 0, j = 0, shared = 0
        while i < a.count || j < b.count {
            if i < a.count, j < b.count, a[i] == b[j] {
                if !a[i].allSatisfy(\.isWhitespace) { shared += 1 }
                flush()
                if let last = words.last, last.kind == .same {
                    words[words.count - 1] = ReviewWords(text: last.text + a[i], kind: .same)
                } else {
                    words.append(ReviewWords(text: a[i], kind: .same))
                }
                i += 1; j += 1
            } else if j < b.count, i == a.count || table[i][j + 1] >= table[i + 1][j] {
                added += b[j]; j += 1
            } else {
                removed += a[i]; i += 1
            }
        }
        flush()
        let meaningful = { (tokens: [String]) in tokens.filter { !$0.allSatisfy(\.isWhitespace) }.count }
        guard shared > 0, shared * 2 >= max(meaningful(a), meaningful(b)) else { return nil }
        return words
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
enum LongestCommon {
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

/// A line range keeps the source it was picked from, even after switching comparisons.
struct ReviewSelection: Equatable, Sendable {
    enum Side: String, Sendable { case old = "before", new = "after" }
    enum Source: Equatable, Sendable {
        case file(draft: Bool)
        case diff(side: Side, revision: String)

        var label: String {
            switch self {
            case .file(let draft): draft ? "unsaved draft" : "working file"
            case .diff(let side, let revision): "\(side.rawValue), \(revision)"
            }
        }

        var isFile: Bool { if case .file = self { true } else { false } }
        var revision: String? { if case .diff(_, let revision) = self { revision } else { nil } }
    }
    let path: String
    let source: Source
    let first: Int
    let last: Int

    func extended(to line: Int) -> Self {
        Self(path: path, source: source, first: min(first, line), last: max(last, line))
    }

    var range: String { first == last ? "L\(first)" : "L\(first)-\(last)" }

    var reference: String { "`\(path):\(range)` (\(source.label)) " }

    func numbered(in file: ReviewFile) -> [Int: String] {
        guard case .diff(let side, _) = source else { return [:] }
        var lines: [Int: String] = [:]
        for line in file.lines {
            if let number = side == .old ? line.old : line.new { lines[number] = line.text }
        }
        return lines
    }

    func excerpt(in file: ReviewFile) -> [String] {
        let lines = numbered(in: file)
        return (first...last).compactMap { lines[$0] }
    }

    static func side(_ side: Side, of file: ReviewFile) -> String {
        file.lines.filter { side == .new ? $0.kind != .removed : $0.kind == .removed }.map(\.text).joined(separator: "\n")
    }
}

struct ReviewComment: Equatable, Sendable, Identifiable {
    enum Anchor: Equatable, Sendable {
        case lines(ReviewSelection)
        case page(path: String, reference: String)
    }

    let id = UUID()
    let target: Anchor
    let text: String
    /// Original line text for `.lines` comments, used to re-anchor across edits; empty for `.page`.
    let anchor: [String]
    private var start: Int?

    var detached: Bool {
        guard case .lines = target else { return false }
        return start == nil
    }

    var path: String {
        switch target {
        case .lines(let selection): selection.path
        case .page(let path, _): path
        }
    }

    func isPage(_ path: String) -> Bool {
        if case .page(let itemPath, _) = target { itemPath == path } else { false }
    }

    /// The original line selection for a `.lines` comment; nil for `.page`.
    var original: ReviewSelection? {
        guard case .lines(let selection) = target else { return nil }
        return selection
    }

    /// The current (possibly re-anchored) line selection for a `.lines` comment; nil for `.page`.
    var selection: ReviewSelection? {
        guard case .lines(let original) = target else { return nil }
        guard let start else { return original }
        return ReviewSelection(path: original.path, source: original.source,
                               first: start, last: start + original.last - original.first)
    }

    init(selection: ReviewSelection, text: String, anchor: [String]) {
        target = .lines(selection)
        self.text = text
        self.anchor = anchor
        start = selection.first
    }

    init(pagePath: String, reference: String, text: String = "") {
        target = .page(path: pagePath, reference: reference)
        self.text = text
        anchor = []
        start = nil
    }

    func anchored(in lines: [Int: String]) -> Self {
        guard case .lines(let original) = target else { return self }
        var next = self
        next.start = nil
        guard let first = anchor.first, anchor.count == original.last - original.first + 1 else { return next }
        for (number, text) in lines where text == first {
            if anchor.enumerated().allSatisfy({ lines[number + $0.offset] == $0.element }) {
                guard next.start == nil else { next.start = nil; return next }
                next.start = number
            }
        }
        return next
    }

    static func oneLine(_ text: String) -> String {
        let space = UnicodeScalar(" ")
        let cleaned = String(String.UnicodeScalarView(text.unicodeScalars.map { scalar in
            (scalar.value < 0x20 || scalar.value == 0x7f) ? space : scalar
        }))
        return cleaned.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }

    static func message(_ comments: [ReviewComment], receipts: [String] = []) -> String {
        let body = comments.enumerated().map { index, comment in
            let prefix = comments.count > 1 ? "(\(index + 1)) " : ""
            switch comment.target {
            case .lines:
                let changed = comment.detached ? "· lines since changed " : ""
                let words = oneLine(comment.text)
                let reference = comment.selection?.reference ?? ""
                return prefix + reference + changed + words
            case .page(_, let reference):
                return prefix + oneLine(reference)
            }
        }.joined(separator: " ")
        let suffix = receipts.isEmpty ? "" : " [already ran: \(receipts.map(oneLine).joined(separator: "; "))]"
        return body + suffix + " "
    }
}

@MainActor @Observable
final class ReviewComments {
    struct PageKey: Hashable, Sendable {
        let path: String
        let buildID: String
    }

    struct PageItem: Equatable, Sendable {
        let reference: String
        let text: String
    }

    var items: [ReviewComment] = []
    var selection: ReviewSelection? { didSet { if oldValue != selection { draft = "" } } }
    var draft = ""
    /// Saved opaque runtime state per rendered review page build. Swift owns this
    /// so Desktop pages never touch `localStorage` and cannot resurrect sent notes.
    private var pageStates: [PageKey: JSONValue] = [:]
    /// Incremented after `insert(into:)` succeeds so a mounted `TaskHTMLView` clears itself.
    private(set) var pageClearGeneration = 0

    func select(path: String, source: ReviewSelection.Source, line: Int, extend: Bool) {
        if extend, let current = selection, current.path == path, current.source == source {
            selection = current.extended(to: line)
        } else {
            selection = ReviewSelection(path: path, source: source, first: line, last: line)
        }
    }

    func add(anchor: [String]) {
        guard let selection, !draft.allSatisfy(\.isWhitespace) else { return }
        items.append(ReviewComment(selection: selection, text: draft, anchor: anchor))
        self.selection = nil
    }

    /// Answer a review page's `ready` handshake. Returns saved state for `(path, buildID)`,
    /// or `nil` when absent, cleared after send, or rebuilt with a new `buildID` (clearing
    /// any stale state and items left over from an earlier build of `path`).
    func pageReady(path: String, buildID: String) -> JSONValue? {
        if let saved = pageStates[PageKey(path: path, buildID: buildID)] { return saved }
        if removeStalePageStates(for: path, keeping: buildID) {
            items.removeAll { $0.isPage(path) }
        }
        return nil
    }

    /// Store `state` for `(path, buildID)` and replace `path`'s `.page` entries in `items`.
    func syncPage(path: String, buildID: String, state: JSONValue, items pageItems: [PageItem]) {
        _ = removeStalePageStates(for: path, keeping: buildID)
        pageStates[PageKey(path: path, buildID: buildID)] = state
        let firstIndex = items.firstIndex { $0.isPage(path) }
        items.removeAll { $0.isPage(path) }
        let replacements = pageItems.map { ReviewComment(pagePath: path, reference: $0.reference, text: $0.text) }
        if let firstIndex, firstIndex <= items.count {
            items.insert(contentsOf: replacements, at: firstIndex)
        } else {
            items.append(contentsOf: replacements)
        }
    }

    @discardableResult
    private func removeStalePageStates(for path: String, keeping buildID: String) -> Bool {
        let staleKeys = pageStates.keys.filter { $0.path == path && $0.buildID != buildID }
        for stale in staleKeys { pageStates.removeValue(forKey: stale) }
        return !staleKeys.isEmpty
    }

    func reconcile(_ document: TaskFileDocument) {
        let indices = items.indices.filter {
            guard let original = items[$0].original else { return false }
            return original.path == document.path && original.source.isFile
        }
        guard !indices.isEmpty else { return }
        let lines = document.text.utf8.split(separator: 10, omittingEmptySubsequences: false).enumerated().map {
            ($0.offset + 1, String(decoding: $0.element.dropLast($0.element.last == 13 ? 1 : 0), as: UTF8.self))
        }
        let numbered = Dictionary(uniqueKeysWithValues: lines)
        for index in indices { items[index] = items[index].anchored(in: numbered) }
    }

    func reconcile(path: String, revision: String, file: ReviewFile?) {
        for index in items.indices {
            guard let selection = items[index].original,
                  selection.path == path, selection.source.revision == revision else { continue }
            items[index] = items[index].anchored(in: file.map { selection.numbered(in: $0) } ?? [:])
        }
    }

    func insert(into conversation: (String) -> Bool) {
        guard !items.isEmpty else { return }
        guard conversation(ReviewComment.message(items, receipts: pageReceipts())) else { return }
        items.removeAll()
        pageStates.removeAll()
        pageClearGeneration += 1
    }

    private func pageReceipts() -> [String] {
        var ran: [String] = []
        for key in pageStates.keys.sorted(by: { $0.path < $1.path }) {
            guard case .object(let fields) = pageStates[key],
                  case .array(let receipts) = fields["receipts"] else { continue }
            for entry in receipts {
                guard case .object(let receipt) = entry,
                      case .string(let kind) = receipt["kind"] else { continue }
                if case .string(let summary) = receipt["summary"], !summary.isEmpty {
                    ran.append("\(kind) → \(summary)")
                } else {
                    ran.append(kind)
                }
            }
        }
        return ran
    }
}
#endif
