# Decisions — wave viewer draft, next pass (2026-07-07)

Jack resolved the three review findings:

## 1. RunStatus: bias to `lf`

Align `RunStatus` to the vocabulary `lf` actually emits — its lowercase tokens
(`running`, `ok`, `waiting`, `failed`, `pending`, …), not the lfd int enum. Drop
the invented `cancelled`. An unknown status must be **loud** (surface it), never a
silent `?? .pending`. When `lf` and lfd disagree, `lf` wins.

## 2. Runs UI: keep it simple

Runs is **not** the most important window. Do not build the origin-grouped
chart/history — a plain, minimal runs list is enough. Don't grow `lf runs` for
origin data now. The plan (objective + projects) is the centerpiece; runs are
secondary.

## 3. Kill the HTTP-to-lfd path in this PR

`LocalWaveService` — and **any** HTTP-to-lfd-as-API code (`WaveServiceProtocol`,
`httpBaseURL` data reads) — does **not** survive this PR. Converge every data
read on `RegistryQuery` (subprocess `lf … --json`, daemon-less). `RegistryQuery`
already covers `waves()`, `status()`, `recentRuns()`; reroute the remaining
~22 consumers (RepoState, PortfolioRepoState, SessionState, OutputBuffer,
WavesView, auth/connection) onto it and delete the HTTP service + its tests.
Keep one implementation.

## Minor (fold in)
- `RunSnapshot.toRun` `area: "."` placeholder — drop the field from `Run` if the
  snapshot can't supply it.
- `createdAt: … ?? Date()` — avoid the nondeterministic default.
