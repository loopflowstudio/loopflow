---
description: Keep the Wave objective and portfolio computable.
default_agent: codex
action_style: procedural
---
Clarify the Wave before choosing more work.

Read the exact Wave's GOAL/MEMORY, recent human conversation, cache-only PM
snapshot, and current Project/Task state. A Wave owns its durable objective,
memory, cadence, budget, and Project selection. Each Project belongs to exactly
one Wave and owns its own KRs.

- Reconcile new human direction with the current objective and portfolio.
- Correct `GOAL.md` only when the Wave objective, measures, bounds, or cadence
  no longer ask the honest question.
- Correct Project definitions or KRs through `lf pm project update`; Linear is
  authoritative. KRs state observable proof, not tasks or implementation
  receipts.
- Demote individual cleanup into a Task under a broader Project. Promote a
  durable independent operating context into a Wave, never a child Project.
- Do not implement repository changes. Every file-writing change begins as a
  Linear Task under a Project.

Leave a concise statement of the current objective and the one or two tensions
the pursuit phase should act on. The Wave runner advances the flow; write no
loop bit.
