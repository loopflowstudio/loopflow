## Try it!

```bash
cargo test -p loopflow asana_html
cargo test -p loopflow asana
cargo clippy -p loopflow -- -D warnings
```

What to look for:
- `asana_html` round-trip tests prove headings, lists, bold, italics, code, strikethrough, links, and `<hr>` survive markdown ↔ Asana HTML conversion.
- `asana` client tests prove task reads prefer `html_notes`, older tasks still fall back to plaintext `notes`, and create/update requests now send `html_notes` instead of `notes`.

Validation from this branch:
- `cargo fmt --check` ✅
- `cargo test -p loopflow asana_html` ✅ (6 passed)
- `cargo test -p loopflow asana` ✅ (20 passed)
- `cargo clippy -p loopflow -- -D warnings` ✅

## Intent

Preserve formatting when loopflow syncs Asana task descriptions. Before this change, Asana descriptions round-tripped through plaintext `notes`, so markdown structure was lost on export and re-import. This branch adds a focused converter for Asana's supported HTML subset and routes task description sync through `html_notes` so loopflow keeps the same markdown-rich description model it already uses internally.

## Assumptions

- Asana continues to accept the supported HTML subset used here (`h1`-`h3`, `p`, `ul`, `ol`, `li`, `strong`, `em`, `code`, `s`, `a`, `hr`).
- Asana derives plaintext `notes` from `html_notes`, so writing only `html_notes` is sufficient for task descriptions.
- Existing tasks created by older loopflow builds may still only have `notes`, so reads must keep the fallback path.

## Key decisions

- Added a hand-rolled converter in `rust/loopflow/src/lfd/pm/asana_html.rs` instead of a new dependency; the supported surface area is small and testable.
- Read `html_notes` first, then fall back to `notes`, to keep old tasks readable without a migration.
- Write `html_notes` only on create/update to avoid drift between two authoritative description fields.
- Support one nested list level and flatten deeper nesting into text for now.

## Not included

- Rich-text support for project descriptions
- Nested lists deeper than one level
- Underline, attachments, images, or arbitrary HTML preservation
