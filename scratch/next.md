# Next: design the Work server topology

## Status

The six independent Feedback-runtime slices are implemented and reviewed. Do
not extend their APIs or begin a server implementation from this file. The next
pass is a design investigation: choose process ownership, then carve the first
implementation slice.

The design must answer the question the current code cannot: what exactly does
a Home, Wave, Project, or Task server do, and who wakes a stopped Project when
its Task asks the parent for Feedback?

## Start from current truth

Trace these implementations before proposing a host:

- `rust/loopflow/src/work_runner.rs`
- `rust/loopflow/src/project/runner.rs`
- `rust/loopflow/src/task/`
- `rust/loopflow/src/ops/project.rs`
- `rust/loopflow/src/store/children.rs`
- `rust/loopflow/src/store/durable.rs`
- `rust/loopflow/src/flowloop/wave.rs`
- `rust/loopflow/src/wave/`
- the Home resident and `lfd`

In particular, trace one parent-routed checkpoint through both halves that are
currently disconnected:

1. opening and re-arming Feedback through `set_flow_position`,
   `route_feedback`, and `rearm_feedback_attention`;
2. Project-observable Task events through the child outbox, best-effort
   `wake_project`, clean-checkout validation, Run reservation,
   `lf __work project`, and `child_attention`.

The Feedback route does not currently call `wake_project`. A stopped Project
therefore receives no nudge at all merely because its child opened Feedback.
Name every point where process exit, app exit, a dirty checkout, absent or lost
nudge, stale lease, or remote Home can strand durable input.

## Decisions to make

1. Choose one Home-owned mechanism that derives useful Ready Work from durable
   state: a Home-wide scan or lightweight per-Work actors. A Task callback or
   CLI process may nudge it but cannot be correctness-critical.
2. Draw which processes are long-lived and which are replaceable: Home
   resident, Wave listener, Wave cadence, Work executor, Run, Launch, and
   provider process.
3. Decide whether open Feedback retains an active Run/Launch or ends the Run in
   a typed Wait. Do not add Blocked, approval, or reviewer-session state.
4. Use one one-hop control protocol for Wave-to-Project and Project-to-Task.
   Keep direct User review explicit and Wave-to-human communication on the
   human Chat/Steer surface.
5. Make CLI, Mac, and unattended parent review call the same durable controls;
   they may differ only in whether User or parent Run authority supplies input.
6. Identify the shared Run reservation, Launch supervision, provider recovery,
   Steer delivery, interrupt, Wait settlement, and liveness code. Work kinds
   should contribute domain prompt, flow, evidence, and closure policy only.
7. Name the exact current files, commands, DTOs, fields, and loops that the new
   ownership model deletes or shrinks. Do not retain compatibility launch paths.
8. Assign one owner to open-PR settlement. `--match-head-commit` fences only
   the merge/auto-merge request it accompanies; do not assume GitHub permanently
   pins auto-merge across a later maintainer push. Either make stale-head
   observation timely and authoritative or replace remote auto-merge with an
   exact-head merge after requirements clear.
9. Reconcile a merged Complete request from its stored head, disposition, and
   GitHub merge fact even when the Task executor and worktree are gone. A local
   workspace may add safety evidence when it exists; its survival cannot be a
   prerequisite for observing an already-settled remote fact.

## Design done when

- [ ] Home, Wave, Project, Task, Run, Launch, and provider ownership fit in one
      process diagram with exactly one owner for dispatch, liveness, retry,
      streaming, and remote nudge.
- [ ] A transaction/race description proves useful Ready input reserves exactly
      one Run whether input wins before or after the prior Run ends.
- [ ] A stopped Project answers immediate-child Feedback through the same path
      as a live Project; direct User review remains available.
- [ ] Ad hoc CLI launch, Mac app exit, parent process exit, listener restart,
      dirty canonical checkout, and failed best-effort nudge cannot lose input
      or strand Ready Work.
- [ ] Status names the exact durable Wait or Ready fact rather than reporting a
      generic blocked lifecycle.
- [ ] The design decides Run-versus-Wait behavior for open Feedback without
      making a presentation process or provider continuation the source of
      truth.
- [ ] Wave-to-Project and Project-to-Task use the same one-hop API and authority
      rule; Wave-to-human uses Chat/Steer rather than Feedback escalation.
- [ ] CLI, Mac, and unattended review are shown as clients of the same controls.
- [ ] The design closes or explicitly constrains the later-maintainer-push race
      between a head-pinned Auto request and stale-head observation.
- [ ] A merged Complete request settles from durable store/GitHub evidence after
      the Task worktree is removed; recovery does not skip remote reconciliation
      merely because local execution state is gone.
- [ ] The deletion ledger covers the replaced Wave listener/resident and
      Project/Task start, wake, handoff, resume, attach, interrupt, and recovery
      paths down to files and stored fields.
- [ ] The implementation plan specifies a deterministic behavioral proof that
      starts a Task from an ad hoc CLI, stops its parent, opens parent-routed
      Feedback, and observes the owning Home wake exactly one Project that can
      Steer or continue it.
- [ ] The implementation plan is sliced so the first change deletes a coherent
      old path and proves behavior; no temporary second lifecycle is required.

## Boundary

The server design may revise the topology sections in `docs/architecture.md`
and the deferred list in `scratch/feedback-runtime.md`. It must preserve the
completed contraction: explicit Feedback continuation, User-or-parent reviewer,
file-only Wave memory, no Radio, no ambient Wave chat context, no implicit PR
Review state, and no false Session identity.
