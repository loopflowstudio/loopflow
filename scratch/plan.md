# LOO-869 plan

`selections::work_in` reads Task state (`task_details::states_in`), published PRs
(`project_adoption::published_in`) and owners (`task_ownership::owners_in`) once
for the repository's Task list instead of three queries per Task.

Remaining: gate.

Check: `target/debug/deps/loopflow-* store::sqlite::selections::` → 22 passed (implement).
On 2026-11-12 an outside SIGSTOP repeatedly stopped test binaries here and in other
worktrees (state `T`, even under `setsid` with no terminal; source not found in
this repo or `~/.lf/logs/lf.log`). The run passed after resuming its own PID 10 times.
Gate may see the same thing.
