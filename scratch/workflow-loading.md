# Workflow delivery before landing — LOO-462

Jack Heart requires repairing slow Workflow loading before landing (October 14).
This supersedes the shipping gate's proposed follow-up disposition. The stopped
ship driver is eb389a07-673b-46cc-a3be-5705e466bab5; PR #1531 was verified open,
without auto-merge. Minor release remains authorized only after verified merge.
Earlier design and gate evidence survive at 35e1efa74d:scratch/get-me-using-lf.md.

The real bundled Work reader delivered its first Task frame at 6.006 s; direct
Workflow show took 0.11 s. Temporary per-projection instrumentation on current
source found an initial unscoped Planning read taking 4.994 s before accepting
the queued Task scope. Task projection itself took 0.569 s. Frames also waited
for every projection before delivery. Temporary diagnostics were removed.

The reader now accepts requests arriving during setup before projecting, reads
selected Task/Wave first, and publishes each ready frame immediately. One SQLite
snapshot still owns every joined reading; confirmation remains last. No new
reader, Swift schema access, loading-state disguise or cache eviction.

Real-store CLI measurements using this checkout's development CLI: first cold
Task 3.222 s, subsequent cold reads 0.756/0.847 s; switching LOO-462/466/472 in one
reader 0.786/0.689/0.857 s. These measure scope request to Task frame, not rendered
Workflow or responsive provider input. Scripts /tmp/loo462-measure-{current,switch}.py.

Review: delivering earlier does not change revision/answer fencing; the writer
already delivers independent parts, not an atomic UI transaction. Checkpoints
still follow the entire snapshot. Portfolio acquisition remains synchronous and
can still delay a request arriving during an in-flight projection; no claim of
interruptible queries or global responsiveness bound is made.

Checks: materialized `cargo test -p loopflow --lib work_watch::` PASS 25/25; fmt, diff check and all-target Clippy PASS (/tmp/loo462-workflow-{tests,clippy}.log). Initial integration-target invocation ran zero tests; corrected to --lib, not counted as proof.
