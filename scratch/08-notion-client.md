---
asana_id: '1213717741038313'
linear_id: 9147c3a0-8f29-4d0c-b16f-d0ac3a5e72e5
---
# 08: Notion client

## Problem

Teams that plan in Notion databases have no way to use `lf ops pm` workflows. Asana and Linear are covered; Notion is the third major PM surface. Adding it means any team with a Notion workspace can point a wave at a database and get bidirectional item sync.

## Approach

Add `NotionClient` in `rust/loopflow/src/lfd/pm/notion.rs` implementing all 10 `PmProvider` methods against Notion's REST API (v2022-06-28). Wire it into the existing provider dispatch, auth, config, and frontmatter machinery. Follow the exact same structural patterns as `AsanaClient` and `LinearClient` — no new abstractions, no shared HTTP helpers beyond what already exists.

Add a markdown↔blocks converter (`pm/notion_blocks.rs`) so items are real Notion pages — authored and edited on either side.

Auth via OAuth, matching the Asana/Linear pattern. `lfq auth notion` opens the browser, catches the redirect, stores the token.

### Notion API mapping

| PmProvider method | Notion API call |
|---|---|
| `create_team` | Create a page (the "workspace" page that holds databases) |
| `find_team` | Search pages by title |
| `create_project` / `create_project_in_team` | Create a database as a child of the team page |
| `list_projects` | Search databases whose parent is the team page |
| `list_items` | POST `/databases/{id}/query` with pagination, then `GET /blocks/{page_id}/children` per item for body content |
| `create_item` | POST `/pages` with database parent and properties, then `PATCH /blocks/{page_id}/children` to append body blocks |
| `update_item` | PATCH `/pages/{id}` for properties + delete old blocks and append new blocks for body |
| `complete_item` | PATCH `/pages/{id}` setting the status property |
| `comment` | POST `/comments` with `parent.page_id` and a paragraph block |

### Database schema convention

Notion databases are schema-flexible. `NotionConfig` specifies which property names map to loopflow concepts:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct NotionConfig {
    /// Property name for the title column. Default: "Name".
    #[serde(default)]
    pub title_property: Option<String>,
    /// Property name for the status/checkbox column. Default: "Status".
    #[serde(default)]
    pub status_property: Option<String>,
    /// The value that means "done" in the status property. Default: "Done".
    #[serde(default)]
    pub done_value: Option<String>,
    /// Property name for priority. Default: "Priority".
    #[serde(default)]
    pub priority_property: Option<String>,
}
```

Defaults: title → `"Name"`, status → `"Status"`, done → `"Done"`, priority → `"Priority"`. This covers Notion's default database template and lets teams with custom schemas override without code changes.

### Client structure

```rust
pub struct NotionClient {
    client: reqwest::Client,
    token: String,
    config: NotionConfig,
    base_url: String,
}
```

Same pattern as `AsanaClient`: production constructor sets `base_url` to `https://api.notion.com/v1`, test constructor (`#[cfg(test)]`) accepts an arbitrary base URL for the test server.

Auth: `Authorization: Bearer {token}` + `Notion-Version: 2022-06-28` on every request. Token comes from the credential store via OAuth — no env var fallback. Rate-limit retry uses the shared `RATE_LIMIT_RETRIES` and `retry_after_delay` from `pm::mod.rs`.

### Markdown↔blocks conversion (`pm/notion_blocks.rs`)

Items are real markdown pages in the repo and real Notion pages in the database. The converter handles bidirectional translation.

**Markdown → Notion blocks** (`markdown_to_blocks`):

Parse markdown into Notion block JSON. Each block type maps directly:

| Markdown | Notion block type |
|---|---|
| `# Heading` | `heading_1` |
| `## Heading` | `heading_2` |
| `### Heading` | `heading_3` |
| plain paragraph | `paragraph` |
| `- item` | `bulleted_list_item` |
| `1. item` | `numbered_list_item` |
| `- [ ]` / `- [x]` | `to_do` |
| `` ```lang `` | `code` |
| `> quote` | `quote` |
| `---` | `divider` |

Inline formatting within any block's rich text:

| Markdown | Notion annotation |
|---|---|
| `**bold**` | `bold: true` |
| `*italic*` | `italic: true` |
| `` `code` `` | `code: true` |
| `~~strike~~` | `strikethrough: true` |
| `[text](url)` | `href` on the text span |

One level of nesting supported (indented list items become children of the parent block). Deeper nesting flattens — acceptable for wave items.

**Notion blocks → Markdown** (`blocks_to_markdown`):

Walk the block tree from `GET /blocks/{page_id}/children`, recursing into blocks that have `has_children: true`. Emit markdown for each block type. Rich text spans reconstruct inline formatting.

Unrecognized block types emit their plain text content as a paragraph — graceful degradation rather than data loss.

**Testing**: Pure functions, no HTTP. Test with markdown→blocks→markdown round-trips and known Notion JSON fixtures.

### Description handling

Each item is a full Notion page. The page body holds the description as native Notion blocks, converted to/from markdown via `notion_blocks.rs`.

- **`list_items`**: Query the database for pages, then `GET /blocks/{page_id}/children` per page to read the body. N+1 API calls — inherent to Notion's API. Convert blocks → markdown for the `PmItem.description` field.
- **`create_item`**: `POST /pages` to create the page with properties, then `PATCH /blocks/{page_id}/children` to append body blocks converted from the markdown description.
- **`update_item`**: When description changes, delete existing blocks and append new ones. Properties update via `PATCH /pages/{id}`.

### Priority handling

On `create_project`, create the database with a `select` property named per `config.priority_property` (default `"Priority"`) with options: Urgent, High, Medium, Low. On `create_item`, set the select value. On `list_items`, read the select value and map via `PriorityBucket::from_semantic_label`. Missing/unrecognized values default to `Low`.

### Team concept

Notion doesn't have teams. We use a top-level page as the "team" container — databases (projects) are created as children of this page. `create_team` creates a page in the workspace; `find_team` searches for it by title using `POST /search`.

### OAuth

`NotionOAuthBroker` follows the `LinearOAuthBroker` pattern — PKCE flow with a localhost redirect listener.

- Authorize: `https://api.notion.com/v1/oauth/authorize`
- Token exchange: `https://api.notion.com/v1/oauth/token` (Basic auth with client_id:client_secret, not POST body)
- Redirect: `http://localhost:19223/oauth/callback` (port 19223 to avoid collision with Linear's 19222)
- Env vars for app credentials: `NOTION_CLIENT_ID`, `NOTION_CLIENT_SECRET`
- `Provider::Notion` with `api_key_env_name` → `None` (OAuth only, no env var fallback)

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Description as rich_text property | No N+1, simpler reads | 2000 char limit. Notion is a writing surface — items need to be real pages with full markdown content. Property storage makes Notion a dumb mirror. |
| Plaintext-only descriptions (single paragraph block) | Much simpler | Silently drops formatting on round-trip. If someone edits in Notion with headings and lists, that content gets flattened on sync back. Not honest. |
| Internal integration token (`NOTION_API_KEY`) | Simpler auth setup | Inconsistent with Asana/Linear which use OAuth. OAuth is the standard auth path for all PM providers. |
| Shared HTTP/retry helper trait across all providers | Less duplication | Each provider's HTTP layer has enough quirks (GraphQL vs REST, auth headers, error shapes) that a shared trait would be a leaky abstraction. Copy the pattern, not the code. |
| Notion as a "database-only" provider (skip team/project hierarchy) | Simpler, fewer API calls | Breaks `lf ops pm init` which expects `create_team` → `create_project_in_team`. The team-as-page pattern costs ~20 lines and keeps the full workflow. |
| Status as checkbox instead of select | Simpler completion model | Select is more flexible (supports In Progress, Blocked, etc. later) and matches Notion's default database template. Checkbox is a fallback we could add later. |

## Key decisions

**Items are real Notion pages with full markdown↔blocks conversion.** Notion is a writing surface, not a mirror. People author and edit on both sides. The converter handles headings, lists, code blocks, quotes, inline formatting, and one level of nesting. Unrecognized block types degrade to plain text paragraphs.

**N+1 API calls for `list_items` are accepted.** Notion separates page properties from page content. Each item needs a `GET /blocks/{page_id}/children` call to read its body. This is inherent to Notion's API — no workaround exists.

**OAuth only, no API key fallback.** `lfq auth notion` opens the browser for OAuth. Consistent with Asana and Linear. `Provider::Notion` has `api_key_env_name` → `None`.

**Notion-Version header is pinned to `2022-06-28`.** This is Notion's stable API version. We don't negotiate versions — pin and move forward when needed.

**`find_team` uses `/search` endpoint.** Notion's search is workspace-wide and eventually consistent. In practice it finds pages within seconds of creation. The alternative (listing all pages) doesn't exist in Notion's API. Results are filtered client-side by parent ID.

**Priority is a select property, not a number.** Notion supports both, but select with semantic labels (Urgent/High/Medium/Low) is more readable in the Notion UI and matches Asana's enum approach.

**No workspace ID in config.** The OAuth token is scoped to the workspace the user authorizes. No need for a workspace config field.

## Scope

### In scope

- `NotionClient` implementing all 10 `PmProvider` methods
- Markdown↔Notion blocks converter (`pm/notion_blocks.rs`) with round-trip tests
- `NotionOAuthBroker` in `provider_auth.rs` following the Linear OAuth pattern
- `NotionConfig` in `engine/config.rs` with property name overrides
- `Notion` variant in `PmProviderKind`, `Provider`, `RoadmapItemFrontmatter`
- `notion_project` field in `WavePmConfig` and `PmConfig`
- `build_client_with_team` dispatch for Notion
- `project_key` returning `"notion_project"` for Notion
- Tests using the shared `test_server` pattern covering all 10 methods

### Out of scope

- Notion-specific CLI subcommands beyond what `lf ops pm` already provides
- Database template customization beyond property name mapping
- Notion comments with rich text formatting
- Nested blocks deeper than one level (flattened gracefully)

## Implementation plan

### 1. Markdown↔blocks converter (`pm/notion_blocks.rs`, new file)

Pure functions, no HTTP dependency. ~300 lines.

- `markdown_to_blocks(md: &str) -> Vec<Value>` — parse markdown, emit Notion block JSON
- `blocks_to_markdown(blocks: &[Value]) -> String` — walk block tree, emit markdown
- Rich text span handling for inline formatting (bold, italic, code, strikethrough, links)
- One level of nesting for list items
- Round-trip tests with known markdown↔JSON fixtures

### 2. Wiring (touch existing files)

**`pm/mod.rs`**: Add `Notion` to `PmProviderKind`. Add `notion_id` to `RoadmapItemFrontmatter`, extend `id_for`/`set_id`/`clear_id`/`is_empty`. Make `RATE_LIMIT_RETRIES` and `retry_after_delay` `pub(crate)`.

**`engine/config.rs`**: Add `NotionConfig` struct and `notion: NotionConfig` field to `Config`.

**`provider_auth.rs`**: Add `Notion` to `Provider` enum with `api_key_env_name` → `None`, `display_name` → `"Notion"`. Add `NotionOAuthBroker` following the `LinearOAuthBroker` pattern (PKCE, localhost redirect on port 19223, Basic auth for token exchange).

**`wave_config.rs`**: Add `notion_project: Option<String>` to `WavePmConfig`, extend `project_for`.

**`ops/pm.rs`**: Add `PmProviderKind::Notion` branch to `build_client_with_team`, `project_key`, and `PmConfig::project_for`. Token resolved from credential store (provider `"notion"`), no env var fallback. Import `NotionClient`.

### 3. NotionClient (`pm/notion.rs`, new file)

~500-600 lines following the `AsanaClient` pattern. Core methods:

- `send_json` — generic request helper with rate-limit retry loop
- `create_team` — `POST /pages` with title property under workspace root
- `find_team` — `POST /search` filtering for pages by title, client-side parent filter
- `create_project` / `create_project_in_team` — `POST /databases` with parent page ID, title, status select, and priority select schema
- `list_projects` — `POST /search` filtering for databases, client-side parent filter
- `list_items` — `POST /databases/{id}/query` with pagination, then `GET /blocks/{page_id}/children` per item, `blocks_to_markdown` for descriptions
- `create_item` — `POST /pages` with database parent and properties, then `PATCH /blocks/{page_id}/children` with `markdown_to_blocks` output
- `update_item` — `PATCH /pages/{id}` for properties + delete old blocks / append new blocks when description changes (early exit on noop)
- `complete_item` — `PATCH /pages/{id}` setting status property to done value
- `comment` — `POST /comments` with page parent and paragraph block

### 4. Tests

- **`notion_blocks.rs` tests**: Pure round-trip tests. Markdown → blocks → markdown for each supported block type. Known Notion JSON fixtures → expected markdown output.
- **`notion.rs` tests**: Same `test_server` pattern as `asana.rs`. Spawn mock server, queue responses, call methods, assert on captured requests and return values. One test per method minimum.

## Done when

```bash
cargo test -p loopflow notion     # all Notion client + blocks tests pass
cargo test -p loopflow pm         # existing PM tests unaffected
cargo clippy -- -D warnings       # no new warnings
```

- `lfq auth notion` completes OAuth flow and stores token
- `lf ops pm init` with `provider: notion` creates a database in Notion
- `lf ops pm pull` / `lf ops pm status` work with Notion-backed waves
- Items round-trip: markdown in repo ↔ native Notion blocks in database
- Asana and Linear paths are unchanged (no behavioral diff for existing providers)
