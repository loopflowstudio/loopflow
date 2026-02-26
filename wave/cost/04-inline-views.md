# 04: Inline Concerto Views

Progressive disclosure of token data at every level of the Concerto hierarchy.

## What to build

Token counts woven into existing surfaces — no new navigation concepts.

- **Portfolio card**: token summary next to diff totals
- **WaveRunRow**: tokens and model badge per run
- **WaveDetailPanel**: per-step token count in flow pills
- **Session transcript**: per-turn usage below turn separators

Reads from the usage API (stage 02). Swift model additions: `TurnUsage`, `ContextSnapshot`, `SessionUsage` structs. Extend `SessionState` to accumulate live usage from the SSE stream.

## Done when

Opening a wave in Concerto shows token counts at each hierarchy level. Live sessions show tokens accumulating per turn.
