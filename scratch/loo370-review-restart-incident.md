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
