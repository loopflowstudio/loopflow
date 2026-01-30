# Design Review: Merged Branch Detection in `lf ops next`

## What was implemented

`lf ops next` now detects when a branch is already merged and starts fresh from `origin/main` instead of failing or stacking on a merged branch.

**Two behaviors:**
1. **Open PR** — enables auto-merge, creates stacked branch from current HEAD (existing behavior)
2. **Already merged** — resets to `origin/main`, creates fresh branch, updates wave metadata

**Merged detection logic:**
- If PR exists and state is `MERGED` → already merged
- If no PR exists, checks if branch commits are ancestors of `origin/main` via `git merge-base --is-ancestor`

## Key choices

| Decision | Why |
|----------|-----|
| Check `is_ancestor` only when no PR | Avoids redundant git operations; PR state is authoritative when available |
| Update wave metadata on fresh start | Wave tracks current branch; stale branch causes UI confusion |
| Single `_fetch_main()` before merge check | Ensures `origin/main` is current without redundant fetches |
| `_fresh_start()` returns branch name or None | Consistent with other functions in module; caller handles errors |

## How it fits together

```
lf ops next
    │
    ├─ PR exists and MERGED? ──────────────────┐
    │                                          │
    ├─ PR exists and OPEN? ─► enable auto-merge, stack from HEAD
    │
    └─ No PR? ─► fetch main ─► is_ancestor? ───┤
                                               │
                         ┌─────────────────────┘
                         ▼
                   already_merged = true
                         │
                         ▼
                   _fresh_start()
                         │
                         ├─ checkout origin/main
                         ├─ create new branch
                         ├─ push with upstream
                         └─ update wave metadata
```

## Risks and bottlenecks

**Race condition on merge check** — If commits land on main between `fetch` and `is_ancestor` check, detection could be stale. Acceptable since fresh start is idempotent.

**Wave update assumes single wave per worktree** — `get_wave_by_worktree()` returns first match. Multiple waves pointing at same worktree would only update one.

**Redundant fetch when PR merged** — When PR state is MERGED, `_fetch_main()` is called in `_fresh_start`. When no PR and branch is merged, `_fetch_main()` is called twice (before `is_ancestor` check and again in `_fresh_start`). Impact is negligible since git fetch is fast for no-op.

## What's not included

- Detection of partially merged branches (some commits in main, some not)
- Interactive prompt to choose between stacking vs fresh start
- Remote branch cleanup for the old merged branch

## Test coverage

Tests added/updated:
- `test_next_starts_fresh_when_already_merged` — verifies fresh start path when PR is MERGED
- `test_next_fails_without_pr` — updated to mock `_fetch_main` and `_is_branch_merged`
- `test_next_creates_worktree_with_suffix` — updated to mock `_get_pr_state`

All paths through `next_worktree()` are now covered by tests.

## Files changed

| File | Change |
|------|--------|
| `src/loopflow/lfops/next.py` | Added merged detection logic, `_fetch_main`, `_is_branch_merged`, `_fresh_start` |
| `tests/test_next.py` | Added `test_next_starts_fresh_when_already_merged`, updated mocks for new code paths |
| `scratch/*.md` | Removed stale design docs |
