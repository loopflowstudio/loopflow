# 09: MemGPT/Letta Exploration Step

Run explicit experiments after core infra works to choose the compaction strategy with real data.

## Why this is its own step

Compaction quality depends on workload shape. We should test options against live harness traces, not lock policy from theory alone.

## What exists after this

- reproducible experiment harness for compaction options
- benchmark corpus from real chat turns
- decision doc with chosen strategy and rejected alternatives

## Commit slices

### C1 — Build experiment harness (~250-450 LOC)

- export/import chat transcripts + memory snapshots
- offline evaluator that replays turns with different compaction policies

### C2 — Implement candidate policies (~300-550 LOC)

- policy A: summarize history into structured block sections
- policy B: rolling recent window + decision/task extraction
- policy C: aggressive dedupe + unresolved-task pinning

### C3 — Record results + choose default (~200-350 LOC)

- metrics: token reduction, answer quality regressions, memory drift
- write selected policy + thresholds into `scratch/` decision report

## Constraints

- Keep this step measurable (not vibes-only).
- Use token-budgeted replay runs.
- Include failure cases (long sessions, dense tool activity, repeated edits).

## Done when

```bash
uv run python scripts/chat_compaction_eval.py --report scratch/compaction-eval.md
```

Expected: report includes clear winner + rollout thresholds.
