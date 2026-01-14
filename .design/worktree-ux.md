# Worktree UX

Split loopflow into focused CLIs (`lfwt`, `lfpr`, `lfops`) and add worktree helper functions.

## Review

**Verdict:** Needs work

### Issues in new code

1. **`get_pr_state` is unused in `list_all`.** The function exists (worktrees.py:66) but `list_all` extracts PR state from the `wt list --format json` output instead of calling it. Either use the function to fill gaps when `ci.state` is missing, or delete it since `wt` already provides this.

2. **Flag collision in `lfpr commit` design.** The design doc shows `-a/-A` for both `--add/--no-add` and `--auto`. Pick one—suggest `-A` for `--no-add` and reserve `-a` for `--auto` since that's the convention elsewhere in loopflow.

3. **`_get_diff_for_target` duplicates `diff_against`.** The lfwt.py function (line 58) reimplements branch diff logic that `diff_against` in worktrees.py already handles. Consolidate or clarify why both exist.

4. **Temp file cleanup missing.** `_open_in_ide` writes `.diff-temp.diff` and `.diff-{target}.diff` files but never removes them. These accumulate in the repo root.

### Style notes

- Imports are at top of file
- Private functions prefixed with `_`
- No `Args:`/`Returns:` docstrings
- No backwards-compatibility shims

## Design notes

**User intent:** "lfpr - landing and github integration. lfwt - wt functionality above wt. lfd - background agents and monitoring. lf - prompting cli only"

**Constraints:**
- `lfwt` delegates to `wt` for create/remove/switch—only adds list/diff/compare
- PR state via `gh pr view --json state`, no GitHub API library
- Default commands: `lfwt` → list, `lfpr` → view, `lfops` → doctor
- No inter-CLI dependencies (each standalone)

**Not implemented:** `lfpr`, `lfops`, Maestro Swift changes, entry points in pyproject.toml. The design doc covers these but only `lfwt` and worktrees.py extensions exist in the diff.
