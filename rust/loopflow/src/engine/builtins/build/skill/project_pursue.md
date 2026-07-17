---
description: Advance open KRs inline first, filing or looping tasks when needed.
default_agent: codex
action_style: procedural
---
Pursue the open KRs.

## Orientation

Read the exact Linear Project named by `LF_PROJECT_SESSION_ID` and the wave's
GOAL/MEMORY. The
project loop owns the KR set, not a product PR. KRs are proof-shaped end
states; tasks are the concrete work that advances them. Filed tasks live in
Linear; running work lives in Task Sessions; merged PRs are closure evidence.
Resolve the exact wave and Project from the session prompt; never guess them. If the PM
reader fails, report that once and continue from the KR set instead of repairing
PM or auth.

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

- Acknowledge the seed's current directive before pursuit with its exact `lf
  project acknowledge` command. State the resulting priority or plan change.
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
  same Task Session through review and merge.
- When a separate Task depends on an open parent PR and should begin now, start
  it with `lf task run <child> --stack-on <parent> --directive "..."`. The child
  keeps its own worktree and worker; never create a second simultaneously open
  PR inside the parent Task.
- The Project Session owns no worktree or PR branch. Never edit, commit,
  test, or open a PR from the canonical main checkout; delegate every
  repository mutation to a Task Session.
- Use `lf task follow-up`, `steer`, `interrupt`, `wait`, and `resume`. Do not
  create another worktree or session for review feedback or CI repair.
- Answer routine Task decisions with `lf task decide`. When the choice needs
  Wave judgment, call `lf project request-decision <project-id> <prompt>
  --option <choice> --option <choice> --wait`, then continue the same Project
  and Task transcripts from the answer.
- Never start another Project or Wave from Project pursuit, and never collapse
  the remaining Project into one anonymous task.
- Discovered debt becomes a task under an existing KR unless it
  reveals a broader standing quality frontier. Do not turn individual cleanup
  into a project-shaped KR.

Do not check off a KR unless its observable condition is already true.
