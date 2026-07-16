# Resolve Session bodies through the current Home lf

## Problem

A durable Project or Task Session used to pin its `lf` binary, database path,
and `LF_HOME` at creation time (`ChildExecutionContext`). When resumed after an
`lf` upgrade, reinstall, or move, the launch path read that *historical* context
and tried to exec the old binary. If the old path was gone or rejected the
current DB migration, the Session was stranded: worktree, provider history,
commands, and generation sequence intact in the store, but nothing could launch.

That is the failure behind W2-177, W2-178, W2-218, W2-224 — their `lf_bin` paths
point at binaries that no longer exist or reject the shared registry migration.

## Design

**Resolve the current Home lf at the launch boundary. Never pin.**

1. **No launch identity in domain state.** `ChildExecutionContext` is removed
   from `ProjectSession`/`TaskSession` and from new-session writes. The historical
   `lf_bin`/`db_path`/`lf_home` columns physically remain but are written NULL and
   never read — they are dead schema until an earned table rebuild drops them.

2. **A distinct current-Home resolver.** `current_home_execution_context()`
   resolves the current Home lf, store, and `LF_HOME`, ignoring every
   `LF_CONTROL_*` value. This matters in release builds: the old
   `pinned_execution_context()` prefers `LF_CONTROL_BIN`/`LF_CONTROL_HOME`/
   `LF_CONTROL_DB_PATH` — precisely the historical pin a legacy body carries.
   `pinned_execution_context()` is retained only for vendor-subprocess control
   propagation (a running body handing its own session context to a provider CLI).

   The launch boundary is the single choke point `child.rs::launch()` funnels
   through — `launch_task_process` and `launch_project_process` — so resume,
   supervisor wake, and handoff wake all get the current Home.

3. **Provenance describes what ran.** Each `ChildProcessGeneration` carries an
   immutable `BinaryProvenance` (version / provenance / source_identity). The
   reserving launcher records *nothing* — nothing has run yet. The child stamps
   its own binary's provenance when it boots (`ChildProcessGeneration::mark_booted`,
   called from the task/project runners at lease activation). So the audit row is
   B (what booted the generation), never A (what launched it).

## Store

Migration `0.11.018_session_body_provenance` adds `process_provenance_json` to
`task_sessions` and `project_sessions`. Provenance is written on reserve and on
lease activation, read back by the row mappers, and shown by `format_child_body`
(`binary <version> (<provenance>)`, or `binary unknown` for a pre-field / never-
booted generation).

## Proof

- `current_home_binary_never_resolves_through_the_control_pin` — the current-Home
  selector picks B where the old release override picks the `LF_CONTROL_BIN` A.
- `launch_{task,project}_process_ignores_control_bin_and_resolves_current_home` —
  a real `LF_CONTROL_BIN` plus a missing current `LF_BIN` fails at resolution;
  the control pin is never consulted.
- `generation_provenance_is_the_booting_binary_not_the_launcher` — launcher A
  reserves with A's provenance; `mark_booted` at activation overwrites it, and the
  persisted audit row records B (`BinaryProvenance::current()`), not A.
- Provenance round-trips through insert, reserve, and activate for both session
  kinds; migration collision gate stays green with `.018` after main's `.017`.

## Done when

- `cargo test` green (migration collision gate included).
- A Session created under binary A, with A removed, resumes through binary B with
  worktree, provider history, directives, and generation sequence intact.
- `lf status` shows the booting binary's provenance on active generations.
