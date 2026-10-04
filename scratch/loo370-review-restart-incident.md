# Replaced review blocks autonomous Task restart — LOO-370

Jack Heart requested a 5whys investigation and autonomous implementation,
verification and landing of the resulting fixes on October 4. Jack then explicitly
authorized editing the Loopflow database directly to unbreak the Task immediately.
Infrastructure owns this recovery/control-plane defect. Preserve LOO-370's own
Run-model cleanup scope and ongoing implementation; file/reuse separate repair
Tasks for independent outcomes. No further interactive approval gates are needed
for the incident source fixes. Do not mutate unrelated Tasks or fabricate outcomes.

## Observed failure

`lf task restart LOO-370 --flow code ...` replaced feature Flow
`69217c2a-11d9-4bd0-831f-ad95786d4e72` with code Flow
`b2476b7b-a8b2-4b61-9c04-d146871206d3`, then timed out at worker startup.
Task status showed idle implement and no worker claim. A same-Flow start refused:

- Review `task_5341076d02ed439798b9349bf7c5cdb6:69217c2a-11d9-4bd0-831f-ad95786d4e72:feature:review_kickoff:0` awaits completion.
- Execs `02c0220f-01f6-4d49-9c14-ca54b66fb3c6` and
  `9f7a4496-bddb-47f4-8f06-77e0c8393327` have live or unresolved execution.

`lf session complete` rejected that exact review as no longer waiting.
This is not an ungranted product approval: Jack authorized replacement and
fully autonomous development/landing. The former review was no longer attached
to the current Flow but still affected Task admission.

## Fresh inspection and emergency recovery

Earlier analysis called the Execs stale/unresolved. Exact receipt inspection
corrected that: both were live. PID 49116 was the review service; child 50014
was its lf review process; Codex 55146 shared process group 49116. Code-mode host
74562 descended from Codex. Receipt start times matched OS process ages. No
provider PID was stored on the Session itself; no new provider input was recorded
there after its original capture/observations. Missing snapshot matching was not
valid evidence of absence.

The database was backed up through SQLite's online backup API to owner-only
`/Users/jack/.lf/backups/loo370-review-repair-1791125402499139000/loopflow.db`.
The same directory retains repair provenance, process receipts and the exact
Session row before the edit. Do not copy the database into Git or output secrets.

After rechecking the exact three process-group members, the operator sent
SIGTERM to group 49116 and verified all four owned processes exited. Then one
transaction set completed_at and an explicit administrative-recovery ready_summary
on the exact obsolete review, conditional on its capture and the old/new Flow
relationship. It changed no Exec outcome, Flow, capture or event history.
This records retirement, not successful product review or fabricated execution.
The supported same-Flow start was then retried; refresh Task status for its result.

## Source evidence to investigate

`ops/task/lifecycle.rs::execution_blockers` treats every uncompleted non-managed
review as a blocker; its driver Execs also enter the blocker set. Replacing the
managed Flow makes its old review non-managed. `ops/human_session.rs::owned_target`
rejects a Flow review with no waiting Flow. Trace the restart transaction and
process retirement contract, including interruption/partial failure. Do not
settle on bypassing all review or liveness checks.

Prove restart from a real waiting review, exact old driver/provider retirement,
retained history, original Task/PR/checkout identity, and admission of the saved
replacement Flow. Include interruption/retry, live old writers and unrelated
independent reviews. An idle/mechanical old Flow fixture misses this failure.

Keep emergency recovery, source prevention, installed acceptance and merge
settlement as separate claims. LOO-370's implementation must not be competed with.

## Recovery readback and causal investigation (2026-10-04)

Installed `lf task status LOO-370 --json` now reports running implement in the
same replacement b2476b7b-a8b2-4b61-9c04-d146871206d3. Exec
46f29d1b-e217-46e7-9072-e9a94e92915e drives that step; both original review
Execs record interrupted/130 at 1791125405. The old review records administrative
completion at 1791125445. Task, PR row and checkout identities are retained.
Emergency recovery succeeded at admission; no source prevention or shipment is
established. No emergency mutation was repeated.

Causal chain verified against source at 13cc752f256b610bf35e222e509c8629d709e952:

1. Admission rejects the previous unfinished review after replacement because
   `ops/task/lifecycle.rs::execution_blockers` checks non-managed Sessions.
   Replacement makes the previous review non-managed; its live Execs also block.
2. Completion cannot resolve this because `human_session.rs::owned_target`
   rejects a FlowReview with no waiting Flow. The two readers disagree about
   whether this retired Flow still owns actionable review work.
3. `restart_task_async` calls `stop_task_worker`, then `restart_task_flow` and
   launches the replacement. Stop checks only the captured Flow claim and
   `pending_flow_step_exec`; that SQL reads operation/current_capture, not the
   pending review's separate service/driver. No claim is treated as dead there.
   The old review service and provider were proven live in this incident.
4. The replacement transaction marks the Flow replaced and clears the Task's
   invocation pointer, but does not retire/fence the pending review. A valid
   compare against the saved Flow does not prove its Session writers exited.
5. The identity-preservation restart regression supplies no previous Flow.
   Other stop tests exercise driver/step cases; those cannot establish waiting
   review replacement. The exact earlier review worker authority error is
   retained in Exec history; its initiating race remains unproven and is not
   required to explain the replacement defect.

## Selected prevention and proof

Jack Heart authorized autonomous implementation, verification, publication and
landing without human review gates. Keep one separate repair Task covering
restart through waiting review retirement and replacement admission. LOO-373
owns retained landing/deleted tmux cwd failures; LOO-370 owns Run-name cleanup.
Neither is a substitute for this outcome.

The repair must use exact saved review/driver/provider ownership, preserve
history and Task/PR/checkout identity, and distinguish retirement from successful
review. Stop/fence old writers before replacement becomes runnable. Interruption
between stop, retirement and replacement must be retryable without fabricating
success or skipping independent work. Unknown process identity stays unresolved;
Task membership and causal ancestry alone never grant signaling authority.

Prove the public restart/admission path with an actual waiting review and owned
live test processes, retained capture/native history, old-writer rejection,
interruption/retry, and an unrelated independent review that still blocks.
Use disposable Homes and stub provider side effects, not LOO-370 as a destructive
fixture. A no-Flow or idle mechanical fixture alone is insufficient. Installed
acceptance, source proof and verified merge remain separate claims.

Check: `lf task status LOO-370 --json` — replacement implement running; retained
old review Execs interrupted; no additional data/process mutation.

## Launch decision and execution

Selected prevention is [LOO-377](https://linear.app/loopflow/issue/LOO-377/restart-tasks-without-stranding-the-previous-review),
prepared at `/Users/jack/src/loopflow.restart-tasks-without-stranding-the`.
The incident was copied byte-for-byte before launch. Linear creation succeeded
while the refresh timed out; `lf repo refresh infrastructure` recovered the same
issue, without duplication. No separate prevention project was created.

Two `flow start code` calls left saved invocation
`a69b3a18-c0ee-4405-ae06-e50ee95c4aac` idle at implement, with no step Exec or
Session, after the detached worker failed to report running within ten seconds.
No surviving matching tmux pane was observed; the exited child's stderr was not
available. This does not prove its cause is LOO-373's deleted-cwd defect.
An ordinary independent `lf --task LOO-377 : ...` contribution was launched
instead. Its first call rejected the inherited parent LF_FLOW_STEP as belonging
to another Flow. Omitting only that Flow-step token allowed the independent
contribution to reach Codex; caller Session/Exec attribution remained intact.
The contribution owns implementation and authorized delivery; the idle Flow's
position must not be fabricated or treated as completed work.

## Implementation review and verification

LOO-377 implemented the repair in PR #1429. Review identified two additional
boundaries inside the accepted scope: preserve process identities before native
client receipt cleanup so interruption remains retryable, and share the review
launch lock so retirement cannot miss a child between reservation and spawn.
The repair records the separate service Exec, fences the Session, retains exact
PID/start-time evidence, waits for exit and reuses admission's independent-work
checks. No successful review result is synthesized.

Focused public-command and store tests passed. The local broader gate stopped at
13.3 GiB free versus the 32 GiB reserve; no active work was deleted. Hosted CI's
first Rust run had 2,210 passes and two fixture failures: a stale-decision fixture
bypassed retirement, and a direct launch-lock fixture tried to claim a review.
Commit cd3e0a0f3 repairs those setups; the public waiting-review regression passed
in the first hosted run. Final hosted verification and merge passed; see the outcome below.

Publication/auto-merge succeeded, then installed landing reconciliation reported
`invalid stored landing placement: home`. LOO-373 already owns that defect. It is
separate from GitHub's merge evidence and does not authorize another emergency
DB edit or source-binary promotion.

## Verified delivery outcome (2026-10-04)

[PR #1429](https://github.com/loopflowstudio/loopflow/pull/1429) merged at
2026-10-04T15:50:44Z as c5dc238b0afb53dbe7098f30e2dee8085b53e1e3.
Both PR-head and merge-queue CI passed. The corrected Rust run recorded 2,212
passes and 16 skips. The contribution ended successfully after supported
reconciliation recorded this merge. Repository-wide reconciliation still reports
`invalid stored landing placement: home`, owned by LOO-373; installed acceptance
of review-restart prevention is not established. No release or installation was
requested by this repair. LOO-377's never-started managed code Flow remains idle;
its steps were not fabricated as successful. Source implementation and delivery
were performed by the independent Task contribution instead.

Check: GitHub PR readback — MERGED at the exact commit above; hosted CI green.

Final LOO-370 readback reports its same code Flow finished with no further steps
scheduled. Its Task remains ready; this does not establish its separate cleanup
scope landed. LOO-377 readback records the exact merge while remaining ready with
its idle saved code Flow. Neither Task's completion was manufactured.

Re-entry confirmed the same recovery and merge facts through installed Task
status and GitHub PR readback. Review of the published diff confirms the waiting
review, interruption/retry, launch serialization and independent-work regressions.
No further prevention Task or duplicate implementation is warranted for this
incident. The next distinct proof is installed acceptance after ordinary release;
LOO-373 retains landing reconciliation. No emergency mutation was repeated.
