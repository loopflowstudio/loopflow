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

Inspect whether the requested work already has a Task, prepared context, or
running worker before filing or launching. Write a Task title and opening from
the user's situation, problem, and desired experience. Keep speculative solution
ideas tentative and accepted constraints binding; detailed solutions belong in
a separate design.

For problem-first work:

```bash
lf task start <project> "<desired experience>" --flow <chosen-flow> <<'BRIEF'
<short user-problem brief>
BRIEF
```

For an existing Task, use its identity instead of filing another:

```bash
lf task run <existing-issue> --flow <chosen-flow>
```

Stdin becomes the durable Task description; `--directive` supplies worker
direction and does not replace that brief.

Use an explicit human-selected `--flow`; otherwise use the Project's current
recommendation. Read the actual Flow before describing its review gates. Do not
infer policy from obsolete fix/feature flags or first/loop/finally settings.

## Existing-design handoff

When the human asks to file a Task and run a Flow from an existing design,
keep the design separate. Reuse the Task if it is the same work; otherwise file
a short user-problem brief under the selected Project, with a design reference,
its maturity, and open questions. Do not invent ownership.

```bash
lf pm task create --project <project> --title "<desired experience>" --notes "<brief; design reference and maturity>"
lf task prepare <issue> --json
# Copy the selected design and required evidence into the returned worktree's scratch/.
lf task run <issue> --flow <chosen-flow>
```

Inspect the current context first: a design already in the Task worktree needs
no transfer. For a separate source, copy the actual documents and supporting
files before launch, preserve relative references, and check their contents in
the destination. A path alone does not supply context. Preparation launches no
worker; put any initial directive on preparation, since an already prepared
Task rejects a new `run --directive`. Do not overwrite newer destination work.

The destination becomes the working design; retain source provenance without
maintaining competing active copies. Markdown under its recursive `scratch/`
tree enters worker context; other assets remain on disk. Preserve material
needed after scratch cleanup in durable documentation or existing records.
Do not pipe the design into Task creation: stdin becomes the Task description.

Use the human-selected Flow and inspect its contents when explaining where it
begins; otherwise use the Project recommendation. Continue the design already
present without treating its draft choices as approved. Report the Task link,
destination design path, selected Flow, and observed launch result. Verify
supplied context separately from worker startup.

If implementation already exists in the source checkout, preserve it and its
writer. Document transfer does not adopt a checkout; current preparation does
not adopt an unbound existing branch/worktree. Report that gap before launching
a competing implementation. No automatic scratch-transfer flag is available.
