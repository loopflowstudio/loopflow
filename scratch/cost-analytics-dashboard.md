# Analytics Dashboard

## Problem

An operator managing 10+ parallel waves has no way to see token trends over time or understand what's eating their context budget. The backend infrastructure is complete — usage endpoints, aggregation, context snapshots, cost rates — but there's no surface for it. The operator can't answer "which wave is expensive?", "is implement or gate burning more tokens?", or "why is diff consuming 70% of my context?"

## Approach

Three deliverables:

### 1. Time-series API (`GET /v0/usage/timeseries`)

The existing `/usage/summary` returns flat aggregates — good for snapshots, useless for trends. Add a purpose-built endpoint that returns time-bucketed groups.

```
GET /v0/usage/timeseries?bucket=day&group_by=wave&from=2026-02-01T00:00:00Z&to=2026-02-28T00:00:00Z
```

Response:

```json
{
  "object": "usage_timeseries",
  "bucket": "day",
  "group_by": "wave",
  "buckets": [
    {
      "period": "2026-02-01",
      "groups": [
        { "key": "engbot", "tokens": { "input": 45200, "output": 8100, ... }, "sessions": 3, "turns": 12 },
        { "key": "ux", "tokens": { "input": 12300, "output": 2100, ... }, "sessions": 1, "turns": 4 }
      ]
    },
    {
      "period": "2026-02-02",
      "groups": [...]
    }
  ]
}
```

Accepts same filters as `/usage/summary` (wave, flow, step, model, from, to). `bucket` is `day`, `week`, or `month`. Bucketing uses session `created_at`.

This is a new endpoint, not overloading `/usage/summary`. Time-series data has fundamentally different shape (two-dimensional: time x grouping) vs flat aggregates.

### 2. `cost_usd` computation at ingestion

When a `TurnUsage` event is recorded, compute `cost_usd` from `lookup_cost_rates(harness, model)`. Record the price at transaction time — if rates change later, historical data reflects the rate that was active when the session ran.

In `SessionManager` (or wherever `TurnUsage` events are created from harness output), before persisting:

```rust
if usage.cost_usd.is_none() {
    if let Some(rates) = lookup_cost_rates(&session.harness, model) {
        usage.cost_usd = Some(compute_cost(&usage, &rates));
    }
}
```

`compute_cost` multiplies each token category by its per-million-token rate:

```rust
fn compute_cost(usage: &TurnUsage, rates: &CostRates) -> f64 {
    let mtok = |n: u64| n as f64 / 1_000_000.0;
    mtok(usage.input_tokens) * rates.input_per_mtok
        + mtok(usage.output_tokens) * rates.output_per_mtok
        + mtok(usage.cache_read_tokens.unwrap_or(0)) * rates.cache_read_per_mtok
        + mtok(usage.cache_write_tokens.unwrap_or(0)) * rates.cache_write_per_mtok
}
```

OpenCode sessions get real USD costs. Claude/Codex sessions keep `cost_usd: None` (subscription — no per-token billing).

CostRates are already populated for OpenCode's three Zen models (kimi-k2, qwen3-coder, qwen3-max) in `providers.rs`. No additional rate work needed.

### 3. Concerto analytics view (SwiftUI)

A **global** analytics view, not a per-wave tab. The whole point is cross-wave comparison. Accessible from a top-level navigation item in the sidebar (macOS) and root TabView (iOS).

Two lenses, switchable via segmented picker:

**Work lens** — Line chart of total tokens over time (daily default). Grouping selector switches between wave/flow/step/model, rendering each group as a separate series. Period picker: 7d / 30d / 90d. Tap a data point to see the breakdown.

**Prompt lens** — Stacked bar chart of input token composition by source (step, diff, area, repo_doc, direction, wave, clipboard, etc.). Each bar is one session or one time bucket. Filterable by wave/flow/step. Surfaces the "token tax" — which context sources dominate.

Uses **SwiftUI Charts** (built-in, available at our deployment targets: macOS 15+, iOS 18+). No third-party dependency.

Swift DTOs mirror the Rust types:

```swift
struct UsageTimeseries: Decodable {
    let object: String
    let bucket: String
    let groupBy: String
    let buckets: [TimeseriesBucket]
}

struct TimeseriesBucket: Decodable, Identifiable {
    let period: String
    let groups: [UsageSummaryGroup]
    var id: String { period }
}

struct UsageSummaryGroup: Decodable, Identifiable {
    let key: String
    let tokens: TokenTotals
    let sessions: Int
    let turns: Int
    var id: String { key }
}

struct TokenTotals: Decodable {
    let input: Int
    let output: Int
    let reasoning: Int
    let cacheRead: Int
    let cacheWrite: Int
}
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Per-wave analytics tab | Simpler scope, but no cross-wave comparison | Cross-wave comparison is the primary use case — "which wave is expensive?" requires seeing all waves together |
| Client-side time bucketing (N requests to `/usage/summary`) | No new endpoint | Pushes compute to Swift, multiplies HTTP requests, laggy on mobile |
| Compute `cost_usd` at query time | Always uses latest rates | Wrong mental model — cost should reflect the rate at transaction time. Also adds compute to every analytics query |
| Third-party charting library | Possibly richer charts | SwiftUI Charts handles line/bar/stacked natively, no dependency to manage, and we're already at macOS 15+ |
| Unify Session and AgentRun types | Cleaner model | High blast radius for questionable value. Analytics only needs Session (where token data lives). AgentRun tracks process lifecycle, not usage. Document the relationship explicitly instead |

## Key decisions

**Global view, not per-wave tab.** The wave item's finish line says "opening the analytics tab" — singular. The value is cross-wave comparison. Per-wave token counts already exist inline from Phase 04.

**New `/timeseries` endpoint vs extending `/summary`.** Time-series data has two dimensions (time x grouping). Overloading `/summary` with a `bucket` param would make the response shape conditional — sometimes flat groups, sometimes nested buckets. A separate endpoint with a clear contract is simpler.

**Ingestion-time cost computation.** Financial data records the price at transaction time. If OpenCode changes kimi-k2 pricing, yesterday's sessions should reflect yesterday's price, not today's. This also keeps query-time aggregation fast.

**Don't unify Session/AgentRun.** The wave item says "unify or make the relationship explicit." Making it explicit is sufficient: add doc comments to both types documenting the mapping (`Session` tracks conversation/usage, `AgentRun` tracks process lifecycle, linked through `wave_run_id`). The analytics surface only queries Session data.

**Relative comparison is the default.** "3.2M tokens" is meaningless. "3.2M tokens, 2x your other waves" is actionable. The work lens shows multiple series by default (all waves), making comparison the natural interaction.

## Scope

**In scope:**
- `GET /v0/usage/timeseries` endpoint with day/week/month bucketing
- Compute and persist `cost_usd` on TurnUsage for per-token providers
- Global analytics view in Concerto (macOS + iOS) with work and prompt lenses
- SwiftUI Charts integration
- Swift DTOs for timeseries and summary data
- `WaveServiceProtocol` methods for fetching analytics data
- Doc comments on Session/AgentRun documenting their relationship

**Out of scope:**
- Caching/materialized views (add when latency exceeds 200ms, per wave README)
- `lfq usage` CLI command (Phase 06)
- Cost caps, auto-downgrade, billing UI (per wave vision: "not here")
- Dynamic context budget sourcing (separate concern)
- Session/AgentRun type unification (documenting the relationship is enough)

## Done when

1. `cargo test -p loopflow` passes with new timeseries endpoint tests
2. `cost_usd` is populated on TurnUsage for OpenCode sessions
3. Concerto shows an "Analytics" navigation item that opens the analytics view
4. Work lens renders a line chart grouped by wave with period picker
5. Prompt lens renders a stacked bar chart of context sources
6. `swift test --package-path swift` passes with new DTO decode tests

**Wave goals advanced:**
- *"Provide a dedicated analytics surface with work lens and prompt lens"* — this is the phase
- *"Capture per-turn token data... with model and source metadata"* — cost_usd computation completes the data model
