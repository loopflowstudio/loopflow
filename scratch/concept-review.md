# Concept review: Flow steps and their processes

LOO-298 · 2026-09-29 · Reviewed at `fd9cf980b` against Jack Heart's
[accepted model](data-model-one-table-per.md) and [Exec-per-step design](exec-per-step.md).
This autonomous review supplies evidence, not Jack's acceptance or Flow navigation.

## Experience and usage

Keep the supported interaction in `docs/lf.md`:

```sh
lf task run LOO-298
lf flow list --sessions --for-task LOO-298 --json
lf exec list --task LOO-298 --json
lf exec show EXEC_ID --json
lf flow resume FLOW_SESSION
```

The accepted experience is one saved Flow, visible step processes, and continuing
conversations. Recovery uses the same Task/Flow identity. No new user command,
step object, or manual choice of driver is needed.

Proposed recovery wording for `docs/lf.md`, after the agent conversion is proved:
“Resuming waits for the selected step if it is still working, then consumes its
recorded result once. A missing completion stays unknown. Explicit retry preserves
that earlier evidence.” Mechanical children implement the first sentence now;
agent children and their surviving-provider path remain unfinished. Retain the
existing engine-exit conditions beside `--retry`; this wording does not broaden
retry authority. Existing skill instructions to continue a saved Task remain
appropriate. Keep `__flow-step` out of customer skills and examples.

## Current ownership

| Fact | Owner and transition |
| --- | --- |
| Actual lf process and observed command result | Exec; process entry and terminal observation |
| Captured graph, selected boundary and advancement | FlowSession; driver claim and fenced settlement |
| Conversation, native history and provider outcomes | AgentSession and its history; current conversation driver |
| Mechanical effect | Selected Flow history start/result referencing the child Exec |
| Task execution | Task selects the managed root FlowSession; startup claim transfers to the actual worker |

The normal mechanical path is driver → child Exec → stored operation result →
driver settlement. If the driver dies, resume observes the selected child before
replacing the claim. Successful child exit alone is insufficient; settlement
reads the saved result. Unknown process identity cannot authorize another effect.
This represents the accepted objects without another durable lifecycle.

## Findings and simplification

1. **Agent execution still has a different process owner.**
   [`execute_step`](../rust/loopflow/src/lf/commands/flow.rs) accepts only mechanical
   operations. `CliFlowExecutor::run_skill` reserves before invoking its launcher;
   `SavedLauncher::launch` calls `run_saved` in the driver, while
   [`TaskLauncher::launch`](../rust/loopflow/src/controller/task/mod.rs) prepares
   and starts its harness there. The comment claiming each saved step is an
   `lf <skill>` already describes the target more strongly than the code.
   Move agent capture reservation and execution into the same child entry,
   then remove the two launchers. Preserve Task seed, account/agent selection,
   steers, attachment, interruption, retries and exact typed-output consumption.
   Moving only provider spawn would leave capture ownership wrong.

2. **Process ancestry and execution authority must remain distinct.**
   [`agent_parent`](../rust/loopflow/src/store/sqlite/execs.rs) resolves the
   current driver only when provider generation and origin still match; otherwise
   it retains the proven origin. A frozen environment parent cannot replace
   this behavior after handoff. Mechanical recovery already follows the selected
   operation Exec through `wait_for_step`, rather than signaling every descendant.
   Keep that distinction when deleting redundant attribution fields in the next
   planned reduction. Preserve shared-engine siblings and imported unknown parents.

3. **The cutover summary contains superseded evidence.**
   [`docs/architecture-reference.md`](../docs/architecture-reference.md#cutover-status)
   still says the valid Codex decision retry fails original-turn transport.
   [Current evidence](evidence.md) and [the execution handoff](parallel-execution.md)
   retain the passing structured-result replacement with real Codex and synthetic
   Responses. Reconcile this row during documentation work: the fixture failure
   is closed; configured-provider acceptance remains unproven. Likewise keep the
   mechanical-only process conversion explicit until agent steps move. A table
   inventory or vocabulary rename cannot establish the remaining behavior.

No new product decision is necessary for the agent conversion. Jack's final
review still needs the choices retained in [questions](questions.md): prospective
versus post-hoc usage after bind, the retained blocked-feedback interaction, and
runtime-pass interpretation. This review selects none of them. Imported history
without process evidence must never become an Exec merely to simplify naming.

## Proof and next action

Reused `.lf/tmp/cut-i/exec-review-full.log`: **2,032 passed, 16 skipped**,
including distinct mechanical child Execs, success before later failure,
surviving-child recovery and Task stop after driver death. All 466 Rust/Cargo/
fixture hashes in `exec-review-source.json` match this checkout. Inspected retained
architecture and all-target Clippy passes. No behavioral suite was rerun.
The [slice review](exec-per-step.md#slice-review--2026-09-29) owns detailed evidence.

Next implementation proof: start an agent step under its own Exec and capture;
kill its Flow driver while the step or provider survives; resume the saved Flow
without a second turn and consume the exact completion once. Exercise direct
child, agent-issued child/grandchild and post-handoff parentage. Retain Task
input/account/steer/attachment behavior and shared-engine exclusion throughout.

Then follow the accepted order: attribution/parent reduction, separate naming
commit, populated public import and canonical counterpart, final documentation.
[Remaining work](remaining-work.md), [import preservation](import-preservation.md)
and [Chapters](chapters.md) retain all integrated, configured-provider, Desktop,
incident and real-Home obligations. Mechanical fixture success does not complete
Exec-per-step or establish code-complete concept-review acceptance.
