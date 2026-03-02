---
requires: code on branch
produces: landed PR + rotated worktree
---
Land the current branch. Write PR content, rebase, and merge.

## Goal

Get this branch merged. Write a PR title and body that help reviewers understand the change, rebase onto main, and submit to the merge queue.

## API

```
lf ops land --title "..." --body "..." [-m "commit message"] [--create-pr] [--local]
lf ops rebase
lf ops commit -m "message"
```

## Workflow

### 1. Understand the branch

```bash
git log origin/main..HEAD --oneline
git diff origin/main...HEAD --stat
```

Check `scratch/` for context. If `scratch/<branch>-review.md` exists (from gate), use it to understand the change and inform the PR body.

### 2. Commit uncommitted changes

```bash
git status
```

If the working tree is dirty, write a clear commit message and commit:

```bash
git add -A
git commit -m "descriptive message"
```

### 3. Rebase onto main

```bash
lf ops rebase
```

If rebase fails with conflicts, spawn a subagent to resolve them. The subagent should:
- Read conflicting files with `git status`
- For files central to this branch: keep the branch's changes
- For files outside the branch's scope: accept main's version
- Run `git add <file>` then `git rebase --continue`
- Push with `git push --force-with-lease`

If conflicts are too complex, abort with `git rebase --abort` and note the issue in `scratch/questions.md`.

### 4. Write PR title and body

**Title:** lowercase, concise, area prefix when focused.

Examples:
- `mobile: quote replies on iOS`
- `cost: add analytics dashboard with timeseries API`
- `fix worktree cleanup on branch delete`

**Body:** markdown. Structure:

1. **Usage** — code block showing how to try it or see it in action
2. **Summary** — one paragraph on what changed and why
3. **Changes** — optional bullet list for larger PRs

### 5. Land

```bash
lf ops land --title "<title>" --body "<body>" --create-pr
```

Include `-m "<message>"` if you committed changes in step 2.

If lint or CI failures prevent landing, fix them and retry.

## Notes

- Do not ask questions. Make decisions and proceed.
- If the PR already has a good title and body, you may run `lf ops land` without `--title`/`--body` to keep the existing content.

## Adaptation

If you discovered repo-specific landing conventions — merge strategy, branch protection rules, CI wait behavior, cleanup steps — copy this step to `.lf/steps/land.md` and encode them.
