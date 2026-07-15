# Open items — W2-93 PR1

## Blocker (external): GitHub GraphQL rate limit

`lf pr open` is blocked by `GraphQL: API rate limit already exceeded for user ID
37011` — the token is saturated (many concurrent wave agents). REST resets did
not clear it, so it is the hourly GraphQL window.

State when this was hit:
- All work is committed and **pushed** to `origin/jack-heart/deliver-stacked-tasks-after-parent`.
- The PR is **not yet created**. A background poller (`gh api rate_limit` →
  `lf pr open`) is retrying hands-off; if it also lapses, open the PR manually
  once the limit clears. Title/body are in `scratch/pr-body.md`.
- `lf pr open` left one cosmetic commit ("lf pr open: prepare branch") carrying
  `scratch/pr-body.md`; scratch is removed on land, so it is harmless.

## Assumption

Interpreted the W2-93 "stack" as a Task's serial `TaskPr` chain (one worktree,
concurrent open PRs), not cross-Task worktree stacking — see the design note's
Exclusions. Reversible; flag in review if cross-Task stacking was intended.
