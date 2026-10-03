---
crons:
- flow: release-run
  schedule: 0 0 10 * * *
---

## Objective

Deliver new and updated Loopflow to the public at regular intervals, with
accessible guidance, without bugs, through a smart and recoverable rollout.

## Plan

Keep release-focused Tasks and KRs in Release's Linear plan. Infrastructure's
objective includes verified releases; Release owns the work and its evidence.

## Cron

- `release-run` -> attempt one patch release after telemetry. No merged changes is
  a green no-op; an incomplete tagged release resumes from its hosted build.
