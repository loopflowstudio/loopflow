# Task conversation and ordinary background Flows

Draft architecture; accepted interaction direction from Jack Heart, October 4, 2026.
Placement: Product, with shared runtime/schema changes requiring coordination with
Infrastructure. Jack Heart requested updating LOO-353 and running kickoff, then review-design before implementation.

## What to build

Keep interactive Task work in one ongoing conversation; delegate all headless work
through the ordinary `lf -b` API and show every associated Flow with comparable
graph, step, output and history views, without a privileged managed Task Flow.

## Accepted direction

Jack confirmed that design and review stages return to the existing Task conversation
instead of opening another interactive Session. All headless work uses separate
Sessions through `lf -b`; substantial implementation in the Task conversation is
discouraged. Incidental edits can remain inline. Additional deliberately opened
conversations remain possible; a primary conversation does not define membership.

Jack clarified product naming: interactive conversations are Sessions; headless
Sessions are presented as Runs. Both remain Sessions in infrastructure and the
data model; no naming-driven migration or new Run entity. This does not rename Exec.
Jack noted that retaining the shared Session identity naturally supports conversion
between interactive and headless use. Preserve history and attribution across that
change; product labels follow the current mode, not the original launch mode.

Jack requested that `--waiting` represent current response/review attention,
separate from `--interactive`, which selects the conversation surface regardless
of activity. Jack subsequently chose one visible state, **Waiting**, without
confidence labels or a distinction between explicit questions and ordinary yielded
answers. `--waiting` selects Waiting Sessions. Provider-specific detection and
occasional false positives are accepted in favor of a simple experience.

Jack proposed removing mutable control of a running Flow and potentially its database
state. Jack clarified that crash recovery belongs to whoever called `lf flow`,
normally the Task Session under SOP. Automatic Flow resumption is not a retention
requirement for a database cursor. The exact review handoff and observation storage
remain design work. This supersedes earlier design proposals for switching a
running captured Flow, but does not modify running work or existing review gates.

## Current system and evidence

Source inspection on October 4 in the Product checkout, with the corresponding core
types and Desktop views also inspected in the main checkout:

- `rust/loopflow/src/durable.rs`: `FlowSession` combines captured invocation, cursor,
  version, attempt, pending review, worker generation/claim and failure.
- `rust/loopflow/src/store/sqlite/flows.rs`: `tasks.current_invocation_id` selects
  one managed Flow; cursor writes and worker claims participate in write fencing.
- `rust/loopflow/src/ops/run.rs`: `TaskWorkerExec` launches a separate `task __worker`
  path. `store/flows.rs` exposes claim/reclaim, retry, checkpoint and review writes.
- `rust/loopflow/src/lf/commands/flow.rs`: managed resume routes through Task worker
  claims; interactive boundaries branch between managed and ordinary review paths.
- `rust/loopflow/src/ops/flow_session.rs`: ordinary human steps allocate a fresh
  `FlowReview` Session and retain its pending ID/feedback on the Flow.
- `rust/loopflow/src/store/sqlite/flow_inventory.rs`: Task-filtered Flow inventory
  and per-Flow detail already expose association, graph and projected cursor.
- `swift/LoopflowMac/Views/TaskWorkView.swift` lists all associated Flows;
  `TaskFlowView.swift` supplies richer presentation for the singular pinned Flow.

The inspected CLI source spells batch execution `--mode batch`. `lf -b` is Jack's
requested API; confirm the target CLI surface during implementation rather than
assuming that shorthand is already shipped.

## Proposed deletion boundary

Delete — do not maintain: the singular managed Flow pointer, managed/unmanaged
runtime branches, Task-specific worker launcher and claim APIs, and Task UI controls
whose purpose is mutating/replacing that selected running Flow. Cut over their
exclusive tests and DTO fixtures together. Preserve unrelated Session/Exec ownership,
Task association, process identity, history and external-effect evidence.

Reuse Flow inventory/detail and graph rendering for every associated invocation.
Task conversation chooses and launches ordinary background work. Observation never
claims a worker or implies that missing process evidence means completed execution.

The proposed runner owns its cursor in memory and publishes read-only observations.
The caller owns recovery after failure; no database-backed controller automatically
reclaims or resumes the Flow. Observations still need provenance and a lifecycle.
Do not move the same mutable controller into JSON files or add a second supervisor.

## Session log attention (accepted direction, October 4)

Jack Heart requested emphasizing Sessions waiting for a conversational response
and hiding or minimizing Sessions currently working. Exact presentation remains
open. Prefer a prominent waiting group and a compact, expandable working group;
retain selected/visible panes rather than moving focus when activity changes.
This revises the earlier rejection of attention-oriented Session presentation,
without changing which Sessions belong to a Task.

Current `ops/human_session.rs` exposes separate Session state and `SessionAttention`
(`Review`, `Reply`). `Active` indicates a client exists, not that it is mid-turn.
`Reply` currently derives from an interactive completed turn with no driver outcome;
that means the provider yielded, not necessarily that Jack must answer a question.
Current Flow reviews can signal attention before their agent finishes preparation.
Do not map these coarse values directly to the proposed working/waiting groups.

Present one **Waiting** state across providers. Each adapter uses its available
signals; the baseline heuristic is no outstanding tool calls and N minutes since
the last update. New turn/tool/output activity clears heuristic Waiting. Do not
display “likely waiting,” confidence tiers or separate yielded/question states.
Known failures and closed conversations keep their existing lifecycle treatment;
this heuristic grants no execution, recovery or review-completion authority.
Claude uses only the binary's stream-json path, without SDK, hooks or a separate
permission host. The timeout N remains an implementation choice to settle.
Headless failures return to the Task conversation for supervision, without creating
another interactive conversation. Provider event races need focused tests, but
perfect semantic question detection is not an acceptance prerequisite.

## Open contract and internal slices

This is one proposed coherent runtime/UI cut, not independently shippable overlapping
controllers. This slice is design: settle the review handoff and interrupted-run
behavior before selecting storage or a migration. Then remove Task control authority,
route interactive boundaries to the Task conversation, and generalize Flow views.
Use one migration against the released schema if schema removal is selected.

A review must identify the exact invocation and step occurrence it answers. A reply
in the primary conversation must not advance another waiting Flow or close the
conversation. Decide whether reaching that boundary ends a background segment and
the conversation launches its successor, or suspends a runner for feedback. After
interruption, the caller inspects retained evidence and chooses the next invocation.
Preserve evidence of effects before retrying; historical progress is not a mutable
resume cursor or proof that an uncertain external action never happened.

## Demo and acceptance

From one Task conversation, launch headless work, inspect two associated Flows with
equally rich graphs/output, reach design/review in that same conversation, give
feedback, and continue the correct work. No new review conversation, singular Flow
selection, duplicate provider, or substantial inline implementation is introduced.
Completed history remains inspectable and unknown liveness remains explicit.

Headless gate: Rust Flow/association/review behavior tests, shared DTO fixture tests
and Swift Flow/navigation interaction tests must cover the above and interrupted
execution. Pin exact focused test commands after the runtime boundary is settled;
UI judgment stays in demo. No tests run: design and source inspection only.

## Retained workspace scope

The October 1 workspace checkpoint landed as PR #1369. Preserve remaining
configured provider/remote-Home/file-draft proof, twenty retained-layout samples
against the proposed p95 <100ms target, default Flow/source editing, and website
work from docs/reviews/task-workspace.md. Kickoff must reconcile this work rather
than silently dropping it. LOO-364 shipped repo/Wave primaries, not the complete
Task conversation experience. October 4 direction supersedes earlier restrictions
on attention UI and the proposal for mutable running-Flow switching.
