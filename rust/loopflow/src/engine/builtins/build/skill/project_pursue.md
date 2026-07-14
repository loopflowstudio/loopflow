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

## Work

- Acknowledge the seed's current directive before pursuit with its exact `lf
  project acknowledge` command. State the resulting priority or plan change.
- Read the filed backlog before creating work. File a concrete task when the
  KR needs it; no rule requires every filed task to start immediately.
- Every file-writing task must already have a Linear identity. Start it with
  `lf task run <issue-id> --directive "<delegation brief>"` and supervise the
  same Task Session through review and merge.
- The Project Session owns no worktree or delivery branch. Never edit, commit,
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
