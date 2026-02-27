# Cost

## Vision

Visibility into where tokens go and where they come from, for a solo operator managing 10+ parallel waves. Not cost caps, not auto-downgrade, not OpenCode per-token billing UI, not team-level multi-account aggregation.

"Am I getting value? What's eating my budget? Can I intervene?"

## Strategy

The operator runs a fleet of agents across waves. Today they can see status, diffs, and PRs — but have no idea which waves are expensive, which steps burn tokens, or whether an agent is grinding in circles. Token volume is one signal alongside velocity and quality for managing the fleet.

v0 = tokens, not dollars. Claude and Codex are subscription plans — there's no per-token cost. But token volume matters: it correlates with context quality, rate limits are real, and when the operator scales to a team (or OpenCode with API keys), the same data becomes a cost dashboard with no schema changes.

The backend captures per-turn token data from all three harnesses (Claude, Codex, OpenCode) as persisted `SessionEvent` payloads (`TurnUsage`, `ContextSnapshot`). Three HTTP endpoints serve session/wave/summary aggregation with `group_by` and filters — pure Rust aggregation above the store layer, no new tables. A provider catalog (`GET /v0/providers`) exposes model metadata and rate structures. The next milestone brings this data into Concerto's UI.

## Goals

- Capture per-turn token data from all three harnesses (Claude, Codex, OpenCode) with model and source metadata
- Record prompt composition (where input tokens come from) at session start
- Surface tokens inline at every level of the Concerto hierarchy via progressive disclosure
- Provide a dedicated analytics surface with work lens (tokens by wave/flow/step/model) and prompt lens (token composition by source)
- Elevate Provider into a first-class concept carrying model and metering awareness

## Risks

- **Harness format instability.** Claude, Codex, and OpenCode emit different JSON shapes for token data. Defensive parsing with Option fields — degrade gracefully when fields are missing.
- **Aggregation performance.** Scanning session_events for analytics queries could get slow at scale. v0 computes on demand; add materialized session_usage table if needed.
- **Over-building the analytics UI.** The goal is operational awareness, not a billing dashboard. Charts should answer "where are my tokens going?" not "what's my invoice?"

## Metrics

- Token data captured per-turn from all three harnesses with model metadata
- Context snapshot recorded at session start showing input token composition
- Inline token views visible in Concerto at wave, session, and turn levels
- `lfq usage` shows token breakdown by wave, flow, step, model
