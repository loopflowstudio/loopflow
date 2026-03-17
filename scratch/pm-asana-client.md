# Asana Client

## Problem

Loopflow's PM integration needs a concrete Asana REST client that implements `PmProvider`. The trait and shared types now live in `pm/mod.rs`, credentials flow through `Provider::Asana` and the encrypted token store, and `AsanaConfig` is parsed from `.lf/config.yaml`. What's missing is the HTTP layer that actually talks to Asana.

Teams using Asana for planning need their wave roadmap items to sync: create projects, list/create/update/complete tasks, and post comments. Without this, the PM wave's remaining items (bootstrap CLI, import/export steps, ingest integration, run lifecycle sync) are blocked.

## Approach

Single file: `rust/loopflow/src/lfd/pm/asana.rs`. Convert `pm.rs` into a module directory (`pm/mod.rs` + `pm/asana.rs`) to keep the shared types separate from provider implementations.

`AsanaClient` holds a `reqwest::Client`, the PAT (from token store), and `AsanaConfig` (workspace, default_team). Each `PmProvider` method maps 1:1 to an Asana REST endpoint.

### Structure

```
rust/loopflow/src/lfd/pm/
├── mod.rs          # existing types, trait, RoadmapItemDocument (moved from pm.rs)
└── asana.rs        # AsanaClient impl
```

### API mapping

| Trait method | HTTP | Asana endpoint | Notes |
|---|---|---|---|
| `create_project` | `POST /projects` | Needs `workspace` from config. Uses `default_team` if present. |
| `list_items` | `GET /projects/{id}/tasks` | `opt_fields=name,notes,completed,memberships.section.name` for rank derivation. Pagination via `offset`. |
| `create_item` | `POST /tasks` | `projects: [project_id]`, `name`, `notes` (markdown as plain text). |
| `update_item` | `PUT /tasks/{id}` | Sparse update — only send fields present in `PmItemUpdate`. |
| `complete_item` | `PUT /tasks/{id}` | `{ completed: true }` |
| `comment` | `POST /tasks/{id}/stories` | `{ text: body }` — plain text, not rich text. |

### Auth

Read PAT from the token store via `store.get_provider_token("asana")`. Thread the token as `Authorization: Bearer {pat}` on every request. No refresh logic — Asana PATs don't expire (only manual revocation).

Constructor: `AsanaClient::new(token: String, config: AsanaConfig)` — caller retrieves token from store. Client doesn't own store reference.

### Rate limiting

Asana returns `429` with `Retry-After` header. Retry up to 3 times with the server-specified delay (or 60s fallback). Log via `tracing::warn!` on each retry. Fail after retries exhausted.

### Pagination

`list_items` must handle Asana's offset-based pagination. Asana returns `next_page.offset` when there are more results. Collect all pages into a single `Vec<PmItem>`.

### Rank

Asana tasks within a project have a natural order. Use the position in the response array as the `rank` field (0-indexed). On `create_item`, use `insert_before`/`insert_after` to position if rank matters — for v1, append to end (rank from `PmItemCreate` is advisory, Asana doesn't have a numeric rank field).

### Response deserialization

Asana wraps responses in `{ "data": ... }`. Define minimal response structs:

```rust
#[derive(Deserialize)]
struct AsanaResponse<T> { data: T }

#[derive(Deserialize)]
struct AsanaTask {
    gid: String,
    name: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    completed: bool,
}

#[derive(Deserialize)]
struct AsanaProject { gid: String }

#[derive(Deserialize)]
struct AsanaStory { gid: String }

#[derive(Deserialize)]
struct AsanaPageInfo { offset: Option<String> }

#[derive(Deserialize)]
struct AsanaListResponse<T> {
    data: Vec<T>,
    next_page: Option<AsanaPageInfo>,
}
```

### Error handling

Map HTTP errors to `PmError::Message(...)` with the Asana error body included. Asana error bodies are `{ "errors": [{ "message": "..." }] }` — extract the first message when available.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Use `asana-rs` crate | Less code to write | Abandoned crate, last update 2020. API surface is small enough for raw reqwest. |
| Share `reqwest::Client` via `SafeHttpClient` | Consistent redirect/SSRF handling | Asana client only talks to `app.asana.com` — SSRF protection adds complexity for no benefit. |
| Store `AsanaConfig` in the client, read from store on each call | Lazy config loading | Config doesn't change during a session. Read once at construction. |
| `html_notes` with markdown→HTML conversion | Richer task descriptions in Asana | Conversion is lossy and fragile. Plain `notes` field is readable. Wave README says "accept the formatting loss." |

## Key decisions

**Module directory, not single file.** `pm.rs` becomes `pm/mod.rs` + `pm/asana.rs`. Linear client (item 03) will be `pm/linear.rs`. Keeps provider code isolated while sharing the trait.

**Constructor takes token string, not store reference.** The client is a pure HTTP adapter. Retrieving credentials is the caller's responsibility. This keeps the client testable without store mocks.

**Plain text notes, not HTML.** Asana's `notes` field accepts plain text. `html_notes` would require markdown-to-HTML conversion. The wave README explicitly says to accept the formatting loss.

**Retry-After compliance.** Respect the server's `Retry-After` value instead of implementing our own backoff curve. Asana's 1500 req/min limit is generous — retries should be rare.

**Append-only for rank.** Asana doesn't expose a numeric rank. Tasks have ordering within sections, but reordering requires `insert_before`/`insert_after` calls. For v1, new tasks append to end. The `rank` field in `PmItemCreate` is recorded but doesn't drive Asana positioning. `list_items` derives rank from response order.

## Scope

- In scope: `AsanaClient` implementing all 6 `PmProvider` methods, rate-limit retry, pagination, unit tests with mock HTTP server
- Out of scope: section/column management, task dependencies, custom fields, subtasks, attachments, webhooks

## Done when

```bash
cargo test -p loopflow pm::asana  # unit tests pass with mock server
cargo clippy -- -D warnings       # no warnings
```

All six trait methods (`create_project`, `list_items`, `create_item`, `update_item`, `complete_item`, `comment`) are implemented and tested against a mock HTTP server that returns realistic Asana responses.

Wave goal advanced: "Asana and Linear clients implement `PmProvider`" — this delivers the Asana half.
