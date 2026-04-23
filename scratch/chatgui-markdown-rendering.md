---
status: in-progress
claimed_by: jack-heart.chatgui.20260423_1303
claimed_at: 2026-04-23T21:03:10.020991Z
---
# 02: Markdown Rendering

Rich text in assistant messages. Currently plain text + code fences.

## Done when

Assistant messages render bold, italic, links, lists, headers, and inline code. Code blocks get syntax highlighting for common languages (Swift, Python, Rust, TypeScript, bash).

## Approach

Evaluate existing SwiftUI markdown options:
- `Text` with `AttributedString` from `Foundation` (limited but built-in)
- `swift-markdown` + custom `AttributedString` renderer
- Third-party: MarkdownUI, swift-markdown-ui

## Constraint from streaming work

`MessageRow` caches parsed segments keyed on `content.count` (content length as staleness check). Markdown parsing must either integrate with or replace this cache. If the markdown renderer produces `AttributedString` or a view tree, the segment cache may become the markdown cache — same invalidation strategy, different output type.

If assistant content is ever edited in-place (not just appended), the content-length cache key won't detect the change. For now streaming is append-only so this is fine, but markdown rendering should not depend on this assumption if it can be avoided cheaply.
