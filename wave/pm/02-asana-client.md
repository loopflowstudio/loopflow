# 02: Asana client

**Finish line:** `AsanaClient` implements `PmProvider` over the Asana REST API. All trait methods work against a real Asana workspace.

The shared PM types already exist in `rust/loopflow/src/lfd/pm/mod.rs`, and Asana credentials already flow through `Provider::Asana` in provider auth. This item is the REST mapping layer plus the config-driven project bootstrap details that sit on top.

## What to build

Rust HTTP client using `reqwest`. Asana REST API v1 (`https://app.asana.com/api/1.0/`).

### Endpoints needed

| Trait method | Asana endpoint |
|-------------|----------------|
| `create_project` | `POST /projects` |
| `list_items` | `GET /projects/{id}/tasks` |
| `create_item` | `POST /tasks` |
| `update_item` | `PUT /tasks/{id}` |
| `complete_item` | `PUT /tasks/{id}` (set `completed: true`) |
| `comment` | `POST /tasks/{id}/stories` |

### Auth and config

- `Authorization: Bearer {pat}` header. PAT comes from the existing encrypted provider-token store.
- Project creation should read `.lf/config.yaml` `asana.workspace` and optional `asana.default_team` from `engine::config` rather than asking callers to thread raw IDs everywhere.
- Fail clearly on create-project paths when required workspace/team data is missing.

### Rich text

Asana descriptions use a rich text format. For v1: write markdown as plain text in the `notes` field, not `html_notes`. Accept the formatting loss — readable beats clever conversion here.

### Rate limiting

Asana returns `429` with `Retry-After` header. Implement basic retry with backoff. Log when rate-limited.

## Constraints

- No Asana SDK crate — use `reqwest` directly. The API surface is small enough.
- Deserialize only the fields we need. Asana responses are verbose.
- Keep external IDs as strings all the way through `PmItem` / `pm_id`.
- Test with a real Asana workspace (integration test, not unit test).

## Done when

- `AsanaClient::new(...)` constructs a client from stored auth + config
- All six trait methods work against the Asana API
- `cargo test` passes with a mock HTTP server (for unit tests)
- Integration test against real Asana documents the manual verification
