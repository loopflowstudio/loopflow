# Asana rich text via html_notes

**Finish line:** Asana item descriptions round-trip as markdown instead of plaintext, using Asana's `html_notes` field.

Asana supports a subset of HTML in the `html_notes` field alongside the plaintext `notes` field. Currently the client reads/writes `notes` only — descriptions lose all formatting on sync. With the markdown↔blocks converter landing for Notion, the same principle applies here: if people write formatted descriptions, sync should preserve them.

## What to build

1. Add a markdown↔Asana HTML converter. Asana's HTML subset supports: `<h1>`–`<h3>`, `<p>`, `<ul>`/`<ol>`/`<li>`, `<code>`, `<strong>`, `<em>`, `<s>`, `<a>`, `<hr>`. This maps cleanly to the same markdown subset as Notion blocks.
2. Switch `AsanaClient` reads from `notes` to `html_notes`, converting to markdown.
3. Switch `AsanaClient` writes from `notes` to `html_notes`, converting from markdown.
4. Fall back to `notes` when `html_notes` is empty (backwards compat with items created before the change).

## Constraints

- Asana's HTML subset is limited. Unsupported tags should degrade gracefully (strip to plain text).
- Don't break existing items that only have `notes` content.
- The converter should be its own module (`pm/asana_html.rs`) with round-trip tests, same pattern as `pm/notion_blocks.rs`. The Notion converter's inline formatting span model (`parse_inline` / `rich_text_to_markdown`) is a validated reference for the rich-text parsing approach.

## Done when

- Asana item descriptions preserve headings, lists, bold, italic, code, and links through sync
- Items with only `notes` (no `html_notes`) still read correctly
- Round-trip tests pass for the markdown↔Asana HTML converter
