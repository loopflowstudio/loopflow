# 04: Inline Concerto Views

Progressive disclosure of token data at every level of the Concerto hierarchy.

## What to build

Token counts woven into existing surfaces — no new navigation concepts.

- **Portfolio card**: token summary next to diff totals
- **WaveRunRow**: tokens and model badge per run
- **WaveDetailPanel**: per-step token count in flow pills
- **Session transcript**: per-turn usage below turn separators

Reads from the usage API (Phase 02). Swift model additions: `TurnUsage`, `ContextSnapshot`, `SessionUsage` structs. Extend `SessionState` to accumulate live usage from the SSE stream.

Model badges can use `ModelInfo.display_name` from the static registry (`provider_models.rs`) and `ProviderInfo` from `GET /v0/providers`. Model IDs in `TurnUsage.model` must match registry IDs — verify against production harness output before rendering.

## Context from shipped phases

- Provider schema drift: missing/renamed usage fields in harness output are silently `0`/`None`. Consider surfacing diagnostics for unknown model IDs.
- OpenCode usage capture depends on `active -> idle` status transitions with usage block. Verify rendering handles partial usage data gracefully.

## Done when

Opening a wave in Concerto shows token counts at each hierarchy level. Live sessions show tokens accumulating per turn.
