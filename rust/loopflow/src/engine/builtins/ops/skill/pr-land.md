---
requires: code on branch
produces: landed PR
---
Land the current branch. Prepare the exact head, watch CI, repair, and finish merged.

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

## API

`lf pr land` stages uncommitted changes, rebases, creates or updates the PR,
requests exact-head auto-merge, and watches GitHub. Failing required checks get
one bounded `ci-fix` repair per failed head; a material repair is published and
re-armed. The command returns only after merge or an actionable durable block.
In a Task worktree, bare land settles one PR and keeps the Task open.

Use `lf pr arm` for the one-shot prepare/request/return operation.

```
lf pr land [--complete|-c] [--next <slug>] [-m "commit message"] [--title "..."] [--body "..."]
```

**Do not run git commit, git push, gh pr create, or gh pr merge directly.**
`lf pr land` owns those mutations and the watched repair loop.

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
lf pr land
```

Inside a Task, choose the disposition explicitly:

```bash
lf pr land --next released-upgrade-proof  # another serial PR follows
lf pr land -c                             # this merge completes the Task
```

Do not combine `--next` and `--complete`. Task completion or rotation happens
only after GitHub authoritatively reports the merge.

If you wrote title/body manually, include them:

```bash
lf pr land --title "<title>" --body "<body>"
```

Include `-m "<message>"` if the working tree was dirty in step 1.

If `lf pr land` fails due to rebase conflicts, launch a sub-agent to run the
`rebase-conflicts` skill, then retry `lf pr land`.

## Notes

- If the PR already has a good title and body, run `lf pr land` without `--title`/`--body` to keep existing content.
- In a Task's `finally` phase, `-c` completes directly over already-merged work
  when lifecycle rotation left a provably empty unpublished successor. It does
  not create an empty PR; earlier phases keep the ordinary empty-range refusal.

## Adaptation

If you discovered repo-specific landing conventions — merge strategy, branch
protection rules, CI wait behavior, cleanup steps — encode them. Most belong in
repo docs where all skills benefit. Copy this skill to `.lf/skills/pr-land.md`
when the repo needs landing to work differently.
