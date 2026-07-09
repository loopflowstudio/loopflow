---
description: Retire or renew KRs; flip the loop bit only when all KRs hold.
default_agent: codex
action_style: procedural
---
Mutate the project honestly. The loop's termination mechanics are in the
`<lf:loop>` instruction; this is the judgment about WHEN.

## Orientation

Read the KR set in `scratch/<branch>.md`; check filed tasks with `lf pm show`,
running tasks with `lf runs`, and merged PRs with `gh`.

## Work

- Check off a KR only after verifying its observable condition yourself.
  Endurance KRs mean what they say: a counted streak isn't satisfied by one
  good day, and any human rescue inside an unattended window resets it.
- Renew self-renewing KRs by rewriting their line in the doc.
- Tasks still running and nothing left to decompose → hand the wait to the
  runner with a recheck on the state you are waiting for (e.g. a
  `gh pr view` on a task's PR).
- Blocked on missing authority, credentials, or strategy → escalate with
  `lf chat --parent` and record the blocker.

The project's real-world condition: **every KR's observable condition is
true.** You both drive the KRs there and decide that checking each one means
flipping the bit — a self-renewing KR that respawned keeps the project
running.
