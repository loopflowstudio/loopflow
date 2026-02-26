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

## Phase boundaries

- **01-metering-infra**: Parse token data from all three harnesses, emit TurnUsage and ContextSnapshot events, persist via existing event stream. Rust only.
- **02-usage-api**: HTTP endpoints for session/wave/global usage aggregation. Powers both Concerto and lfq.
- **03-provider-elevation**: Provider gains model awareness and cost rate slots. New /providers endpoint.
- **04-inline-views**: Progressive disclosure in Concerto — tokens on portfolio cards, wave runs, step pills, turn separators.
- **05-analytics-dashboard**: New Concerto tab with work lens (time-series by wave/flow/step/model) and prompt lens (composition by source).
- **06-lfq-usage**: CLI interface for usage data.

## Risks

- **Harness format instability.** Claude, Codex, and OpenCode emit different JSON shapes for token data. Defensive parsing with Option fields — degrade gracefully when fields are missing.
- **Aggregation performance.** Scanning session_events for analytics queries could get slow at scale. v0 computes on demand; add materialized session_usage table if needed.
- **Over-building the analytics UI.** The goal is operational awareness, not a billing dashboard. Charts should answer "where are my tokens going?" not "what's my invoice?"

## Metrics

- All three harnesses emit TurnUsage events with at minimum input_tokens and output_tokens
- ContextSnapshot captures prompt composition at session start
- Concerto displays token counts at every hierarchy level without new navigation
- Analytics view enables grouping by wave, flow, step, and model
