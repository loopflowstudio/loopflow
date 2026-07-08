# Developer experience

Working on loopflow — as a human or as an agent — is frictionless: the
primitives hold under load, verification is fast, nothing hands you a
setup step, and every sharp edge becomes a task the second time it cuts.

## KRs

- Avoidable human-in-the-loop steps found in agent runs fall to 0 for one
  full week; credential expiries (Linear, GitHub, vendor) pre-empt instead
  of blocking runs.
- The worktree/stacking primitives hold (audited 2026-07-08): child base
  SHA persisted at creation (OPEN — computed transiently, never recorded);
  `lf op next` works from the wave home (root cause known: reset_to_main
  occupies main; re-baseline against origin/main instead); #836 re-parent
  gets one live-run verification; the wave-home cross-machine question is
  answered before anything is built.
- Local verification (the full pre-land matrix) gets a measured baseline
  and budgets; GitHub CI holds under 2.5m median as a regression bar
  (already ~1m55s).
- The exec-door story completes: lfq folds into lf or is re-chartered
  (coordinate with datamodel's one-system bet).
