# UX / design

Crafting: the app's design as a whole, adjudicated holistically — not
feature-by-feature. One user for now: Jack. Make loopflow work well for
him; personas return only if a second real user does.

## KRs

- A holistic design pass owns the surface: what's on screen, what's one
  action away, what's gone. Experiments are adjudicated against it — the
  ⌘K palette vs the glanceable list resolves here (one wins, the loser is
  deleted).
- Performance is this bet's physics: the most-used views carry named
  latency budgets (wave-list first paint well under a second cold; detail
  pane; runs ledger), instrumented so regressions are visible and tested so
  they fail. Governing invariant: reads never block on lfd — the list
  paints from `lf ls` without awaiting the daemon.
- Experiments run against the metrics with recorded before/after numbers.
