---
requires: current branch is not main
produces: scratch/<branch>.md
---
Refresh the branch plan so it matches post-rebase reality.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Make `scratch/<branch>.md` an accurate contract for the remaining work on this branch.

Branches sit while main moves. Reconcile the plan against the current diff, upstream changes, and any PR context before downstream steps implement or gate the branch.

## Workflow

1. Confirm the current branch is not `main` or `master`:

   ```bash
   git branch --show-current
   ```

   If it is a base branch, stop and write the reason to `scratch/questions.md`.

2. Rebase onto main:

   ```bash
   lf rebase
   ```

   Resolve trivial conflicts. For non-trivial conflicts, write the blocker and conflicted files to `scratch/questions.md` and stop.

3. Read the evidence:

   - Existing `scratch/<branch>.md`, if present
   - `git diff origin/main...HEAD`
   - `git log --oneline origin/main..HEAD`
   - PR body and comments when a PR exists
   - Upstream changes on main touching the same files or wave area

4. Refresh or synthesize the scratch doc:

   - Existing doc: strike through work already shipped elsewhere, remove stale assumptions, note upstream changes that invalidate the original plan, and restate what remains.
   - Missing doc: synthesize one from the branch name, diff, commit history, and PR body. Describe what the branch is doing based on evidence, not hope.

5. If running inside the ship flow, append the ship strategy block. Detect this with:

   ```bash
   echo "${LOOPFLOW_FLOW_NAME:-}"
   ```

   Append this block only when the value is `ship`:

   ```markdown
   ## Strategy: ship bias

   - Finish only what's trivial and in-scope for this branch
  - Defer anything non-trivial into Linear tasks, or `scratch/questions.md` if waveless
   - Prefer landing over comprehensive — a wave doc captures intent
   ```

## Output

Write `scratch/<branch>.md` with:

```markdown
# <branch>

## Current reality

<What this branch does now, post-rebase>

## Done

- <Evidence-backed completed work>
- ~~<Originally planned work now shipped elsewhere>~~

## Remaining

- <Small, merge-blocking work still in this branch>

## Deferred

- <Non-trivial work to move into Linear tasks, or into scratch/questions.md if waveless>

## Risks / blockers

- <Only real blockers. Empty if none.>
```

Keep it short and actionable. Downstream `implement` reads this literally.

## Guardrails

- Do not create new waves.
- Do not preserve stale plan text for history; git has history.
- Do not force a ship by deleting meaningful work. If nothing mergeable remains, write the blocker to `scratch/questions.md`.
- Waveless branches with non-trivial leftovers go to `scratch/questions.md`, not `wave/<slug>/`.
