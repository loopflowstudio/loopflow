# Asana rich text via html_notes

## Problem

Asana item descriptions lose all formatting on sync. The client reads/writes the plaintext `notes` field only, so headings, lists, bold, links, and code vanish on round-trip. Notion descriptions already preserve formatting via the blocks converter. Asana should match.

## Approach

Hand-roll a markdown-to-Asana-HTML converter in `pm/asana_html.rs`, following the same architecture as `pm/notion_blocks.rs`. No new dependencies — Asana's HTML subset is small enough (10 tags) that a recursive descent parser handles it cleanly without an HTML parsing crate.

Two public functions:

```rust
pub fn markdown_to_asana_html(markdown: &str) -> String
pub fn asana_html_to_markdown(html: &str) -> String
```

### Converter design

**Markdown to HTML** — reuse the same line-by-line block dispatch as `notion_blocks.rs`:
- `#`/`##`/`###` → `<h1>`/`<h2>`/`<h3>`
- Blank-line-separated text → `<p>` wrapped
- `- `/`* `/`+ ` → `<ul><li>`
- `1. ` → `<ol><li>`
- `---` → `<hr>`
- Inline: `**bold**` → `<strong>`, `*italic*` → `<em>`, `` `code` `` → `<code>`, `~~strike~~` → `<s>`, `[text](url)` → `<a href="url">`

**HTML to markdown** — character-by-character scan for `<tag>` boundaries:
- Block tags (`h1`–`h3`, `p`, `ul`, `ol`, `li`, `hr`) drive line structure
- Inline tags (`strong`, `em`, `code`, `s`, `a`) wrap content with markdown markers
- Unknown tags → strip to inner text content (graceful degradation)
- Self-closing `<hr>` / `<hr/>` → `---`

The inline formatting uses the same `InlineStyle { bold, italic, code, strikethrough }` pattern from `notion_blocks.rs`. The style accumulates through nested tags and emits the right markdown wrappers on close.

### AsanaClient integration

1. Add `html_notes` to `TASK_FIELDS` opt_fields string
2. Add `html_notes: String` (with `#[serde(default)]`) to `AsanaTask`
3. `into_pm_item`: prefer `html_notes` converted via `asana_html_to_markdown`, fall back to `notes` when `html_notes` is empty
4. `create_item` / `update_item`: write `html_notes` via `markdown_to_asana_html` instead of `notes`

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Add `scraper` or `lol_html` crate | Handles malformed HTML, battle-tested | Overkill for 10 tags; adds a dependency tree for something simpler than the Notion block parser we already hand-roll |
| Write both `notes` and `html_notes` on update | Maximum backwards compat with old API consumers | Asana auto-generates `notes` from `html_notes` — writing both creates conflicts. Let Asana derive `notes` from our `html_notes` |
| Use pulldown-cmark for the markdown side | Standard parser | We'd still need custom HTML emit for Asana's subset, and the notion_blocks parser already validates the hand-roll approach. Adding a dep for half the pipeline isn't worth it |

## Key decisions

**No new dependencies.** Asana's supported tags are: `h1`–`h3`, `p`, `ul`, `ol`, `li`, `code`, `strong`, `em`, `s`, `a`, `hr`. This is fewer constructs than the Notion block converter handles. A hand-rolled parser keeps the dependency footprint zero and the code greppable.

**Write `html_notes` only, don't dual-write `notes`.** Asana auto-generates the plaintext `notes` field from `html_notes`. Writing both is redundant and can cause drift if the two disagree. The API docs confirm `html_notes` takes precedence.

**Read fallback: `html_notes` first, `notes` second.** Items created before this change (or by integrations that only write `notes`) will have empty `html_notes`. The fallback ensures backwards compatibility without a migration.

**Nested lists: one level only.** Match `notion_blocks.rs` behavior — support one level of nesting (items with 2-space indent children). Asana's UI supports deeper nesting, but one level covers 95% of descriptions and keeps the parser simple. Can extend later if needed.

## Scope

- **In scope:** `pm/asana_html.rs` converter module, `AsanaClient` integration, round-trip tests, graceful degradation for unknown tags
- **Out of scope:** Nested lists beyond one level, `<u>` (underline — not in Asana's subset), images/attachments, task-level `html_notes` on project creation (projects use a different API shape)

## Done when

```bash
cargo test -p loopflow asana_html     # round-trip and edge case tests pass
cargo test -p loopflow asana          # existing Asana client tests still pass
cargo clippy -- -D warnings           # no new warnings
```

- Markdown with headings, lists, bold, italic, code, strikethrough, and links round-trips through `markdown_to_asana_html` / `asana_html_to_markdown`
- `AsanaClient` reads prefer `html_notes`, fall back to `notes`
- `AsanaClient` writes use `html_notes`
- Unknown HTML tags degrade to plain text
- Items with only `notes` (no `html_notes`) read correctly
