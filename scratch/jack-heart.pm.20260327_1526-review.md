# Review: Asana rich text via `html_notes`

## What was implemented

Added an Asana-specific markdown/HTML converter and wired the Asana PM client to preserve formatted task descriptions. Reads now prefer `html_notes` and fall back to plaintext `notes`; writes now send `html_notes` so headings, lists, links, bold, italics, code, strikethrough, and horizontal rules survive round-trip sync.

## Key choices

- Hand-rolled the converter in `rust/loopflow/src/lfd/pm/asana_html.rs` instead of adding an HTML parsing dependency. The supported Asana subset is small enough that a focused parser is simpler than carrying a new crate tree.
- Wrote `html_notes` only on create/update. Asana derives plaintext `notes` from rich text, so dual-writing both fields risks drift.
- Kept a compatibility read path. Older tasks that only have `notes` still import correctly.
- Limited nested markdown list support to one level, matching the design doc and keeping the parser small.

## How it fits together

`asana_html.rs` converts between loopflow's markdown descriptions and Asana's supported HTML subset. `AsanaClient` now requests `html_notes`, turns rich HTML back into markdown during `list_items`, and turns markdown into `html_notes` during `create_item` and `update_item`. The rest of the PM sync pipeline keeps using the existing `PmItem` markdown description field.

## Risks and bottlenecks

- The converter is intentionally narrow. Unsupported or malformed HTML degrades to text rather than trying to preserve every structure.
- Nested lists stop at one level; deeper markdown nesting is flattened into text.
- Project creation still writes plaintext `notes` for project descriptions. This change only covers task/item descriptions.
- Because the parser is hand-rolled, future Asana formatting additions need explicit test coverage before support is added.

## What's not included

- Nested lists beyond one level
- Underline, images, attachments, or arbitrary HTML tags
- Project-level `html_notes`
- A dependency-backed general HTML/Markdown parser

## Validation

Ran the design doc's done-when checks plus formatting:

```bash
cargo fmt --check
cargo test -p loopflow asana_html
cargo test -p loopflow asana
cargo clippy -p loopflow -- -D warnings
```

Results:
- `cargo fmt --check` ✅
- `cargo test -p loopflow asana_html` ✅ (6 tests passed)
- `cargo test -p loopflow asana` ✅ (20 filtered Asana-related tests passed)
- `cargo clippy -p loopflow -- -D warnings` ✅

## Docs updated

- `README.md` now notes that Asana task descriptions preserve basic markdown formatting via `html_notes`, with plaintext fallback for older tasks.
