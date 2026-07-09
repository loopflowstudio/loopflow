---
requires: code on branch
produces: landed PR
---
Land the current branch. Rebase, create/update the PR, and enable auto-merge.

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

`lf pr land` handles the entire mechanical workflow: staging uncommitted changes, rebasing, creating or updating the PR, and enabling auto-merge.

```
lf pr land [--create-pr] [-m "commit message"] [--title "..."] [--body "..."]
```

**Do not run git commit, git push, gh pr create, or gh pr merge directly.** `lf pr land` does all of this. Running those commands manually means auto-merge never gets enabled.

## Workflow

### 1. Understand the branch

```bash
git log origin/main..HEAD --oneline
git diff origin/main...HEAD --stat
```

Check `scratch/` for context. If `scratch/<branch>-review.md` exists (from gate), use it to understand the change and inform the PR body.

Stage and include all changes — committed and uncommitted — in the PR. If the working tree is dirty, compose a commit message for the uncommitted changes (pass it as `-m` in step 3). Never ask which files to include; everything on the branch ships together.

### 2. Prepare PR copy

If `scratch/pr-title.txt`, `scratch/pr-body.md`, and `scratch/.pr-copy-ref` exist (from `lf gate`), `lf pr land` reuses them automatically.

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

### 3. Land

```bash
lf pr land --create-pr
```

If you wrote title/body manually, include them:

```bash
lf pr land --create-pr --title "<title>" --body "<body>"
```

Include `-m "<message>"` if the working tree was dirty in step 1.

If `lf pr land` fails due to rebase conflicts, launch a sub-agent to run the `rebase` step, then retry `lf pr land`.

## Notes

- If the PR already has a good title and body, run `lf pr land` without `--title`/`--body` to keep existing content.

## Adaptation

If you discovered repo-specific landing conventions — merge strategy, branch protection rules, CI wait behavior, cleanup steps — encode them. Most belong in repo docs where all steps benefit. Copy this step to `.lf/steps/land.md` when the repo needs land to work differently — a changed workflow, or team preferences about how landing happens.
