# Worktree UX

Split loopflow into focused CLIs (`lfwt`, `lfpr`, `lfops`) and add worktree helper functions.

## Review

**Verdict:** Ready to ship

Clean split of the monolithic `lf ops` into three focused CLIs. The code follows style guidelines, imports are at top of files, private functions are prefixed with `_`, and no unnecessary docstrings. Tests were updated to reflect the new structure.

Minor observations (not blockers):

1. **Unused fd variable** in lfwt.py:183 and :214 - `tempfile.mkstemp` returns `(fd, path)` but `fd` is assigned and never closed. The file handle leaks, though the OS cleans up temp files anyway. Could use `os.close(fd)` after writing or switch to `tempfile.NamedTemporaryFile(delete=False)`.

2. **`get_pr_state` defined but unused** - Added to worktrees.py:66 but never called. The PR state is instead read from the `wt list --format json` output via the `ci.state` field. The function works but is dead code.

## Design notes

**CLI split rationale:** User intent was "lfpr - landing and github integration. lfwt - wt functionality above wt. lf - prompting cli only". This implementation delivers on that.

**`_get_diff_for_target` vs `diff_against`:** The `_get_diff_for_target` function in lfwt.py handles worktree path lookup and runs diff inside the worktree directory (for accurate HEAD resolution), then falls back to branch refs. `diff_against` is simpler and operates purely on branch names. Both are needed.

**Default commands:** `lfwt` -> list, `lfpr` -> view, `lfops` -> doctor. Sensible defaults that show useful info without requiring args.
