# prune step + Rust improvements

## What to build

A new `lf prune` step that acts as an aggressive backstop for `lf ops wt prune`. The agent examines every worktree, investigates what happened to it, and removes the ones that are clearly done. It also flags worktrees that need human attention (dirty-after-landing).

Alongside the step, improve the deterministic Rust prune logic to catch more cases natively — so the agent has less to do over time.

## Two deliverables

### 1. Rust improvements to `wt prune` and `wt list`

**New prunable signals (in `list_worktrees`):**

- **Remote branch deleted.** `git ls-remote --heads origin <branch>` returns nothing — someone deleted the remote branch. If the local branch has no unpushed commits beyond main, it's prunable.

- **Stale empty worktrees.** Branch has no commits beyond default AND no dirty files. Currently caught by `!has_commits`, but not surfaced clearly in `wt list`.

**New status in `wt list`:**

- **`landed-dirty`** — branch is merged/squash-merged/PR-merged, but worktree has uncommitted changes. Show with red marker. These are NOT auto-removed by `wt prune`, but the user needs to see them.

Currently `wt list` shows `merged` and `dirty` as independent flags. A worktree can be both merged and dirty, but the display doesn't call that out as a special state worth fixing.

**Changes to `WorktreeState`:**

```rust
pub struct WorktreeState {
    pub branch: Option<String>,
    pub path: PathBuf,
    pub base_branch: Option<String>,
    pub merged: bool,
    pub prunable: bool,
    pub dirty: bool,           // NEW: track dirtiness in the struct
    pub remote_gone: bool,     // NEW: remote branch no longer exists
}
```

Adding `dirty` and `remote_gone` to the struct lets the step (and `wt list`) make better decisions without re-running git commands.

**Changes to `wt_prune`:**

- Include `remote_gone && !dirty` worktrees as prunable (branch was deleted upstream, nothing to save).
- `wt prune --force` now also removes remote-gone clean worktrees.
- `wt prune` dry-run output groups by reason: "merged", "remote-gone", "empty".

**Changes to `wt_list`:**

- Show `landed-dirty` status (red) when `merged && dirty`. This replaces the separate `merged` + `dirty` flags for this case.
- Show `remote-gone` status (yellow) when remote branch is deleted but worktree is still around.

### 2. `prune` step (`.lf/steps/prune.md`)

The agent step runs AFTER the deterministic prune. It's the backstop — catches things the Rust code doesn't handle yet and acts on them.

**Workflow:**

1. Run `lf ops wt list --format json` to get structured worktree state
2. Run `lf ops wt prune` (dry run) to see what deterministic code catches
3. Run `lf ops wt prune --force` to remove those
4. For remaining non-main worktrees, investigate each:
   - `gh pr list --head <branch>` — any PR? What state?
   - `git log --oneline -5 <branch>` — recent activity?
   - `git log origin/main --grep="<branch>" --oneline` — commit messages referencing this branch?
   - Check if the work appears in main under different SHAs
5. Remove worktrees that are clearly done (PR merged/closed, work in main)
6. Flag `landed-dirty` worktrees with clear instructions
7. Report what it found and what it did

**Tone:** Aggressive. The step warns at the top that it will delete worktrees. It's a cleanup tool, not a safety tool.

**Output:** Direct action (removes worktrees) + summary of what happened and what needs manual attention.

## Constraints

- `dirty` worktrees are NEVER auto-removed — by the Rust code or the agent. They get flagged.
- The agent should not delete the current worktree or main.
- Remote-gone detection requires network access (ls-remote). If offline, skip gracefully.
- The step should work from any worktree (runs commands against main repo root).

## Done when

1. `lf ops wt list` shows `landed-dirty` and `remote-gone` states
2. `lf ops wt prune --force` removes remote-gone clean worktrees
3. `.lf/steps/prune.md` exists and runs
4. `cargo test --all` passes
5. `cargo clippy -- -D warnings` passes
