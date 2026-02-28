# Analytics Dashboard — Review

## What was implemented

Three deliverables from the design doc, delivered as a single cohesive branch:

1. **`GET /v0/usage/timeseries` endpoint** — Time-bucketed usage data with day/week/month granularity and grouping by wave/flow/step/model/source. Built on a shared `validate_usage_query` → `load_usage_session_data` pipeline that the existing `/usage/summary` endpoint now also uses, eliminating duplication.

2. **`cost_usd` computation at ingestion** — `populate_turn_usage_cost()` runs on every `TurnUsage` event before persistence. Multiplies each token category by `CostRates` from `lookup_cost_rates(harness, model)`. Only applies to OpenCode sessions (where per-token billing exists). Existing cost values are never overwritten.

3. **Concerto analytics view** — Global `AnalyticsDashboardView` accessible from macOS sidebar, iPhone tab bar, and iPad toolbar. Two lenses: Work (line chart, groupable by wave/flow/step/model with period picker) and Prompt (stacked bar chart of input token composition by source). Uses SwiftUI Charts.

## Key choices

| Decision | Why |
|----------|-----|
| Separate `/timeseries` endpoint | Time-series data has two dimensions (time × grouping). Overloading `/summary` would make response shape conditional. |
| `ValidatedUsageQuery` struct + shared pipeline | Both `/summary` and `/timeseries` validate the same params. Extracted once, reused twice. |
| Batch wave_run metadata fetch via `load_wave_run_metadata` | Deduplicates IDs so each wave_run is fetched at most once, avoiding N+1 queries. |
| `TimeBucket::start_date` for week bucketing | Uses Monday start. `format_period` uses `YYYY-MM-DD` for day/week and `YYYY-MM` for month — straightforward to parse on clients. |
| `GroupBy::Source` rejects model filter | Source aggregation uses `ContextSnapshot` (pre-turn), not per-turn tokens. Combining with model filter would be misleading. |
| Ingestion-time cost (`populate_turn_usage_cost`) | Financial data records the price at transaction time. Query-time computation would use current rates on historical data. |
| Global analytics view, not per-wave tab | Cross-wave comparison is the primary use case — "which wave is expensive?" requires seeing all waves together. |
| `showingAnalytics` on `RepoState` | Lightweight boolean toggle. When a wave is selected, analytics dismisses. When analytics opens, wave selection clears. Mutual exclusion without complex routing. |

## How it fits together

```
Client (Concerto)                    Server (lfd)
─────────────────                    ────────────
AnalyticsDashboardView               GET /v0/usage/timeseries
  → RepoState.usageTimeseries()        → validate_usage_query()
    → LocalWaveService.usageTimeseries()  → load_usage_session_data()
      → HTTP GET with query params           → aggregate_timeseries()
                                               → aggregate_summary() per bucket

SessionManager.emit_event()          populate_turn_usage_cost()
  → on TurnUsage events                → lookup_cost_rates(harness, model)
    → compute_usage_cost()               → persist with cost_usd
```

Swift DTOs (`UsageTimeseries`, `UsageSummary`, `TokenTotals`, etc.) mirror Rust response types with `CodingKeys` for snake_case mapping.

## Risks and bottlenecks

- **N+1 wave_run lookups** — `load_wave_run_metadata` fetches each unique wave_run individually. For high session counts, a batch store method would be faster. Current approach deduplicates IDs, so it's bounded by distinct wave_runs, not sessions.
- **Full event scan per session** — `aggregate_session_events` iterates all persisted events. For sessions with thousands of turns, this becomes expensive. A materialized aggregate per session would eliminate this, but the design doc defers caching until latency exceeds 200ms.
- **No pagination on timeseries** — A 90-day query with many waves could return a large response. Acceptable for now given typical usage patterns (tens of waves, not thousands).

## What's not included

- **Caching/materialized views** — Per design doc: add when latency exceeds 200ms.
- **`lfq usage` CLI command** — Deferred to Phase 06.
- **Cost caps, auto-downgrade, billing UI** — Explicitly out of scope per wave vision.
- **Session/AgentRun unification** — Doc comments added to both types documenting the relationship instead.

## Tests

| Suite | Tests | Status |
|-------|-------|--------|
| `usage.rs` unit tests | 6 (aggregate events, wave rollup, model filter, source, day timeseries, week timeseries) | Pass |
| `routes/usage.rs` integration tests | 5 (session usage, wave usage, source+model rejection, filters, timeseries by day) | Pass |
| `mod.rs` cost tests | 3 (populate cost, don't override, integration with session manager) | Pass |
| `UsageAnalyticsDecodingTests.swift` | 2 (timeseries decode, summary decode) | Pass |
| Full Swift suite | 215 tests | Pass |

## Gate fixes applied

- Added `.accessibilityLabel("Analytics")` to iPad analytics toolbar button (icon-only button per VISUAL_DESIGN checklist).
