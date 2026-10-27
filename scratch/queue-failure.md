# LOO-608 merge-queue failure — 2026-10-27

Jack Heart approved shipping the border-free workspace. Candidate head
`31db62405090833d295e36e308256be3044abd65` passed its CI checks. The first
merge-queue revision `b52017cad0eaeaca0f86b1597b9e4018ad39c964` failed
`task_graph_launch_tests::task_run_returns_while_its_graph_keeps_running`:
“the second step never ran: 1”, at
`rust/loopflow/tests/linear/task_graph_launch_tests.rs:892`.

Job: https://github.com/loopflowstudio/loopflow/actions/runs/40524381203/job/122261607927
It ran 447 tests: 446 passed, one failed. The failing test completed in 7.69 s;
this was the child-process-count assertion after Graph completion, not its
60-second deadline. Root cause is not established; do not assume a flaky timeout.

The existing `lf ship`/waited-landing process owns delivery and repair. Do not
launch a competing watcher or discard the test's continuation requirement.

CI repair inspected the recorded run and synced main at `8bf860806ce4bc491b1308ac4728085d93a73372`.
Local sync head `db04d0757c` has the identical tree to failed queue head
`b52017cad0eaeaca0f86b1597b9e4018ad39c964`; there are no workspace-code repairs.
The original PR head's checks and main's library partition 2/5 passed.

A deterministic in-memory SQLite probe confirmed an unrelated race consistent
with the failure: `process_mark` reads MAX(rowid), an exiting status query can
remove that highest row, and a new child can reuse it. `child_process_after`
then misses the child (`rowid > mark`); no Graph step reference protects it
from cleanup after Graph completion. The failed test had already observed a
successful Graph, so its count is not evidence that execution stopped early.
The original failed fixture database is unavailable; this is a source-supported
mechanism, not a replay of that CI interleaving. Follow the ci-fix skill's
unrelated-race path by re-arming; do not broaden this workspace PR.

Checks: `git diff --check` passed; queue-tree equality passed; isolated SQLite
row-number reuse probe passed. No Rust build: no local target exists and the
volume has only 2.6 GiB available. CI owns full test verification after re-arm.
