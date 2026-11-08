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
