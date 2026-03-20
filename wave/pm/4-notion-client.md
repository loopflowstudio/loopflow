---
asana_id: '1213717741038313'
linear_id: 792860f0-3769-4f81-ab01-ff515f1c1e46
---
# 11: Notion client

**Finish line:** Notion databases join the `pm init` / `pm pull` / `pm sync` lifecycle as a full peer to Asana and Linear. Items are real Notion pages with native formatting — bidirectional markdown↔blocks conversion, OAuth auth, the whole thing.

Design doc: `scratch/08-notion-client.md`

## What to build

### NotionClient (`pm/notion.rs`)

Implement all 10 `PmProvider` methods against Notion's REST API (v2022-06-28). Same structural patterns as `AsanaClient` — thin transport adapter, rate-limit retry, test server mocks.

### Markdown↔blocks converter (`pm/notion_blocks.rs`)

Bidirectional conversion so items are real pages on both sides. Headings, paragraphs, lists, code blocks, quotes, dividers, to-do items. Inline formatting: bold, italic, code, strikethrough, links. One level of list nesting. Unrecognized block types degrade to plain text.

### Notion OAuth (`provider_auth.rs`)

`NotionOAuthBroker` following the `LinearOAuthBroker` pattern. PKCE flow, localhost redirect listener on port 19223. No API key fallback.

### Provider wiring

Add `Notion` to `PmProviderKind`, `Provider`, `RoadmapItemFrontmatter`, `WavePmConfig`, `PmConfig`, `build_client_with_team`, `project_key`. `NotionConfig` in `engine/config.rs` with property name overrides (title, status, done value, priority).

## Prereqs

- ~~Bucketed priority model across prompts, ingest, Asana, and Linear~~ — shipped

## Constraints

- Items are real Notion pages — the page body is the description, not a property field.
- N+1 API calls for `list_items` (1 query + 1 block fetch per page) is inherent to Notion's API.
- OAuth only. No `NOTION_API_KEY` env var fallback.
- Keep the adapter thin. Provider clients translate API semantics, nothing more.

## Done when

- `lfq auth notion` completes OAuth flow and stores token
- `lf ops pm init` with `provider: notion` creates a database in Notion
- `lf ops pm pull` / `lf ops pm status` work with Notion-backed waves
- Items round-trip: markdown in repo ↔ native Notion blocks in database
- Asana and Linear paths unchanged
