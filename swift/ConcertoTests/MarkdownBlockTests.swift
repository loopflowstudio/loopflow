import Foundation
import Testing
@testable import LoopflowCore

@Suite("Markdown blocks")
struct MarkdownBlockTests {
    @Test("Parses rich assistant markdown into native blocks")
    func parsesRichAssistantMarkdown() {
        let input = """
        # Heading

        Intro with **bold** and [docs](https://example.com).

        - first
        - second

        > quoted thought

        ```rust
        fn main() {
            println!("hi");
        }
        ```

        ```diff
        --- a/file.rs
        +++ b/file.rs
        @@ -1 +1 @@
        -old
        +new
        ```
        """

        let blocks = parseMarkdownBlocks(input)

        #expect(blocks.count == 6)
        if case .heading(let level, let text) = blocks[0] {
            #expect(level == 1)
            #expect(String(text.characters) == "Heading")
        } else {
            Issue.record("Expected heading block")
        }

        if case .paragraph(let text) = blocks[1] {
            #expect(String(text.characters).contains("Intro with bold"))
        } else {
            Issue.record("Expected paragraph block")
        }

        if case .list(let ordered, let items) = blocks[2] {
            #expect(!ordered)
            #expect(items.map { String($0.characters) } == ["first", "second"])
        } else {
            Issue.record("Expected list block")
        }

        if case .blockquote(let nested) = blocks[3] {
            #expect(nested.count == 1)
        } else {
            Issue.record("Expected blockquote block")
        }

        if case .code(let language, let content) = blocks[4] {
            #expect(language == "rust")
            #expect(content.contains("fn main"))
        } else {
            Issue.record("Expected code block")
        }

        if case .diff(let diff) = blocks[5] {
            #expect(diff.contains("+new"))
        } else {
            Issue.record("Expected diff block")
        }
    }

    @Test("Unclosed fences still render as code")
    func parsesUnclosedFenceAsCode() {
        let blocks = parseMarkdownBlocks("""
        ```bash
        echo hello
        """)

        #expect(blocks.count == 1)
        if case .code(let language, let content) = blocks[0] {
            #expect(language == "bash")
            #expect(content == "echo hello")
        } else {
            Issue.record("Expected code block")
        }
    }

    @Test("Streaming parser keeps the cheap fence split")
    func parsesStreamingMarkdownCheaply() {
        let blocks = parseStreamingMarkdownBlocks("""
        Intro **without inline parse**

        ```swift
        let value = 42
        ```
        """)

        #expect(blocks.count == 2)
        if case .paragraph(let text) = blocks[0] {
            #expect(String(text.characters) == "Intro **without inline parse**")
        } else {
            Issue.record("Expected paragraph block")
        }
    }

    @Test("Syntax highlighter identifies common token kinds")
    func syntaxHighlighterTokenizesCommonLanguages() {
        let tokens = SyntaxHighlighter.tokens(for: "let answer = 42 // truth", language: "swift")

        #expect(tokens.contains(SyntaxToken(text: "let", kind: .keyword)))
        #expect(tokens.contains(SyntaxToken(text: "42", kind: .number)))
        #expect(tokens.contains(SyntaxToken(text: "// truth", kind: .comment)))
    }
}
