# Performance

```bash
lf performance          # compare the last 14 days with tracked budgets
lf performance --json   # consume the same scorecard as structured data
```

`budgets.json` is the policy source. Each scorecard row carries its budget,
measured/eligible coverage, nearest-rank p50 and p95, verdict, and the exact
reason evidence is incomplete. `FAIL` outranks missing coverage when an
observed value already breaches a budget. `PASS` requires complete coverage
and at least 20 samples; smaller complete sets are `COLLECTING`.

## Initial baseline

Observed through `2026-07-21T20:02:07Z` for the preceding 14 days:

| Measure | Coverage | p50 | p95 | Verdict |
|---|---:|---:|---:|---|
| Task launch → first progress | 0/0 | — | — | UNKNOWN — first material progress is not persisted |
| Pre-land changed | 24/40 | 126.5 s | 273.2 s | UNKNOWN — failed runs are censored |
| Pre-land full | 6/12 | 271.9 s | 702.1 s | UNKNOWN — failed runs are censored |
| Land request → merge | 0/0 | — | — | UNKNOWN — GitHub `mergedAt` is not persisted |
| Avoidable repair | 0/0 | — | — | UNKNOWN — no durable landing denominator |
| Build/disk and CPU | 0/0 | — | — | UNKNOWN — resource envelopes belong to LOO-9 |
| Agent total input / Turn | 779/1,315 | 927,350 | 5,270,416 | FAIL — p95 exceeds 5,000,000 |
| Agent output / Turn | 783/1,315 | 3,433 | 18,134 | UNKNOWN — 532 missing reports |
| Reported cost / Turn | 62/1,315 | $1.21 | $6.74 | UNKNOWN — 1,253 missing reports |

Provider coverage exposes why the aggregate is incomplete:

| Provider | Input coverage | Output coverage | Cost coverage |
|---|---:|---:|---:|
| Claude | 58/273 | 62/273 | 62/273 |
| Codex | 721/936 | 721/936 | 0/936 |
| OpenCode | 0/106 | 0/106 | 0/106 |

Pre-land phase observations already support a p95 verdict for `clippy`,
`python`, `rustfmt`, `swift`, and `website`; each is inside budget. Other
phases remain `COLLECTING` below 20 samples or `UNKNOWN` where phase evidence is
missing. This baseline does not backfill any absent value with zero.
