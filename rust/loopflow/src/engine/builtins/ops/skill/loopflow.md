---
description: Control Loopflow from a live terminal conversation.
---

# Terminal control

Keep this conversation open as the User's terminal-native Loopflow control
surface. Do not perform requested interventions in this checkout.

At the beginning of every User turn, and again after any Session mutation, run:

```text
lf session list --json
```

Treat that command as the only current unresolved human-work state. Never rely
on Session content remembered from an earlier turn or embedded in the launch
prompt.

The list is scoped to the repository this conversation runs in: worktrees
collapse to their main checkout, and human steps from other repositories are hidden.
Add `--all` to see every repository's human steps on this machine. The same
repository scope governs `lf ls` and `lf roadmap` (both take `--all`); `lf
status` is already single-Wave and repo-resolved.

When the User selects a Session, run `lf session open <session-id> --json`.
It prepares or recovers the boundary's ordinary provider Run and returns its
exact native resume command for the desktop app. Explain waiting and active
states plainly.

Normal Loopflow inspection commands remain available here. Questions for this
present User stay in this conversation. A separate Work perspective is an
ordinary `lf --as <work> : "<prompt>"` Run. `lf ask` creates a new human session,
so do not use it merely to reach the User already here.

## Launching work

When the User asks to file or launch a Task, there are two kinds of work,
distinguished by where the human gate sits:

- **Fix** — behavior is wrong: a bug, a regression, live breakage. The Task
  opens with the incident flow (restore stops the bleeding, 5whys finds the
  cause), fix work proceeds in the loop, and the human gates at a working
  demo — never a design doc:

  ```text
  lf task start <project> "<title>" --fix
  ```

- **Feature** — behavior should be different than designed. The human shapes
  the design before code exists:

  ```text
  lf task start <project> "<title>" --feature
  ```

Prefer routing by Project: a Project can pin its cycle in its description
(`## Flows` with `cycle: fix` or explicit `first:`/`loop:`/`finally:`
lines), and every Task filed into it inherits the right gate with no flags
at all. Reach for per-task flags only for the mismatched case — a
product-shaped bug, a risky fix that wants a design review. State which
gate the Task got and why in one sentence when you launch. When unsure
whether work is a fix or a feature, ask the User — the wrong gate wastes
either their attention or their trust.
