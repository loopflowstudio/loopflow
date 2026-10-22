---
schema: 1
id: context-input-output-ratio
stage: installed
instrument: context-cost
unit: ratio
window: 7d
freshness: 8d
---

# Input tokens paid per output token

Input tokens divided by output tokens across every Session input started in the
latest complete week, counted in seven-day weeks from 2026-09-30T00:00Z. Input
includes cached reads. Inputs whose provider reported no input or no output
count are excluded. Baseline for the week ending 2026-09-30: about 420.
