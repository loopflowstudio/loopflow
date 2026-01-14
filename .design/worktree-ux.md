# Worktree UX

Split loopflow into focused CLIs (`lfwt`, `lfpr`, `lfops`) and add worktree helper functions.

## Review

**Verdict:** Needs work

### Issues

1. **`get_pr_state` is imported but unused.** lfwt.py imports `get_pr_state` from worktrees.py (line 18) but never calls it. The function exists for direct lookups, but `list_all` extracts PR state from `wt list --format json` instead. Either use `get_pr_state` to fill gaps when `ci.state` is missing, or remove the import.

2. **`_get_diff_for_target` duplicates `diff_against`.** lfwt.py:58 reimplements branch diff logic that `diff_against` in worktrees.py already handles. The lfwt version adds worktree path resolution and branch ref fallbacks, but the core diff command is the same. Consider extending `diff_against` or clarifying why both exist.

3. **Temp file cleanup missing.** `_open_in_ide` writes `.diff-temp.diff` and `.diff-{target}.diff` files but never removes them. These accumulate in the repo root. Either use `tempfile` or add cleanup logic.

4. **`lfpr commit` flag semantics.** `-a/-A` is `--add/--no-add` (stage before commit). This differs from other loopflow CLIs where `-a` means `--auto` mode. Not a bug, but worth noting if you want consistent flag conventions.

### Style compliance

- Imports at top of file
- Private functions prefixed with `_`
- No `Args:`/`Returns:` docstrings
- No backwards-compatibility shims

## Design notes

**User intent:** "lfpr - landing and github integration. lfwt - wt functionality above wt. lfd - background agents and monitoring. lf - prompting cli only"

**Constraints:**
- `lfwt` delegates to `wt` for create/remove/switch--only adds list/diff/compare
- PR state via `gh pr view --json state`, no GitHub API library
- Default commands: `lfwt` -> list, `lfpr` -> view, `lfops` -> doctor
- No inter-CLI dependencies (each standalone)
