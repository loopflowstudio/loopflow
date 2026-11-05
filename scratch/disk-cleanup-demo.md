# LOO-722 scheduled cleanup demo — 2026-11-04

## Decision pass 1: advance

At 18:12 PST, the headless review accepts the inspection repair for the authored
delivery step. This is the agent's decision standing in for Jack Heart, not new
human feedback. Jack Heart previously approved the cleanup policy and explicitly
said “lg2m ship it” for PR #1820 in comment
`05f946d5-d646-4c9e-9fe4-6167266327bf`. The current repair preserves that policy and
adds no deletion authority. No product choice needs resolution.

The preceding implementation intended to fix scheduled process inspection under
build load: use Standard launchd scheduling and the existing bounded two-minute
command deadline instead of special ten/thirty-second inspection limits. Its
real installed run and today's audit demonstrate progress. No concrete repairable
gap in this change warrants returning to implement at this demo boundary.

## Observed experience

- Before: the ordinary 17:17–17:19 cycle failed on the ten-second `ps` limit before
  candidate discovery. That failure remains evidence; the prior clean manual run
  did not establish calendar reliability.
- After: the installed launchd kickstart at 17:55:40–17:58:52 completed with exit
  zero, 291 candidate decisions, no failures, no signals and no removed allocation.
  This was a manual launch of the configured job, not an ordinary calendar event.
- Today's read-only capture revalidated all five installed files against current
  source, Standard scheduling, the unchanged minute-17 calendar, six protected file
  hashes, three protected root identities and private configuration. All passed.
  Launchd is loaded and idle with last exit zero. The installed entrypoint's help
  command also passed through its actual clean environment and working directory.
- Free space in the completed receipt fell independently by 2,951,847,936 bytes
  to 175,365,357,568 bytes, with zero target shortfall. Zero allocation was reclaimed;
  the free-space change must not be credited to cleanup.
- Accounting measured 32 roots, left 102 explicitly unknown, and compared 32
  last-known values: ten grew and none shrank. The prior ordinary sample measured
  29 roots with 105 unknown and five growing comparisons. Different coverage,
  intervals and overlapping roots prevent a sustained-containment conclusion.

No cleanup, schedule mutation or process termination was initiated in this demo.
Existing real-run receipts were inspected rather than replaying completed sweeps.
No new implementation tests were needed for this evidence-only review.

## Accepted design and limitations

The current behavior and commands are in [the operating guide](../scripts/disk-cleanup.md).
The consolidated design, reproduction, test evidence and remaining acceptance are
preserved in Git at `3620f32993:scratch/disk-cleanup-verification.md`; earlier
human review and design remain at `7e969d4f6e:scratch/disk-cleanup-demo.md` and
`7e969d4f6e:scratch/loo-722.md`. Publication cleared those scratch files; their
absence here is not evidence that the review failed or never occurred.

No agreed or proposed policy changes arose in this pass. Complete process evidence,
PID/start verification, fail-closed inspection and all source/history/reservation
protections remain required. LOO-725's already-reparented provider with no remaining
runtime handle or discoverable ancestry remains outside the cleaner's termination
authority; shared history or repository ancestry does not authorize a signal.
This differs from descendants identified before teardown, whose identities remain
tracked through teardown. Transient unknown hook owners still fail closed.

## Evidence and next action

Private receipts, launchd state, schedule, usage history and fresh preservation
summary are under
`~/.lf-machine/maintenance/disk-cleanup/evidence/20261104-LOO-722/inspection-repair/demo-pass-1/`.
`manual/` contains the original successful run; `verification-before-compress.md`
preserves investigation and prior failure evidence. Personal inventories stay
outside the repository. This note is the current headless demo record; no HTML
review or fresh human observation is claimed. The older `scratch/pr-review.html`
describes PR #1820 and is not a walkthrough of this repair.

GitHub confirms PR #1820 and #1821 merged. Current
[PR #1823](https://github.com/loopflowstudio/loopflow/pull/1823) is open at
`224746316feeaebdfa96df8be164c1f69e8cbbff`. Advance the existing Graph through its
authored delivery steps; do not start another run or mark the Task complete here.

Verification must capture the ordinary 18:17 PST run on these bytes and repeat
protection checks using `inspection-repair/capture.py ordinary 1793845020` once
it has completed. Then assess successive comparable scheduled measurements and
address demonstrated recurring growth within the accepted cross-repository scope.
A clean cycle or merge does not prove sustained containment. These are remaining
delivery/outcome checks, not an absent-person blocker or a reason to repeat
implementation without a concrete defect.
