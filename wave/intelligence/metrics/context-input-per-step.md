---
schema: 1
id: context-input-per-step
stage: installed
instrument: context-cost
unit: tokens
window: 7d
freshness: 8d
---

# Input tokens per step

Mean input tokens per Session input started in the latest complete week,
counted in seven-day weeks from 2026-09-30T00:00Z. A step is one captured
Session input; input includes cached reads. Inputs without reported usage are
excluded. The week ending 2026-09-30 is the baseline.
