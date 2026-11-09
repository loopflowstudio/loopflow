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

2026-11-08 conflict recovery: `lf sync --continue` completed onto
`da7e82310348683c3f11d16cb64e7876d1d54236` as merge `ce571a5a4`.
Kept the shared creation receipt path and moved first-turn preparation before
launch, matching main's background Task conversation startup.
`cargo test -p loopflow --lib task_create_run_starts_the_graph_and_admits_the_request_as_first_turn`
(with inherited LF variables unset and stdin closed): 1 passed; linker warned
that the unwind table exceeds 16 MiB. Full checks remain with gate/CI.
No push from the conflict resolver; the waiting sync owns publication.


2026-11-08 realignment: `lf sync --manual` completed against
`95c17782a2dfd782a707f2a9beac6fe8cbdf9be3` (#1852) as merge `e2028e1bb`.
No conflicts; the Work observation backoff and Swift guidance coexist with
creation presentation. `git merge-base --is-ancestor origin/main HEAD`: passed.
No runtime checks rerun for the conflict-free merge; final-tree Swift recovery
checks remain with gate. This sync stayed local.

2026-11-09 publication: `lf sync --manual` and `lf sync --continue` merged
main `4848816f73ccbacd5f1eb26bff7f54814d7c0c8b` as `fef21fd4d`.
Conflicts combined revision-3 creation envelopes with canonical operation names,
retained Git reads, event-driven Swift mailboxes and terminal input draining.
Kept the QA geometry correction and original documentation audit. The branch
CLI built; the migration history check passed. Website sync regenerated 33
routes from `target/debug/lf`. Final merged-tree results are recorded separately
in `loo-810-publication.md`; physical sleep/wake stays at demo review.

2026-11-09 gate: `lf sync --manual` / `lf sync --continue` merged main
`86bc0692e4f593e747bbacc8ed0bf710f5e06af0` (#1865) as `7528c8b0f`.
Combined opaque watch checkpoints with reserved producer capacity. The combined
contract uses revision 4; current docs and shared fixtures agree. Gate results
and the admission-window repair are in `loo-810-publication.md`.
