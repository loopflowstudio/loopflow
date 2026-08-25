---
description: Advance open KRs inline first, filing or looping tasks when needed.
action_style: procedural
---
Pursue the open KRs.

## Orientation

Read the exact Linear Project named in the seed and the wave's GOAL/MEMORY. The
project loop owns the KR set, not a product PR. KRs are proof-shaped end
states; tasks are the concrete work that advances them. Filed tasks live in
Linear; running work lives in Tasks; merged PRs are closure evidence.
Resolve the exact wave and Project from the session prompt; never guess them. If the PM
reader fails, report that once and continue from the KR set instead of repairing
PM or auth.

The seed carries only metrics owned by this Project. Use the few readings that
make the most important outcomes visible to focus Task direction. When KR
pursuit or Task work exposes a decision-relevant blind spot, get the first
useful signal from zero to one through a coherent Task. Once a signal is
trustworthy, get the machinery out of the way: use the reading to choose work
that improves or protects the sponsored outcome, and revisit the instrument
only when it is broken, misleading, or no longer measures what matters. Use a
cross-owned reading only when the Wave explicitly routed it, preserving the
reading's real owner.

A Task worker may propose a metric while building feature work. Review those
proposals as potential sponsorship decisions: adopt a useful proposal by
formalizing its meaning and filing coherent producer work, ask for the missing
evidence that would make it credible, or decline it. Prefer measures whose
movement changes the next Project decision; do not collect proposals into a
backlog of interesting numbers.

For each sponsored metric, decide whether its current reading calls for a worker:

- A Missed target usually gets roughly one worker iterating on the outcome.
  Combine work when one experiment or root cause moves several metrics.
- A Met frontier may keep a worker because better still matters. A Met guardrail
  stays quiet until its alarm trips.
- Unknown or Unavailable evidence calls for instrument repair, investigation,
  or waiting—not blind optimization of the outcome.

This is a momentum default, not a utilization quota. Yield when evidence needs
time, an external dependency blocks progress, or no justified experiment
remains.

The project may read and file its own tasks:

```bash
lf pm show --wave <exact-wave> --project <project> --no-sync
lf pm task create --project <project> --title "..." --notes "..."
```

## Task references

When selecting, supervising, or reporting more than one Task, read `lf roadmap
--wave <exact-wave> --json` for plan-wide rows and `lf status <exact-wave>
--json` for live execution. Render every Task with the shared reference:

```markdown
[identifier](provider URL) — readable active PR/workspace slug — status/next owner
```

Fill the link from `task.identifier` and `reference.issue_url`. Use
`active_pr.slug` from roadmap; for status, match `active_pr` to `prs[].id` and
use that PR's `slug`. Fall back to `reference.workspace.slug`. Take status from
`runtime.status`, or from the roadmap `section` when runtime is absent;
`next_move.owner` supplies next owner. Never reconstruct a provider URL, branch,
or slug from an identifier, title, worktree, or naming convention. Omit only a
link or slug whose snapshot evidence is explicitly absent; keep the Task and
its available status/next owner.

## Work

- Honor every Steer included in the seed and state the resulting priority or
  plan change. Do not create a separate acknowledgement mutation for inputs
  already present in the seed.
- Read the filed backlog before creating work. File a concrete task when the
  KR needs it; no rule requires every filed task to start immediately.
- When one uncertain KR warrants parallel investigation, file independent Tasks
  by approach family rather than duplicate assignments. Keep a compact registry
  of each mechanism, concrete evidence, exact gap, and status. Redirect
  convergence, mark theorem-strength or dependency-strength gaps blocked, and
  reopen a route only for a materially new mechanism. Cross-pollinate only
  after the independent Tasks have exposed their own strengths and failures.
- Every file-writing task must already have a Linear identity. Start it with
  `lf task run <issue-id> --directive "<delegation brief>"` and supervise the
  same Task through review and merge. Include relevant KR and metric evidence
  in the direction so the worker can advance or protect it without receiving
  the entire Wave portfolio. Invite the worker to return a metric proposal when
  feature work reveals a better signal.
- When a separate Task depends on an open parent PR and should begin now, start
  it with `lf task run <child> --stack-on <parent> --directive "..."`. The child
  keeps its own worktree and worker; never create a second simultaneously open
  PR inside the parent Task.
- The Project owns no worktree or PR branch. Never edit, commit,
  test, or open a PR from the canonical main checkout; delegate every
  repository mutation to a Task.
- Use `lf task steer`, `interrupt`, `wait`, and `resume`. Do not create another
  worktree or session for review feedback or CI repair.
- When the choice needs Wave judgment, run `lf ask "<exact question>"` and
  continue the same Turn after the Ask settles.
- Never start another Project or Wave from Project pursuit, and never collapse
  the remaining Project into one anonymous task.
- Discovered debt becomes a task under an existing KR unless it
  reveals a broader standing quality frontier. Do not turn individual cleanup
  into a project-shaped KR.

Do not check off a KR unless its observable condition is already true. A Met
metric is evidence, not automatic completion.
