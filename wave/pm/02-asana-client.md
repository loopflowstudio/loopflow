# 02: Asana client

**Finish line:** `AsanaClient` implements `PmProvider` over the Asana REST API. All trait methods work against a real Asana workspace.

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

### Auth

`Authorization: Bearer {pat}` header. PAT retrieved from provider credential storage.

### Rich text

Asana descriptions use a rich text format. For v1: write markdown as plain text in the `notes` field (plain text description), not `html_notes`. Accept the formatting loss — it's readable, just not styled.

### Rate limiting

Asana returns `429` with `Retry-After` header. Implement basic retry with backoff. Log when rate-limited.

## Constraints

- No Asana SDK crate — use `reqwest` directly. The API surface is small enough.
- Deserialize only the fields we need. Asana responses are verbose.
- Test with a real Asana workspace (integration test, not unit test).

## Done when

- `AsanaClient::new(pat)` constructs a client
- All six trait methods work against the Asana API
- `cargo test` passes with a mock HTTP server (for unit tests)
- Integration test against real Asana documents the manual verification
