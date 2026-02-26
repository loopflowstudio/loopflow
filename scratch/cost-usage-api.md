# Usage API

Aggregate persisted metering events into queryable usage summaries for sessions, waves, and the fleet.

## Problem

Phase 01 shipped: every turn emits `TurnUsage`, every session starts with a `ContextSnapshot`. The data sits in `session_events` rows, queryable only by scanning raw JSON. There's no way to ask "how many tokens did this wave burn?" or "which step is most expensive?" without writing custom SQL.

Phase 02 turns that raw event stream into three HTTP endpoints that power everything downstream — Concerto inline views (Phase 04), the analytics dashboard (Phase 05), and `lfq usage` (Phase 06).

## Approach

Scan-and-aggregate in Rust. No materialized views, no caching layer, no new tables. Session events are small (10–100 per session), and even fleet-level queries touch thousands of rows at most. Compute on demand, optimize later if needed.

Three endpoints, each a different aggregation scope:

### 1. `GET /v0/sessions/{id}/usage` → `SessionUsage`

Sum all `TurnUsage` events for one session. Include `ContextSnapshot` if present.

```json
{
  "object": "session_usage",
  "session_id": "ses_abc123",
  "tokens": {
    "input": 45200,
    "output": 3800,
    "reasoning": 1200,
    "cache_read": 12000,
    "cache_write": 8000
  },
  "turns": 7,
  "context": {
    "sources": { "step": 1200, "direction": 400, "diff": 8500, "repo_doc": 2100 },
    "budget": 200000,
    "total": 12200,
    "diff_tier": "UnifiedDiff"
  },
  "models": { "claude-sonnet-4": 5, "claude-haiku-4-5": 2 },
  "session": {
    "step": "implement",
    "wave": "engbot",
    "status": "ended",
    "created_at": "2026-02-26T10:00:00Z",
    "ended_at": "2026-02-26T10:12:00Z"
  }
}
```

**Implementation**: Call `store.list_session_events(session_id, None)`. Filter for `TurnUsage` and `ContextSnapshot` variants. Sum token fields. Count distinct models. Return with session metadata from `store.get_session(session_id)`.

### 2. `GET /v0/waves/{id}/usage` → `WaveUsage`

Aggregate across all sessions in all runs of a wave.

```json
{
  "object": "wave_usage",
  "wave_id": "engbot",
  "tokens": {
    "input": 890000,
    "output": 72000,
    "reasoning": 15000,
    "cache_read": 210000,
    "cache_write": 45000
  },
  "sessions": 18,
  "turns": 142,
  "models": { "claude-sonnet-4": 95, "claude-haiku-4-5": 47 },
  "by_step": {
    "implement": { "input": 520000, "output": 45000, "sessions": 8, "turns": 85 },
    "gate": { "input": 220000, "output": 18000, "sessions": 6, "turns": 35 },
    "compress": { "input": 150000, "output": 9000, "sessions": 4, "turns": 22 }
  }
}
```

**Implementation**: New store method `list_sessions_for_wave(wave_id)` that joins `sessions` → `wave_runs` on `wave_run_id` → `waves` on `wave_id`. For each session, aggregate its events (same logic as session usage). Roll up into wave-level totals with per-step breakdown from `session.config.step`.

### 3. `GET /v0/usage/summary` → `UsageSummary`

Fleet-wide grouped aggregation. Powers the analytics dashboard.

**Query parameters**:

| Param | Type | Description |
|-------|------|-------------|
| `wave` | string | Filter to one wave |
| `step` | string | Filter to one step |
| `model` | string | Filter to one model |
| `from` | ISO 8601 | Start of time range |
| `to` | ISO 8601 | End of time range |
| `group_by` | string | Dimension: `wave`, `step`, `model`, `source` |

```json
{
  "object": "usage_summary",
  "group_by": "step",
  "from": "2026-02-20T00:00:00Z",
  "to": "2026-02-26T23:59:59Z",
  "groups": [
    {
      "key": "implement",
      "tokens": { "input": 1200000, "output": 98000, "reasoning": 22000 },
      "sessions": 24,
      "turns": 310
    },
    {
      "key": "gate",
      "tokens": { "input": 680000, "output": 52000, "reasoning": 8000 },
      "sessions": 18,
      "turns": 95
    }
  ]
}
```

**Implementation**: New store method `list_sessions_filtered(filters)` that queries sessions with optional wave/step/time predicates. Step and wave filter via `session.config` JSON extraction in SQL (`json_extract(config, '$.step')` for SQLite, `config->>'step'` for Postgres). Time range filters on `sessions.created_at`. Model filtering happens post-scan since model is per-turn, not per-session.

For `group_by=source`, aggregate `ContextSnapshot.sources` across matching sessions — shows where input tokens come from (step prompts, diffs, docs, etc.).

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| SQL aggregation with JSON extraction | Faster at scale, pushes work to DB | Fragments logic across two SQL dialects (SQLite json_extract vs Postgres jsonb). Scan-and-aggregate is simple and the data volume is small for v0. |
| Materialized `session_usage` table | Best query performance, enables SQL-native grouping | Adds a table, a migration, and write-time aggregation logic. Premature — we don't have performance data suggesting it's needed. Add if summary queries exceed 200ms. |
| Cache layer (in-memory LRU) | Eliminates repeated computation | Session events are immutable after session ends, so caching would help. But adds invalidation complexity. Defer until there's measured latency. |
| Aggregate in the store trait | Clean separation | Puts aggregation logic in two places (SQLite + Postgres). Better to aggregate in Rust above the store layer, keeping store methods as data fetchers. |

## Key decisions

**Aggregate in Rust, not SQL.** The store layer fetches raw events; a `usage` module in `http/` or `sessions/` sums them. This keeps the store trait simple (one implementation path), avoids JSON extraction differences between SQLite and Postgres, and makes the aggregation logic testable without a database.

**No new tables or migrations.** All data already lives in `sessions` and `session_events`. New store methods query existing tables with new filters. Schema stays stable.

**Flat token fields, not nested.** `tokens.input` not `tokens.input_tokens`. The `_tokens` suffix is redundant when the parent key is `tokens`. Matches the "name things after what they are" principle.

**Per-step breakdown in wave usage.** The wave endpoint includes `by_step` because that's the first question an operator asks: "which step is burning tokens?" This avoids a follow-up query. No per-run breakdown — that's a drill-down the summary endpoint handles.

**Model counts are turn counts, not session counts.** A session can use multiple models (e.g., haiku for fast checks, sonnet for implementation). `models` maps model name → number of turns that used it. This is more useful than "which model was configured" because it reflects actual consumption.

**Session metadata in session usage.** Include step, wave, status, and timestamps so the caller doesn't need a second request to contextualize the numbers.

**Filters in SQL, grouping in Rust.** The store filters sessions by wave/step/time using SQL (efficient). Grouping by model or source happens in Rust after scanning events (necessary because those dimensions live inside event JSON). Hybrid approach: SQL narrows the set, Rust does the final aggregation.

## Scope

**In scope:**
- Three HTTP endpoints with JSON responses
- New store methods for filtered session queries
- Aggregation module with unit tests
- Integration test: create sessions with events, query usage endpoints

**Out of scope:**
- Materialized views or caching (add when latency warrants)
- `flow` as a filter/group-by dimension (flow lives on wave_run, not session — would require an extra join through wave_runs. Step is the more useful granularity. Add flow later if operators ask for it.)
- Cost in USD (Phase 03 adds model rates; usage API returns tokens only)
- Streaming/live usage (SSE already delivers events; aggregation is for completed or in-progress snapshots)
- Pagination on summary groups (group count is bounded by distinct values of the dimension)

## Implementation plan

### 1. Aggregation module — `rust/loopflow/src/sessions/usage.rs`

Pure functions, no DB dependency. Takes `Vec<PersistedSessionEvent>` + `Session`, returns usage DTOs.

```rust
pub fn aggregate_session_usage(
    session: &Session,
    events: &[PersistedSessionEvent],
) -> SessionUsageDto { ... }

pub fn aggregate_wave_usage(
    wave_id: &str,
    sessions: &[(Session, Vec<PersistedSessionEvent>)],
) -> WaveUsageDto { ... }

pub fn aggregate_summary(
    group_by: GroupBy,
    sessions: &[(Session, Vec<PersistedSessionEvent>)],
) -> UsageSummaryDto { ... }
```

### 2. Store methods — add to `SessionStore` trait

```rust
async fn list_sessions_for_wave(&self, wave_id: &str) -> StoreResult<Vec<Session>>;
async fn list_sessions_filtered(&self, filters: &SessionFilters) -> StoreResult<Vec<Session>>;
```

`SessionFilters`:
```rust
pub struct SessionFilters {
    pub wave: Option<String>,
    pub step: Option<String>,
    pub from: Option<i64>,    // unix timestamp
    pub to: Option<i64>,
}
```

`list_sessions_for_wave` joins through `wave_runs` to find all sessions belonging to any run of that wave. `list_sessions_filtered` applies optional predicates. Step filter uses `json_extract(config, '$.step')` (SQLite) / `config->>'step'` (Postgres).

### 3. HTTP handlers — `rust/loopflow/src/http/routes/usage.rs`

Three handler functions following the existing pattern. Register under `/v0/sessions/{id}/usage`, `/v0/waves/{id}/usage`, `/v0/usage/summary`.

### 4. DTOs — `rust/loopflow/src/http/dto.rs`

Add `SessionUsageDto`, `WaveUsageDto`, `UsageSummaryDto`, `TokenTotals`, `ContextSnapshotDto`. All derive `Serialize, Debug`.

### 5. Tests

- Unit tests for aggregation functions (mock events, verify sums)
- Handler integration tests (create sessions with events via store, hit endpoints, verify JSON)
- Add to e2e smoke test: create a session, emit usage events, query usage endpoint

## Done when

```bash
# Session usage
curl http://localhost:4242/v0/sessions/ses_abc/usage
# → 200 with token totals, turn count, context snapshot, model breakdown

# Wave usage
curl http://localhost:4242/v0/waves/engbot/usage
# → 200 with aggregate totals, per-step breakdown, session/turn counts

# Summary grouped by step
curl "http://localhost:4242/v0/usage/summary?group_by=step&from=2026-02-20T00:00:00Z"
# → 200 with groups array, each group has key + token totals + counts
```

Wave goals advanced: "Surface tokens inline at every level of the Concerto hierarchy" (this API is the data source) and "Provide a dedicated analytics surface with work lens" (summary endpoint powers it).
