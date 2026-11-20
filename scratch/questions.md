# Open questions

- `codex_startup_failures_preserve_input_and_allow_reconnect` ran 44.9s locally
  (load ~100) and 74s/94s in CI shards against the PR profile's 120s limit. It's
  the next test likely to TIMEOUT a PR that didn't slow it. Left out of LOO-988
  because the brief names only the service-tier test. A separate Task could
  split it per failure mode, the way #1982 split the tier test.
- Holder `STARTUP` (20s) stays as the bound when a same-build holder holds the
  lock but refuses connections (stopped, starved, or full backlog). Waiting on
  the lock instead would remove the deadline, but a SIGSTOPped holder would then
  hang `lf` with nothing in the log. Kept the deadline (made for review).
- A replaced server under launchd: QA ran it on a private Machine. `lf server
  status` from the new build reports `ready: false` ("uses another executable"),
  so `lf install server` restarts it (pid changed, next shell held). The error
  now names that command; `lf server restart`, which it named before, is not a
  command. Not run against a real `lf install` promotion.
- `rust/loopflow/src/server/service.rs:955` ("this server hosts no held
  terminals; … restart it with lf server restart") names the same missing
  command. `lf install server` leaves a ready server alone, so no existing
  command restarts that one. Left as is.
- A failed `lf desktop shell --create` keeps its `--id`: after the restart the
  same id answers "Shell startup was not acknowledged; execution is unknown".
  Desktop's retry path for that id was not checked.
