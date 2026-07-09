---
description: Evolve the wave from what it learned.
default_agent: codex
action_style: procedural
---
Mutate the wave.

## Orientation

Read GOAL/MEMORY, `projects/`, recent chat, and the live tasks. The wave never
terminates; it changes shape.

## Work

- Reconcile `projects/` against what actually landed: retire a KR only when
  its condition verifiably holds — endurance KRs mean what they say (a
  counted streak isn't satisfied by one good day; a human rescue inside an
  unattended window resets it). Delete dead bets; git history is the
  tombstone.
- Add durable learnings with `lf memory add` or rewrite memory through the
  server-owned memory command when the accumulated facts need curation.
- Launch, retire, reset, or split sub-waves when the objective needs a new
  center of work.
- Update GOAL.md when the current objective no longer asks the honest question.
- Escalate blockers upward with `lf chat --parent`.

The wave oracle is `Never`: stopping is not a runtime decision.
