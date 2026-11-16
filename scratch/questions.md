# Open choices and outside findings

No product question is open. The execution cut, independent fixtures and five
alternating pairs are complete. Saved-record edits cost 2.6% more elapsed and
2.8% more client CPU, with 38.7% less largest-process RSS. Demo/review should
assess that measured tradeoff alongside command feedback and peak memory.

Implementation choices and earlier alternatives are archived at
`b29e51c552:scratch/questions.md`; the current design and remaining Graph steps
are in `loo-650.md`.

- The optional developer-entry cache inventory follows `scripts/dev-lf` and
  existing daemon idle exit. No Machine wrapper/configuration was installed or
  shared daemon restarted. Ordinary Cargo does not refresh it. Explicit
  `SCCACHE_CONF` remains caller-owned; isolated builds bypass both wrappers/cache.
- The selected compatible experimental cache passed all probes and cohorts.
  Stock 0.19.0 missed cross-root reuse. No source-retention service is selected;
  cached debug paths can outlive a source checkout, with isolated builds as the
  source-debugging path. Measurements establish client costs, not Machine
  concurrency or total CPU budgets.
- Unit-test defaults stay behind `cfg(test)`; `test-support` exposes inspection
  and explicit overrides. The failed native preparation run and ledger repair
  are recorded in the plan. No runtime fixture flag was added.

- November 15 delivery observation: `lf task hold LOO-917` stopped Graph
  `9b4c441a-9a09-40ce-889b-0d56276bd191` and disabled auto-merge, but left its
  native `land --wait-and-fix` step `3ae30c73-d6a2-4ac3-a10f-379b425d0717`
  running as orphan PID 67960 (start 1794795959, foundation checkout cwd). It
  prevented the next ship with “live or unresolved execution”. The exact
  recorded PID/start/cwd/argv matched before SIGTERM; it exited 130, and the
  next ship started. This runtime-control defect is outside the crate cut.
  Cheapest regression producer: hold a disposable Task whose Graph command
  waits, then assert its recorded command process exits and a new ship starts.


- November 15: a broad compiler-process inspection accidentally printed an unrelated Cursor API credential from process arguments. The value was not used or copied; it needs rotation. Further inspection is restricted to known PIDs.
