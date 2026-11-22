#if os(macOS)
import Foundation
@testable import Loopflow
import Testing
@testable import LoopflowMac

@Suite("Grammar highlighting")
struct GrammarTests {
    /// The ink each unit of `text` ends up in once its highlights are painted
    /// in order, read at the first unit of `piece`.
    static func ink(of piece: String, in text: String, path: String) throws -> SyntaxInk? {
        let grammar = try #require(Grammar.named(path), "no grammar for \(path)")
        var inks = [SyntaxInk?](repeating: nil, count: text.utf16.count)
        for highlight in grammar.highlights(of: text) {
            for unit in highlight.range.location..<NSMaxRange(highlight.range) { inks[unit] = highlight.ink }
        }
        let found = (text as NSString).range(of: piece)
        try #require(found.location != NSNotFound, "\(piece) is not in the fixture")
        return inks[found.location]
    }

    /// One line per language, and what a person reading it would see colored.
    static let fixtures: [(path: String, text: String, expected: [(String, SyntaxInk)])] = [
        ("Store.swift", "struct Store { func read() -> Int { return 42 } } // done",
         [("struct", .keyword), ("Store", .type), ("read", .function), ("Int", .type), ("42", .constant), ("// done", .comment)]),
        ("main.rs", #"fn read(path: &str) -> String { let n = 42; "x".to_string() } // done"#,
         [("fn", .keyword), ("read", .function), ("String", .type), ("42", .constant), (#""x""#, .string),
          ("to_string", .function), ("// done", .comment)]),
        ("tool.py", "def read(path):\n    return path.strip(42, 'x').size  # done",
         [("def", .keyword), ("read", .function), ("strip", .function), ("size", .property), ("42", .constant),
          ("'x'", .string), ("# done", .comment)]),
        ("app.ts", "function read(path: string): number { return 42; } // done",
         [("function", .keyword), ("read", .function), ("number", .type), ("42", .constant), ("// done", .comment)]),
        ("View.tsx", #"const view = <Box title="x" />; // done"#,
         [("const", .keyword), ("Box", .type), (#""x""#, .string), ("// done", .comment)]),
        ("app.js", #"const read = (path) => path.trim("x").length; // done"#,
         [("const", .keyword), ("trim", .function), ("length", .property), (#""x""#, .string), ("// done", .comment)]),
        ("README.md", "# Title\n\nSome *word*, **bold** and `code`.\n",
         [("Title", .heading), ("word", .emphasis), ("bold", .strong), ("code", .string), ("`", .punctuation)]),
        ("config.yaml", #"name: "x" # done"#, [("name", .property), (#""x""#, .string), ("# done", .comment)]),
        ("Cargo.toml", "[package]\nname = \"x\" # done\n", [("name", .property), (#""x""#, .string), ("# done", .comment)]),
        ("data.json", #"{"name": 42, "on": true}"#, [(#""name""#, .string), ("42", .constant), ("true", .constant)]),
        ("schema.sql", "SELECT name FROM users WHERE id = 42; -- done",
         [("SELECT", .keyword), ("FROM", .keyword), ("42", .constant), ("-- done", .comment)]),
        ("run.sh", #"if [ -n "$x" ]; then echo "hi"; fi # done"#,
         [("if", .keyword), ("echo", .function), (#""hi""#, .string), ("# done", .comment)]),
        ("page.html", #"<div class="x">Hi</div><!-- done -->"#,
         [("div", .type), (#""x""#, .string), ("<!-- done -->", .comment)]),
        ("style.css", ".box { color: red; width: 42px; } /* done */",
         [("color", .property), ("42", .constant), ("/* done */", .comment)]),
        ("main.go", "func read(path string) int { return 42 } // done",
         [("func", .keyword), ("read", .function), ("int", .type), ("42", .constant), ("// done", .comment)]),
        ("read.c", "int read(const char *path) { return 42; } // done",
         [("int", .type), ("read", .function), ("return", .keyword), ("42", .constant), ("// done", .comment)]),
        ("store.cpp", "class Store { public: int read() { return 42; } }; // done",
         [("class", .keyword), ("Store", .type), ("read", .function), ("42", .constant), ("// done", .comment)]),
        ("store.rb", "def read(path)\n  \"x\" # done\nend",
         [("def", .keyword), ("read", .function), (#""x""#, .string), ("# done", .comment)]),
        ("Store.java", "class Store { int read() { return 42; } } // done",
         [("class", .keyword), ("Store", .type), ("read", .function), ("42", .constant), ("// done", .comment)]),
        ("Store.kt", "fun read(path: String): Int = 42 // done",
         [("fun", .keyword), ("read", .function), ("String", .type), ("42", .constant), ("// done", .comment)]),
    ]

    @Test(arguments: fixtures.indices)
    func eachLanguageColorsWhatAPersonReads(_ index: Int) throws {
        let fixture = Self.fixtures[index]
        for (piece, expected) in fixture.expected {
            let found = try Self.ink(of: piece, in: fixture.text, path: fixture.path)
            #expect(found == expected, "\(fixture.path): \(piece) is \(String(describing: found))")
        }
    }

    @Test func everyGrammarLoadsAndUnknownFilesStayPlain() {
        #expect(Grammar.languages.count == 20)
        for name in Grammar.languages {
            let path = Self.fixtures.map(\.path).first { Grammar.named($0)?.name == name }
            #expect(path != nil, "\(name) has no loaded fixture")
        }
        #expect(Grammar.named("Dockerfile") == nil && Grammar.named("notes.txt") == nil)
        #expect(Grammar.named("build.gradle.kts")?.name == "kotlin")
    }

    /// A row read from the whole file knows it sits inside a docstring; the
    /// same row parsed alone, as its hunk shows it, does not.
    @Test func aDocstringRowColorsFromTheWholeFile() throws {
        let grammar = try #require(Grammar.named("tool.py"))
        let lines = ["def read():", "    \"\"\"", "    inside the docstring", "    \"\"\"", "    return 1"]
        let whole = Grammar.Highlight.byLine(grammar.highlights(of: lines.joined(separator: "\n")), lines: lines)
        #expect(whole[2].map(\.ink) == [.string])
        #expect(whole[2].first?.range == NSRange(location: 0, length: lines[2].utf16.count))
        #expect(!grammar.highlights(of: lines[2]).contains { $0.ink == .string })
    }

    /// Removed rows take the old side's colors by old line number; added and
    /// context rows the new side's by new line number.
    @Test func hunkRowsColorByTheirSide() throws {
        let patch = """
        diff --git a/main.rs b/main.rs
        --- a/main.rs
        +++ b/main.rs
        @@ -10,3 +10,3 @@ mod store;
         fn keep() {}
        -let old = "gone";
        +let new = 42;
         // end
        """
        let file = try #require(ReviewDiff.parse(patch).files.first)
        let highlights = FileHighlights.hunks(of: file, grammar: try #require(Grammar.named(file.path)))
        let removed = try #require(file.lines.first { $0.kind == .removed })
        let added = try #require(file.lines.first { $0.kind == .added })
        #expect(highlights.spans(for: removed).contains { $0.ink == .string })
        #expect(highlights.spans(for: added).contains { $0.ink == .constant })
        #expect(!highlights.spans(for: added).contains { $0.ink == .string })
        #expect(highlights.new[12]?.map(\.ink) == [.comment])
    }

    /// Every bundled theme's syntax inks read at 4.5:1 on the surfaces a row
    /// sits on, the added and removed fills included.
    @Test func everyThemesInksAreReadable() {
        for theme in ThemeDefinition.builtins {
            let inks = SyntaxInks(theme)
            for surface in SyntaxInks.surfaces(of: theme) {
                for (ink, color) in inks.colors {
                    #expect(color.contrast(surface) >= 4.5, "\(theme.name) \(ink) on \(surface.css)")
                }
            }
        }
    }
}
#endif
