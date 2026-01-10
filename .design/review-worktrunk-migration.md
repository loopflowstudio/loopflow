# Review: Worktrunk Migration

## Summary

This branch migrates loopflow from a custom worktree management implementation to the external `worktrunk` CLI tool. The change removes ~400 lines of complex git operations code in favor of delegating to worktrunk, adds a new `lf land` command for local landing workflows, creates a `polish` task that combines review + test, and restructures the command hierarchy to remove the `lf wt` subcommand.

## Issues

### 1. Design artifacts path inconsistency

**Location:** Multiple files use both `.design/` and `<branch>.md` patterns

**Issue:** The diff shows:
- `land-workflow.md` says: "Remove design docs (.design/) contents if present"
- `pr-land-worktrunk.md` says: "Remove `<branch>.md` design doc"
- Old `review.md` mentions: "create (or update) a review guide at `<branch>.md`"
- New implementation in `design.py` uses `.design/` directory

The codebase has migrated from `<branch>.md` at repo root to `.design/*` subdirectory, but the exploration docs (`pr-land-worktrunk.md`) and the `lf land` workflow doc still reference the old pattern.

**Fix needed:** Update `pr-land-worktrunk.md` references from `<branch>.md` to `.design/` to match the implemented behavior.

### 2. Missing test file rename

**Location:** `tests/test_worktrunk.py` (renamed from `tests/test_git.py` according to git status)

**Issue:** Git shows `R tests/test_worktrunk.py -> tests/test_worktrees.py`, but there's still a `tests/test_worktrunk.py` file with tests that import from `loopflow.worktrunk` (which doesn't exist). The actual module is `loopflow.worktrees`.

**Fix needed:** The test file imports and module references are inconsistent. Either:
- The file should be `test_worktrees.py` and import from `loopflow.worktrees`, OR
- The module should be `loopflow.worktrunk.py` to match the external tool name

Current implementation uses `worktrees.py` module, so tests should import from `loopflow.worktrees`.

### 3. Compare command diff fallback logic

**Location:** `src/loopflow/cli/compare.py:74-95`

**Issue:** The `_find_worktree_path` function tries `list_all()` first, then falls back to `get_path()`. But if `list_all()` raises `WorktreeError`, it catches and continues. However, `get_path()` is just a path calculation—it doesn't verify the worktree exists. The function returns `wt_path` even if `wt_path.exists()` is False.

```python
wt_path = get_path(main_repo, name)
if wt_path.exists():
    return wt_path
return None  # ← Should be here but isn't
```

The current code returns `wt_path` whether it exists or not after the for-loop.

**Fix needed:** Add explicit None return when path doesn't exist.

### 4. Polish prompt duplication

**Location:** `.claude/commands/polish.md` and `src/loopflow/prompts/polish.md`

**Issue:** Both files have identical content (50 lines). The pattern in the codebase is:
- `.claude/commands/` = Claude Code integration (accessible via `/polish` in raw Claude sessions)
- `src/loopflow/prompts/` = Bundled with package for `lf meta init` installation

However, only `polish.md` was added to both locations—other tasks like `review`, `implement`, `design` all exist in both locations already, so this is consistent. Not an issue, just noting for completeness.

### 5. Test import error

**Location:** `tests/test_worktrees.py:13`

**Issue:** Tests import from `loopflow.worktrunk` but the actual module is `loopflow.worktrees`:

```python
from loopflow.worktrunk import (
    WorktrunkError,
    create_worktree,
    list_worktrees,
    remove_worktree,
)
```

Should be:
```python
from loopflow.worktrees import (
    WorktreeError,
    create,
    list_all,
    remove,
)
```

**Fix needed:** Update test imports to match actual module name and exported functions.

## Suggestions

### 1. Consider extracting compare diff logic

The `compare.py` module has complex branching logic for finding worktrees vs branches vs refs. Consider extracting `_get_diff_for_target()` into the `worktrees.py` module as a public function `get_diff(repo_root, name, base_ref)` to centralize the "find any git ref and get its diff" logic.

### 2. Document worktrunk requirement more prominently

The migration makes `worktrunk` a hard dependency for worktree operations. The README mentions it briefly ("delegates worktree management to worktrunk"), but users might not realize they need it until they try `lf` commands and get errors. Consider:
- Adding a "Requirements" section to README
- Making `lf meta doctor` check for `wt` and fail fast with a clear message

Actually, looking closer: `lf meta doctor` already checks for `wt` (line 238 in `cli/meta.py`), so this is fine.

### 3. Rename test file module references

File naming: `test_worktrees.py` matches the module name `worktrees.py` better than `test_worktrunk.py`. The git status shows the file was renamed, which is good. Just need to fix the imports as noted in Issues.

## Verdict

**Needs work** - Fix the test imports and compare command path handling before landing.

The core migration is solid: replacing 400+ lines of custom git operations with a mature external tool is a good trade-off. The new `lf land` command and `polish` task are well-designed additions. The main issues are:

1. Test imports reference wrong module name (breaking tests)
2. Compare command has a subtle path existence bug
3. Minor documentation inconsistencies in design docs

Once these are addressed, the branch is ready to ship.
