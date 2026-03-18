---
asana_id: '1213717741038313'
linear_id: 9147c3a0-8f29-4d0c-b16f-d0ac3a5e72e5
---
# 08: Notion client

**Finish line:** `NotionClient` implements `PmProvider`, and a wave can set `pm.rw_provider: notion` or include `notion` in `export_providers` and sync through the same PM commands/lifecycle hooks as Asana and Linear.

## Context

Asana and Linear cover project-management-native tools. Notion covers teams that plan in docs and databases rather than dedicated PM software. The provider-role model and `PmProvider` seam are already in place — this item adds a third transport adapter without creating a second integration stack.

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
3. Add `notion_project` to wave PM config and `project_for(provider)` helpers so the provider-role model can address Notion the same way it addresses Asana and Linear.
4. Add `Notion` to `Provider` in `provider_auth.rs` with `api_key_env_name` → `NOTION_API_KEY`.
5. Route `ops/pm.rs` and `ops/export.rs` through the existing provider-role orchestration so Notion can act as the read/write provider or as an export-only mirror.
6. Use the shared test server (`pm::test_server`) and retry logic (`RATE_LIMIT_RETRIES`, `retry_after_delay`) from `pm::mod.rs`.

## Constraints

- Notion's block model is richer than markdown. For the first pass, store descriptions as a single paragraph block — don't attempt full markdown → Notion block conversion.
- Database property schema (status column name, etc.) should be configurable in `NotionConfig` rather than hardcoded.
- Same test pattern as Asana/Linear: axum test server with request capture.

## Done when

- `cargo test` passes with Notion client unit tests covering all 6 `PmProvider` methods
- `lf ops auth configure notion` stores and retrieves the token
- `lf ops pm init|import|export|status` work for a wave configured with Notion as the read/write or export provider
- Existing Asana and Linear exports are unaffected
