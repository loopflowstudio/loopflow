# Worktree UX

Split loopflow into focused CLIs (`lfwt`, `lfpr`, `lfops`) and add worktree helper functions.

## Implementation

### New CLIs

- **lfwt**: Worktree operations (`list`, `diff`, `compare`, `cd`)
- **lfpr**: PR and landing operations (`create`, `view`, `update`, `land`, `commit`)
- **lfops**: Meta operations (`init`, `install`, `doctor`, `version`, `status`, `stop`, `prune`)

### Key changes

1. Removed `ops` subcommand from `lf` - operations now in separate `lfops` CLI
2. Added PR state tracking to worktree models (Python and Swift)
3. Added `diff_against`, `diff_between`, `get_github_compare_url`, `get_pr_state` to worktrees.py
4. Swift UI gains Create PR and Land PR actions

### Design decisions

**`_get_diff_for_target` vs `diff_against`**: The `_get_diff_for_target` function in lfwt.py handles additional complexity needed by the `compare` command: it first looks up worktree paths and runs the diff inside the worktree directory (for accurate HEAD resolution), then falls back to branch refs. `diff_against` is simpler and operates purely on branch names.

**Temp files**: Changed to use `tempfile.mkstemp` with descriptive suffixes. Files are written to system temp directory and cleaned up by the OS.

**Flag semantics**: `lfpr commit -a` means `--add` (stage changes), not `--auto` mode. This is intentional since `lfpr` commands don't have auto/interactive modes.

## Design notes

**User intent:** "lfpr - landing and github integration. lfwt - wt functionality above wt. lfd - background agents and monitoring. lf - prompting cli only"

**Constraints:**
- `lfwt` delegates to `wt` for create/remove/switch--only adds list/diff/compare
- PR state via `gh pr view --json state`, no GitHub API library
- Default commands: `lfwt` -> list, `lfpr` -> view, `lfops` -> doctor
- No inter-CLI dependencies (each standalone)
