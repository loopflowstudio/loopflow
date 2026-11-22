#if os(macOS)
import Foundation
import Loopflow
import SwiftTreeSitter
import SwiftUI
import Synchronization
import TreeSitterGrammars

/// A language's tree-sitter grammar and the highlight query that names its
/// pieces, chosen by file name. The parsers come from `TreeSitterGrammars`;
/// the queries are this repository's, under `Grammars/<language>/`.
final class Grammar: Sendable {
    let name: String
    private let language: Language
    private let query: Query
    /// Markdown's inline grammar, run over each `inline` node of the block parse.
    private let inline: Grammar?

    /// A named piece of text, in UTF-16 units.
    struct Highlight: Equatable, Sendable {
        let range: NSRange
        /// The query's capture name: `function.method`, `string.special`.
        let capture: String
        var ink: SyntaxInk? { SyntaxInk(capture: capture) }
    }

    private init(name: String, language: Language, query: Query, inline: Grammar?) {
        self.name = name
        self.language = language
        self.query = query
        self.inline = inline
    }

    /// The grammar for a path, by extension or file name; nil for a language
    /// without one, which reads as plain text.
    static func named(_ path: String) -> Grammar? {
        let file = (path as NSString).lastPathComponent.lowercased()
        guard let name = byExtension[(file as NSString).pathExtension] ?? byName[file] else { return nil }
        return loaded(name)
    }

    /// Every named piece of `text`, ordered for painting: each piece starts at
    /// or after the one before, and a piece inside another follows it, so
    /// painting in order leaves the innermost color on top. Pieces without an
    /// ink are left out, so a broad capture such as `variable` never hides a
    /// narrower one that colors.
    func highlights(of text: String) -> [Highlight] {
        let parser = Parser()
        guard (try? parser.setLanguage(language)) != nil, let tree = parser.parse(text) else { return [] }
        var found = captures(in: tree, text: text)
        if let inline, let root = tree.rootNode {
            var ranges: [TSRange] = []
            Self.collect("inline", in: root, into: &ranges)
            let inlineParser = Parser()
            if !ranges.isEmpty, (try? inlineParser.setLanguage(inline.language)) != nil {
                inlineParser.includedRanges = ranges
                if let inlineTree = inlineParser.parse(text) { found += inline.captures(in: inlineTree, text: text) }
            }
        }
        found.sort { first, second in
            if first.range.location != second.range.location { return first.range.location < second.range.location }
            if first.range.length != second.range.length { return first.range.length > second.range.length }
            return first.pattern < second.pattern
        }
        // Where two patterns name the same text, the later one in the query
        // wins, as the queries are written: `(pair (bare_key)) @property`
        // follows `(bare_key) @type`.
        var result: [Highlight] = []
        for capture in found {
            let highlight = Highlight(range: capture.range, capture: capture.name)
            if result.last?.range == capture.range { result[result.count - 1] = highlight } else { result.append(highlight) }
        }
        return result
    }

    private func captures(in tree: MutableTree, text: String) -> [(range: NSRange, name: String, pattern: Int)] {
        var found: [(range: NSRange, name: String, pattern: Int)] = []
        for match in query.execute(in: tree).resolve(with: .init(string: text)) {
            for capture in match.captures {
                guard let name = capture.name, SyntaxInk(capture: name) != nil, capture.range.length > 0 else { continue }
                found.append((capture.range, name, capture.patternIndex))
            }
        }
        return found
    }

    private static func collect(_ type: String, in node: Node, into ranges: inout [TSRange]) {
        if node.nodeType == type {
            ranges.append(node.tsRange)
            return
        }
        for index in 0..<node.childCount {
            if let child = node.child(at: index) { collect(type, in: child, into: &ranges) }
        }
    }

    // MARK: Languages

    /// A grammar's parser and the query files it reads, in order. Where two
    /// files name the same text, the later one wins, so a language's own file
    /// follows the one it builds on.
    private struct Spec: Sendable {
        let parser: @Sendable () -> OpaquePointer?
        let queries: [String]
        var inline: String?
    }

    private static let specs: [String: Spec] = [
        "bash": Spec(parser: { tree_sitter_bash() }, queries: ["bash/highlights"]),
        "c": Spec(parser: { tree_sitter_c() }, queries: ["c/highlights"]),
        "cpp": Spec(parser: { tree_sitter_cpp() }, queries: ["c/highlights", "cpp/highlights"]),
        "c_sharp": Spec(parser: { tree_sitter_c_sharp() }, queries: ["c_sharp/highlights"]),
        "css": Spec(parser: { tree_sitter_css() }, queries: ["css/highlights"]),
        "dockerfile": Spec(parser: { tree_sitter_dockerfile() }, queries: ["dockerfile/highlights"]),
        "go": Spec(parser: { tree_sitter_go() }, queries: ["go/highlights"]),
        "html": Spec(parser: { tree_sitter_html() }, queries: ["html/highlights"]),
        "ini": Spec(parser: { tree_sitter_ini() }, queries: ["ini/highlights"]),
        "java": Spec(parser: { tree_sitter_java() }, queries: ["java/highlights"]),
        "javascript": Spec(parser: { tree_sitter_javascript() },
                           queries: ["javascript/highlights", "javascript/highlights-jsx"]),
        "json": Spec(parser: { tree_sitter_json() }, queries: ["json/highlights"]),
        "just": Spec(parser: { tree_sitter_just() }, queries: ["just/highlights"]),
        "kotlin": Spec(parser: { tree_sitter_kotlin() }, queries: ["kotlin/highlights"]),
        "make": Spec(parser: { tree_sitter_make() }, queries: ["make/highlights"]),
        "markdown": Spec(parser: { tree_sitter_markdown() }, queries: ["markdown/highlights"], inline: "markdown_inline"),
        "markdown_inline": Spec(parser: { tree_sitter_markdown_inline() }, queries: ["markdown_inline/highlights"]),
        "objc": Spec(parser: { tree_sitter_objc() }, queries: ["c/highlights", "objc/highlights"]),
        "python": Spec(parser: { tree_sitter_python() }, queries: ["python/highlights"]),
        "ruby": Spec(parser: { tree_sitter_ruby() }, queries: ["ruby/highlights"]),
        "rust": Spec(parser: { tree_sitter_rust() }, queries: ["rust/highlights"]),
        "sql": Spec(parser: { tree_sitter_sql() }, queries: ["sql/highlights"]),
        "swift": Spec(parser: { tree_sitter_swift() }, queries: ["swift/highlights"]),
        "toml": Spec(parser: { tree_sitter_toml() }, queries: ["toml/highlights"]),
        "tsx": Spec(parser: { tree_sitter_tsx() },
                    queries: ["javascript/highlights", "javascript/highlights-jsx", "typescript/highlights"]),
        "typescript": Spec(parser: { tree_sitter_typescript() },
                           queries: ["javascript/highlights", "typescript/highlights"]),
        "yaml": Spec(parser: { tree_sitter_yaml() }, queries: ["yaml/highlights"]),
        "zig": Spec(parser: { tree_sitter_zig() }, queries: ["zig/highlights"]),
    ]

    static let languages: [String] = specs.keys.filter { $0 != "markdown_inline" }.sorted()

    private static let byExtension: [String: String] = [
        "sh": "bash", "bash": "bash", "zsh": "bash",
        "c": "c", "h": "c",
        "cc": "cpp", "cpp": "cpp", "cxx": "cpp", "hh": "cpp", "hpp": "cpp", "hxx": "cpp",
        "css": "css", "go": "go", "html": "html", "htm": "html", "java": "java",
        "js": "javascript", "jsx": "javascript", "mjs": "javascript", "cjs": "javascript",
        "json": "json", "kt": "kotlin", "kts": "kotlin", "md": "markdown", "markdown": "markdown",
        "py": "python", "pyi": "python", "rb": "ruby", "rs": "rust", "sql": "sql", "swift": "swift",
        "toml": "toml", "ts": "typescript", "mts": "typescript", "cts": "typescript", "tsx": "tsx",
        "yaml": "yaml", "yml": "yaml", "zig": "zig", "zon": "zig",
        "cs": "c_sharp", "dockerfile": "dockerfile", "ini": "ini", "cfg": "ini", "conf": "ini",
        "just": "just", "mk": "make", "m": "objc", "mm": "objc",
    ]

    private static let byName: [String: String] = [
        ".bashrc": "bash", ".zshrc": "bash", ".zprofile": "bash", ".bash_profile": "bash",
        "gemfile": "ruby", "rakefile": "ruby", "dockerfile": "dockerfile", "containerfile": "dockerfile",
        "makefile": "make", "gnumakefile": "make", "justfile": "just", ".justfile": "just",
    ]

    /// Each grammar's query compiles once, on its first use; one that fails
    /// to load is remembered as missing.
    private static let cache = Mutex<[String: Grammar?]>([:])

    private static func loaded(_ name: String) -> Grammar? {
        if let known = cache.withLock({ $0[name] }) { return known }
        let grammar = load(name)
        cache.withLock { $0[name] = grammar }
        return grammar
    }

    private static func load(_ name: String) -> Grammar? {
        guard let spec = specs[name], let pointer = spec.parser(),
              let directory = Bundle.loopflowMacResources.resourceURL?.appendingPathComponent("Grammars") else { return nil }
        let language = Language(language: pointer)
        var source = Data()
        for file in spec.queries {
            guard let data = try? Data(contentsOf: directory.appendingPathComponent(file + ".scm")) else { return nil }
            source.append(data)
            source.append(0x0A)
        }
        do {
            let query = try Query(language: language, data: source)
            return Grammar(name: name, language: language, query: query, inline: spec.inline.flatMap(loaded))
        } catch {
            DesktopLog.home.error("Grammar highlights did not compile", ["grammar": name, "error": "\(error)"])
            return nil
        }
    }
}

extension Grammar.Highlight {
    /// Split highlights over `lines` joined by newlines into each line's own,
    /// in that line's UTF-16 units. A piece across lines, such as a block
    /// comment, colors its part of each.
    static func byLine(_ highlights: [Self], lines: [String]) -> [[Self]] {
        var starts: [Int] = []
        var offset = 0
        for line in lines {
            starts.append(offset)
            offset += line.utf16.count + 1
        }
        var result = Array(repeating: [Self](), count: lines.count)
        var first = 0
        for highlight in highlights {
            let start = highlight.range.location, end = NSMaxRange(highlight.range)
            while first + 1 < starts.count, starts[first + 1] <= start { first += 1 }
            var index = first
            while index < lines.count, starts[index] < end {
                let lineEnd = starts[index] + lines[index].utf16.count
                let from = max(start, starts[index]), to = min(end, lineEnd)
                if to > from {
                    result[index].append(Self(range: NSRange(location: from - starts[index], length: to - from),
                                              capture: highlight.capture))
                }
                index += 1
            }
        }
        return result
    }
}

/// The colored pieces of a changed file's rows: removed rows by their old line
/// number, context and added rows by their new one.
struct FileHighlights: Equatable, Sendable {
    var old: [Int: [Grammar.Highlight]] = [:]
    var new: [Int: [Grammar.Highlight]] = [:]

    func spans(for line: ReviewLine) -> [Grammar.Highlight] {
        (line.kind == .removed ? line.old.flatMap { old[$0] } : line.new.flatMap { new[$0] }) ?? []
    }

    /// These colors with each side that has text read from a parse of the
    /// whole file instead, line by line; a side without text keeps its hunks'.
    func reading(old: String?, new: String?, grammar: Grammar) -> FileHighlights {
        var result = self
        if let old { result.old = Self.numbered(old, grammar: grammar) }
        if let new { result.new = Self.numbered(new, grammar: grammar) }
        return result
    }

    private static func numbered(_ text: String, grammar: Grammar) -> [Int: [Grammar.Highlight]] {
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        var result: [Int: [Grammar.Highlight]] = [:]
        for (index, spans) in Grammar.Highlight.byLine(grammar.highlights(of: text), lines: lines).enumerated()
        where !spans.isEmpty {
            result[index + 1] = spans
        }
        return result
    }

    /// Each hunk parsed from its own lines: the newer side from its context and
    /// added lines, the older from its context and removed lines. A hunk that
    /// opens inside a comment or string reads its first lines as code.
    static func hunks(of file: ReviewFile, grammar: Grammar) -> FileHighlights {
        var result = FileHighlights()
        var hunk: [ReviewLine] = []
        func flush() {
            for side in [ReviewLine.Kind.added, .removed] {
                let lines = hunk.filter { $0.kind != (side == .added ? .removed : .added) }
                guard !lines.isEmpty else { continue }
                let texts = lines.map(\.text)
                let spans = Grammar.Highlight.byLine(grammar.highlights(of: texts.joined(separator: "\n")), lines: texts)
                for (line, spans) in zip(lines, spans) where !spans.isEmpty {
                    if side == .added, let number = line.new { result.new[number] = spans }
                    if side == .removed, let number = line.old, line.kind == .removed { result.old[number] = spans }
                }
            }
            hunk = []
        }
        for block in file.blocks {
            switch block {
            case .hunk: flush()
            case .context(let line): hunk.append(line)
            case .change(let change): hunk += change.removed + change.added
            case .moved(let move): hunk += move.lines
            }
        }
        flush()
        return result
    }
}

// MARK: Inks

/// What a highlight is, as far as color goes. Every capture a query names
/// maps to one of these by its leading words, or to none and stays `text`.
enum SyntaxInk: Hashable, Sendable, CaseIterable {
    case keyword, function, type, string, constant, property, comment, punctuation
    case heading, emphasis, strong, link

    init?(capture: String) {
        let words = capture.split(separator: ".")
        guard let first = words.first else { return nil }
        let second = words.count > 1 ? words[1] : ""
        switch first {
        case "keyword", "conditional", "repeat", "include", "exception", "storageclass": self = .keyword
        case "function", "method": self = .function
        case "type", "constructor", "tag", "attribute", "namespace", "module": self = .type
        case "string", "character", "escape": self = .string
        case "number", "float", "constant", "boolean": self = .constant
        case "property", "field": self = .property
        case "comment": self = .comment
        case "punctuation", "operator", "delimiter": self = .punctuation
        case "variable":
            switch second {
            case "member", "other": self = .property
            case "builtin": self = .keyword
            default: return nil
            }
        case "markup", "text":
            switch second {
            case "heading", "title": self = .heading
            case "italic", "emphasis": self = .emphasis
            case "strong": self = .strong
            case "raw", "literal": self = .string
            case "link", "uri", "reference": self = .link
            default: return nil
            }
        default: return nil
        }
    }
}

/// One theme's syntax colors. Each comes from the theme's terminal palette,
/// so a custom theme colors code the way its terminal does, mixed toward the
/// theme's text just far enough to read at 4.5:1 on the surface and on the
/// added and removed row fills. Red and green stay with removed and added lines.
struct SyntaxInks: Equatable, Sendable {
    let colors: [SyntaxInk: ThemeRGB]

    /// The surfaces a highlighted row can sit on.
    static func surfaces(of theme: ThemeDefinition) -> [ThemeRGB] {
        let palette = theme.palette.definition
        return [palette.background, palette.surface, palette.successTone.fillRGB, palette.failureTone.fillRGB]
    }

    init(_ theme: ThemeDefinition) {
        let palette = theme.palette.definition
        let ansi = theme.terminal.indexed
        let surfaces = Self.surfaces(of: theme)
        func readable(_ color: ThemeRGB) -> ThemeRGB {
            for step in 0...100 {
                let mixed = color.mixed(with: palette.text, amount: Double(step) / 100)
                if surfaces.allSatisfy({ mixed.contrast($0) >= 4.52 }) { return mixed }
            }
            return color.readable(on: surfaces, dark: theme.isDark)
        }
        colors = [
            .keyword: readable(ansi[5]),
            .function: readable(ansi[4]),
            .type: readable(ansi[3]),
            .string: readable(ansi[6]),
            .constant: readable(ansi[13]),
            .property: readable(ansi[12]),
            .heading: readable(ansi[4]),
            .comment: readable(palette.textTertiary),
            .punctuation: readable(palette.textSecondary),
            .link: readable(palette.textSecondary),
        ]
    }

    /// nil for emphasis and strong, which change the font rather than the color.
    func color(_ ink: SyntaxInk) -> Color? { colors[ink]?.color }

    /// Inks are worked out once per theme, on its first use.
    static func of(_ theme: ThemeDefinition) -> SyntaxInks {
        if let known = cache.withLock({ $0[theme] }) { return known }
        let inks = SyntaxInks(theme)
        cache.withLock { $0[theme] = inks }
        return inks
    }

    private static let cache = Mutex<[ThemeDefinition: SyntaxInks]>([:])
}

extension AttributedString {
    /// Paint highlights on one line; attributes already there (changed-word fills) stay.
    mutating func paint(_ highlights: [Grammar.Highlight], inks: SyntaxInks) {
        let length = NSAttributedString(self).length
        for highlight in highlights {
            guard let ink = highlight.ink, highlight.range.location < length else { continue }
            let clipped = NSRange(location: highlight.range.location,
                                  length: min(highlight.range.length, length - highlight.range.location))
            guard let range = Range(clipped, in: self) else { continue }
            switch ink {
            case .emphasis: self[range].inlinePresentationIntent = .emphasized
            case .strong: self[range].inlinePresentationIntent = .stronglyEmphasized
            case .heading:
                self[range].inlinePresentationIntent = .stronglyEmphasized
                self[range].foregroundColor = inks.color(ink)
            default: self[range].foregroundColor = inks.color(ink)
            }
        }
    }
}

extension Bundle {
    /// The bundle holding LoopflowMac's copied resources (Grammars, GhosttyResources),
    /// in a packaged app, a SwiftPM build and an Xcode build.
    static var loopflowMacResources: Bundle {
        #if SWIFT_PACKAGE
        if let resources = Bundle.main.resourceURL,
           let packaged = Bundle(url: resources.appendingPathComponent("LoopflowSwift_LoopflowMac.bundle", isDirectory: true)) {
            return packaged
        }
        return .module
        #else
        return .main
        #endif
    }
}
#endif
