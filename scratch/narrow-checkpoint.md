# Provenance checkpoint validation — 2026-09-23

The checkpoint at `335bb8fa4` preserves exact Task flow history; it does not
establish full native capture or a working Watch surface. Durable ownership and
capture lessons are folded into [Product memory](../wave/product/MEMORY.md#passive-task-watch-loo-293-branch-evidence-2026-09-23).
The active Task design retains the full finish line.

## Reproduce the focused proof when provenance changes

```bash
env -u LF_RUN_ID LOOPFLOW_BUILD_PROVENANCE=development cargo test -p loopflow --lib durable_store_tests --no-fail-fast
env -u LF_RUN_ID LOOPFLOW_BUILD_PROVENANCE=development cargo test -p loopflow --lib failure_releases --no-fail-fast
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Expected behavior: repeated skill names retain distinct stages; worker and human
bindings retain exact Runs; Iterate returns to its recorded target; retries keep
separate attempts. Restart, completion, and reopen retain settlement. Stale claims,
changed same-ID plans, and failed ledger writes roll back. Flow facts never enter
parent observation delivery; resumable failure release leaves history unchanged.

Prior working-tree receipts: eight store tests and three failure-release tests
passed, plus formatting and full-target Clippy. Those commands included later
output work. The isolated checkpoint review separately passed the same eight and
three tests with development provenance; see `review-slice.md` for its evidence
boundary. This documentation pass reruns no code tests and supplies no Watch,
publication, Task-completion, or configured human-demo proof.
