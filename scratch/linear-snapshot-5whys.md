# ETU-89 creation followed by snapshot timeout — 2026-10-04

Jack confirmed ETU-89 is the training-regime issue in the reported incident and
requested this repair in the same branch as the LOO-304 deadlock fix.

Observed report: Linear created the issue, but Loopflow timed out refreshing its
local snapshot and required recovery before staging/launching. Local recovery
evidence `/tmp/etude-training-final.json` identifies ETU-89 with available
planning and a retained auto-code Flow at kickoff. This proves recovery of the
existing issue, not active worker progress or the latency of the original request.

Fresh `lf mon list --all --search 'Compare training regimes' --json` found
failed Exec `2b8862c4-3c4c-488c-8931-7bf227ad7401` (started 1791100923,
finished 1791100938). Its recorded error identifies committed issue
`1657335c-ba53-49e0-acf0-250b59b31dc8` and explicitly says the Wave refresh
exceeded its five-second deadline. The full command took fifteen seconds;
the receipt does not separate network time from local persistence time.

## Why

1. Creation reported incomplete even after Linear committed because
   `task_pm::create_and_load_task` required a forced Wave snapshot refresh before
   loading the created issue. The error explicitly acknowledged the committed
   issue but prevented local preparation from proceeding.
2. That refresh can fail independently of the issue: `try_timed_refresh` gives
   context resolution, provider reads, persistence, and readback five seconds
   total. `fetch_pm_snapshot_for_projects` lists issues for every checked Project.
   A slow historical Project or local store wait can therefore block a new Task.
3. The command did more work than its success condition required. Creation had
   already refreshed planning and searched the current Project for its durable
   marker; afterwards it swept the Wave again to learn one issue's identity and
   ownership. An exact issue reader already exists and persists that record.
4. Existing tests treated post-write snapshot failure as an expected recovery
   burden, proving duplicate-safe retries but not asking whether that snapshot
   was needed. Edit/completion uses the same overly broad reconciliation pattern.

The structural cause is coupling a committed single-issue mutation to a broad
cache refresh. This is distinct from the dispatch mutex cycle. The deadlock can
also delay planning persistence, but available evidence does not establish that
it caused ETU-89's particular timeout. No evidence establishes provider outage,
rate limiting, a bad credential, or a universal five-second latency requirement.

## Repair and limits

Confirm the exact issue after creation and edit/completion. Preserve creation
markers, ownership checks, explicit committed-but-unconfirmed errors, and the
original issue ID. Use the exact record's observed timestamp for Task plans;
do not falsely refresh the entire Wave's freshness. If exact confirmation fails,
retry resolves the same marker/issue, never blindly repeats issue creation.

Pre-write ownership/planning reads and explicit Wave refreshes remain necessary
in their existing paths. This repair does not make unavailable Linear reads
succeed or claim to eliminate every possible planning timeout. It removes the
unrelated post-write prerequisite and the Home-wide database stall identified
in the other incident.

Proof: isolated stateful Linear tests cover successful creation/edit while the
post-write Wave snapshot is unavailable, exact-confirmation failures, preserved
provider edits, no duplicate issues, and no accidental launch of backlog work.
The existing issue is already recovered; this investigation does not mutate or
launch ETU-89.
