# 10: Compaction Rollout

Implement the chosen MemGPT/Letta-informed policy from the exploration step.

## What exists after this

- production compaction flow with token thresholds
- structured compacted memory blocks
- compaction telemetry (before/after token counts)

## Commit slices

### C1 — Implement selected policy in runtime (~300-550 LOC)

- integrate chosen algorithm into `POST /chat/compact`
- enforce preservation of decisions/preferences/open tasks

### C2 — Add automatic threshold trigger (~250-450 LOC)

- trigger compaction when prompt assembly exceeds token budget
- keep manual compact endpoint

### C3 — Add tests + telemetry assertions (~250-450 LOC)

- correctness tests for preserved invariants
- telemetry shape tests for compaction events

## Constraints

- Never silently drop unresolved tasks or user preferences.
- Keep memory human-editable after compaction.
- Maintain deterministic behavior under same inputs.

## Done when

```bash
cargo test -p loopflow chat_compaction
```

Expected: compaction behavior and invariants are covered by tests.
