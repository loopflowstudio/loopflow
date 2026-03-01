# 02: Cross-Repo File Tracking and Commits

**Finish line:** Sessions that modify files across multiple repos detect which repo each file belongs to and produce separate commits per repo.

## What to build

When a session modifies files across multiple repos, detect which repo each file belongs to and produce separate commits per repo.

## Key insight

lf already doesn't prevent writing to other repos — if an area points at `/other/repo/src/`, the agent can edit files there. The missing piece is commit handling: today's commit logic assumes all changed files belong to one repo.

This stage makes commits multi-repo-aware. No new access control — lf takes paths, lfd provides convenience, neither enforces write boundaries.

## Context from prior sprints

`resolve_related_repos(store, repo_id)` returns `Vec<RelatedRepoContext>` with `repo_id: RepoId` and `path: PathBuf` for each related repo. This provides the repo roots needed for file classification.

## Key functions

- `classify_files(changed_paths, repo_roots) -> HashMap<RepoRoot, Vec<Path>>` — Group changed files by repo root (most specific root wins). Repo roots come from `RelatedRepoContext.path`.
- Extend `commit_workflow` to iterate over repos and commit in each.

## Constraints

- A file belongs to exactly one repo (most specific repo root).
- No rollback across repos — if one commit succeeds and another fails, report the failure.
- Worktree paths resolve correctly when determining repo ownership.
- Single-repo sessions are completely unaffected.

## Done when

- A session modifying files across multiple repos produces separate commits in each
- Single-repo sessions behave identically to today
