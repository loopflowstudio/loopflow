# Remaining work to code-complete review

LOO-298 · Realigned 2026-09-30 for Jack Heart. Order follows Jack's direction
"do the deepest cuts first." Work top down. Decisions and their wording are in
[questions](questions.md); the accepted model is
[data-model-one-table-per.md](data-model-one-table-per.md).

## Model now in force

- **Exec** is one actual lf process. Every Flow step is its own Exec: skills,
  routers and reviews run the ordinary `lf skill` command; ops run their own
  command. An agent-issued lf command's parent is the lf process driving that
  agent (Jack, 2026-09-29).
- **AgentSession** is one conversation. A captured input is an event in its
  history that names its Exec; imported history with no process keeps none.
- **FlowSession** is one started Flow. A definition may reference other Flows;
  starting it compiles them into one graph of skills and ops. Subflows and loop
  passes are lenses over that graph and its history, never their own
  FlowSessions (Jack, 2026-09-29 and 2026-09-30).
- **Flow decisions** are typed results of the selected successful turn. The
  in-turn decide, route and blocked commands are gone. Blocked requires a reason
  and returns Ask feedback to another turn of the same conversation.
- **Where running a skill directly and running it as a Task step disagree,
  the direct behavior wins** (Jack). Task specifics are input to the same command.

## Order, deepest first

1. **Every step runs its ordinary lf command.** Ops and skills are converted and
   both launchers are deleted (`9cb7b4c86`, net −1,242). Common Task seed/name,
   checkout context after landing, PATH step selection and schema-gap preservation
   are checkpointed at `2ebfd9f51`. Public scripted account/name/context,
   driver recovery and Chapter preservation passed. Retain managed repeated
   decisions, failure release, explicit retry and configured account/control
   proofs. Task controls already use the common command;
   do not build another transport. Typed `blocked` with a required reason now
   replaces `lf flow blocked`; the public taskless proof recovers the keyed Ask
   and continues three deciding turns in the same native conversation. See
   [blocked decisions](blocked-decision.md); managed/interactive acceptance is
   still distinct. See [the current audit](task-command-equivalence.md) and
   [the before/after inventory](exec-per-step.md#direct-skill-command-cut--2026-09-29).
2. **Remove loop passes as FlowSessions** (Jack, 2026-09-30). The contract text
   is updated (`d61295196`); the code is not. A forward migration folds existing
   child passes into their root, preserving history, selected completions,
   reviews and active cursor and return counters. Then drop
   `flow_sessions.parent_id`, `flow_current_child`, `validate_flow_parent`,
   `retain_flow_parent`, and `managed_flows` if it only serves this. Delete child
   claim transfer, deepest-current-child reads, root driver-lock indirection and
   `FlowFilter.parent_id`. Retry and next pass stay on the cursor. Rewrite
   behavior tests to positions; delete child-row-only tests. Report net lines.
3. **Simplify attribution and the parent tree** (Jack). Before and after tables
   are in [exec-per-step.md](exec-per-step.md); the inventory is
   `.lf/tmp/attribution-tree/inventory.md`. Delete the dead per-turn token and
   `LF_PARENT_RUN_ID`. One owner per attribution fact; derive the rest. Keep the
   lookup that resolves an agent-issued command's parent through Session and
   provider generation. Jack's final September 30 precedence is explicit `--as`
   on this command (including Task shortcuts), then checkout ownership, then
   an ancestor's explicit `--as` through `LF_AS`. The current attribution slice
   deletes `agent_work_in`, `LF_TASK_ORIGIN`, claim/manifest Task discovery and
   the `LF_PARENT_RUN_ID` writer; PR/installation checks use the common reader.
   The public cross-checkout proof passes. The Task-shortcut public proof failed
   on hosted `36cbb3d4b`: its inherited OpenCode selection cannot enforce the
   checkout boundary. The fixture now selects Claude for Y's mechanical Flow;
   confinement is unchanged. The subsequent
   [slice review](review-task-command-slice.md) executed the corrected public
   proof successfully in disposable Linux after Docker recovered, plus three
   public attribution checks. Post-CI repairs pass the complete
   materialized Rust matrix (2,024 passed, 17 skipped); see the retained failures
   and final source receipt in evidence. Keep the dead per-turn token and Flow membership
   provenance audit open. See [evidence](evidence.md).
4. **One naming commit** (Jack). Exec instead of launch or run for one agent
   start under one lf process; Session names for conversation things; "compile"
   for turning a definition into its graph; "parent" only for Exec to Exec, so
   template provenance is removed unless a real consumer needs it. Swift's
   `TaskFlowView` currently displays its `from …` breadcrumb; this consumer is
   reported in the audit for Jack, not renamed. Proposals go in a before and
   after table for Jack.
5. **Released-populated import**, executed through the public binary and on a
   materialized copy. Obligations: [import-preservation.md](import-preservation.md).
6. **Docs, skills and generated pages** for final behavior, including the stale
   Codex-retry row in the architecture reference's cutover status.

Done and verified on hosted CI: structured-result decisions, captured input as a
Session event, typed history readers, saved-Flow discovery with Desktop paging,
worker claim handoff, mechanical steps as child Execs.

## Obligations that still bind

These are not implementation items and are not closed by fixtures:

- **Configured acceptance.** Real accounts for Codex, Claude and OpenCode;
  interactive reconnect; a configured Desktop run.
- **Dense measurements.** Cold and warm CLI list and detail on a representative
  store, with startup, SQL and payload separated.
- **Chapters.** Remaining proofs in [chapters.md](chapters.md).
- **Incident.** Cancellation with a live owned child still needs a settlement
  proof; causal ancestry never authorizes a signal.
- **Real-Home conversion and release.** Backup, rehearsal, quiescence and
  separate authority. No branch binary touches an installed Home.
- **Integrated finish.** Full affected suites once on final bytes, migration and
  architecture checks, formatting and all-target Clippy, then the saved Flow's
  delivery steps.

The superseded checklist and its receipts are in history at
`d61295196:scratch/remaining-work.md`.
