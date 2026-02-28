# Cost

## Vision

Visibility into where tokens go and where they come from, for a solo operator managing 10+ parallel waves. Not cost caps, not auto-downgrade, not OpenCode per-token billing UI, not team-level multi-account aggregation.

"Am I getting value? What's eating my budget? Can I intervene?"

## Strategy

The operator runs a fleet of agents across waves. Today they can see status, diffs, and PRs — but have no idea which waves are expensive, which steps burn tokens, or whether an agent is grinding in circles. Token volume is one signal alongside velocity and quality for managing the fleet.

Two billing models coexist: subscription (Claude/Codex OAuth) and pay-per-token (API keys, OpenCode). Token volume matters for both — it correlates with context quality and rate limits. But for API key users, tokens are dollars, and loopflow needs to make that visible.

Auth type (oauth vs apikey) is set per-provider in the auth wave. Cost tracking uses this to split usage into subscription vs metered buckets and compute dollar costs for metered sessions.

## Goals

- Capture per-turn token data from all three harnesses (Claude, Codex, OpenCode) with model and source metadata
- Record prompt composition (where input tokens come from) at session start
- Surface tokens inline at every level of the Concerto hierarchy via progressive disclosure
- Provide a dedicated analytics surface with work lens (tokens by wave/flow/step/model) and prompt lens (token composition by source)
- Elevate Provider into a first-class concept carrying model and metering awareness
- Split usage by auth type (subscription vs API key) and compute dollar costs for metered sessions

## Risks

- **Harness format instability.** Claude, Codex, and OpenCode emit different JSON shapes for token data. Defensive parsing with Option fields — degrade gracefully when fields are missing.
- **Aggregation performance.** Scanning session_events for analytics queries could get slow at scale. v0 computes on demand; add materialized session_usage table if needed.
- **Over-building the analytics UI.** The goal is operational awareness, not a billing dashboard. Charts should answer "where are my tokens going?" not "what's my invoice?"
- **Auth type changes mid-session.** If a user switches from API key to OAuth while a session is running, the billing model for that session is mixed. Record auth type per-turn, not per-session.

## Metrics

- Token capture rate: % of turns with complete token data across all three harnesses (target: 100%)
- Input token composition: % breakdown by source (scratch, wave docs, area docs, repo docs) per session
- Per-wave token spend: total input + output tokens per wave per day
- Dollar cost accuracy for metered sessions: computed cost vs actual provider invoice variance (target: <5%)
