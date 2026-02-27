# 03: Cross-Repo File Tracking and Commits

## What to build

When a session modifies files across multiple repos (parent + children), detect which repo each file belongs to and produce separate commits per repo.

## Key functions

- `classify_files(changed_paths, repo_roots) -> HashMap<RepoRoot, Vec<Path>>` — Given changed file paths and known repo roots (parent + children), group files by repo.
- Extend commit logic to iterate over repos and commit in each.

## Behavior

1. After a session produces changes, lf scans all modified files.
2. For each file, determine which repo root it falls under (parent or a child).
3. Stage and commit in each repo separately.
4. Commit messages can be shared or per-repo (start with shared, refine later).

## Constraints

- A file belongs to exactly one repo (the most specific repo root that contains it).
- If a commit in one repo succeeds but another fails, don't roll back the first — just report the failure.
- Worktree paths need to resolve correctly when determining repo ownership.

## Done when

- A session that modifies files in both parent and child produces separate commits in each repo
- Single-repo sessions are unaffected
