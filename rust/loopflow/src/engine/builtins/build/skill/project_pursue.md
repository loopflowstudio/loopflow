---
description: Decompose open KRs into running tasks.
default_agent: codex
action_style: procedural
---
Pursue the open KRs.

## Orientation

Read the KR set in `scratch/<branch>.md` and the wave's GOAL/MEMORY. The
project loop owns the KR set, not a product PR. KRs are proof-shaped end
states; tasks are the concrete work that advances them. Filed tasks live in
Linear; running hands live in `lf runs`; merged PRs are closure evidence.

## Work

- Read the filed backlog before creating work. File a concrete task when the
  KR needs it; no rule requires every filed task to start immediately.
- Inhabit one task whose next move needs the wave's memory/chat with
  `lf loop task "<one-PR-sized statement>" --wave <wave>`. Delegate any other
  self-sufficient task with `--detach`. The seed is the whole handoff.
- A detached task must report, publish live learnings, and leave a PR; otherwise
  its private transcript makes it invisible.
- Discovered debt becomes a task under an existing KR unless it
  reveals a broader standing quality frontier. Do not turn individual cleanup
  into a project-shaped KR.

Do not check off a KR unless its observable condition is already true.
