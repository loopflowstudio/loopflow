import Foundation

public enum MarkdownBlock: Equatable {
    case paragraph(AttributedString)
    case heading(level: Int, AttributedString)
    case list(ordered: Bool, items: [AttributedString])
    case blockquote([MarkdownBlock])
    case code(language: String?, content: String)
    case diff(String)
    case rule
}

public func parseMarkdownBlocks(_ content: String) -> [MarkdownBlock] {
    var parser = MarkdownBlockParser(content: content, parseInlineMarkdown: true)
    return parser.parse()
}

public func parseStreamingMarkdownBlocks(_ content: String) -> [MarkdownBlock] {
    var parser = MarkdownBlockParser(content: content, parseInlineMarkdown: false)
    return parser.parse()
}

private struct MarkdownBlockParser {
    private let lines: [String]
    private let parseInlineMarkdown: Bool
    private var index = 0

    init(content: String, parseInlineMarkdown: Bool) {
        self.lines = content.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        self.parseInlineMarkdown = parseInlineMarkdown
    }

    mutating func parse() -> [MarkdownBlock] {
        var blocks: [MarkdownBlock] = []

        while index < lines.count {
            let line = lines[index]
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            if trimmed.isEmpty {
                index += 1
                continue
            }

            if isCodeFence(trimmed) {
                blocks.append(parseCodeBlock(language: codeFenceLanguage(trimmed)))
            } else if let heading = parseHeading(line) {
                blocks.append(heading)
                index += 1
            } else if isRule(trimmed) {
                blocks.append(.rule)
                index += 1
            } else if isBlockquote(line) {
                blocks.append(parseBlockquote())
            } else if let marker = listMarker(line) {
                blocks.append(parseList(ordered: marker.ordered))
            } else {
                blocks.append(parseParagraph())
            }
        }

        return blocks
    }

    private func inline(_ text: String) -> AttributedString {
        guard parseInlineMarkdown,
              let parsed = try? AttributedString(
                  markdown: text,
                  options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
              ) else {
            return AttributedString(text)
        }
        return parsed
    }

    private func isCodeFence(_ trimmed: String) -> Bool {
        trimmed.hasPrefix("```")
    }

    private func codeFenceLanguage(_ trimmed: String) -> String? {
        let suffix = trimmed.dropFirst(3).trimmingCharacters(in: .whitespaces)
        return suffix.isEmpty ? nil : String(suffix)
    }

    private mutating func parseCodeBlock(language: String?) -> MarkdownBlock {
        index += 1
        var codeLines: [String] = []

        while index < lines.count {
            let line = lines[index]
            if line.trimmingCharacters(in: .whitespaces) == "```" {
                index += 1
                break
            }
            codeLines.append(line)
            index += 1
        }

        let content = codeLines.joined(separator: "\n")
        let normalizedLanguage = language?.lowercased()
        if normalizedLanguage == "diff" || normalizedLanguage == "patch" {
            return .diff(content)
        }
        return .code(language: language, content: content)
    }

    private func parseHeading(_ line: String) -> MarkdownBlock? {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        guard trimmed.hasPrefix("#") else { return nil }

        var level = 0
        for character in trimmed {
            guard character == "#" else { break }
            level += 1
        }

        guard (1...6).contains(level) else { return nil }
        let markerEnd = trimmed.index(trimmed.startIndex, offsetBy: level)
        guard markerEnd < trimmed.endIndex,
              trimmed[markerEnd] == " " else { return nil }

        let textStart = trimmed.index(after: markerEnd)
        let text = String(trimmed[textStart...]).trimmingCharacters(in: .whitespaces)
        return .heading(level: level, inline(text))
    }

    private func isRule(_ trimmed: String) -> Bool {
        guard trimmed.count >= 3 else { return false }
        let collapsed = trimmed.filter { !$0.isWhitespace }
        guard let first = collapsed.first,
              first == "-" || first == "*" || first == "_" else { return false }
        return collapsed.count >= 3 && collapsed.allSatisfy { $0 == first }
    }

    private func isBlockquote(_ line: String) -> Bool {
        line.trimmingCharacters(in: .whitespaces).hasPrefix(">")
    }

    private mutating func parseBlockquote() -> MarkdownBlock {
        var quoteLines: [String] = []

        while index < lines.count {
            let line = lines[index]
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard trimmed.hasPrefix(">") else { break }

            var remainder = String(trimmed.dropFirst())
            if remainder.hasPrefix(" ") {
                remainder.removeFirst()
            }
            quoteLines.append(remainder)
            index += 1
        }

        var parser = MarkdownBlockParser(
            content: quoteLines.joined(separator: "\n"),
            parseInlineMarkdown: parseInlineMarkdown
        )
        return .blockquote(parser.parse())
    }

    private struct ListMarker {
        let ordered: Bool
        let contentStart: String.Index
    }

    private func listMarker(_ line: String) -> ListMarker? {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return nil }

        if let first = trimmed.first,
           (first == "-" || first == "*" || first == "+"),
           trimmed.dropFirst().first == " " {
            let start = trimmed.index(trimmed.startIndex, offsetBy: 2)
            return ListMarker(ordered: false, contentStart: start)
        }

        var digitEnd = trimmed.startIndex
        while digitEnd < trimmed.endIndex, trimmed[digitEnd].isNumber {
            digitEnd = trimmed.index(after: digitEnd)
        }
        guard digitEnd > trimmed.startIndex,
              digitEnd < trimmed.endIndex,
              trimmed[digitEnd] == "." else { return nil }
        let spaceIndex = trimmed.index(after: digitEnd)
        guard spaceIndex < trimmed.endIndex, trimmed[spaceIndex] == " " else { return nil }
        return ListMarker(ordered: true, contentStart: trimmed.index(after: spaceIndex))
    }

    private mutating func parseList(ordered: Bool) -> MarkdownBlock {
        var items: [AttributedString] = []

        while index < lines.count {
            let line = lines[index]
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard let marker = listMarker(line), marker.ordered == ordered else { break }
            let text = String(trimmed[marker.contentStart...])
            items.append(inline(text))
            index += 1
        }

        return .list(ordered: ordered, items: items)
    }

    private mutating func parseParagraph() -> MarkdownBlock {
        var paragraphLines: [String] = []

        while index < lines.count {
            let line = lines[index]
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            if trimmed.isEmpty ||
                isCodeFence(trimmed) ||
                parseHeading(line) != nil ||
                isRule(trimmed) ||
                isBlockquote(line) ||
                listMarker(line) != nil {
                break
            }

            paragraphLines.append(line)
            index += 1
        }

        return .paragraph(inline(paragraphLines.joined(separator: "\n")))
    }
}

public enum SyntaxTokenKind: Equatable {
    case plain
    case keyword
    case string
    case comment
    case number
}

public struct SyntaxToken: Equatable {
    public let text: String
    public let kind: SyntaxTokenKind

    public init(text: String, kind: SyntaxTokenKind) {
        self.text = text
        self.kind = kind
    }
}

public enum SyntaxHighlighter {
    public static func tokens(for content: String, language: String?) -> [SyntaxToken] {
        guard let normalizedLanguage = normalize(language) else {
            return [SyntaxToken(text: content, kind: .plain)]
        }
        return tokenize(content, language: normalizedLanguage)
    }

    private static func normalize(_ language: String?) -> String? {
        guard let language else { return nil }
        let lowercased = language.lowercased()
        switch lowercased {
        case "swift", "rust", "python", "py", "bash", "sh", "zsh", "json", "yaml", "yml", "toml", "markdown", "md", "diff", "patch":
            return lowercased
        default:
            return nil
        }
    }

    private static func tokenize(_ content: String, language: String) -> [SyntaxToken] {
        var tokens: [SyntaxToken] = []
        var index = content.startIndex

        func append(_ range: Range<String.Index>, kind: SyntaxTokenKind) {
            guard !range.isEmpty else { return }
            let text = String(content[range])
            if let last = tokens.last, last.kind == kind {
                tokens[tokens.count - 1] = SyntaxToken(text: last.text + text, kind: kind)
            } else {
                tokens.append(SyntaxToken(text: text, kind: kind))
            }
        }

        while index < content.endIndex {
            if let range = commentRange(in: content, from: index, language: language) {
                append(range, kind: .comment)
                index = range.upperBound
            } else if let range = stringRange(in: content, from: index) {
                append(range, kind: .string)
                index = range.upperBound
            } else if content[index].isNumber,
                      let range = numberRange(in: content, from: index) {
                append(range, kind: .number)
                index = range.upperBound
            } else if isIdentifierStart(content[index]),
                      let range = identifierRange(in: content, from: index) {
                let word = String(content[range])
                append(range, kind: keywords(for: language).contains(word) ? .keyword : .plain)
                index = range.upperBound
            } else {
                let next = content.index(after: index)
                append(index..<next, kind: .plain)
                index = next
            }
        }

        return tokens
    }

    private static func commentRange(in content: String, from index: String.Index, language: String) -> Range<String.Index>? {
        let suffix = content[index...]
        let prefixes: [String]
        switch language {
        case "python", "py", "bash", "sh", "zsh", "yaml", "yml", "toml":
            prefixes = ["#"]
        case "swift", "rust", "json", "markdown", "md":
            prefixes = ["//"]
        case "diff", "patch":
            if suffix.hasPrefix("+") || suffix.hasPrefix("-") || suffix.hasPrefix("@@") {
                let end = content[index...].firstIndex(of: "\n") ?? content.endIndex
                return index..<end
            }
            return nil
        default:
            prefixes = []
        }

        guard prefixes.contains(where: { suffix.hasPrefix($0) }) else { return nil }
        let end = content[index...].firstIndex(of: "\n") ?? content.endIndex
        return index..<end
    }

    private static func stringRange(in content: String, from index: String.Index) -> Range<String.Index>? {
        let quote = content[index]
        guard quote == "\"" || quote == "'" || quote == "`" else { return nil }

        var current = content.index(after: index)
        var escaped = false
        while current < content.endIndex {
            let character = content[current]
            if escaped {
                escaped = false
            } else if character == "\\" {
                escaped = true
            } else if character == quote {
                return index..<content.index(after: current)
            }
            current = content.index(after: current)
        }
        return index..<content.endIndex
    }

    private static func numberRange(in content: String, from index: String.Index) -> Range<String.Index>? {
        var current = index
        while current < content.endIndex {
            let character = content[current]
            guard character.isNumber || character == "." || character == "_" else { break }
            current = content.index(after: current)
        }
        return index..<current
    }

    private static func identifierRange(in content: String, from index: String.Index) -> Range<String.Index>? {
        var current = index
        while current < content.endIndex, isIdentifierPart(content[current]) {
            current = content.index(after: current)
        }
        return index..<current
    }

    private static func isIdentifierStart(_ character: Character) -> Bool {
        character.isLetter || character == "_"
    }

    private static func isIdentifierPart(_ character: Character) -> Bool {
        character.isLetter || character.isNumber || character == "_"
    }

    private static func keywords(for language: String) -> Set<String> {
        switch language {
        case "swift":
            return ["actor", "any", "as", "async", "await", "case", "catch", "class", "continue", "default", "defer", "do", "else", "enum", "extension", "false", "for", "func", "guard", "if", "import", "in", "init", "let", "nil", "private", "protocol", "public", "return", "self", "static", "struct", "switch", "throw", "throws", "true", "try", "var", "while"]
        case "rust":
            return ["as", "async", "await", "break", "const", "continue", "crate", "else", "enum", "false", "fn", "for", "if", "impl", "in", "let", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "use", "where", "while"]
        case "python", "py":
            return ["and", "as", "async", "await", "break", "class", "continue", "def", "elif", "else", "False", "for", "from", "if", "import", "in", "is", "lambda", "None", "not", "or", "pass", "raise", "return", "True", "try", "while", "with", "yield"]
        case "bash", "sh", "zsh":
            return ["case", "do", "done", "elif", "else", "esac", "fi", "for", "function", "if", "in", "select", "then", "until", "while"]
        case "json", "yaml", "yml", "toml":
            return ["true", "false", "null"]
        case "markdown", "md":
            return []
        default:
            return []
        }
    }
}
