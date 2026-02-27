# Inline Token Views in Concerto

## Problem

The operator manages 10+ parallel waves but has zero visibility into token consumption. They can see status, diffs, and PRs — but not which waves are expensive, which steps burn tokens, or whether an agent is grinding in circles. Phases 01-03 shipped the backend: metering events, aggregation endpoints, and a provider catalog with model rates. The data exists. Concerto doesn't show it.

Wave goal: "Surface tokens inline at every level of the Concerto hierarchy via progressive disclosure."

## Approach

Four layers of work, bottom-up:

### 0. Backend: RepoId on sessions + usage filters

Denormalize `RepoId` (`owner/repo`, from PR #478) onto `SessionConfig` so sessions carry their repo identity directly. `Wave.repo` should also migrate to `RepoId` (or carry one alongside the path). Sessions already stamp `wave`, `step`, `repo_root` at creation — `repo_id` follows the same policy.

Extend `SessionFilters` with two new fields:
- `repo: Option<RepoId>` — matches `config.repo_id`
- `interactive: Option<bool>` — matches `config.client_has_ui`

Expose both as query params on `GET /v0/usage/summary`. This gives the portfolio a single-call aggregation path: `GET /v0/usage/summary?group_by=wave&repo=loopflowstudio/loopflow` returns all waves for that repo with token totals.

### 1. Swift models and SSE wiring

Add `TurnUsage` and `ContextSnapshot` structs to LoopflowCore mirroring the Rust types. Add `turnUsage` and `contextSnapshot` cases to `AgentSessionEvent`. Parse them in `LocalWaveService.parseSessionEvent()`.

Extend `SessionState` with live usage accumulation:
- `turnUsages: [String: TurnUsage]` — keyed by turn ID, populated as `turnUsage` events arrive
- `contextSnapshot: ContextSnapshot?` — populated once at session start
- `totalUsage: TokenTotals` — running sum, updated on each `turnUsage` event

`TokenTotals` is a Swift struct with `input`, `output`, `reasoning`, `cacheRead`, `cacheWrite` (all `UInt64`), plus a computed `total` property. Matches the Rust `TokenTotals` shape.

### 2. HTTP usage client

Add a `UsageService` protocol to LoopflowCore with methods that call the existing Rust endpoints:
- `sessionUsage(_ id: String) async throws -> SessionUsageResponse`
- `waveUsage(_ id: String) async throws -> WaveUsageResponse`

Response types mirror the DTOs: `SessionUsageResponse` wraps `TokenTotals`, turn count, context snapshot, and model breakdown. `WaveUsageResponse` adds per-step breakdown and session count.

`LocalWaveService` adopts the protocol. Portfolio and WaveRunRow use these for historical data (completed sessions/waves). Live sessions use the SSE-accumulated data from `SessionState`.

### 3. Session transcript inline view

**Session transcript** — per-turn usage line after each assistant turn completes. Render below the last assistant message in a turn, before the next user message. Format: `"↳ 2.4K in · 5.8K out"` in caption style, muted foreground. Show model name if it differs from the session's primary model (multi-model sessions). Context snapshot renders once at the top of the transcript as a compact bar showing token composition by source.

### Deferred to next sprint

**Portfolio card** — token summary in the repo summary line. Currently: `"3 waves · 1 blocked · +142 -38"`. Add: `"· 1.2M tokens"`. Fetch via `GET /v0/usage/summary?group_by=wave&repo=owner/repo` — one call per repo card, returns all waves with token totals. Sum the groups for the card headline. Cache in `PortfolioRepoState` as `totalTokens: UInt64?`.

**WaveRunRow** — compact badge per completed run. Show `"45K tokens"` and a model badge (e.g., `"opus"`) next to the run duration. Model name from the most-used model in the run. Fetch wave usage with step breakdown, distribute to runs. For the active run, use live `SessionState.totalUsage`.

**WaveDetailPanel flow pills** — per-step token count below elapsed time in each pill. `"implement 2m30s\n45K in · 97K out"`. When running, the current step's count ticks up live from `SessionState`. Completed steps show final counts from wave usage endpoint.

### Token formatting

Human-readable token counts: `1,234` → `"1.2K"`, `1,234,567` → `"1.2M"`. A single `formatTokenCount(_ count: UInt64) -> String` utility.

### Model badges

Short model display name from `GET /v0/providers` response. Cache provider list on app launch. Map model ID from `TurnUsage.model` to display name. Unknown model IDs render as-is (no crash, no diagnostics in the UI — the data is still useful).

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Dedicated cost tab in WaveDetailPanel | Separate surface, discoverable | Breaks the "no new navigation concepts" constraint. Token data is a secondary metric — weaving it into existing views keeps progressive disclosure intact. |
| Aggregate-only (wave-level totals, no per-turn) | Simpler, fewer API calls | Loses the main debugging signal: "which turn burned tokens?" The operator needs granularity to spot grinding agents. |
| Compute all aggregation client-side from SSE events | No new HTTP calls for live data | Only works for active sessions. Historical data requires replaying all events. The Rust endpoints already do this. |
| Dollar costs instead of token counts | More directly actionable | Claude and Codex are subscription — no per-token cost. Token volume is the universal metric. Dollar display deferred to Phase 05 analytics. |

## Key decisions

**Live vs historical split.** Active sessions get real-time token counts from SSE events accumulated in `SessionState`. Completed sessions/waves fetch from HTTP usage endpoints. This avoids replaying event history on the client while keeping live sessions responsive.

**RepoId as repo identity.** Sessions carry `repo_id: RepoId` (denormalized at creation, same as `wave` and `step`). All usage filtering uses `RepoId` (`owner/repo`), not absolute paths. This aligns with the portfolio DAG from PR #478 and enables cross-repo aggregation in a future phase.

**Flat token counts, not cost.** v0 shows token volume everywhere. Cost calculation (using `ModelRate` from Phase 03) is Phase 05 scope. The UI renders `UInt64` token counts, and the model is ready for a `costUSD: Double?` field later with no schema change.

**Context snapshot as composition bar.** The `ContextSnapshot.sources` map (step, direction, diff, area, repo_doc, etc.) renders as a horizontal stacked bar at the top of the session transcript. Each source gets a proportional segment with a label. This answers "where did my input tokens come from?" at a glance.

**Model badge shows most-used model.** A run may use multiple models (e.g., haiku for tool calls, opus for reasoning). The badge shows the model with the most turns. Multi-model detail available on tap/hover in the future.

## Scope

- **This sprint:** Backend `RepoId` on sessions + `SessionFilters` extensions. Swift `TurnUsage`, `ContextSnapshot`, `TokenTotals` models. SSE event parsing for `turn_usage` and `context_snapshot`. `SessionState` live accumulation. `UsageService` protocol with HTTP client. Transcript per-turn usage view. Token formatting utility.
- **Next sprint:** Portfolio card, WaveRunRow badge, flow pills per-step tokens. Model badge from provider catalog.
- **Future phases:** Dollar cost display (Phase 05). Analytics dashboard with charts (Phase 05). `lfq usage` CLI (Phase 06). Cost alerts or budgets. Historical trend lines. Per-tool-call token attribution. Cross-repo token aggregation via parent/child DAG (the `RepoId` + edge data from PR #478 makes this possible once per-repo filtering lands).

## Implementation order (this sprint)

1. **Backend: RepoId + filters** — Denormalize `repo_id` onto `SessionConfig`. Add `repo` and `interactive` to `SessionFilters` and summary endpoint query params. Rust tests.
2. **Swift models + SSE parsing** — `TurnUsage`, `ContextSnapshot`, `TokenTotals` in LoopflowCore. Parse events in `LocalWaveService`. Tests.
3. **SessionState accumulation** — Wire `turnUsage` and `contextSnapshot` events into `SessionState`. Running totals. Tests.
4. **UsageService + HTTP client** — Protocol, response types, `LocalWaveService` adoption. Tests.
5. **Token formatting utility** — `formatTokenCount`. Tests.
6. **Transcript per-turn usage** — Render `↳ in · out` after assistant turns. Context snapshot bar at top.

## Done when

`cargo test` passes with `SessionFilters` repo/interactive tests. `swift test --package-path swift` passes with new model and parsing tests. Starting a live session in Concerto shows tokens accumulating per turn in the transcript.
