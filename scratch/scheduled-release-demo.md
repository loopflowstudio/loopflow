# Scheduled release accounting demo — October 2, 2026

LOO-285 interactive review, source HEAD `e2eccdd257f0f3e2cf34db7583f7309eb0084394`.
Design: [scheduled release opportunities](account-for-scheduled-release-opportunities.md).
Earlier evidence: [baseline summary](evidence/baseline-summary.json) and the
design's retained focused checks. Those synthetic checks are not this demo.

## Observed installed experience

The assistant ran read-only installed commands in the assigned checkout:

- `lf --version`: `lf 0.12.32`.
- `lf home id`: `home_39860354aaca640c2ccb50bf6ca609d8`.
- `lf release history --wave infrastructure --days 35 --json`: exit 2,
  `unrecognized subcommand 'history'`. Installed release help omits history;
  installed cron help also omits disposition. The branch's documented accounting
  experience is unavailable through this installed CLI.
- `lf cron list --json`: two loaded Infrastructure jobs, release at 10:00 and
  telemetry at 09:00, both Flow targets on the main Home. Both select
  `/Users/jack/.lf/bin/lf-d7bf7c66843517e437c870e2dea0beb52717f4633f73441eedba01ff5e7bc401`.
  That selected job executable's version was not independently inspected.
- `lf cron history --wave infrastructure --days 35 --json`: 63 physical
  receipts, 31 successful release processes and 32 failed telemetry processes.
  Latest release is `cron_8966213049724428b16d65f6dbd1a8c1`, scheduled,
  September 29 at 17:00:02 UTC, succeeded. Latest telemetry is
  `cron_4f91cd1f1f984b1cafb0ea3f94a59a18`, scheduled,
  September 29 at 16:00:05 UTC, failed. This moving window does not erase the
  earlier 36 failed telemetry receipts or prove product publication.
- `lf cron preflight --wave infrastructure`: passed for one declared job.
  `lf cron preflight --wave infrastructure/release`: refused because that Wave
  is not registered on this machine. Authored nested Release ownership and
  installed Infrastructure job ownership differ; no sync or registration was run.

No receipt later than September 29 appeared in that read. The cause is unknown;
loaded jobs and preflight success do not establish subsequent executions.
No production release, installation, firing, schedule change, disposition,
Home transfer, or verification waiver was performed.

## Feedback and remaining review

Jack Heart's existing restrictions remain authoritative. No new acceptance or
design change has been expressed in this review. These observations are the
assistant's command results, not a claim that Jack personally exercised or
accepted the experience. The assistant requested feedback on keeping review open
for installed demonstration versus recording a blocked demo and preparing a gate
and delivery proposal. Until answered, that choice remains unresolved.

The configured-path demonstration is blocked by the installed command's absence.
This is not evidence of a new independent publisher defect. Two adjacent original
dues, two automatic executions, at least one artifact publication, required checks
and no manual repair remain unproved. Existing local focused tests do not replace
the affected-suite gate or configured acceptance.

Recommended next action: carry the installed-command and placement observations
into a concrete gate/delivery proposal under the existing Task. After authorized
delivery and supported installation, inspect obligation ownership and history,
then observe the qualifying automatic pair with exact public and verification
evidence. Diagnose the missing recent receipts without forcing a firing or
silently changing cadence. No implementation change is agreed by this note.

## Delivery direction — October 4, 2026

Jack Heart explicitly waived the interactive demo and requested landing PR
#1419: “get it landed. I dont need to demo”. Continue delivery autonomously,
including integration repair, affected-suite gate, publication and landing.
The waiver removes the interactive demo boundary; it does not claim installed
acceptance or the two automatic executions happened. Preserve those evidence
limits in durable documentation and PR copy. Complete this exact review and
let its following loop-decide advance the saved Flow toward delivery.
