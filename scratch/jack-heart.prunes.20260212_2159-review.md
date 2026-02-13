# wt: detect squash merges and polish list output

## What was implemented

Three changes bundled in one branch:

1. **Squash-merge detection** — `lf wt list` and `lf wt prune` now detect branches that were squash-merged into main (not just fast-forward/rebase merges). Uses `git merge-tree --write-tree` to simulate the merge and compare resulting trees.

2. **Merged PR detection** — A single batched GitHub GraphQL call checks whether any worktree branches have merged PRs. Runs in parallel with the squash-merge check.

3. **Polished `wt list` output** — Aligned columns, color-coded status (merged/active/dirty), diff stats (+N -M (K files)), current worktree marker (`*`), short names instead of full paths.

4. **Stream newline normalization** — `render_event` for `Text` events now always appends a trailing newline. The executor trims it back before logging, keeping log lines clean.

5. **`wt prune` fetches before checking** — Fetches `origin/<default>` before listing worktrees so squash-merge detection has up-to-date refs.

## Key choices

**`merge-tree --write-tree` for squash detection** — Compares the tree that *would* result from merging the branch into target against target's actual tree. If identical, the branch contributes nothing new. This works regardless of how the merge happened (squash, rebase, cherry-pick). Alternative: `git cherry` or commit-message matching — both fragile.

**Batched GraphQL for PR checks** — One query with aliased fields per branch instead of N sequential `gh pr list` calls. Gracefully degrades (returns empty set) if `gh` isn't installed or auth fails.

**String-scan JSON parsing** — `merged_pr_branches` scans stdout for `"headRefName":"<branch>"` instead of pulling in a JSON parser. Acceptable because the response structure is fixed and the function already degrades gracefully on any parse failure.

**Parallel merge checking** — Squash-merge checks (thread-per-branch) and PR checks (single GraphQL call) run concurrently via `thread::spawn`. Results merge into `HashSet`s joined before the main loop.

## How it fits together

`list_worktrees` is the core: it collects branches, fans out squash-merge and PR checks in parallel, then assembles `WorktreeState` with the merged flag set from any of: `is_ancestor` (fast-forward), `is_squash_merged` (tree comparison), or `merged_pr_branches` (GitHub API). Both `wt list` and `wt prune` consume this.

`wt list` builds a `Row` per worktree with display info (short name, dirty flag, diff stat) and renders with column alignment. `wt prune` now fetches before listing so refs are current.

## Risks and bottlenecks

- **GraphQL rate limits** — One call per `list_worktrees` invocation. With many worktrees this is fine (single query), but rapid repeated calls (scripts, watch loops) could hit GitHub rate limits.
- **Thread-per-branch** — Squash-merge check spawns one thread per non-default branch. Fine for typical worktree counts (<20), but unbounded.
- **`gh` dependency** — PR detection silently fails without `gh` CLI. Users without it only lose PR-based merge detection; squash-merge and ancestor checks still work.
- **GraphQL string scanning** — Brittle if GitHub changes JSON formatting (e.g., adds spaces after colons). Fixed during gate: branch names with quotes are now escaped.

## What's not included

- No tests for `is_squash_merged`, `merged_pr_branches`, or `wt_diff_stat` — these shell out to git/gh and would need integration test infrastructure.
- No batching/pooling for squash-merge threads.
- No caching of merge status across invocations.
