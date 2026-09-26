# Native Markdown rendering seam

Parent inspected the initial cycle-3 `MarkdownBlocks.swift` while its author was
still working. It hand-parses heading/list/fence prefixes; this observation is
not a verdict on the final contribution.

A local SDK probe (`/tmp/loo291-markdown-intents.swift`, result
`/tmp/loo291-markdown-intents.log`) confirms Foundation's existing
`AttributedString(markdown:)` produces presentation intents for headings,
nested lists/items, quotes, fenced code, hard line breaks, table rows/cells and
retains inline emphasis/link attributes. This is experimental SDK evidence, not
a product test or visual result.

Compression candidate: render those standard parsing results in native SwiftUI
instead of keeping a second partial Markdown grammar. No new dependency or web
renderer is needed. Preserve paragraph/list continuation, literal code, links and
source text; retain a readable exact-source outcome for parse failure. Review
against actual Description content and a meaningful focused native example.
