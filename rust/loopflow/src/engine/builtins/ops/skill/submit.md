---
requires: code on branch
produces: PR ready, assigned to a human to merge
---
Submit the current branch for a human to land. Rebase, clear scratch, create/update the PR, mark it ready, and assign it — then stop. Nothing merges until a human clicks merge.

Use `submit` (not `land`) whenever a person should land the work by hand. `land` is for headless/auto runs where loopflow merges hands-off; `submit` leaves the one required merge click to a human. (GitHub blocks approving your own PR, so the gate is the merge click, not a review approval — the button unlocks once checks pass.)

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- If a `wave/<name>/` directory matches this work, skim its `GOAL.md`, `MEMORY.md`, `projects/`, and live tasks (`lf pm show --wave <name>`).
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## API

`lf pr submit` handles the entire mechanical workflow: staging uncommitted changes, rebasing, creating or updating the PR, marking it ready, and assigning it to the human who will merge. It does **not** arm auto-merge.

```
lf pr submit [--create-pr] [-m "commit message"] [--title "..."] [--body "..."]
```

**Do not run git commit, git push, gh pr create, or gh pr ready directly.** `lf pr submit` does all of this. Running those commands manually skips the assignment and leaves the PR in an inconsistent state.

## Workflow

### 1. Understand the branch

```bash
git log origin/main..HEAD --oneline
git diff origin/main...HEAD --stat
```

Check `scratch/` for context. If `scratch/<branch>-review.md` exists (from gate), use it to understand the change and inform the PR body.

Stage and include all changes — committed and uncommitted — in the PR. If the working tree is dirty, compose a commit message for the uncommitted changes (pass it as `-m` in step 3). Never ask which files to include; everything on the branch ships together.

### 2. Prepare PR copy

If `scratch/pr-title.txt`, `scratch/pr-body.md`, and `scratch/.pr-copy-ref` exist (from `lf gate`), `lf pr submit` reuses them automatically.

If those files are missing or stale, write title/body manually and pass `--title` + `--body`.

**Title guidelines:** lowercase, concise, area prefix when focused.

Examples:
- `mobile: quote replies on iOS`
- `cost: add analytics dashboard with timeseries API`
- `fix worktree cleanup on branch delete`

**Body:** markdown. Structure:

1. **Usage** — code block showing how to try it or see it in action
2. **Summary** — one paragraph on what changed and why
3. **Changes** — optional bullet list for larger PRs

### 3. Submit

```bash
lf pr submit --create-pr
```

If you wrote title/body manually, include them:

```bash
lf pr submit --create-pr --title "<title>" --body "<body>"
```

Include `-m "<message>"` if the working tree was dirty in step 1.

If `lf pr submit` fails due to rebase conflicts, launch a sub-agent to run the `rebase` step, then retry `lf pr submit`.

## Notes

- The PR is left ready and assigned — the assignee's merge click lands it. Don't enable auto-merge or merge on their behalf.
- If the PR already has a good title and body, run `lf pr submit` without `--title`/`--body` to keep existing content.

## Adaptation

If you discovered repo-specific submit conventions — assignee, branch protection rules, CI wait behavior — encode them. Most belong in repo docs where all steps benefit. Copy this step to `.lf/steps/submit.md` when the repo needs submit to work differently.
