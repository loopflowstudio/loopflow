---
description: Retire or renew project KRs.
default_agent: codex
action_style: procedural
---
Mutate the project honestly.

## Orientation

Read the KR set in `<lf:message>`, inspect open task state when needed, and
keep the wave goal in view.

## Work

- Mark milestone KRs done only when their observable condition is satisfied.
- Renew self-renewing KRs by editing the KR item, not by changing the runtime.
- Escalate to the parent wave with `lf chat --parent` when the KR set is
  blocked by missing authority, missing credentials, or unclear strategy.

You decide the project is done by setting the bit, not by saying so: mark
each genuinely-finished KR completed in Linear. The runner only reads the KR
set back; it terminates when every kr-labeled item is completed.
