# Architecture review and feedback-control slice

## Intent

> "We should start with the architectural design in order to gain broader
> context ... However we are not taking on the scope of fully focusing all the
> architecture today. We ultimately want to get to carving out the right slice
> that addresses the issues that I just discussed."

Use a top-down architecture review to recover the current truth, update
`docs/architecture.md`, and select one coherent implementation slice around
Wave/Project/Task feedback. Do not attempt the entire remaining architecture
cutover in this work.

The target user experience has natural stopping points where either a human or
an LLM can enter the same feedback workflow before or after code is written.
CLI-first operation, a closed desktop app, and a missing parent process must not
strand durable work.

## Evidence so far

The branch has landed most of the durable control vocabulary:

```text
Work -> Epoch -> Run -> Launch -> optional Turn
            \-> Basis <- Steer / selected evidence
            \-> Wait
```

- Review rows, review dispositions, handoffs, and `ChildCommand` are gone.
- Review is derived from interactive flow position, Launch attention, and its
  route to either User or parent Work.
- One opaque Run lease authorizes child control.
- Project and Wave code contain child-Review control lanes.
- Project and Task execution still runs through legacy Session/body ownership,
  process generations, statuses, leases, and duplicated runners.

`docs/architecture.md` currently contradicts itself: its opening says parent
loops do not prioritize child attention, while its implementation frontier says
Project and Wave drain child Review before background work. It also references
`scratch/implementation-plan.md`, which is not present. The architecture page
must distinguish current behavior, remaining bridge, and future target using
the code as evidence.

## Questions the design must answer

1. What exact durable fact means "feedback is wanted now"?
2. Is a Project a continuously resident server, or stable Work that may have no
   process and is woken from durable attention?
3. Who advances a Task when its Review is routed to a Project but no Project
   executor is alive?
4. Which feedback operations are genuinely distinct from `steer`, `interrupt`,
   `close_review`, and ordinary Run wake/stop?
5. What command/API surface should humans and headless controllers share?
6. Which guarantees survive without the desktop app, Wave server, Project
   process, provider continuation, or a clean canonical checkout?

## Feedback routing decision

Unattended feedback routes to immediate parent Work:

```text
Task Review -> Project Work
Project Review -> Wave Work
Wave/subwave Review -> its parent Wave when one exists
```

Human-attended feedback routes to the authenticated User. Automated review does
not impersonate User. After the control model is stable, prompt/context quality
must be evaluated independently: the parent needs enough durable child output,
Work truth, flow intent, workspace/PR/CI evidence, and the exact question to
make a good judgment.

## Feedback concept inventory

The live system currently expresses related ideas in several overlapping
forms:

| Representation | What it currently means | Disposition candidate |
| --- | --- | --- |
| `Skill.interactive` | both provider launch mode and a conversational flow step | split the two meanings |
| `InteractionPolicy::{Require, Defer}` | actually selects User vs parent reviewer | replace with an explicit reviewer/route |
| `TaskPhasePlan.interaction_policy` | persists reviewer choice across legacy Session launches | delete with Session; put reviewer on durable flow position |
| `FlowAction::{WaitInteractive, DeferInteractive}` | duplicates the same route choice in flow execution | collapse to one Review/checkpoint action |
| `FlowPosition.interactive` | says the current step is a feedback interval | replace boolean with optional typed reviewer/route |
| Launch `attention_kind/work` | copies the Review route onto provider lifetime | remove route ownership from Launch |
| Launch `attention_at` | says the reviewer currently owes a response | retain the distinction, but key it by durable Basis/Turn rather than only a timestamp |
| `Review` | derived conversational interval | keep as a projection, not stored aggregate |
| `ChildReview` | parent-facing Review plus rendered child evidence | collapse into the same Review projection/context renderer |
| `ReviewGateState` | obsolete Requested/Active/Approved/ChangesRequested disposition model | delete; production only constructs Active |
| `TaskAction::Review` | present a GitHub PR or resolve a completion gate | separate PR presentation from flow feedback |
| `AfterMerge::Review` | keep Task open after merge | rename as Task continuation intent, not Review disposition |
| Swift `TaskAttention*` / `NowGroup` | scheduling/UI projection across all kinds of work | retain as a projection; do not make it feedback truth |
| `ChildBodyHandoff*` | legacy Session/body process transition | delete with Session controller |

The likely normalized ownership is:

```rust
enum Reviewer {
    User,
    Parent,
}

struct FlowPosition {
    // existing flow coordinates
    reviewer: Option<Reviewer>,
    pending_feedback: Option<Basis>,
}

struct Review { // derived projection
    work: WorkRef,
    position: FlowPosition,
    launch: Option<LaunchId>,
}
```

The exact pending-feedback key remains open. It should identify the durable
boundary/request that awaits a response and supply ordering/idempotency; a bare
timestamp is weak identity. The important ownership rule is already visible:
the feedback stop belongs to Work/flow, while Launch is only its current
transport and presentation surface.

## Current failure seam

A Task Review routed to its Project is stored against stable Work/Run facts,
but waking the parent crosses back into the legacy Session controller:

```text
Task Run routes Review -> Project Work attention
Task event -> observation_outbox -> resolve ProjectSession successor
                              -> launch ProjectSession body (best effort)
                              -> warning only when wake fails
```

This half-migration is not a safe foundation for a feedback-only patch. The
Project can have durable attention without a corresponding shared-runtime wake,
and the warning path intentionally leaves it queued. Completing the Work/Run
execution migration is therefore a likely prerequisite to the feedback/API
slice, not unrelated architecture work.

The candidate boundary is now:

1. finish shared Work/Run reservation, wake, execution, stop, settlement, and
   recovery for Wave, Project, and Task;
2. delete Session/body execution authority and ProjectSession-chain routing;
3. express human and headless feedback as the same durable Review/Steer/close
   protocol over that runtime;
4. collapse `lf task`, `lf project`, and generic `lf work` commands around the
   resulting concepts.

This remains narrower than finishing every open architecture question: Home
migration, provider usage normalization, opaque-TUI handback, historical Epoch
diagnostics, and unrelated UI work are outside unless they block the cutover.
