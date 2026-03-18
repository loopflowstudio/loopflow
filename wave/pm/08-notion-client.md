---
asana_id: '1213717741038313'
linear_id: 9147c3a0-8f29-4d0c-b16f-d0ac3a5e72e5
---
# 08: Notion client

**Finish line:** `NotionClient` implements `PmProvider` using Notion's REST API, and `lf ops export --wave <wave>` works end-to-end with `provider: notion` in wave config.

## Context

Asana and Linear cover project-management-native tools. Notion covers teams that plan in docs and databases rather than dedicated PM software. The `PmProvider` trait and export dispatcher already handle multi-provider dispatch — this item adds a third transport adapter without changing the shared machinery.

## What to build

### NotionClient

1. Implement `PmProvider` for Notion's REST API (databases as projects, pages as items).
2. Map `create_project` → create a database in a parent page.
3. Map `list_items` → query the database, paginate with `start_cursor`.
4. Map `create_item` / `update_item` → create/update pages with title and body properties.
5. Map `complete_item` → set a status/checkbox property to done.
6. Map `comment` → add a comment block to the page.
7. Auth via `NOTION_API_KEY` (internal integration token), stored through `lf ops auth configure notion`.

### Provider wiring

1. Add `Notion` variant to `PmProviderKind`.
2. Add `notion_id` field to `RoadmapItemFrontmatter` and extend `id_for`/`set_id` dispatch.
3. Add `Notion` to `Provider` in `provider_auth.rs` with `api_key_env_name` → `NOTION_API_KEY`.
4. Add Notion branch to export dispatcher in `ops/export.rs`.
5. Use the shared test server (`pm::test_server`) and retry logic (`RATE_LIMIT_RETRIES`, `retry_after_delay`) from `pm::mod.rs`.

## Constraints

- Notion's block model is richer than markdown. For the first pass, store descriptions as a single paragraph block — don't attempt full markdown → Notion block conversion.
- Database property schema (status column name, etc.) should be configurable in `NotionConfig` rather than hardcoded.
- Same test pattern as Asana/Linear: axum test server with request capture.

## Done when

- `cargo test` passes with Notion client unit tests covering all 6 `PmProvider` methods
- `lf ops auth configure notion` stores and retrieves the token
- `lf ops export --wave <wave>` creates/updates Notion database pages for a wave configured with `provider: notion`
- Existing Asana and Linear exports are unaffected
