# Performance

Concerto's responsiveness is a measured, first-class concern — not a vibe. The
most-used views carry latency budgets that are monitored and regression-tested;
experiments are ground against those metrics with recorded before/after numbers.

Governing invariant: **reads never block on lfd.** The repo/wave list paints from
`lf` (daemon-less); the bundled daemon is pubsub-only and must never gate a read.

## Important views (measure these)

- **Repo + wave list first-paint** — the cold-start dashboard. The surface that
  was "forever to load."
- **Wave detail / plan pane open** — time from click to plan rendered.
- **Runs ledger render** — secondary, but on the list.

## KRs

- Every important view has a named responsiveness budget and meets it (e.g.
  wave-list first content well under a second, cold, independent of daemon state).
- Reads never block on lfd: the wave list renders from `lf ls` without awaiting
  `SharedDaemon.manager.start()`. First instance: the startup gate in
  `WavesView.syncRepoStates` (see scratch/questions.md).
- Timings are instrumented (os_signpost / a timing log) so a regression is
  *visible*, and a regression test fails when a key view exceeds its budget.
- Experiments run against the metrics with recorded before/after numbers — a
  perf-research loop in the shape of `lf ux-research` (propose → measure → learn).

## Notes

- Coordinate with the systems wave's Concerto CI work (branches
  `systems/ci-concerto-idle`, `systems/ci-concerto-speedup`) — reuse, don't
  duplicate.
- Budgets start unset; the first job of monitoring is to establish the real
  numbers, then ratchet.
