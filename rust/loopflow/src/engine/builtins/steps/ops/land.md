---
requires: code on branch
produces: landed PR + rotated worktree
---
Land the current branch. Write PR content, then call `lf ops land` to rebase, create/update the PR, and enable auto-merge.

## API

`lf ops land` handles the entire mechanical workflow: staging uncommitted changes, rebasing, creating or updating the PR, and enabling auto-merge. Your job is to understand the branch and write the PR content.

```
lf ops land --title "..." --body "..." [--create-pr] [-m "commit message"]
```

**Do not run git commit, git push, gh pr create, or gh pr merge directly.** `lf ops land` does all of this. Running those commands manually means auto-merge never gets enabled.

## Workflow

### 1. Understand the branch

```bash
git log origin/main..HEAD --oneline
git diff origin/main...HEAD --stat
```

Check `scratch/` for context. If `scratch/<branch>-review.md` exists (from gate), use it to understand the change and inform the PR body.

Stage and include all changes — committed and uncommitted — in the PR. If the working tree is dirty, compose a commit message for the uncommitted changes (pass it as `-m` in step 3). Never ask which files to include; everything on the branch ships together.

### 2. Write PR title and body

**Title:** lowercase, concise, area prefix when focused.

Examples:
- `mobile: quote replies on iOS`
- `cost: add analytics dashboard with timeseries API`
- `fix worktree cleanup on branch delete`

**Body:** markdown. Structure:

1. **Usage** — code block showing how to try it or see it in action
2. **Summary** — one paragraph on what changed and why
3. **Changes** — optional bullet list for larger PRs

### 3. Land

```bash
lf ops land --title "<title>" --body "<body>" --create-pr
```

Include `-m "<message>"` if the working tree was dirty in step 1.

If `lf ops land` fails due to rebase conflicts, launch a sub-agent to run the `rebase` step, then retry `lf ops land`.

## Notes

- If the PR already has a good title and body, run `lf ops land` without `--title`/`--body` to keep existing content.

## Adaptation

If you discovered repo-specific landing conventions — merge strategy, branch protection rules, CI wait behavior, cleanup steps — copy this step to `.lf/steps/land.md` and encode them.
