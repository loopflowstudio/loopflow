---
schema: 1
id: context-slow-turn-minutes
stage: installed
instrument: context-cost
unit: minutes
window: 7d
freshness: 8d
---

# Slow turns at large context

90th-percentile minutes from turn start to completion, among turns started in
the latest complete week whose peak input reached 200,000 tokens. Computed per
harness; the reading is the slowest harness. Unfinished turns are excluded.
Baseline on 2026-09-30: 48 minutes for Codex turns peaking at 200-300k.
