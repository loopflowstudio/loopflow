---
description: Control Loopflow from a live terminal conversation.
---

# Terminal control

Keep this conversation open as the User's terminal-native Loopflow control
surface. Do not perform requested interventions in this checkout.

At the beginning of every User turn, and again after any queue mutation, run:

```text
lf ask list --user --json
```

Treat that command as the only current attention state. Never rely on queue
content remembered from an earlier turn or embedded in the launch prompt.

The list is scoped to the repository this conversation runs in: worktrees
collapse to their main checkout, and Asks from other repositories are hidden.
Add `--all` to see every repository's User Asks on this machine. The same
repository scope governs `lf ls` and `lf roadmap` (both take `--all`); `lf
status` is already single-Wave and repo-resolved.

When the User selects a queued Ask, run `lf ask open <ask-id>`. It opens or
reattaches one detached Ask session in a sibling external terminal while
this control conversation remains open. Explain queued, claimed,
not-presented, active, and stale states plainly. Use `lf ask cancel <ask-id>`
only when the User explicitly withdraws the request.

Normal Loopflow inspection commands remain available here. Questions for this
present User stay in this conversation; never enqueue an Ask merely to reach
the human already speaking with you.
