# Context cost and turn time

```bash
lf usage --weekly                 # trend by week, then the latest week's breakdown
lf usage --weekly --json          # every week, step, harness, Task and turn band
lf usage --weekly --task LOO-348  # one Task's weeks; reads only, publishes nothing
lf wave status intelligence       # the four Wave readings and their targets
```

Jack Heart requested this instrument on 2026-09-30 (LOO-348) so LOO-346's
budgets and later prompt changes are judged by numbers. Weeks are seven days
counted from 2026-09-30T00:00Z; the first row is the baseline week ending there.
A step is one captured Session input. Input includes cached reads. A `*` after
a step count means some steps reported no usage and sit outside the ratios.
Silence is a gap over ten minutes between retained events inside a turn that
later completed.

An unfiltered run publishes the latest complete week to
`wave/intelligence/metrics/context-*.md`. `lf telemetry-daily` runs it, so the
Wave's daily cron keeps the readings fresh.

## September 30 baseline

Measured by hand from 1,349 run records in `~/.lf` and the pinned Machine during
the LOO-298 speed research. The report recomputes its own baseline row from the
store; where the two disagree, the difference is population, not drift.

| Measure | Baseline |
|---|---:|
| Input : output | about 420 : 1 (3.9B input, 11.5M output a week) |
| Silent over 10 min, Codex | 213 h; 343 of 346 gaps were slow turns that finished |
| Silent over 10 min, Claude | 2.4 h |

Codex turn minutes by peak input, 1,171 turns:

| Peak input | Median | p90 |
|---|---:|---:|
| under 100k | 1.3 | 6.2 |
| 100–150k | 5.2 | 13 |
| 150–200k | 5.9 | 34 |
| 200–300k | 7.5 | 48 |

Output stayed near 7–8k tokens above 100k input. Input per step had no hand
baseline; the report's baseline-week row supplies it.

## KRs

The chapter plan owns KRs and targets. The targets below hold the baseline, so
a reading is judged against 2026-09-30 until LOO-346 sets budgets. Add them to
the Intelligence plan beside its existing KRs once the contracts are on main:

```json
{
  "metric_targets": [
    {"metric_id": "context-input-output-ratio", "target": {"kind": "at_most", "value": 420}},
    {"metric_id": "context-slow-turn-minutes", "target": {"kind": "at_most", "value": 48}},
    {"metric_id": "context-silent-hours", "target": {"kind": "at_most", "value": 215.4}}
  ],
  "krs": [
    {"text": "Context cost falls from the 2026-09-30 baseline: input:output below 420:1, p90 turn time at 200k+ input below 48 min, silent time below 215 h a week, read weekly from lf usage --weekly.", "holds": false}
  ]
}
```
