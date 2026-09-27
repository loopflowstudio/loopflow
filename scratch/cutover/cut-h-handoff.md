# Cut H handoff — Flow cursor into the invocation row (unfinished)

Stopped on 2026-09-27 when Jack took over LOO-298. Nothing in this cut was
compiled, tested or committed as code. The partial edits are saved as
[cut-h-wip.patch](cut-h-wip.patch) against `e20fb5e22`; the source tree was
restored so the branch builds. Apply with `git apply scratch/cutover/cut-h-wip.patch`.

## Goal

One owner for every Flow's cursor: the `flow_invocations` row, with or without
a Task. Delete `flows/<id>/position.json`, `StepToken`, `FlowRun` and the file
I/O in `ops/flow_run.rs`. See "One implementation" in
[the design](../data-model-one-table-per.md).

## Decisions taken for this cut, none reviewed by Jack in detail

1. Launch facts are columns on `flow_invocations`.
2. The stale-writer fence is the position version; the step token goes away.
3. `lf flow decide` and `lf flow route` fence on the invocation row.
4. A saved Flow refuses to run when the store cannot be written.

## What the patch contains (written, never compiled)

- Draft `own_flow_launch`: six nullable columns on `flow_invocations`
  (`cwd`, `message`, `model`, `wave_selector`, `task_selector`, `work_selector`).
- `session::FlowInvocation`: the taskless row type.
- Row functions in `store/sqlite/sessions.rs`: `create_flow`, `flow`,
  `flow_waiting`, `recover_flow`, `begin_flow_step`, `save_flow_cursor`,
  `set_flow_failure`.
- Removes `save_flow`, `waiting_flow`, `end_flow`; `create_session` loses its
  `review` parameter.

## What breaks if the patch is applied alone

- `store/sessions.rs:38-127` wrappers still call the removed functions.
- `create_session` callers still pass three arguments: `ops/human_session.rs:395`,
  `ops/flow_session.rs:140`, `ops/session_import.rs:230`,
  `lf/commands/run.rs:925`, test at `store/sqlite/durable.rs:2144`.
- `lf/commands/flow.rs:269,467` and `ops/human_session.rs:1212` call removed functions.

## Choices embedded in the patch that differ from the brief

- The Flow name stays in `invocation_json`, with no column, to avoid a second owner.
- The boundary's `completed` flag is gone; one write settles the step, clears
  the claim and bumps the version. A `flow_run` test of the intermediate state
  would need rewriting.
- The driver lock is kept: a row claim cannot prove its claimer is alive.
- `FlowPosition.task_id` stays non-optional, so the Task controller and the CLI
  driver remain two drivers over one table. Full unification was not attempted.

## Next steps

1. Re-key `record_flow_verdict` and `record_flow_route` in
   `store/sqlite/durable.rs` on the Run; apply Task checks only when the row
   names a Task. Delete `ops::task::task_verdict`.
2. Move the driver in `lf/commands/flow.rs` to the row and delete
   `ops/flow_run.rs`.
3. Re-key `ops/flow_session.rs` on the Session id, move the old file shape into
   `ops/session_import.rs`, then write the acceptance tests and run the proofs.
