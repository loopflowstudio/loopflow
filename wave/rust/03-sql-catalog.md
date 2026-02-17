# 03: Shared SQL Catalog

Define each query once. Render per-dialect placeholder syntax at the edges.

## What exists after this

One SQL string per query operation. A thin dialect layer converts `?` placeholders to `$1..$N` for Postgres. Row mapping already shared via `StoreRow` trait. Adding a new query means writing it once.

## Current state

~45 SQL statements duplicated across `sqlite.rs` (940 lines) and `postgres.rs` (1074 lines). The SQL is nearly identical — same column lists, same WHERE clauses, same ORDER BY. Differences:

- Placeholder syntax: `?1` (sqlite) vs `$1` (postgres)
- Driver API: `conn.prepare().query_map()` vs `client.query()`
- Parameter passing: `params![]` vs `&[]`

Additionally, `store/mod.rs` (1284 lines) has ~226 lines of `pub async fn` forwarding methods on `Store` that dispatch to `SqliteStore` or `PostgresStore` by matching on `StoreBackend`. These are mechanical: match, call, return. The catalog may reduce some of this repetition if query dispatch can be generalized.

Row mapping is already shared: `StoreRow` trait in `store/rows.rs` abstracts over `rusqlite::Row` and `tokio_postgres::Row`. Functions like `map_wave_row()` work for both backends.

## Approach

### Step 1 — SQL catalog module

Create `lfd/store/catalog.rs`. Each query is a function returning the SQL string:

```rust
pub fn list_waves(repo_filter: bool) -> &'static str {
    if repo_filter {
        "SELECT id, name, repo, flow, direction, area, paused, status, iteration, created_at
         FROM waves WHERE repo = {p1} ORDER BY created_at DESC"
    } else {
        "SELECT id, name, repo, flow, direction, area, paused, status, iteration, created_at
         FROM waves ORDER BY created_at DESC"
    }
}
```

Use `{p1}`, `{p2}` as dialect-neutral placeholders.

### Step 2 — Dialect rendering

```rust
pub enum Dialect { Sqlite, Postgres }

pub fn render(sql: &str, dialect: Dialect) -> String {
    // Replace {p1} with ?1 or $1, {p2} with ?2 or $2, etc.
}
```

Keep it simple. No query builder, no ORM. Just string replacement at startup or compile time.

### Step 3 — Backend deduplication

Each backend's trait impl calls `catalog::list_waves(true)`, renders for its dialect, and executes. The query logic shrinks to: get SQL, bind params, map rows.

### Step 4 — Parity tests

Snapshot tests that render each catalog query for both dialects and compare against golden files. If a query changes, both backends update together.

## Key files

| File | Lines | What changes |
|------|-------|-------------|
| `lfd/store/catalog.rs` | new (~200) | All SQL strings defined once |
| `lfd/store/sqlite.rs` | ~940 → ~450 | Query bodies replaced with catalog + render |
| `lfd/store/postgres.rs` | ~1074 → ~550 | Same reduction |
| `lfd/store/mod.rs` | ~1284 | Forwarding methods unchanged; catalog reduces backend code they dispatch to |
| `lfd/store/rows.rs` | unchanged | Already shared |

## Risks

- **Edge cases in SQL between backends**: SQLite and Postgres have minor syntax differences beyond placeholders (e.g., `UPSERT` syntax, type casting). The catalog needs to handle these — likely with per-dialect query variants for the ~5 queries that differ.
- **mod.rs forwarding layer**: Stage 02 added ~226 lines of `pub async fn` on `Store` that match on backend and call the corresponding method. The catalog doesn't directly reduce this — it's a dispatch pattern, not SQL duplication. But it's worth noting that the store module is now 3298 lines across three files. The catalog should shrink it by ~1000 lines.

## Done when

- Each SQL query defined in one place
- `sqlite.rs` and `postgres.rs` don't contain duplicated SQL strings
- Snapshot tests verify both dialect renderings
- No regression in store test suite
