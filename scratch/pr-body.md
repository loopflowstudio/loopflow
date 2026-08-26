## Evaluate

```bash
uv run python scripts/test.py --rust --website
cargo test -p loopflow ops::pr_landing::tests --lib
cargo test -p loopflow ops::pr::tests --lib
```

The final materialized Rust phase passed 2,113 tests with 2 skipped in 1,025
seconds. Website/docs passed 70 tests with 3 skipped in 41 seconds. The landing
fixtures show an unusable repair provider returning control and writing the same
actionable block to the landing and CI incident; a successful fixture closes the
repair receipt only after Loopflow re-arms an advanced head.

## Why it matters

PR #1237 consumed its repair claim, lost its provider without a terminal event,
and left the watcher heartbeating after the provider and test subprocesses had
disappeared. Landing repair is now a controller-owned, finite operation with
enough durable evidence to finish or explain itself.

## What changed

- Fetch and bound the exact failed GitHub Actions job logs before provider
  launch, sharing URL parsing with `lf wt ci --logs`.
- Require a clean recorded branch and head, acquire the repair writer
  exclusively, and remove ambient Run, Home, control, writer, and Git-operation
  authority from the provider child.
- Enforce a ten-minute provider deadline with process-group teardown, then
  accept only a non-scratch uncommitted delta on the failed head.
- Keep commit, rebase, push, and re-arm authority in Loopflow.
- Persist hosted evidence URLs and digest, provider capture, deadline, and
  finish time; derive and render the repair outcome through `lf ci`.

## Risks / Not included

Unavailable hosted logs now block before launch and name the exact check URL.
Generic provider terminal-event semantics remain LOO-265, and orphaned Exec
reconciliation is separate. The standard aggregate runner's resource preflight
was blocked by unrelated active worktree build roots during gate; its exact
selected Rust and website phases were run under the same repository-owned
process-group deadlines without mutating those roots.
