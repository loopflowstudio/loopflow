# Wait for green CI before Task review

## Problem

A published Task PR currently moves from Iterate into Gate and immediately starts
`task-gate`. Required CI and semantic review therefore run in parallel: a Project
review can begin while the same head is pending or red, and a later CI repair can
invalidate evidence the review already consumed. The mechanical reconciliation
that could correct this runs only when `inspect_outcome` follows a full Project
provider iteration, even though the Project runner already has a five-second Task
supervision tick.

The W2-298 canary exposed the corresponding repair exit failure. Generation 3
pushed head `fdaa9da06`, then failed before parking with `gate flow is task-gate,
but its playhead is ci-fix`. The generic `TurnCompleted` path finishes the
one-step repair playhead and immediately calls `record_task_flow_position`; that
function correctly rejects writing a `ci-fix` cursor into a Session whose durable
lifecycle phase is Gate/`task-gate`. The dedicated `settle_ci_fix_turn` exit sits
later and never gets the chance to park the Task or accept its wake.

Make ownership sequential. CI owns an open PR until the current head has a
head-matched passing required-check observation. Only then may the existing Gate
flow begin review. This removes avoidable Project review churn and advances
Developer Efficiency's proof that agent runs require zero avoidable human repair,
without adding another lifecycle state or wake path.

## The demo

Publish a Task PR whose required check is pending. `lf status` shows CI as the
owner and no `task-gate` interaction review exists. While the Project provider
remains mid-turn, let the existing observation cache record the same head red or
green: the supervision tick enqueues the one durable repair wake for red, or
launches the parked Gate once for green, without waiting for the Project LM
iteration to finish. The red repair pushes a new head, parks back in Gate, and
accepts its wake without changing the durable `task-gate` cursor.

## Approach

Keep the existing lifecycle and implement the sequence at its current boundaries:

1. At the end of Iterate, reconcile the active PR exactly as today, compute the
   proposed terminal status/reason, and call `TaskSession::enter_gate`. This
   persists the existing gate proposal, increments the existing gate cycle, and
   resets the existing Gate cursor.
2. Start `task-gate` inline only when the open PR's `fresh_ci()` reading is
   `CiState::Passing`. Otherwise set the Task to `Waiting`, finish the ordinary
   body, and leave the Session parked in Gate with its proposal intact. Pending,
   failing, stale-head, absent, and degraded readings cannot open review.
3. Move the mechanical per-Task loop out of `inspect_outcome` into one
   `ops::task` helper. It lists the Project's Tasks, adopts safe recovery state,
   reconciles the PR, refuses dirty between-PR state, reconciles process liveness
   and completion, rotates/relaunches settled work where already allowed, and
   handles inactive open PRs. Reuse that helper from both `inspect_outcome` and
   `supervise_project_task_bodies`; retain no second copy in the Project runner.
4. For an inactive Task with an open PR, the shared helper applies the CI cut:
   - a fresh failing reading calls the existing `queue_ci_fix_command`; its
     incident identity, command ensure/link/claim path, and launch semantics stay
     untouched;
   - pending or unknown readings do nothing;
   - a fresh passing reading relaunches only when the Task is already in Gate.
     The Gate cursor must still be at its entry coordinate (`phase_cursor == 0`,
     `phase_iteration == 0`); an advanced or completed Gate is not a CI-waiting
     Gate. The direct relaunch resumes that persisted cursor, so the first
     provider work is `task-gate`, never Iterate.
5. Run the shared helper at the start of the existing Task supervision tick,
   before the tmux-installed/live-session branch used for lease recovery. Feed
   the helper's freshly reconciled Task rows into the existing strand/stall
   sweep, filtering that sweep to the historical Project Session exactly as
   today. This gives PR progress a prompt path even when no lease recovery is
   possible and avoids listing the same Tasks twice on one tick.
6. Keep the current ci-fix exit. A repair is one bounded body and exits before
   Gate advancement. If it pushed a new head, the Task remains Gate/Waiting while
   the new head is pending or unknown. A later call to the shared Project
   reconciliation helper owns the green transition.
7. Correct the generic `TurnCompleted` ordering at its narrow authority cut.
   Continue to call `finish_task_flow_turn` for the real one-step `ci-fix`
   playhead. Continue to re-read the Session, synchronize terminal state, and
   persist that reconciled Session. Guard only the durable cursor write:
   `if ci_fix_wake.is_none() { record_task_flow_position(&mut session, &flow)?; }`.
   Whenever `ci_fix_wake` is armed, the repair playhead is intentionally
   out-of-band and owns no durable Task lifecycle cursor. The later existing
   branch then reconciles the pushed PR and calls `settle_ci_fix_turn`, which
   parks the body before accepting/failing/superseding the exact wake it
   serviced. That settlement must leave `lifecycle_phase`, `phase_cursor`,
   `phase_iteration`, `gate_cycle`, and the complete `gate_proposal` unchanged.
8. Exercise that ordering through the real runner, not by calling its exit
   helper. Split only the environment-owned construction from the existing body
   implementation: `run_task_session_inner` opens the store and supplies
   `Box::new(default_create_harness)` to a private `run_task_session_with(store,
   session_id, lease, create_harness)` core. Use the same boxed `Fn` shape already
   used by `flowloop::wave::CreateBodyHarness`; add no factory trait, public API,
   or production branch. The regression supplies a scripted `Harness` that
   receives the runner's real event sender.

Add one non-persisted `TaskPr` predicate for the shared authority cut: an open PR
is review-ready only when `fresh_ci()` is passing. Use that predicate in both the
Task runner and shared Project reconciliation helper. It is a predicate over the
existing `CiObservation`, not a new CI classification.

Make the existing legal-action model match the lifecycle boundary: an open PR
with pending or unknown current-head CI recommends `NoAction`; only passing CI
recommends `Review`; failing CI keeps the existing repair recommendation. Then
derive the Wave snapshot's next-move owner and reason from that action model
instead of independently matching every `CiState` in `next_move_for_task`.
`TaskAction`, `NextMoveOwner`, and all DTO shapes remain unchanged.

The relaunch is exactly once without another durable marker. Project observation
checks that the Task is inactive and still at the Gate entry cursor, and
`reserve_task_process` atomically turns the first launch into a live generation.
A concurrent observation sees the active reservation and cannot create a second
generation. After Gate advances, its existing cursor prevents a later green poll
from treating the completed review flow as a CI wait. While the first interactive
step is open, the existing interaction-review identity at that phase coordinate
continues to own review deduplication.

The existing `TASK_SUPERVISION_INTERVAL` remains five seconds. The shared helper
uses the current `reconcile_task_pr` cache contract: successful GitHub reads are
reused for 60 seconds and degraded reads back off for five minutes. The tick can
act immediately on a failing or passing `CiObservation` already persisted by a
Task boundary; external state changes become actionable on the next existing
cache refresh. No interval, timeout, refresh mode, or background process is
added.

Do not make `record_task_flow_position` accept `ci-fix`, rewrite
`lifecycle_phase`, substitute a `task-gate` playhead, or add a compatibility
flow. The mismatch is valid evidence that the durable lifecycle and repair flow
have different owners; the fix is to avoid persisting the out-of-band cursor.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Can Gate be parked before its flow starts? | `TaskSession::enter_gate` already persists the phase, epoch, zeroed cursor/iteration, incremented gate cycle, and complete `TaskGateProposal`. `resume_task_phase` selects `phase_plan()` from that persisted phase. | Park in Gate; add no wait phase, flag, or proposal copy. |
| What makes a CI reading authoritative for the reviewed code? | `TaskPr::fresh_ci()` returns a reading only when its `head_sha` equals the PR's current recorded head. Reconciliation refreshes the PR and required checks together. | Gate readiness uses `fresh_ci()` exclusively; never read raw `ci_observation`. |
| Can “no required checks” be distinguished from a failed GitHub read today? | `merge_gate_state` returns `None` for both an empty required-check set and unreadable/absent check output. The directive requires unknown CI to park and forbids a new classification. | `None` is conservatively not review-ready. Repositories that want automatic Task review must configure a required check; changing that contract is out of scope. |
| Does failing CI need another wake? | `queue_ci_fix_command` already derives `ChildCommandKind::CiFix` from the current `CiIncident`, ensures it by incident identity, links the command before launch, and arms only a matching current failure. The existing focused lifecycle proof passes. | Project observation keeps calling the same enqueue function. No command kind, marker, poller, or direct Task wake is added. |
| Does a ci-fix accidentally advance Gate? | `settle_ci_fix_turn` explicitly exits before Gate, successor steps, or PR rotation. A pushed head makes `decide_open_pr_status` return `Waiting`; the command is settled independently. The generic completion path currently writes the repair playhead before that exit, so the path cannot remain structurally unchanged. | Preserve `finish_task_flow_turn` and Session reconciliation, bypass only `record_task_flow_position` whenever `ci_fix_wake` is armed, then let `settle_ci_fix_turn` park and settle the command without changing the Gate cursor, iteration, cycle, or proposal. |
| Why did W2-298 fail before that exit? | The generic `TurnCompleted` block calls `finish_task_flow_turn`, synchronizes the Session, and records the active playhead before the later `ci_fix_wake` branch. `record_task_flow_position` requires the root flow to equal the durable phase flow, so `ci-fix` versus Gate/`task-gate` fails by design. | Keep turn completion and Session reconciliation, but condition only the durable cursor write on `ci_fix_wake.is_none()`. Then reach the existing repair settlement. |
| Does the existing repair test guard this ordering? | `a_repaired_head_accepts_the_wake_and_parks_without_a_gate` passes, but it calls `settle_ci_fix_turn` directly from an Iterate-shaped fixture. It never carries a `ci-fix` playhead through generic `TurnCompleted` while the Session is in Gate. | Drive the private real-runner core with a persisted Gate Session, a `CiFix` command claimed by the predecessor generation and reclaimed by this generation, and an injected scripted harness. A direct `settle_ci_fix_turn` test is supporting coverage only. |
| What is the executable runner seam? | `task/runner.rs` currently opens the store and hardcodes `default_create_harness` inside one large function. `flowloop/wave.rs` already separates that same construction behind `CreateBodyHarness = Box<dyn Fn(&str, ApprovalPolicy, UnboundedSender<ConversationEvent>) -> Result<Box<dyn Harness>> + Send>` while production supplies `default_create_harness` and tests supply scripted harnesses. | Apply that existing closure shape to a private `run_task_session_with(SharedStore, TaskSessionId, &ChildWriteLease, CreateTaskHarness)` core. Production behavior is unchanged; the in-crate test can use its temp store and emit real conversation events. |
| Can repeated green observations start duplicate review bodies? | `launch_task_process` reserves a generation atomically and treats an already-active reservation as success without minting another. Gate cursor and interaction-review persistence already protect resumed review work. | Use the normal direct relaunch after a strict Gate + passing predicate; add no “green handled” field. |
| Can green relaunch a Gate again after review has finished? | `enter_gate` starts at cursor/iteration 0/0; every completed Gate step records the advanced playhead through `record_task_flow_position`. The three-step `task-gate` flow (`demo`, `code-review`, `gate`) therefore carries its own progress marker. | Green relaunch is legal only at the Gate entry coordinate. An advanced/completed Gate keeps its existing outcome and is never restarted by CI. |
| Will a green relaunch bypass the open-PR supervisor bar too broadly? | The ordinary supervisor bar intentionally forbids every open PR because of W2-129. Project observation currently has a narrower internal direct-relaunch path for lifecycle-owned work. | Keep the general bar unchanged. Permit the direct path only after `phase == Gate`, inactive body, open PR, and fresh passing CI are all proven together. |
| Does this change the CI autonomy metric? | Incident creation, trigger attribution, `responded_at`, human-assistance query, and green/merged stamps all live below the Gate transition and are independent of review launch. | Do not touch incident schema, command attribution, or the strict human-assistance window. Existing metric tests remain byte-for-byte authoritative. |
| Is another poller needed to make observation prompt? | The Project runner already calls `supervise_project_task_bodies` on `TASK_SUPERVISION_INTERVAL` every five seconds while its provider turn is active. | Put reconciliation on that tick. Add no timer, process, task, or webhook. |
| Will a five-second tick hammer GitHub? | `reconcile_task_pr` reuses successful observations for 60 seconds and degraded observations for five minutes. A just-parked Task already carries the reading that decided its wait. | Preserve the cache unchanged. The tick mostly reads durable evidence; it does not force-refresh GitHub. |
| Does the current tmux guard suppress PR observation? | `supervise_project_task_bodies` returns immediately when tmux is unavailable because it currently owns only lease recovery. PR reconciliation itself needs no tmux until an eligible command or Gate relaunch starts a body. | Call the shared mechanical helper before the tmux guard; keep the guard around only strand/stall observation. |
| Which Tasks belong to the shared pass? | `inspect_outcome` matches the Linear Project id so Tasks born under a terminal predecessor route to the live successor; the lease tick currently narrows to the historical `project_session_id`. | Shared reconciliation preserves Linear Project-id routing. The subsequent lease sweep retains its exact historical-session filter. |
| Can the tick act on stale in-memory Tasks after a command launches? | `queue_ci_fix_command` launches through a cloned `ChildSession`, while direct Gate relaunch mutates the caller's clone. Reusing the pre-action vector for lease recovery would represent those paths differently. | Return or re-read post-reconciliation Task rows before the lease sweep and before `inspect_outcome` fingerprints them. |
| What currently proves the foundations? | The focused action-model, ci-fix restart-bar, and end-to-end single-wake lifecycle tests all pass on the branch. | Extend those fixtures around the new boundary instead of replacing the incident tests. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep the Task body alive and poll CI | Simple local control flow, but burns a provider/process generation, makes Project observation secondary, and needs timeout/recovery behavior. | Violates the parked-body model and duplicates the existing Project poll owner. |
| Wait for `inspect_outcome` after each Project LM iteration | Reuses today's code with no extraction. | A long Project turn leaves red or green evidence unactioned even though the parent process is alive and already ticking. |
| Add a dedicated CI timer beside Task supervision | Makes CI cadence independently tunable. | Adds a second scheduler and concurrent owner. The existing five-second tick is already the correct parent-owned execution surface. |
| Add a `WaitForCi` lifecycle phase or persisted boolean | Makes the wait explicit in storage. | Adds state, migrations, DTO drift, transition edges, and compatibility work for a condition already represented by Gate + PR CI. |
| Mint a second “resume review” durable command | Gives green a receipt. | Creates a second wake mechanism beside `CiFix`; dedup and attribution would have two ledgers for one PR. Atomic process reservation already supplies exactly-once launch. |
| Start `task-gate` but defer only its interactive review step | Avoids changing the phase transition. | The Gate provider still runs against unverified code, and non-interactive Gate steps could consume or mutate evidence before CI is green. |
| Teach Gate to accept or translate a `ci-fix` cursor | Makes the current persistence call succeed. | Conflates the out-of-band repair playhead with durable lifecycle progress and could skip or corrupt a later Gate review step. |

## Key decisions

- **Gate is the parking coordinate.** Enter it before waiting so the proposal and
  lifecycle intent survive body exit and a later relaunch resumes the right flow.
- **Passing means current-head passing, not cached green.** A moved head returns
  to unknown until reconciliation observes that exact head.
- **Unknown fails closed.** The current read surface cannot prove required checks
  passed, so review does not begin.
- **Project owns external progress.** The Task runner may continue inline when it
  already sees green, but once parked only Project observation may relaunch it.
  Both Project entry points call one mechanical implementation.
- **The supervision tick is the latency path, not a new owner.** It invokes the
  same helper as `inspect_outcome`; it does not reimplement CI decisions or mint
  a different wake.
- **The Gate cursor is the exactly-once marker already in the model.** Zero/zero
  means Gate has not begun; any advanced coordinate means CI no longer owns its
  launch. Status-reason strings never participate in control flow.
- **Review semantics begin after the cut and remain unchanged after it.** Existing
  reviewer policy, interaction-review creation, gate fingerprint, dispositions,
  changes-requested return to Iterate, and proposal approval all stay intact.
- **No automatic retry after a no-op repair.** The existing bounded incident
  identity remains spent according to the current ci-fix verdict. This task
  sequences review after CI; it does not loosen the one-repair bound.
- **A repair playhead is executable state, not lifecycle state.** Finish it and
  reconcile the Session, but never copy its cursor into Gate. The durable
  task-gate cursor and proposal remain byte-for-byte unchanged across repair.

Wild success is boring: red and pending heads disappear from the Project review
queue, a repaired head quietly waits, and the first review always evaluates a
green commit. Wild failure would be a hidden second owner—an ordinary resume,
status recommendation, or stale green read starting Gate early. The shared
current-head predicate plus event-sequence tests are aimed directly at that
failure.

## Scope

- In scope: the Iterate-to-Gate boundary in `task/runner.rs`; the inactive
  open-PR branch currently in `project_session/runner.rs`; extraction of that
  complete mechanical loop into `ops::task`; reuse by `inspect_outcome` and the
  existing Task supervision tick; a shared current-head passing predicate on
  `TaskPr`; the narrow `ci_fix_wake` cursor-persistence guard in
  `task/runner.rs`; open-PR action/next-owner status consolidation; focused
  lifecycle and status tests.
- In scope for the canary regression: a private store-plus-harness runner seam
  matching the existing Wave runner's boxed harness creator; no exported test
  surface and no alternate lifecycle implementation.
- Out of scope: migrations; new Task/PR fields; DTO changes; new CI states;
  timers, pollers, webhooks, refresh modes, or polling cadence; required-check
  configuration; incident identity or autonomy accounting; review
  policy/disposition behavior; PR publication, rotation, merge queue, or
  completion behavior.
- Out of scope per the infrastructure Wave: a generic CI orchestration platform
  or multi-product deploy abstraction.

## Done when

- A pending current-head observation leaves the Session inactive in Gate,
  preserves the gate proposal, creates no interaction review, and reports CI as
  next owner.
- An unknown or stale-head observation behaves the same way.
- A failing current-head observation parks the ordinary flow and produces only
  the existing incident-keyed `CiFix` command; repeated Project observations
  still produce one command.
- While the Project provider is still mid-turn, one invocation of the existing
  supervision tick reconciles a parked red Task and persists/launches its existing
  incident-keyed `CiFix` command. The proof does not call `inspect_outcome`.
- A ci-fix push settles its command but leaves the Task inactive in Gate while
  the new head is pending or unknown; no review starts.
- An in-crate regression calls the private real-runner core, not
  `settle_ci_fix_turn`. Its store begins with a persisted Gate Session carrying
  sentinel `phase_cursor`, `phase_iteration`, gate cycle, and gate proposal, plus
  a durable `CiFix` command still `Claimed` by a dead predecessor generation.
  The new generation reclaims and arms that command through the production
  claim path. Its injected scripted harness receives the real ci-fix seed,
  changes fake GitHub from the failing head to a pending successor head, expires
  the cached PR observation, and emits real `TurnStarted`/`TurnCompleted` events.
  The runner completes `finish_task_flow_turn`, performs Session reconciliation,
  bypasses only `record_task_flow_position`, reaches `settle_ci_fix_turn`, parks
  the process, and accepts the exact wake. The Task remains in Gate and every
  sentinel cursor/proposal field is byte-for-byte unchanged.
- The W2-298 failure is pinned directly: sabotaging the guard so
  `record_task_flow_position` runs for `ci_fix_wake` reproduces `gate flow is
  task-gate, but its playhead is ci-fix` before settlement and leaves the wake
  `Claimed`. The real-runner regression fails under that sabotage; the existing
  direct settlement helper test is not accepted as this proof.
- A fresh passing observation starts or relaunches Gate. Repeating the Project
  observation creates one process generation and one `task-gate` review, not
  two.
- A later fresh passing observation after Gate has advanced or completed creates
  no generation and reopens no review.
- `inspect_outcome` and `supervise_project_task_bodies` both consume the same
  extracted reconciliation helper; the Project runner contains no second
  per-Task PR/liveness/completion loop.
- Gate proposal, gate fingerprint behavior, review dispositions, strict
  autonomous incident reporting, and existing ci-fix lifecycle tests remain
  green.
- The action model and Wave next-move surface agree for unknown, pending,
  failing, active-repair, passing, and active-review states; the standalone
  duplicated `CiState` status match is removed.
- `cargo fmt --check` passes.
- `cargo clippy -p loopflow --lib --tests -- -D warnings` passes.
- Focused Task runner, Project observer, action model, and ci-fix lifecycle tests
  pass, followed by `cargo test -p loopflow --lib`.

This directly advances Developer Efficiency's “avoidable human-in-the-loop setup
or repair steps fall to zero” KR and protects “no Task strands on a dead body”:
waiting CI owns no Task process, while green relaunch uses the existing atomic
generation reservation rather than a manual resume.

## Measure

Use the lifecycle event sequence as the measure, without adding telemetry:

- Before: `Iterate completed -> Gate running -> review requested`, regardless of
  CI.
- After: `Iterate completed -> Gate waiting -> supervision tick observes CI ->
  one repair or Gate generation -> review requested after passing`.

For a failed head, the existing incident report must retain the same identity,
one trigger command, one response stamp, and unchanged `human_assisted`
classification. A green head with two consecutive Project observations must show
one new Task generation and one review request.

For the repair exit, the measured sequence is `Gate cursor N -> ci-fix turn
pushes -> ci-fix playhead finishes -> Session terminal state syncs -> durable Gate
cursor remains N -> Task parks -> wake Accepted`. No Task lifecycle-position
event or cursor write may appear between repair completion and settlement.
