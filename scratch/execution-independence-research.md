# Execution independence research · 2026-10-04

Jack Heart requested this independent research and authorized one follow-up Task. This contribution leaves LOO-367's product review, canonical design, questions, memory and HTML with the primary conversation.

## Scope and inspected evidence

Repository: loopflowstudio/loopflow (origin git@github.com:loopflowstudio/loopflow.git).
Checkout: /Users/jack/src/loopflow.start-and-finish-tasks-without.
Branch HEAD: c5dbcb72ce1c1016c0febfa0484aaee70e421ac3.
Available local main: 04105d34059d3516b9c159c874aca9a8a07daa85.
Supplied PR base: 8c72e591e78a68227255fd86bfff6a939ded5e8b.
No fetch, switch, sync, commit, production edit, worker launch or runtime test was performed. Local main is not claimed to be the latest remote revision. Line anchors below refer to branch HEAD unless explicitly marked main; production files were clean at initial inspection.

Read AGENTS.md and the existing design/review direction in scratch/jack-heart/start-and-finish-tasks-without.md, scratch/questions.md and scratch/task-purpose-demo.md. Jack's tentative worktree default and explicit no-landing cleanup requirement remain LOO-367's open product work. Neither is absorbed here.

## Existing shared owners: separate entry points do not prove duplicate engines

- Direct Flow capture enters commands::flow::execute → drive (rust/loopflow/src/lf/commands/flow.rs:75,122,375). Managed controller::task::drive_task calls that same drive with its exact worker claim (controller/task/mod.rs:32–53). Main also has drive (:374), drive_loop (:408) and one CliFlowExecutor (:668).
- drive_loop and CliFlowExecutor own graph traversal and checkpointing (commands/flow.rs:409,629). execute_child (:868) launches captured skill steps through ordinary lf skill and operations through __flow-step. execute_step (:806) calls ops::execute_flow_command. Model/provider retry is not a second Task-specific traversal engine.
- Invocation resume checks whether the Flow is the Task's selected managed member, routes it through task_run for worker authority, otherwise drives the saved Flow directly (commands/flow.rs:145–197). Desktop RegistryQuery.resumeFlow (swift/Loopflow/Services/RegistryQuery.swift:188) invokes flow resume; TaskFlowView.swift:1183 uses it for independent members.
- Both paths share prepare_native_retry (:236), wait_for_step (:254), recover_native_flow (:283). Managed launch calls prepare_native_retry at ops/task.rs:5472. Unknown Exec identity preserves pending effects; native history recovery does not manufacture a driver claim. The Codex and Claude readers are provider-specific adapters, not evidence of duplicate Flow engines.
- Human review deliberately branches at commands/flow.rs:705: selected managed Flow uses controller::task::park_at_review (:146), independent/taskless Flow uses ops::flow_session::reserve (:43). Both use AgentSession, captured membership, human_session::publish_prepared_input (:333), and the shared session action surface (human_session::complete :1359).
- Managed completion validates the exact review token and actor, then calls complete_task_review (controller/task/mod.rs:79). Independent completion persists the Session then resumes the saved Flow (flow_session.rs:99); its regression explicitly retains the cursor until feedback consumption (:270–286). Managed complete_flow (human_session.rs:830) instead resumes via exec_task_process. That difference includes real authority and settlement responsibilities: deleting it wholesale would be unsafe.

## Ranked opportunities

### 1. Confirm cancellation/deletion without refreshing unrelated planning — selected follow-up

Observation: ops/pm.rs::delete_task (:1555; effect/confirmation :1623–1641) records provider deletion and local confirmation before mandatory refresh_pm_snapshot (:1648). The error explicitly says deletion is confirmed. store/sqlite.rs::confirm_task_deletion (:690) owns the immediate transaction preserving completed outcomes and recording removal.

Observation: ops/task/lifecycle.rs::apply_abandon (:337–370) cancels, abandons local work, applies prepared Git cleanup, then refreshes the Wave. task_delete (:257) calls abandon first for registered unfinished Tasks. A refresh failure can therefore both misreport an already-settled cancellation and prevent the next requested deletion stage. Actual retained-work failures remain intrinsic prerequisites.

Counterexample supporting the finding: ops/pm/task_planning_tests.rs::task_deletion_planning_retries_snapshot_failure_without_repeating_deletion (:828) intentionally fails the post-deletion snapshot. assert_planning_deletion (:832; :922–937) asserts an error while deletion confirmation exists and the local snapshot excludes the issue. Its later successful retry consumes a one-shot fixture failure; no fresh execution of this test was performed here.

Main retains the same paths: pm.rs:1630–1661, lifecycle.rs:342–375, test :620. This is not duplication already deleted by LOO-367. LOO-367's pm_update_async (:1244–1322) already confirms the affected issue, and complete_planning_task (:1326) retains a single summary marker; reuse these lessons without reopening that implementation.

Interpretation: operation success is coupled to a broader observation after its own authoritative result exists. This is the strongest concrete beneficiary outcome, more defensible than filing an engine rewrite. Cancellation needs narrow authoritative readback, not merely removal of its last refresh; deletion already has its acknowledgment/trash/retained-confirmation owner.

### 2. Converge review preparation's agent-selection mechanics — secondary finding

Observation: independent review reserve (ops/flow_session.rs:50–53) picks flow.model or repository config directly. Managed select_review_agent (ops/human_session.rs:401–423) uses ops::task::resolve_task_agent (:1327), which delegates to engine::exec::resolve_agent (:244) with the captured skill. Main retains the independent selection at the same lines and managed selection at :403.

Concrete counterexample to investigate: a human skill declares its own agent, no Task/Flow override exists, and repository default differs. The independent reserved Session records the repository default; managed preparation uses the skill-aware resolver. This is observed source-selection divergence; actual provider launch across both paths was not exercised, so effective end-to-end divergence remains a proof obligation. An explicit Task agent override is intentional and must retain precedence.

A small common selection operation could remove divergent mechanics while leaving Task override policy and exact review authority at their owners. Do not infer from this finding that all review reservation/settlement should merge. No second Task filed: LOO-367 retains explicit-Flow parity acceptance and LOO-364 owns primary Session/Flow switching; this finding can inform those owners without silently changing their scope.

### 3. Preserve newer main's local restart planning policy when integrating — existing work only

Observation: this branch resolve_managed_task_planning takes a freshness argument (ops/task.rs:1434), restart requests Force (:5340), managed startup Auto (:5431), and the executor requests Auto (commands/flow.rs:676). Main resolve_managed_task_planning (:1303–1337) explicitly uses PmRefresh::Never for existing work; main commit acd6654f9 (#1413) includes restart without Linear. c5dc238b0 (#1429) also retires the previous review safely.

This is a branch/main integration obligation, not a new architecture Task. LOO-326 explicitly owns the restart timeout incident; LOO-377 owns prior-review retirement. No integration was performed. Preserve LOO-367's owning-repository/captured-cwd and completion changes alongside those newer policies rather than restoring remote prerequisites.

## Ownership and overlap

Ran lf wave list --json and lf wave status infrastructure --json from the supplied checkout. Infrastructure owns execution continuity, exact authority and architecture reduction; registered repository is /Users/jack/src/loopflow. Its active Project is Chapter demo-20260924, ID 5a3aaee8-a95a-4726-9578-22a4700270ac. The Tasks response was state=ok, truncated=false, unavailable_tasks=[].

Read complete relevant briefs from that response: LOO-305 (completed cancellation/deletion foundation); LOO-360 (canceled disposable abandon check, not a repair backlog); LOO-367 (admission/completion and current review); LOO-366 (Project availability); LOO-364 (primary Sessions); LOO-326 (offline restart and release/CI resilience); LOO-322 (unattended Flow convergence); LOO-370 (retired Run cleanup). Also inspected LOO-377 and LOO-373 descriptions for review/startup recovery. No exact active cancellation/deletion-confirmation repair was found. The selected follow-up is intentionally limited to retirement operations, not LOO-367's implemented issue update/completion cut.

## Proof limits and review

Read-only source comparison and existing fixture assertions support the findings; they do not establish fresh test passes, configured provider acceptance or Desktop interaction. No auth repair or installed-Home migration was attempted. No need for a second driver was demonstrated. The review criterion is one clear operation outcome backed by its own evidence; keep freshness acquisition and destructive authority distinct.

## Complete follow-up brief

Title: Finish Task cancellation and deletion without a whole-Wave refresh

Jack Heart requested this follow-up for people using ordinary CLI commands, agents and Desktop to retire unwanted work. A confirmed cancellation or deletion should finish using authoritative facts for that issue, without requiring unrelated Projects and issues to refresh.

Source evidence, inspected October 4, 2026: LOO-367 branch `c5dbcb72ce1c1016c0febfa0484aaee70e421ac3` and available local main `04105d34059d3516b9c159c874aca9a8a07daa85` both retain this dependency. In `rust/loopflow/src/ops/pm.rs::delete_task` (branch lines 1623–1654; main 1630–1661), provider deletion and local `confirm_task_deletion` precede a mandatory `refresh_pm_snapshot`. An unrelated refresh failure then reports “Deletion is confirmed, but planning refresh failed.” The existing `task_deletion_planning_retries_snapshot_failure_without_repeating_deletion` test (branch `ops/pm/task_planning_tests.rs:828`; main :620) asserts an error despite retained deletion confirmation. `ops/task/lifecycle.rs::apply_abandon` similarly cancels the issue, records local abandonment and applies prepared Git cleanup before requiring the Wave refresh (branch :337–370; main :342–375). Registered deletion composes abandonment first, so that refresh can also prevent the subsequent trash operation.

Desired behavior: cancellation and deletion reconcile the affected issue through existing planning ingestion and deletion-confirmation owners. Their results distinguish confirmed effects, unknown provider outcomes and genuinely incomplete cleanup. Unrelated Wave refresh must not determine success or trigger another mutation. Preserve a useful local view without inventing a second planning store.

Acceptance:

- Exercise public `lf task abandon` and `lf task delete`, each for planning-only and registered Tasks, with authoritative issue operations available while an unrelated Wave snapshot fails. Confirm canceled/deleted provider state and corresponding local state; confirmed operations succeed.
- Exercise both direct CLI invocation and an agent-issued CLI operation from a Task-managed Flow targeting a separate eligible fixture Task. Equivalent facts yield equivalent results without waiving caller or target authority. Verify the shared state and explanations consumed by Desktop; no new Desktop action is required.
- Lose cancellation/deletion responses, fail local confirmation, interrupt between existing lifecycle stages, then retry through the other entry point. Recover by stable issue identity and authoritative canceled/trash facts or retained confirmation; preserve original outcomes and history without duplicate destructive effects.
- Refuse unknown outcomes, changed ownership, live or unresolved associated work, dirty/unmerged work and open or uncertain PRs. Resolve blockers through their existing owners and prove a later retry succeeds. List omission never proves deletion.
- Retain completed Task outcomes, PR history and terminal times. Extend the existing stateful provider fixtures and assert stored/provider results, not merely mock calls.

Constraints: build on completed LOO-305 and existing lifecycle/store owners. LOO-367 retains admission/completion, explicit Flow startup, optional placement and the ongoing worktree-default/cleanup product review. Do not refile or redesign those cuts. LOO-366 owns Project availability; LOO-326 and LOO-377 own restart/review recovery. Preserve exact managed claims, generation fencing, provider revision ordering, unresolved effects and retained-work protections. No force bypass, service split, generic policy framework, new workflow lifecycle or duplicate execution driver.

Open decisions: whether any optional refresh runs separately or is omitted; how to report confirmed provider cancellation with incomplete local cleanup; the minimum issue-specific cancellation observation needed to update existing normalized planning safely. Settle these in design before implementation. Research established source behavior and existing test expectations, not a fresh runtime reproduction.

This filing authorizes planning only; it does not authorize implementing, launching a worker, completing LOO-367’s review, publishing or landing.

## Filing result

Created [LOO-379 — Finish Task cancellation and deletion without a whole-Wave refresh](https://linear.app/loopflow/issue/LOO-379/finish-task-cancellation-and-deletion-without-a-whole-wave-refresh) through lf task create --wave infrastructure without --run.
Provider ID: dcc598b6-aea9-4c82-bf66-796940323f4a; returned revision 2026-10-04T18:15:39.810Z; state unstarted.
Repository: loopflowstudio/loopflow, registered at /Users/jack/src/loopflow.
Wave: infrastructure, ID 6155f18a-1b7f-418c-9af3-8d6fa5ce4989.
Project: Chapter demo-20260924, ID 5a3aaee8-a95a-4726-9578-22a4700270ac.
The successful creation response returned the complete brief with its creation marker and canonical URL. No uncertain-write retry was needed; no worker was started or existing Task edited.
Only this artifact was written by the research contribution. Initial context check after publication reported scratch 10,052/12,000 tokens and no budget overflow; final filing appendix remains subject to the final context check.
