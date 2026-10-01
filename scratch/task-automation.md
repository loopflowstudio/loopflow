# Keep selected Tasks progressing through verified landing

2026-09-30 · LOO-332 · Product · Kickoff recommendation for Jack Heart

**Status:** Jack accepted the outcome: selected Tasks keep progressing without
Desktop or the initiating command, preserving intended steps and reviews. Jack
reopened the ownership choice on September 30: TaskSession, cron, or primary
Wave Session. The combination recommended below is a proposal, not a recorded
approval. This pass changes only scratch documentation.

The existing implementation is retained. Main `6c7335607` is already integrated
through `f841e14c4` and `136e1de63`; inspection baseline is `b797856f3`.
LOO-298 and LOO-358 supply the landed Session/Flow/Exec and checkout-association
model. This replaces the earlier plan's active-stack assumption. The source
conversation is [discord.md](discord.md); prior implementation receipts and
integration proof are preserved in [task-automation-evidence.md](task-automation-evidence.md).

## Recommendation and experienced outcome

**Use a finite repository cron as the wakeup floor, the TaskSession as the Task's
ongoing conversation and immediate operating path, and the Wave Session for
broader recovery and direction.** All call the same Rust reconciliation and
admission operations. FlowSession remains the sole execution cursor. Neither a
conversation nor the clock becomes a second authority over it.

Jack selects work once. Progress continues, CI repairs appear with that Task,
and a required review stays available without another nudge or stolen focus.
Reopening Desktop shows when the repository was last checked, what is running,
and the exact reason something waits. An idle minute costs no agent turn.
A stopped conversation cannot prevent mechanical merge settlement.

LOO-332 supplies reliable wakeup, admission, finite delivery and Desktop controls.
LOO-353 Unit 2 supplies primary conversations and safe automatic input delivery.
The finite operation works before Unit 2 lands; Unit 2 calls it when available.
Do not recreate primary Sessions inside LOO-332 or make their unfinished input
transport a prerequisite for repairing CI. Normal Flow progression continues
without a TaskSession turn between steps.

## Demo

Enable automation for a repository and select a Task with its intended Flow.
Close Desktop and the initiating terminal. A second selected Task waiting for
Jack's review stays parked; unselected backlog stays untouched. Kill the first
Task's driver after a recorded launch in a controlled demonstration. A later
check recovers the same captured Flow without repeating a completed step or
replacing a still-live provider. Its `lf pr land -c` returns; a subsequent check
starts one CI repair, then observes merge and finishes local/Linear closure.

Reopen Desktop: the Task's history explains the interruption, repair and landing;
the other Task still offers the same review. With Unit 2 available, the same
TaskSession explains the history and the Wave Session can investigate a blocker.
Those conversations improve the experience; their absence never erases progress.
Configured installation and destructive failure injection remain explicit demo
actions. Headless gate proves the mechanics in disposable Homes.

## Ownership alternatives

| Question | (a) Task's own Session | (b) Finite repository cron | (c) Wave's primary Session |
| --- | --- | --- | --- |
| Who notices? | TaskSession reads its Flow, Session outcomes and PR evidence on a turn. It needs a wake for changes after that turn. | A mechanical tick reads eligible Tasks, pending landing intents, current GitHub evidence and unfinished settlement. | Wave Session receives an operational observation and reads the affected Task's original evidence. |
| How quickly? | During an active turn, at the next observation. Idle detection has no bound until Unit 2's wake transport works. A conversation open in a pane is not a running observer. | Proposed every minute: ordinarily the next tick plus bounded observation/admission time. No hard wall-clock promise while asleep, offline or overloaded. | After observation delivery and any current conversation turn. No fixed bound; broad reasoning and Jack's conversation can delay it. |
| Owner absent? | History survives, but nothing executes. Another trigger must resume it; otherwise Jack must return. | launchd starts another finite process independently of Desktop and provider Sessions. Host logout/offline and disabled jobs remain explicit limits. | Pending observations survive if recorded, but no recovery executes without delivery/reconnection. Unit 2 explicitly leaves Desktop-absent operation outside its current mechanism. |
| Authority and claims? | AgentSession owns conversation/turns. It requests existing Task controls; Flow claim, Session driver generation and PR generation still fence effects. | Scheduler supplies no work authority. Admission must atomically check selection, checkout activity and reviews, then reserve the exact Flow or repair. | Wave association grants no control over Task processes. It must use the same Task admission and exact Flow/PR controls as the other callers. |
| LOO-361 deaths? | Replacing a watcher with an agent that waits on it retains the lost-owner failure. A dead TaskSession cannot restart itself. Native recovery may preserve its provider, but needs a caller. | Next tick can recover after the initiating process dies; failure to start remains recorded and retryable. It cannot cure the underlying process-kill cause, nor assume a dead driver means a dead provider. | Can diagnose a common cause across Tasks if awake. If its own process dies, multiple Tasks lose the recovery path until another trigger wakes it. |
| What Jack experiences | One place to discuss intent, current work and Flow changes. Excellent continuity; alone, progress can silently wait for Jack. | Predictable background progress and last-check evidence with no conversation required. Some polling delay; uncertainty must be visible. | One place to discuss priorities and systemic blockers. Useful judgment, but sole ownership makes Task progress depend on a busy broader conversation. |
| Decision | Keep as the immediate path and conversational owner, using Unit 2. Insufficient alone for unattended progress. | Recommended reliability floor; implement here. | Keep accepted automatic blocker handling in Unit 2. Do not make it the sole delivery clock. |

A GitHub webhook could shorten CI detection, but supplies neither interrupted
pre-PR Task recovery nor missing expected-check deadlines, and needs a reachable
receiver. It does not replace the floor. A separate OS job per Task multiplies
installation/lifecycle bookkeeping without improving the claim boundary. One
repository job also discovers taskless authorized landings. A resident Wave or
watcher is excluded by Jack's direction.

## Findings that change the implementation plan

1. **Primary Sessions are product direction, not an available wake service.**
   Read-only source: [LOO-353 design at 38b544413](https://github.com/loopflowstudio/loopflow/blob/38b544413a612c58d297ab1010412e6492ad3a0b/scratch/growth-thoughts.md#unit-2--primary-task-wave-and-repo-sessions).
   Unit 2 gives the TaskSession Flow operation and Jack's two switch timings;
   Wave Sessions autonomously handle operational blockers. It explicitly requires
   a native-provider input/receipt spike and leaves Desktop-absent operation
   unresolved. Some mechanism examples still use pre-cutover Run vocabulary;
   use AgentSession and FlowSession, not those obsolete example shapes.
   Local `lf task status LOO-353 --json` and `LOO-361 --json` both returned
   `no Task exists` in their selected context. This is not global absence. The
   LOO-361 death frequency and eight incidents/five autonomous repairs are Jack's
   supplied context, not newly measured evidence or a diagnosed cause.

2. **This branch and main have different delivery lifecycles.** At pinned main
   `6c7335607`, `ops/pr_landing.rs::watch_armed_pr` waits and repairs. This branch
   has already replaced that loop with finite reconciliation and exact
   `flow_events` landing links. Preserve the working cut, including merge-queue
   precedence and release's separate lifecycle. LOO-338's watcher remains the
   deployed bridge until the complete replacement is proven; a branch-local
   deletion is not installed reliability.

3. **Current cron cannot express the proposed operation.**
   `ops/cron.rs::daily_time_of` accepts only a fixed daily hour/minute;
   `CronSpec` identifies Wave plus Flow/Skill; `spawn_cron_target` launches and
   waits for that target. Extend this existing owner to a typed repository
   operation and minute schedule. Do not schedule an LLM Flow every minute or
   invent a synthetic Wave to fit the old shape.

4. **The landed Task claim is narrower than automation admission.**
   `store/sqlite/flows.rs::claim_task_worker_in` fences the managed Flow's
   version/generation and rejects its human position; eligibility checks ready
   Work, abandonment and chapter eligibility. It does not serialize against all
   independent checkout Sessions or a CI repair. `store/sqlite/task_work.rs`
   provides the shared membership set; `ops/task/lifecycle.rs` already derives
   associated execution/completion blockers. Reuse those facts, but do not copy
   completion's exemptions into launch admission. In particular, a live managed
   worker is not exempt from competing repair exclusion.
   The existing `task_work::TaskSession` is a membership DTO, not Unit 2's
   primary-conversation role; do not give it a second meaning or table.

5. **Finite observation is not yet finite admission.**
   `ops/pr_landing.rs::reconcile_claimed` still waits for `exec_ci_fix` under
   `run_driver_operation`, with a landing heartbeat. A slow repair therefore
   occupies the whole repository check. `record_ci_response` records response
   before launch: process death in that gap can leave unchanged CI permanently
   classified as already repaired. Replace this with a durable reference to the
   reserved repair Session/input and reconcile its actual outcome. A dead launch
   and a completed-but-blocked repair require different next actions.

6. **CI evidence exists but actionable cases are missing.**
   `classify_github_observation` detects required integration, yet maps it to
   `Degraded`; the action table never launches repair. Queue membership correctly
   precedes ordinary PR-head integration/check interpretation. Preserve that
   ordering. Pending/absent checks have no durable deadline/rerun identity yet.
   `settle_task_landing` preserves merged evidence and reports Linear writeback
   pending, but interrupted-settlement proof is still required.

7. **A launchd schedule outlives Desktop, not the host session.** Calendar jobs
   missed during sleep run on wake; powered-off time is not execution. Per-user
   agents run while their user is logged in. Select `StartCalendarInterval`
   minute entries (all 60 minutes for the default), retaining daily schedules;
   no `KeepAlive` observer. [Apple timed-job semantics](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/ScheduledJobs.html),
   [user-agent lifetime](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html).
   launchd can kill remaining members of a finished job's process group. Reuse
   the existing detached Task-worker launch path for admitted work and prove
   survival after the tick exits, including when no detached host is running
   beforehand. Merely calling `spawn` is insufficient. This is a failure case
   to test, not an attribution of LOO-361's cause.
   [Apple process-group contract](https://github.com/apple-oss-distributions/launchd/blob/main/man/launchd.plist.5).

## Chosen architecture

```text
launchd tick ───────────────┐
TaskSession immediate call ├─> shared finite reconcile + admission
Wave Session recovery call ┘       │
                                  ├─> captured FlowSession + exact worker claim
                                  ├─> reserved ci-fix AgentSession/input
                                  └─> mechanical landing/Task/Linear settlement
Desktop reads the same outcomes; no scheduling policy in Swift.
```

### Persisted owners and authorization

- **Task:** checkout, selected managed Flow, explicit automation selection/hold.
  Add the selection/hold fact to Task's existing owner; `started_at` cannot mean
  automatic enrollment because independent work also sets it. No work starts
  from backlog, repository discovery or PR discovery alone. An authorized pending
  landing remains eligible for delivery even without Task automation selection.
- **FlowSession:** captured definition, cursor, return counts, exact successful
  completions, pending review and worker claim. Resume it unchanged. With no
  selection, task-operate resolves an explicit choice or the Project's default
  once. A finished Flow does not automatically start another because a PR is open.
- **AgentSession / Exec:** provider history, captured input and process/driver
  identity. Repair is an ordinary Task-associated Session, never a replacement
  feature Flow. Recovery keeps native history and distinguishes surviving engine,
  dead driver, failed start and completed outcome.
- **Landing and CI incident:** authorized disposition (`-c`, bare, `--next`),
  PR/head, merge request/queue evidence, generation, merge fact, CI attempt timing
  and repair reference. Add deadline/rerun identity here, not in cron receipts.
  GitHub remains authoritative for remote checks and merge; Linear for remote
  Task completion. `pm_writeback` retains unfinished publication of local closure.
- **Existing cron owner:** canonical repository, placed Home, executable, cadence,
  enabled declaration and latest execution receipt. Generalize its existing spec,
  installation and receipts; do not create another job registry or Task cursor.
  A schedule is permission to check already-selected work, never permission to
  merge arbitrary PRs or broaden Task scope.

Proposed CLI surface: `lf task reconcile --json` performs one repository pass;
`lf task automate ISSUE on|off` records selection/hold; `lf cron sync --repo`
installs the repository declaration. These names are new proposals, not current
commands. Retain `lf pr reconcile` as the delivery-only entry into the same
implementation, and keep `land`/`arm` sharing preparation. Session callers may
request a specific Task through that shared operation without rescanning every
Task. No agent judgment is needed for healthy continuation or proven merge.

### Admission and handoff

Use a short per-checkout admission lock across re-reading shared membership,
review/hold state and reserving a launch in the existing store. All relevant
Session, Ask/review and Flow reservation writers must participate; locking only
the scheduler leaves the review-opening race intact. The lock is not held
through network calls or an agent turn. It serializes reservations, not arbitrary
intentional independent work. Automatic admission sees every existing live or
unresolved checkout participant and waits; explicit launches retain their own
existing semantics and become visible to the next automated admission.

Under that boundary, reserve the exact Flow claim or ordinary repair Session and
link the incident before launching. The durable reservation keeps overlapping
cron/TaskSession/Wave calls from admitting another writer after the lock is
released. Child startup hands authority from recorded launcher to worker through
existing claims/generations. Failure before acknowledgement remains a recoverable
reservation, not success and not a second queue. Observe exact Exec identity and
provider state before reclaim; a lease timeout or exited parent alone is not
proof the work stopped. Unknown stays unresolved and visible.

TaskSession handing off its own work must not block itself. Exempt only the exact
validated initiating Session/input and mechanical admission Exec for that transfer;
never exempt all Sessions in its checkout, all descendants or an entire Wave.
An idle primary conversation must not permanently block its Flow merely by
existing. Active unrelated turns and unresolved Ask/review boundaries do block
an automatic launch. Unit 2 must reconcile that distinction using the same
reservation path, including queued Flow switches, without inventing control from
membership. Fresh required review wins if published before admission; later
review publication sees the reservation rather than racing an unrecorded launch.

Use a nonblocking repository tick lock only to avoid duplicate observation cost;
per-Task/PR claims still enforce effects for direct callers. No network transaction
or SQLite lock spans the whole repository pass. Inspect all associated work before
cleanup/closure too; automation must not delete independent work after merge.

### One bounded check and recovery

A pass enumerates selected Tasks and retained nonterminal landing/closure intents,
including locally completed Tasks with pending Linear settlement. It also scans
known open Task PRs for evidence; an unarmed/unselected PR is observation only.
Scanning only GitHub's open PR list would lose merged-but-unsettled Tasks and
interrupted work with no PR. Taskless authorized landings use their PR claim and
checkout exclusion, never Task closure.

1. Read selection, hold, captured Flow and associated activity on the placed Home.
2. Refresh relevant external evidence with bounded calls. Re-read PR/head and
   merge intent before an action; do not act on a stale cached failure.
3. Settle proven mechanical effects; otherwise reserve one permitted launch or
   return a concrete wait/blocked/unknown reason. Continue to other Tasks after a
   per-Task error. Two launch sources must converge on the same reservation.
4. Return after detached startup acknowledgement or a bounded startup failure.
   Do not wait for CI, a provider turn, a human Session or merge.

Proposed budgets: 60-second cadence, 45-second pass deadline, at most 10 seconds
per external operation/start acknowledgement (shortened by remaining budget).
Order overdue work by oldest recorded observation so a slow Task cannot starve
later ones; retain deferred work for the next pass. Acceptance target: for ten
eligible Tasks on a healthy awake Home, notice/admit a failure within two ticks
(120 seconds), not including provider startup/completion. Report overdue coverage
instead of claiming the target when workload or dependencies exceed it. Idle
checks launch zero providers. Cadence/budgets are engineering targets to measure,
not existing performance facts.

After a driver death, recover the same recorded occurrence once its exact old
execution is proven dead or its surviving provider is safely reattached. Preserve
completed outputs and consume them once. Proposed retry limit: one automatic
recovery of an unchanged startup/runtime failure, then retain a blocker for new
evidence or explicit retry. Repeated ticks do not reset it. This is separate from
CI rerun allowance. Successful continuation resets the consecutive-start failure,
not the history. Shared loss across Tasks belongs in Wave judgment; it must not
cause a storm of identical repair agents. No new automatic Wave conversation is
launched here while Unit 2's turn-delivery contract is unproven.

### CI, landing and closure

| Fresh evidence | Result |
| --- | --- |
| Failed required leaf checks | Reserve one `ci-fix` for the current incident when checkout admission permits. Preserve land-time precondition exclusions. |
| Required integration despite green CI | Reserve `ci-fix` to sync/rebase; obsolete-base alone is insufficient unless integration is actually required. |
| Merge-queue membership | Observe queue/integrated-check evidence; do not repair the original head merely because it appears behind. |
| Pending, queued or missing expected checks under 30 minutes | Wait, with persisted attempt start time. |
| Same CI attempt exceeds 30 minutes | Admit diagnosis; allow one automatic timeout-only rerun in that incident lineage. A rerun restarts its attempt clock, not its allowance. |
| Green, waiting for review or merge | Wait or exercise only the already-authorized merge request. Never approve a review. |
| Auto-merge revoked / manual merge intent | Keep that choice; no automatic re-arm or merge. |
| Unavailable provider evidence | Record failed observation; no inferred CI failure, idle Task or successful merge. |
| Verified merge | Save merge evidence, apply saved Task/PR-chain disposition, retry pending Linear writeback without an agent. |
| Closed without merge | Keep an unresolved delivery outcome; never complete successfully. |

CI attempt identity includes PR/head and provider check/run attempt identifiers;
use durable first observation/publication timing for expected checks that never
start. A changed head or explicit rerun opens a new attempt while retaining
history. A new timeout-created attempt cannot launder the one-rerun bound.
A completed repair with unchanged evidence and blocker stays blocked. A reserved
repair that never began resumes its reservation rather than being counted as a
completed unsuccessful repair. A repair that dies after publishing first
reconciles its native outcome and current remote head; never blindly replays edits.

`ci-fix` syncs, repairs, verifies, publishes and restores the saved disposition,
then returns. It does not restart the feature Flow, switch its captured definition
or manufacture commits to reset a deadline. Fresh head/claim validation also
applies when repair returns. Mechanical Flow landing stays on its exact
`flow_events` operation/landing until settlement; a tick must then wake that same
Flow so later steps can execute. Bare landing leaves the Task open; `--next`
rotates once; `-c` waits for the existing completion gate and local/Linear success.
Reconciliation retains already-merged intent even if Task/PR pointers moved.

### Desktop, absent states and stopping

Expose enabled/cadence, selected/held Tasks, last successful check, last failed
observation, admitted work and blocker through one explicit Rust DTO and fixtures
mirrored in Swift. A successful idle check differs from a failed read or a useful
check that found blocked work. No installed schedule means Disabled, not Healthy.
Stale/missing receipts mean unknown coverage, never all Tasks idle. Job install,
update and removal are idempotent; reopening the app does not duplicate jobs.
Placement changes route through the owning Home instead of launching locally.

Proposed stop behavior: disabling the repository schedule stops new automated
admissions; already-running work finishes under its existing authority. Holding a
Task also prevents a Session caller from automatically restarting it. Neither
silently revokes an already-requested GitHub merge. Controls show that pending
request and retain the explicit operation to revoke it. Terminal Tasks never
start new work, but incomplete authorized writeback can still settle. Independent
unknown liveness and pending human review remain visible until resolved.

## Implementation sequence and deletion

One coherent replacement; internal cuts are not separate partial product claims.

1. **This slice: complete finite delivery recovery.** Retain existing watcher
   deletion and `flow_events` linkage. Add actionable integration evidence,
   durable CI attempt/deadline and reservation/outcome distinction. Prove
   `land -c` exits, a later check handles green-but-needs-integration, a repair
   never started can recover, and interrupted Linear settlement converges.
   Focused test: `cargo test -p loopflow --test land_tests` with those scenarios.
2. **Shared admission and detached handoff.** Extend Task/Session reservation
   boundaries; reuse `TaskWork` and exact liveness evidence. Connect selected
   Task reconciliation to the common driver, including waking a parked landing
   occurrence. Move CI repair out of the tick process and prove actual child
   survival, concurrent admission, review races and bounded failed-start recovery.
3. **Repository schedule and Desktop consumer.** Generalize `CronSpec`, target
   dispatch, daily-only `CronSchedule`, labels (Home plus canonical repository)
   and receipts; preserve existing Wave daily jobs with the same implementation.
   Add shared commands/DTO fixtures and Desktop controls. Cover disabled state,
   resync, placed-Home mismatch, failed reads and overdue checks.
4. **Consumer cutover and configured demo.** Update delivery skills/docs and
   verify release, taskless callers, Session callers and Flow completion use the
   same handoff. Coordinate LOO-338's watcher retirement with this proven path.
   LOO-353 consumes the operation when Unit 2's primary input path is ready;
   primary Session provisioning and Flow switching remain its responsibility.

Keep deleted: `watch_armed_pr`, `watch_armed`, `wait_for_landing`,
`supervise_pr_landing`, their persistent observation loops and exclusive watcher
fixtures. Remove the synchronous `exec_ci_fix` wait/heartbeat from the tick when
repair admission is detached; preserve useful execution capture, account routing,
claim fencing and repair conclusion recording. Do not create another landing
implementation for cron or primary Sessions. Do not remove release's separately
owned finite release lifecycle just because it waits for its own result.
Forward-migrate released Task/landing/incident records; existing armed delivery
must remain discoverable. Do not migrate or promote a development build into the
installed Home to prove this plan.

## Acceptance at gate and review

Extend existing Rust suites and add `scheduled_task_tests` for the integrated
scenario. In a private Home with controlled GitHub/Linear/provider processes:
select Task A, park Task B at a review, leave C unselected and D held; launch two
checks plus a Session-originated check. A gets one driver, B/C/D get none. Kill
A's driver, preserve a live child in one variant and kill both in another. The
next check either observes/reconnects the existing work or recovers once. Reach
`land -c`, let that process exit, fail CI, run one detached repair, observe merge,
fail Linear writeback once and retry. Assert unchanged Flow/review identities,
exactly one repair per occurrence, preserved disposition and eventual closure.
An open but unselected PR receives no repair or merge request. Include a taskless
landing and a queued PR so the scan cannot accidentally narrow scope.

Additional boundaries in those suites: review opens during admission; stale
head/generation; death before spawn and after remote publish; timeout clock across
restart/rerun; permanent launch failure without repeated agent creation; missing
expected checks; green required-rebase; removed auto-merge; closed-unmerged PR;
`--next` retries; associated independent work preventing cleanup; repository job
resync/disable and two Home placements. A primary conversation alone does not
block its own authorized handoff, and shared scope alone grants no exemption.

Headless gate commands (new suite names are implementation deliverables):

```sh
cargo test -p loopflow --test scheduled_task_tests --test land_tests --test release_tests --test dto_fixtures
cargo test -p loopflow --lib ops::cron::tests
cargo test -p loopflow --lib controller::task::tests
swift test --package-path swift --no-parallel --filter TaskAutomationTests
uv run python scripts/test.py --reuse-passing --loopflow
```

Expected: the integrated scenario and affected shared-model/UI/build checks pass
without a display or approval dialog. Keep OS launchd lifecycle proof on a
capable macOS runner using a disposable Home/job and real child processes;
unavailable execution is deferred there, not silently replaced by a plist mock.
Configured demo separately proves actual scheduled progress with Desktop closed
and Jack's judgment of clarity. No fixture establishes installed acceptance.

Success: Jack works elsewhere while selected Tasks arrive at landing or their
intended review, with no babysitting. Failure: the clock restarts intentionally
stopped work, an idle primary blocks forever, two agents repair one head, or
check receipts look healthy while Tasks are stranded. The admission races,
selection controls, outcome references and coverage receipts target those cases.

This serves Product's shared CLI/Desktop contract and supports longer Desktop
work sessions. Automation uptime is not proof of the chapter's three-day usage
or three two-hour crash-free sessions, nor external-product progress. Keep those
Wave signals distinct; this Task's measurable signals are failure-to-admission
latency, duplicate admissions (zero), and merge-to-settlement convergence.

## Open choices for Jack

1. **Ownership and delivery order.** Recommend cron as the reliable floor now,
   TaskSession as the ongoing Task operator, Wave Session for broader recovery.
   LOO-332 need not wait for Unit 2's native-input work. The alternative of making
   a primary Session mandatory would delay unattended reliability until that
   mechanism is proven. Accepted Wave autonomous recovery remains in Unit 2.
2. **Enrollment and stop intent.** Recommend explicit per-Task opt-in, with
   selected Flow launches offering/recording that choice; no bulk enrollment of
   historically started Tasks. Hold/Off stops future admissions, while existing
   GitHub merge requests remain explicit. Decide whether every deliberate new
   Task launch should instead opt in by default and how interruption maps to Hold.
3. **Availability promise.** Recommend a one-minute default on the placed Home,
   one unchanged-failure recovery and one timeout-only CI rerun before surfacing
   a blocker. This covers Desktop closed while the host session is available.
   If Jack expects progress through logout or laptop shutdown, use an explicitly
   placed always-on Home; automatic cross-Home failover is outside this Task.

These are review choices, not blockers to the comparison or permission to
implement them. No new code, live schedule, publication or installation occurred.

Check: 2026-09-30 source/claim/cron and pinned LOO-353 design review completed; `git diff --check` passed; no builds/tests run for this documentation-only pass.
