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
- In production, does `lf install` ever replace the running server's executable
  in place? If it does, the server spawns a holder of the new build and waits on
  the old place, the same failure as 1b. Slice 2's build check turns that into a
  named error, but it isn't verified against the install layout.
