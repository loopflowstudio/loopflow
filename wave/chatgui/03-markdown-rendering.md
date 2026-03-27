# 03: Markdown Rendering

Rich text in assistant messages. Currently plain text + code fences.

## Done when

Assistant messages render bold, italic, links, lists, headers, and inline code. Code blocks get syntax highlighting for common languages (Swift, Python, Rust, TypeScript, bash).

## Approach

Evaluate existing SwiftUI markdown options:
- `Text` with `AttributedString` from `Foundation` (limited but built-in)
- `swift-markdown` + custom `AttributedString` renderer
- Third-party: MarkdownUI, swift-markdown-ui

Performance is critical — rendering must not regress the streaming work from stage 01. Markdown parsing should be incremental or cached.
