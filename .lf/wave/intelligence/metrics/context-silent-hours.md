---
schema: 1
id: context-silent-hours
stage: installed
instrument: context-cost
unit: hours
window: 7d
freshness: 8d
---

# Hours spent silent inside turns

Hours of gaps longer than ten minutes between consecutive retained events
inside a turn that later completed, summed over Session inputs started in the
latest complete week. Waiting between turns is excluded. Baseline measured on
2026-09-30 over 1,349 records: 213 hours for Codex and 2.4 for Claude.
