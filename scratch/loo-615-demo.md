# LOO-615 demo — 2026-10-27 (headless; agent standing in for Jack Heart)

Experience reviewed: Process rows exist while an lf process runs and are gone at
exit unless referenced or agent-issued; the prune backstop on a real store.
Design: `scratch/loo-615-delete-process-rows.md`. No person took part. The
decisions below are the agent's, made on Jack Heart's behalf.

## Observed

- **Clone of the live store.** APFS clone of `~/.lf/loopflow.db` into
  `/tmp/loo615-demo.X3a6`, with the draft indexes applied by hand (custom homes
  do not apply drafts). `retention::prune` ran through a throwaway ignored test,
  reverted afterwards. `lf disk clean` itself was not run: on a cloned registry
  it would prune Jack's real worktrees.
  - processes: 953,191 → **104,998** (closed 4,338 crash leftovers, forgot 848,193), 285 s including VACUUM
  - kept: agent 7,146 · agent-issued lf 87,546 · other referenced lf 10,306
  - `pragma foreign_key_check`: empty
  - store file 10.5 GB → 287 MB (VACUUM included; processes were most of it)
  - finished children of still-running parents: 507
- **Live tree** (branch `lf`, `LF_HOME` = the pruned clone): `lf wave list --json`
  and `lf task list --json` were seen as unfinished rows while running, and the
  row count was unchanged after exit. `lf history processes --limit 3` (rejected
  arguments, exit 2) never wrote a row. The same `task list` with
  `LF_AGENT_CALLER` inherited left one row (`via_agent=1`, this Session as caller).
- **Fast-step Graph:** `graph_tests::a_step_that_finishes_before_the_graph_looks_keeps_its_result` ok (76 s at load ~30).

## Not caused by this branch

`lf task status LOO-615` fails with `serialization error: missing field
'interactive'` from the installed 0.14.17 on the live `~/.lf` store as well. It
is a separate defect and needs its own Task.

## Not demonstrated here

`lf land` while Desktop polls diff/files. Swift tests cover the nil cwd
(`RegistryQueryTests|TaskFilesTests`, 43 ok). No display session was available
for the app. Gate still owes the quiet `store_contention` base/branch comparison
and the Work-watch frame check.

## Decision

Advance. Every Done-when item that a headless run can show held on Jack's real
data: ~105k rows as predicted, and the FK check stayed empty.
