# Review: Notion PM provider

## What was implemented

Added Notion as a first-class PM provider for `lf ops pm` and `lfq auth`.

The branch adds a `NotionClient` that implements the full `PmProvider` surface, a markdown↔Notion blocks converter so roadmap items sync as real page bodies, Notion OAuth wiring in provider auth, and the config/frontmatter/wave-config plumbing needed to store Notion project and item IDs alongside existing Asana and Linear support.

## Key choices

- **Notion items are real pages, not a text property.** Descriptions sync through page blocks so headings, lists, code blocks, quotes, dividers, to-dos, and inline formatting round-trip instead of being flattened.
- **Notion uses the existing PM shape instead of a special-case workflow.** Teams map to a top-level page, projects map to child databases, and wave config/frontmatter reuse the same provider dispatch patterns as Asana and Linear.
- **OAuth is the only supported auth path.** `lfq auth notion` / `lf ops auth notion` use a localhost callback on port `19223`; there is no Notion API-key fallback in PM ops.
- **Schema mapping is configurable.** `.lf/config.yaml` can override the Notion title/status/done/priority property names instead of hard-coding one database template.
- **Notion OAuth docs now match the implementation.** The reviewer-facing wave item now describes the shipped browser OAuth flow and Basic-auth token exchange instead of calling it PKCE.

## How it fits together

`provider_auth.rs` stores a Notion OAuth token, `ops/pm.rs` resolves that token and builds `NotionClient`, and `notion.rs` translates the generic PM operations into Notion REST calls. For item bodies, `notion_blocks.rs` converts markdown to block JSON on create/update and converts fetched block trees back to markdown on pull/status/sync.

## Risks and bottlenecks

- **Read amplification:** `list_items` is inherently N+1 because each page query needs a separate block-children fetch for the body.
- **Body rewrites:** `update_item` deletes top-level blocks and re-appends the new body, so concurrent edits in Notion and locally can still conflict at the page-body level.
- **Block coverage is intentionally partial:** unsupported Notion block types degrade to paragraphs, and list nesting is only preserved one level deep.
- **Local OAuth assumptions:** the flow assumes `NOTION_CLIENT_ID`, `NOTION_CLIENT_SECRET`, and localhost port `19223` are available during auth.

## What's not included

- Rich-text formatting for Notion comments
- Nested list/block structures deeper than one level
- A Notion API-key or internal-integration fallback path
- A broader PM model redesign beyond adding Notion to the existing provider architecture

## Validation

- `cargo test -p loopflow notion`
- `cargo test -p loopflow pm`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo fmt --check`
- `uv run pytest python/tests/`

All passed locally.
