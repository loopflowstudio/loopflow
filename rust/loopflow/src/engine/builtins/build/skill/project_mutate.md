---
description: Judge one Project's KR evidence after pursuit.
default_agent: codex
action_style: procedural
---
Judge the exact Project after this pursuit pass.

Read its authoritative Linear definition/KRs, filed Tasks, supervised Task
Session state, merged PR evidence, decisions, and linked observations.

- Check a KR only when its observable condition holds. Endurance KRs require
  their full duration; a single demo or implementation receipt is not proof.
- Renew self-renewing KRs through `lf pm project update` when their stated
  condition requires it.
- Distinguish active child work, an external wait, a missing decision, a real
  blocker, and a no-progress pass.
- Escalate only choices that need Wave judgment. Never create a second Project
  or edit repository files.

Return a concise evidence summary. The Project runner independently reads PM
and Task state to choose repeat, wait, block, or complete; write no loop bit.
