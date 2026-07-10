---
description: Evolve the wave from what it learned.
default_agent: codex
action_style: procedural
---
Mutate the wave.

## Orientation

Read GOAL/MEMORY, `lf pm show --wave <wave> --json`, recent chat, and the live tasks. The wave never
terminates; it changes shape.

## Work

- Reconcile Linear Projects against what actually landed: retire a KR only when
  its condition verifiably holds — endurance KRs mean what they say (a
  counted streak isn't satisfied by one good day; a human rescue inside an
  unattended window resets it). Write changed KRs with `lf pm project update`;
  archive dead bets in Linear.
- Add durable learnings with `lf memory add` or rewrite memory through the
  server-owned memory command when the accumulated facts need curation.
- Launch, retire, reset, or split sub-waves when the objective needs a new
  center of work.
- Update GOAL.md when the current objective no longer asks the honest question.
- Escalate blockers upward with `lf radio pub --parent`.

The wave oracle is `Never`: stopping is not a runtime decision.
