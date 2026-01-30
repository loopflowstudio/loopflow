# Design Review: Merged Branch Detection in `lf ops next`

## What was implemented

`lf ops next` now detects when a branch is already merged and starts fresh from `origin/main` instead of creating a stacked branch.

### Detection logic

When `lf ops next` runs, it checks:
1. If there's a PR, get its state (OPEN, MERGED, CLOSED)
2. If PR state is MERGED → already merged
3. If no PR exists, fetch `origin/main` and check if branch commits are ancestors of main

### Behavior change

| Scenario | Before | After |
|----------|--------|-------|
| Open PR | Enable auto-merge, stack from HEAD | Same |
| PR merged | Would fail (no PR found) | Fresh start from main |
| No PR, commits in main | Would fail | Fresh start from main |
| No PR, commits not in main | Would fail | Fail (or create PR with `--create-pr`) |

### Branch naming fix

`parse_branch_base()` now handles nested timestamps recursively:
```
foo.20260129_2255.20260129_2318.aurora-rondo → foo
```

This happens when a wave iterates multiple times, accumulating timestamp suffixes.

## Key choices

| Decision | Why |
|----------|-----|
| Check `git merge-base --is-ancestor` | Detects squash-merged commits even without PR |
| Recursive timestamp stripping | Wave branches can accumulate multiple suffixes |
| Update wave state on fresh start | Keeps wave metadata consistent with actual branch |
| Fetch before checking | Ensures we compare against latest main |

## How it fits together

```
lf ops next
    │
    ├─► Check PR state
    │       │
    │       ├─► MERGED → fresh start
    │       │
    │       └─► No PR
    │               │
    │               ├─► Commits in main → fresh start
    │               │
    │               └─► Not in main → create PR or fail
    │
    └─► Fresh start path
            │
            ├─► Checkout origin/main (detached)
            ├─► Generate new branch name
            ├─► Create and push branch
            └─► Update wave if exists
```

## Risks and bottlenecks

**Network dependency** — Requires `git fetch` to detect merged state. Offline use will fail.

**Squash merge detection** — The `--is-ancestor` check works for squash merges only if the original branch commits are ancestors (which they are after a clean squash).

**Wave update failure** — If the daemon isn't running, wave update is skipped silently. The user must manually sync wave state.

## What's not included

- Automatic PR cleanup (closing old PRs)
- Detection of rebased branches (not squash-merged)
- Notification when starting fresh vs stacking

## Test coverage

| Test | What it verifies |
|------|------------------|
| `test_parse_branch_base_strips_trailing_timestamp` | Handles `.timestamp` suffix |
| `test_parse_branch_base_nested_timestamps` | Handles double timestamps |
| `test_next_fails_without_pr` | Fails correctly when not merged and no PR |

## Files changed

| File | Change |
|------|--------|
| `src/loopflow/lfops/next.py` | Added merged detection, fresh start path |
| `src/loopflow/lf/naming.py` | Recursive timestamp stripping |
| `tests/test_naming.py` | New tests for nested timestamps |
| `tests/test_next.py` | Updated test for merged detection |
