# Verification speed

Audited 2026-07-08: GitHub CI is already fast — median total ~1m55s, the
critical path is loopflow-ui-test (~2m06s) with everything else parallel
beneath it. A 25% cut buys ~30s; that half of the old KR is dead. What was
never measured is the LOCAL loop.

## KRs

- Local verification (the full TESTING.md matrix an agent runs before
  landing) gets a measured baseline; the slow stages get budgets.
- GitHub CI holds under 2.5m median as suites grow (regression bar, not an
  optimization target).

## Notes

If the local baseline comes back already-fast, delete this bet.
