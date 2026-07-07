---
description: Make the project KR set measurable for the project flowloop.
default_agent: codex
action_style: procedural
---
Clarify the project artifact: the wave's kr-labeled Linear items.

## Orientation

Read the KR set in `<lf:message>`, then read `wave/<wave>/GOAL.md`,
`wave/<wave>/MEMORY.md`, and `scratch/questions.md` if present.

## Work

- Make every KR measurable enough for the oracle: completed or not completed.
- If a KR is vague, update its Linear title or notes to state the observable
  condition.
- If the project has no real KRs, record that as a blocker and stop; the
  runtime refuses an empty KR set.

Do not decompose work in this phase unless the clarification is trivial and
directly unblocks the next phase.
