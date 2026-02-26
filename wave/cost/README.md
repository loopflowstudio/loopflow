# Cost

Visibility into where tokens go and where they come from, for a solo operator managing 10+ parallel waves.

## Vision

"Am I getting value? What's eating my budget? Can I intervene?"

The operator runs a fleet of agents across waves. Today they can see status, diffs, and PRs — but have no idea which waves are expensive, which steps burn tokens, or whether an agent is grinding in circles. Token volume is one signal alongside velocity and quality for managing the fleet.

v0 = tokens, not dollars. Claude and Codex are subscription plans — there's no per-token cost. But token volume matters: it correlates with context quality, rate limits are real, and when the operator scales to a team (or OpenCode with API keys), the same data becomes a cost dashboard with no schema changes.

### Not here
- Cost caps, auto-downgrade, behavioral modification
- OpenCode per-token billing UI (infrastructure supports it, display doesn't)
- Team-level multi-account aggregation

## Goals

- Capture per-turn token data from all three harnesses (Claude, Codex, OpenCode) with model and source metadata
- Record prompt composition (where input tokens come from) at session start
- Surface tokens inline at every level of the Concerto hierarchy via progressive disclosure
- Provide a dedicated analytics surface with work lens (tokens by wave/flow/step/model) and prompt lens (token composition by source)
- Elevate Provider into a first-class concept carrying model and metering awareness

## Phase status

| # | Phase doc | Scope | Status |
|---|---|---|---|
| 01 | `01-metering-infra.md` | Persist `turn_usage` and `context_snapshot` runtime events from all harnesses | Shipped (2026-02-26) |
| 02 | `02-usage-api.md` | Usage aggregation endpoints for session/wave/global summaries | Next |
| 03 | `03-provider-elevation.md` | Provider/model metadata and `/providers` endpoint | Later |
| 04 | `04-inline-views.md` | Progressive inline token views in Concerto | Later |
| 05 | `05-analytics-dashboard.md` | Dedicated analytics tab with work + prompt lenses | Later |
| 06 | `06-lfq-usage.md` | `lfq usage` CLI views over usage API | Later |

### Phase 01 retrospective (shipped 2026-02-26)

What shipped:

- Added `TurnUsage` and `ContextSnapshot` as persisted `SessionEvent` payloads with `turn_usage` / `context_snapshot` event types.
- Wired usage extraction into Claude (`result`), Codex (`turn/completed`), and OpenCode (`session.status` idle transition) harness mappings.
- Emitted `ContextSnapshot` once at session start before harness startup.
- Added tests for harness parsing, serde round-trips, and context-breakdown conversion.
- Kept schema stable: new data persists in existing `session_events.data` JSON without migrations.

Carry-forward follow-ups:

- Harden provider schema drift detection so missing/renamed usage fields are visible in tests or diagnostics (not silently treated as `0`/`None`).
- Source `ContextSnapshot.budget` from session-level configuration if/when dynamic context budgeting lands.
- Add regression coverage around OpenCode status transition semantics used for usage capture (`active -> idle` + usage block).

## Risks

- **Harness format instability.** Claude, Codex, and OpenCode emit different JSON shapes for token data. Defensive parsing with Option fields — degrade gracefully when fields are missing.
- **Aggregation performance.** Scanning session_events for analytics queries could get slow at scale. v0 computes on demand; add materialized session_usage table if needed.
- **Over-building the analytics UI.** The goal is operational awareness, not a billing dashboard. Charts should answer "where are my tokens going?" not "what's my invoice?"

## Metrics

- All three harnesses emit TurnUsage events with at minimum input_tokens and output_tokens
- ContextSnapshot captures prompt composition at session start
- Concerto displays token counts at every hierarchy level without new navigation
- Analytics view enables grouping by wave, flow, step, and model
