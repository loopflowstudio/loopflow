# 02: Prune Step

**Finish line:** `.lf/steps/prune.md` exists and successfully cleans up done worktrees when run with `lf prune`.

## What to build

An agent step that runs after deterministic `wt prune`. The backstop — catches what Rust doesn't handle yet.

### What sprint 01 shipped

`wt list --format json` now includes `dirty` (bool) and `remote_gone` (bool) on every `WorktreeState`. `list_worktrees()` computes these once via three parallel threads (squash checks, PR-merged checks, `ls-remote`). `wt prune --force` removes remote-gone clean worktrees alongside merged ones. `wt list` shows `landed-dirty` (red) and `remote-gone` (yellow) as first-class states. The prunable predicate: `!is_default && (merged || !has_commits || (remote_gone && !dirty))`.

The step can rely on all of this. No Rust changes needed.

### Workflow the step follows

1. `lf ops wt list --format json` — structured worktree state
2. `lf ops wt prune --force` — remove merged, remote-gone (clean), and empty
3. `lf ops wt list --format json` — re-read state after deterministic prune
4. Flag `landed-dirty` worktrees (`merged && dirty`) with clear instructions
5. For remaining non-main worktrees, investigate via `gh pr list --head <branch>`
6. Remove clearly-done worktrees (`lf ops wt remove <name>`) — PR merged/closed, clean
7. Print summary: what Rust pruned, what the agent pruned, what needs human attention

### Key decisions

**PR state is the primary agent signal.** `gh pr list --head <branch>` tells you definitively whether work landed. Commit-based heuristics (grepping main for branch names) are a fallback, not the primary path.

**Never remove dirty worktrees.** The invariant is absolute. Dirty worktrees get flagged with instructions, never deleted. This is what makes aggressive cleanup safe.

**Run from any worktree.** The step resolves the main repo root first. It never removes the worktree it's running from.

**Aggressive tone.** The step warns up front that it will delete worktrees. No hedging. Cleanup tool, not safety tool.

**Degrade gracefully offline.** If `gh` fails or `ls-remote` is unavailable, skip PR checks and report what couldn't be verified.

### Step format

Follows PROMPT_STYLE. Frontmatter with `requires:` and `produces:`. Headless-compatible.

## Done when

1. `.lf/steps/prune.md` exists with proper frontmatter
2. Step runs successfully from any worktree
3. Step never removes dirty worktrees or main
4. Step reports what it did and what needs manual attention
