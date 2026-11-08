# LOO-810 sync

2026-11-08: runtime checkpoint and the current prose audit saved before sync.
First merge stopped on uncommitted docs; the prose checkpoint retained them.

`lf sync --manual` then `lf sync --continue` completed against main 1c757e9fe2028c0073414dc6826395931a5aa743.
Combined session.move and Session model/blocked fields from main with creation
receipts and provider transitions; retained the audit’s linked operation reference.
The merged-tree interrupted-publication test passed. The candidate CLI is
`/Users/jack/src/loopflow.loo-810-raise-the-loopflow/target/debug/lf`.
`LF_BIN=$PWD/target/debug/lf uv run python website/dev.py sync-docs`
regenerated all 33 routes with the flattened branch command tree.

Realignment, 2026-11-08: `lf sync --manual` then `lf sync --continue`
completed against main `f61c153eb40e2246b73a87b785dbf799cbc1e98d` (#1856).
The `server/service.rs` conflict kept the bounded outbox and incorporated the
upstream request-ID regression tests. The 29 server tests, 4 documented-command
tests and `cargo clippy --all-targets -- -D warnings` passed on the merged tree.
The branch remains local; sync neither published nor moved the Task's Graph.
