# Concept review: one Flow, ordinary commands

LOO-298 · Review of `d61295196` and the existing working notes for Jack Heart.
This is autonomous review evidence, not Jack's acceptance or a navigation verdict.

The shared skill command removes the substantial duplication Jack identified.
The next model cut is already decided: one started Flow owns all its passes.
Code-complete review is still blocked by the remaining implementation and proof
obligations in [remaining work](remaining-work.md). Two consequential choices
remain unresolved: managed-account policy at each agent boundary, and how the
driver's pinned executable relates to LOO-334's official-lf-per-step direction.

## Experience and owners

The existing usage remains appropriate:

```sh
lf -b implement
lf session list --interactive false --task LOO-298 --json
lf session connect SESSION
lf flow example
lf flow resume FLOW_SESSION
```

Accepted target: starting a Flow compiles its definitions once. Skills execute
through the ordinary skill command and ops through child command dispatch.
Iterate changes the position within that same Flow; Retry retains the pass.
Resuming after driver death observes surviving work and consumes its exact
recorded completion without sending another turn. Removing pass FlowSessions
is not yet implemented; these commands alone do not establish that behavior.

| Concept | Owns | Operation that changes it |
| --- | --- | --- |
| Exec | One actual lf process, causal parent and observed command outcome | Command admission and journal completion |
| AgentSession | Conversation, current driver, native identity and subordinate history | Reserve captured event, execute/connect, rename/bind |
| FlowSession | Compiled graph, cursor, return counts, selected result and orchestration claim | Start/resume; consume the selected completion with cursor advancement |
| Task | Work identity and selected managed Flow | Task start/continuation; review preparation follows that selection |
| Pass/subflow | Position or definition grouping within the Flow | Display/history lens; no independent claim or lifecycle |

The process tree and conversation lifetime remain different facts. Driver death
does not prove provider death. A resumed conversation does not reopen an old Exec,
and a successful command parking at review does not complete its Flow.

## Findings and consequences

**Keep the common command; resolve the account policy explicitly.**
`lf/commands/flow.rs::execute_child` executes the current binary's ordinary
`skill` command with the saved boundary token. `run.rs::run_flow_skill` reserves
the capture under that child's Exec, supplies Task input and execution scope,
then uses the shared prompt/provider path. This matches Jack's direction.

However, `ops/task.rs::preflight_task_execution` requires a usable managed
account, while the child calls only `task_execution_boundary`. Task startup
skips provider preflight for an op boundary. Thus startup approval cannot prove
the later selected provider meets the retained account policy. This confirms
the preceding review's source finding; no real credential fallback was attempted.
Jack's approval of direct retry, precedence and recording behavior does not
explicitly waive this restriction. The conservative implementation is to carry
the existing policy into the common command's account selection, preserving
inherited leases and failover. Removing that policy requires Jack's decision;
restoring a separate Task launcher would reintroduce the original problem.
The boundary also currently rejects OpenCode: common harness support does not
establish managed OpenCode acceptance.

**Reconcile executable selection before treating the hidden entry as final.**
The September 30 operator realign records LOO-334's direction that each step
picks up official lf. Current `execute_child` uses `std::env::current_exe()` and
inherits the saved Flow's store and claim. Reusing ordinary command semantics
does not resolve which installed version executes them. The review does not
choose between these directions. The integration must specify when a step
adopts official bytes and how those bytes consume the existing captured graph,
store schema and authority. Switching only the executable could strand the
saved boundary; pinning forever would miss the stated update behavior. Preserve
this tension in [questions](questions.md) without inventing another launcher
or allowing a branch binary to migrate installed data.

**Remove the pass lifecycle, preserving its evidence first.**
`store/sqlite/flows.rs::checkpoint_in` still creates a new invocation on Iterate,
copies the graph/cursor, transfers the claim, and recursively returns completed
children to waiting ancestors. `current_flow_in`, `flow_root`, `managed_flows`
and the inventory's `parent_id` expose that representation across readers.
These are concrete deletion targets under Jack's latest decision, not a naming
problem or another product choice.

The forward migration must retain event sequence identities: `flow_events`
references exact Session events and mechanical `operation_start` events, while
the active Flow selects those references. Folding ownership into the root must
also carry the active descendant's cursor, return counters, pending review and
selection; keeping only the waiting root cursor would rewind work. Preserve
historical outcomes and imported missingness. Do not manufacture success from
an old child's terminal state, renumber event evidence, or grant old workers new
authority by transplanting their claim without reconciling its identity.

The preceding review repaired managed review preparation after claim release:
`CliFlowExecutor::run_skill` now checks the Task's selected Flow instead of
inferring ownership from a live worker claim. Keep that rule when the recursive
Task Flow lookup becomes a direct root lookup. Separately attributed Flows must
continue using ordinary review preparation.

**Task control transport exists; its retry proof remains incomplete.**
`ops/task_input.rs::TaskInput` owns durable steer/interrupt cursors, attached stdin
and comment refresh; `run_flow_skill` supplies it to the common executor.
The working checklist's “a channel … for live steers, interrupts and attached
input” should not prompt another implementation. The retained public fixtures
exercise attached input and interruption. Account failover with those controls,
repeated managed decisions, both-dead explicit retry, pre-publication/provider
failure release, and keyed unblock reassessment remain the named proof gaps in
[the command inventory](exec-per-step.md). A failed fixture quoted in agent
output must never become current capability-blocker authority.

## Next proof and retained limits

For the pass removal, use a populated pre-migration Flow with completed passes
and an active nested pass waiting at review or holding a selected completion.
After upgrade, assert one FlowSession, unchanged event references/history,
Task identity/Started/PR, and the exact active position. Resume using the saved
graph after definition removal; consume once, retry at the same position, then
Iterate by advancing counters. Retain stale-worker and separately attributed
Flow exclusions. Run the corresponding materialized migration proof. Existing
behavior tests should change to position assertions; row-count-only child tests
can disappear. Report net lines after that implementation, not for this review.

The preceding review is retained at
`e54efaf44:scratch/skill-command-review.md`. Its coverage ledger records **2,022
unique eligible passes and 15 ignored tests**, combining an interrupted full
matrix with its exact remainder. This review compared both final-matrix and
managed-command receipts against the checkout: all **463 relevant hashes** match
each receipt. No behavioral suite was rerun, and no new hosted result was read.
The repaired public proof uses real lf/tmux with scripted Linear/Codex.

The named `shared_driver_parks_after_releasing_its_claim_at_a_review` test was
**deleted**, not repaired under that name. Its earlier failure remains recorded.
The retained store settlement test and strengthened public managed-review
fixture provide replacement evidence within their documented scope.

Attribution reduction, the separate Exec/Session naming commit, executed
released-populated import and canonical counterpart, final Chapter preservation,
docs and configured-provider/Desktop acceptance remain required. Preserve the
prospective-usage binding assumption for Jack's review; it is not a decision to
rewrite historical ownership. The cancellation/live-child incident and prepared
real-Home conversion procedure remain obligations in the existing plan. This
review establishes no installation, promotion, shipment or Task completion.
