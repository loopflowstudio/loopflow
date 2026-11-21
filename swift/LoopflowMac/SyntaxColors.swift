#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// Syntax colors for the Diff and the file editor: comments, strings, numbers
/// and keywords, read one line at a time. A language is chosen by the file's
/// name; an unknown one gets no color. Only a block comment carries from one
/// line to the next, and only where the caller reads the lines in order.
struct SyntaxLanguage: Sendable {
    let keywords: Set<String>
    let lineComments: [[unichar]]
    let block: (open: [unichar], close: [unichar])?
    let quotes: Set<unichar>

    init(_ keywords: String, line: [String] = ["//"], block: (String, String)? = ("/*", "*/"), quotes: String = "\"'") {
        self.keywords = Set(keywords.split(separator: " ").map(String.init))
        lineComments = line.map { Array($0.utf16) }
        self.block = block.map { (Array($0.0.utf16), Array($0.1.utf16)) }
        self.quotes = Set(quotes.utf16)
    }

    enum Kind: Equatable, Sendable { case comment, string, number, keyword }

    struct Span: Equatable, Sendable {
        /// UTF-16 offsets within the line.
        let range: NSRange
        let kind: Kind
    }

    static func named(_ path: String) -> SyntaxLanguage? {
        let name = (path as NSString).lastPathComponent.lowercased()
        return byExtension[(name as NSString).pathExtension] ?? byName[name]
    }

    /// The colored spans of one line. `inBlock` says whether the line starts
    /// inside a block comment and is left saying whether it ends in one.
    func spans(_ line: String, inBlock: inout Bool) -> [Span] {
        let text = Array(line.utf16)
        var spans: [Span] = []
        var index = 0
        func starts(_ word: [unichar], at start: Int) -> Bool {
            start + word.count <= text.count && !word.isEmpty && Array(text[start..<start + word.count]) == word
        }
        func find(_ word: [unichar], from start: Int) -> Int? {
            guard start <= text.count - word.count else { return nil }
            return (start...text.count - word.count).first { starts(word, at: $0) }
        }
        func add(_ kind: Kind, from start: Int, to end: Int) {
            if end > start { spans.append(Span(range: NSRange(location: start, length: end - start), kind: kind)) }
        }
        while index < text.count {
            if inBlock, let block {
                let end = find(block.close, from: index).map { $0 + block.close.count }
                add(.comment, from: index, to: end ?? text.count)
                inBlock = end == nil
                index = end ?? text.count
                continue
            }
            let unit = text[index]
            if let comment = lineComments.first(where: { starts($0, at: index) }),
               // `#` opens a comment only at the start of a word: `$#` and `a#b` are not one.
               comment != Self.hash || index == 0 || Self.isSpace(text[index - 1]) {
                add(.comment, from: index, to: text.count)
                break
            }
            if let block, starts(block.open, at: index) {
                let end = find(block.close, from: index + block.open.count).map { $0 + block.close.count }
                add(.comment, from: index, to: end ?? text.count)
                inBlock = end == nil
                index = end ?? text.count
            } else if quotes.contains(unit) {
                var end = index + 1
                while end < text.count, text[end] != unit { end += text[end] == Self.backslash ? 2 : 1 }
                end = min(end + 1, text.count)
                add(.string, from: index, to: end)
                index = end
            } else if Self.isDigit(unit) {
                var end = index + 1
                while end < text.count, Self.isWord(text[end]) || text[end] == Self.dot { end += 1 }
                add(.number, from: index, to: end)
                index = end
            } else if Self.isWord(unit) {
                var end = index + 1
                while end < text.count, Self.isWord(text[end]) { end += 1 }
                if keywords.contains(String(utf16CodeUnits: Array(text[index..<end]), count: end - index)) {
                    add(.keyword, from: index, to: end)
                }
                index = end
            } else {
                index += 1
            }
        }
        return spans
    }

    private static let hash: [unichar] = Array("#".utf16)
    private static let backslash = "\\".utf16.first!
    private static let dot = ".".utf16.first!

    private static func isSpace(_ unit: unichar) -> Bool { unit == 32 || unit == 9 }
    private static func isDigit(_ unit: unichar) -> Bool { unit >= 48 && unit <= 57 }
    private static func isWord(_ unit: unichar) -> Bool {
        isDigit(unit) || unit == 95 || (unit >= 65 && unit <= 90) || (unit >= 97 && unit <= 122) || unit > 127
    }

    // MARK: Languages

    private static let cFamily = "if else for while do switch case default break continue return goto struct union enum typedef static const void int char long short unsigned signed float double sizeof extern inline volatile true false NULL nullptr class public private protected virtual override namespace using template typename new delete this try catch throw auto bool"
    private static let script = SyntaxLanguage(
        "if else for while do switch case default break continue return function const let var class extends new delete this super import export from as async await try catch finally throw typeof instanceof in of void yield static get set true false null undefined interface type enum implements public private protected readonly declare namespace abstract",
        quotes: "\"'`")
    private static let hashed = SyntaxLanguage("true false null yes no on off", line: ["#"], block: nil)
    private static let shell = SyntaxLanguage(
        "if then else elif fi for while until do done case esac in function return exit local export readonly set unset shift break continue",
        line: ["#"], block: nil)
    private static let c = SyntaxLanguage(cFamily)

    private static let byName: [String: SyntaxLanguage] = [
        "dockerfile": hashed, "makefile": hashed, "justfile": hashed,
    ]

    private static let byExtension: [String: SyntaxLanguage] = [
        "swift": SyntaxLanguage(
            "import let var func struct class enum protocol extension actor init deinit if else guard switch case default for while repeat in return break continue throw throws rethrows try catch do defer async await static final private fileprivate internal public open mutating nonisolated override lazy weak unowned some any where as is nil true false self Self super typealias associatedtype subscript inout indirect",
            quotes: "\""),
        // `'` opens a lifetime far more often than a character.
        "rs": SyntaxLanguage(
            "fn let mut const static struct enum trait impl mod use pub crate super self Self as where if else match for while loop in return break continue async await move dyn ref type unsafe extern true false",
            quotes: "\""),
        "py": SyntaxLanguage(
            "def class if elif else for while in not and or is return yield import from as try except finally raise with lambda pass break continue global nonlocal async await assert del None True False self",
            line: ["#"], block: nil),
        "rb": SyntaxLanguage(
            "def class module if elsif else unless case when while until for in do end return yield begin rescue ensure raise require include extend self nil true false and or not then super",
            line: ["#"], block: nil),
        "go": SyntaxLanguage(
            "package import func var const type struct interface map chan if else switch case default for range return break continue go defer select fallthrough goto nil true false",
            quotes: "\"'`"),
        "java": SyntaxLanguage(cFamily + " package import extends implements interface final abstract synchronized throws instanceof super boolean byte null"),
        "kt": SyntaxLanguage(cFamily + " package import fun val var object interface data sealed when is in as null companion override open internal suspend"),
        "sql": SyntaxLanguage(
            "select from where and or not null insert into values update set delete create table index view trigger drop alter add column primary key foreign references join left right inner outer on group by order having limit offset as distinct union all begin commit rollback if exists default unique check integer text real blob SELECT FROM WHERE AND OR NOT NULL INSERT INTO VALUES UPDATE SET DELETE CREATE TABLE INDEX VIEW TRIGGER DROP ALTER ADD COLUMN PRIMARY KEY FOREIGN REFERENCES JOIN LEFT RIGHT INNER OUTER ON GROUP BY ORDER HAVING LIMIT OFFSET AS DISTINCT UNION ALL BEGIN COMMIT ROLLBACK IF EXISTS DEFAULT UNIQUE CHECK INTEGER TEXT REAL BLOB WHEN THEN END CASE ELSE",
            line: ["--"], quotes: "'"),
        "css": SyntaxLanguage("important inherit initial unset none auto", line: []),
        "json": SyntaxLanguage("true false null", line: [], block: nil, quotes: "\""),
        "c": c, "h": c, "m": c, "mm": c, "cc": c, "cpp": c, "hpp": c, "cs": c,
        "js": script, "jsx": script, "mjs": script, "cjs": script, "ts": script, "tsx": script,
        "sh": shell, "bash": shell, "zsh": shell,
        "yaml": hashed, "yml": hashed, "toml": hashed, "ini": hashed, "cfg": hashed, "conf": hashed,
    ]
}

extension SyntaxLanguage.Kind {
    /// Inks every theme keeps readable on its surfaces. Red and green stay
    /// with removed and added lines.
    func color(in palette: LoopflowPalette) -> Color {
        switch self {
        case .comment: palette.textTertiary
        case .string: palette.availableTone.ink
        case .number: palette.warningTone.ink
        case .keyword: palette.workingTone.ink
        }
    }
}

extension AttributedString {
    /// Color one line's syntax; attributes already there (changed-word fills) stay.
    mutating func colorSyntax(of line: String, path: String, palette: LoopflowPalette) {
        guard let language = SyntaxLanguage.named(path) else { return }
        var inBlock = false
        for span in language.spans(line, inBlock: &inBlock) {
            guard let range = Range(span.range, in: self) else { continue }
            self[range].foregroundColor = span.kind.color(in: palette)
        }
    }
}

extension NSTextStorage {
    /// Files past this many UTF-16 units keep one color; coloring runs on every edit.
    static let syntaxLimit = 200_000

    /// Recolor the whole buffer. Only colors change, never characters.
    func colorSyntax(path: String, palette: LoopflowPalette) {
        guard let language = SyntaxLanguage.named(path), length <= Self.syntaxLimit else { return }
        beginEditing()
        addAttribute(.foregroundColor, value: NSColor(palette.text), range: NSRange(location: 0, length: length))
        var inBlock = false
        var location = 0
        for line in string.components(separatedBy: "\n") {
            for span in language.spans(line, inBlock: &inBlock) {
                addAttribute(.foregroundColor, value: NSColor(span.kind.color(in: palette)),
                             range: NSRange(location: location + span.range.location, length: span.range.length))
            }
            location += line.utf16.count + 1
        }
        endEditing()
    }
}
#endif
