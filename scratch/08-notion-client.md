---
asana_id: '1213717741038313'
linear_id: 9147c3a0-8f29-4d0c-b16f-d0ac3a5e72e5
---
# 08: Notion client

## Problem

Teams that plan in Notion databases have no way to use `lf ops pm` workflows. Asana and Linear are covered; Notion is the third major PM surface. Adding it means any team with a Notion workspace can point a wave at a database and get bidirectional item sync.

## Approach

Add `NotionClient` in `rust/loopflow/src/lfd/pm/notion.rs` implementing all 10 `PmProvider` methods against Notion's REST API (v2022-06-28). Wire it into the existing provider dispatch, auth, config, and frontmatter machinery. Follow the exact same structural patterns as `AsanaClient` and `LinearClient` — no new abstractions, no shared HTTP helpers beyond what already exists.

### Notion API mapping

| PmProvider method | Notion API call |
|---|---|
| `create_team` | Create a page (the "workspace" page that holds databases) |
| `find_team` | Search pages by title |
| `create_project` / `create_project_in_team` | Create a database as a child of the team page |
| `list_projects` | Search databases whose parent is the team page |
| `list_items` | POST `/databases/{id}/query` with pagination via `start_cursor` |
| `create_item` | POST `/pages` with parent database ID |
| `update_item` | PATCH `/pages/{id}` with property updates |
| `complete_item` | PATCH `/pages/{id}` setting the status/checkbox property |
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

Auth: `Authorization: Bearer {token}` + `Notion-Version: 2022-06-28` on every request. Rate-limit retry uses the shared `RATE_LIMIT_RETRIES` and `retry_after_delay` from `pm::mod.rs`.

### Description handling

Notion uses a block model, not plaintext. For this first pass:

- **Write**: descriptions become a single `paragraph` block with one `text` rich-text object. No markdown parsing.
- **Read**: extract text content from all blocks in the page body by calling `GET /blocks/{page_id}/children` and concatenating `rich_text[].plain_text` values, separated by newlines.

This is intentionally simple. Full markdown↔blocks conversion is a future item.

### Priority handling

On `create_project`, create the database with a `select` property named per `config.priority_property` (default `"Priority"`) with options: Urgent, High, Medium, Low. On `create_item`, set the select value. On `list_items`, read the select value and map via `PriorityBucket::from_semantic_label`. Missing/unrecognized values default to `Low`.

### Team concept

Notion doesn't have teams. We use a top-level page as the "team" container — databases (projects) are created as children of this page. `create_team` creates a page in the workspace; `find_team` searches for it by title using `POST /search`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Full markdown→blocks conversion | Rich descriptions in Notion | Massive complexity for marginal value in a sync tool. Paragraph-only is honest about what we support. Add later when there's demand. |
| Shared HTTP/retry helper trait across all providers | Less duplication | Each provider's HTTP layer has enough quirks (GraphQL vs REST, auth headers, error shapes) that a shared trait would be a leaky abstraction. Copy the pattern, not the code. |
| Notion as a "database-only" provider (skip team/project hierarchy) | Simpler, fewer API calls | Breaks `lf ops pm init` which expects `create_team` → `create_project_in_team`. The team-as-page pattern costs ~20 lines and keeps the full workflow. |
| Status as checkbox instead of select | Simpler completion model | Select is more flexible (supports In Progress, Blocked, etc. later) and matches Notion's default database template. Checkbox is a fallback we could add later. |

## Key decisions

**Notion-Version header is pinned to `2022-06-28`.** This is Notion's stable API version. We don't negotiate versions — pin and move forward when needed.

**Description reads require a second API call.** Notion separates page properties from page content (blocks). `list_items` will make N+1 calls (1 query + 1 blocks fetch per page) to get descriptions. This is unavoidable without Notion changing their API. For large databases, we could skip description fetching and only load on demand — but for now, correctness over performance.

**`find_team` uses `/search` endpoint.** Notion's search is workspace-wide and eventually consistent. In practice it finds pages within seconds of creation. The alternative (listing all pages) doesn't exist in Notion's API.

**Priority is a select property, not a number.** Notion supports both, but select with semantic labels (Urgent/High/Medium/Low) is more readable in the Notion UI and matches Asana's enum approach.

**No workspace ID in config.** Unlike Asana, Notion's internal integration token is scoped to a single workspace at creation time. No need for a workspace config field.

## Scope

### In scope

- `NotionClient` implementing all 10 `PmProvider` methods
- `NotionConfig` in `engine/config.rs` with property name overrides
- `Notion` variant in `PmProviderKind`, `Provider`, `RoadmapItemFrontmatter`
- `notion_project` field in `WavePmConfig` and `PmConfig`
- `build_client_with_team` dispatch for Notion
- `project_key` returning `"notion_project"` for Notion
- Tests using the shared `test_server` pattern covering all 10 methods
- `lf ops auth configure notion` reading `NOTION_API_KEY`

### Out of scope

- Markdown ↔ Notion block conversion
- OAuth flow for Notion (internal integration tokens only for now)
- Notion-specific CLI subcommands beyond what `lf ops pm` already provides
- Database template customization beyond property name mapping
- Notion comments with rich text formatting

## Implementation plan

### 1. Wiring (touch existing files)

**`pm/mod.rs`**: Add `Notion` to `PmProviderKind`. Add `notion_id` to `RoadmapItemFrontmatter`, extend `id_for`/`set_id`/`clear_id`/`is_empty`. Make `RATE_LIMIT_RETRIES` and `retry_after_delay` `pub(crate)`.

**`engine/config.rs`**: Add `NotionConfig` struct and `notion: NotionConfig` field to `Config`.

**`provider_auth.rs`**: Add `Notion` to `Provider` enum with `api_key_env_name` → `"NOTION_API_KEY"`, `display_name` → `"Notion"`.

**`wave_config.rs`**: Add `notion_project: Option<String>` to `WavePmConfig`, extend `project_for`.

**`ops/pm.rs`**: Add `PmProviderKind::Notion` branch to `build_client_with_team`, `project_key`, and `PmConfig::project_for`. Import `NotionClient`.

### 2. NotionClient (`pm/notion.rs`, new file)

~400-500 lines following the `AsanaClient` pattern. Core methods:

- `send_json` — generic request helper with rate-limit retry loop
- `create_team` — `POST /pages` with title property under workspace root
- `find_team` — `POST /search` filtering for pages by title
- `create_project` / `create_project_in_team` — `POST /databases` with parent page ID, title, and priority select schema
- `list_projects` — `POST /search` filtering for databases with parent page
- `list_items` — `POST /databases/{id}/query` with pagination, then `GET /blocks/{page_id}/children` per item for descriptions
- `create_item` — `POST /pages` with database parent and properties
- `update_item` — `PATCH /pages/{id}` with changed properties (early exit on noop)
- `complete_item` — `PATCH /pages/{id}` setting status property to done value
- `comment` — `POST /comments` with page parent and paragraph block

### 3. Tests

Same pattern as `asana.rs` tests: spawn test server with queued responses, create client with test base URL, call methods, assert on captured requests and return values. One test per method minimum.

## Done when

```bash
cargo test -p loopflow notion     # all Notion client tests pass
cargo test -p loopflow pm         # existing PM tests unaffected
cargo clippy -- -D warnings       # no new warnings
```

- `lf ops auth configure notion` stores `NOTION_API_KEY` via the credential store
- `lf ops pm init` with `provider: notion` creates a database in Notion
- `lf ops pm pull` / `lf ops pm status` work with Notion-backed waves
- Asana and Linear paths are unchanged (no behavioral diff for existing providers)
