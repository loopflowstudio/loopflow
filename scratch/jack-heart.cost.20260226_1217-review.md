# Usage API — Review Guide

## What was implemented

Three HTTP endpoints that aggregate persisted session metering events into queryable usage summaries:

1. **`GET /v0/sessions/{id}/usage`** — Token totals, turn count, context snapshot, and model breakdown for a single session.
2. **`GET /v0/waves/{id}/usage`** — Aggregate usage across all sessions in all runs of a wave, with per-step breakdown.
3. **`GET /v0/usage/summary`** — Fleet-wide grouped aggregation with filters (wave, flow, step, model, time range) and grouping dimensions (wave, flow, step, model, source).

Supporting changes:
- Two new store methods (`list_sessions_for_wave`, `list_sessions_filtered`) in both SQLite and Postgres backends.
- Pure aggregation module (`sessions/usage.rs`) with no DB dependency.
- DTOs in `http/dto.rs` that reuse aggregation types via `#[serde(flatten)]`.

## Key choices

| Decision | Why |
|----------|-----|
| Aggregate in Rust, not SQL | Avoids fragmenting logic across SQLite `json_extract` vs Postgres `jsonb` operators. Data volume is small (10–100 events per session). One code path, testable without a database. |
| No new tables or migrations | All data lives in `sessions` + `session_events`. New store methods query existing tables with new joins/filters. Schema stays stable. |
| Flat token fields (`tokens.input`, not `tokens.input_tokens`) | Redundant suffix inside a `tokens` object. Consistent with "name things after what they are." |
| Model counts = turn counts | A session can use multiple models. Mapping model → turn count reflects actual consumption, not config. |
| `group_by=source` + `model` filter → 400 | Cross-granularity ambiguity (source is session-level snapshot, model is turn-level). Explicit rejection with clear error. |
| Wave/flow filters join through `wave_runs` | Canonical metadata lives on `wave_runs`, not `session.config`. Avoids drift. |
| Time range on `sessions.created_at` | Simple, predictable. Event-time slicing deferred. |

## How it fits together

```
HTTP handler (routes/usage.rs)
  → store.list_session_events / list_sessions_for_wave / list_sessions_filtered
  → pure aggregation functions (sessions/usage.rs)
  → DTO construction (http/dto.rs)
  → JSON response
```

The store layer is a data fetcher. Aggregation lives above it. DTOs reuse the aggregation types (`TokenTotals`, `StepUsageAggregate`, `UsageSummaryGroupAggregate`) directly — no duplicate structs.

## Risks and bottlenecks

- **N+1 event loading**: Wave and summary endpoints load events per-session in a loop. Fine at current scale (tens of sessions per wave). If wave sessions grow to thousands, batch the event query or add a materialized `session_usage` table.
- **No caching**: Every request rescans events. Session events are immutable after session ends, so caching is safe. Defer until latency exceeds 200ms.
- **Dynamic SQL in `list_sessions_filtered`**: Builds SQL from optional predicates. Tested via integration tests but SQL injection risk is mitigated by parameterized queries (positional `?N` / `$N`).

## What's not included

- **Cost in USD** — Phase 03 adds model rates; this API returns token counts only.
- **Materialized views or caching** — Add when measured latency warrants.
- **Pagination on summary groups** — Group count is bounded by distinct dimension values.
- **Streaming/live usage** — SSE already delivers events; these endpoints aggregate completed or in-progress snapshots.
- **E2E smoke test coverage** — Unit and integration tests cover the new code; e2e smoke test extension deferred.

## Test coverage

| Layer | Tests | What they verify |
|-------|-------|-----------------|
| Unit (sessions/usage.rs) | 4 | Aggregation math: token sums, step rollup, model filter, source extraction |
| Integration (routes/usage.rs) | 4 | Full handler path: session creation → event seeding → endpoint call → JSON assertions |

All 8 tests pass. `cargo fmt`, `cargo clippy -- -D warnings` clean.

## Gate fix

- Replaced `()` error type on `GroupBy::FromStr` with `InvalidGroupBy` struct per CLAUDE.md style guide.
