# Review: dirty + remote_gone in WorktreeState

## What was implemented

Added `dirty: bool` and `remote_gone: bool` to `WorktreeState`, populated in `list_worktrees()` so all consumers get the data for free. Three files changed:

- **`engine/worktrees.rs`** — Added `list_remote_branches()` (single `ls-remote --heads origin` call), added `dirty`/`remote_gone` fields, updated prunable logic to include `remote_gone && !dirty`.
- **`lf/commands/ops/mod.rs`** — `wt_list` shows `landed-dirty` (red) and `remote-gone` (yellow) states. `wt_prune` groups dry-run output by reason (merged/remote-gone/empty). `wt_remove` uses `wt.dirty` instead of re-calling `is_clean`. Removed unused `is_clean` import.
- **`lf/output.rs`** — Added `red` to `Colors` struct.

Also: design doc (`scratch/01-rust-improvements.md`), wave plan (`wave/prune/`).

## Key choices

- **Bools over enum.** `dirty` and `remote_gone` compose with existing `merged` and `prunable`. An enum would need combinatorial variants.

- **Single `ls-remote` over per-branch calls.** One network round-trip. Offline returns empty set — all `remote_gone = false`, no false positives.

- **`remote_gone && !dirty` is prunable.** Aggressive. If the remote branch was deleted, the work was either merged (detection missed it) or abandoned. Committed work survives in reflog. The wave item originally said `!has_unpushed_beyond_main` but the design doc drops that guard.

- **`landed-dirty` is display-only.** Computed from `merged && dirty` in the view layer. The struct stores raw bools for JSON consumers and the future agent step.

- **`dirty` always computed.** It's a local `git status --porcelain` (fast), both `wt_list` and `wt_prune` need it, and JSON output should include it.

## How it fits together

`list_worktrees()` is the single source of truth. Three parallel threads:
1. Squash-merge checks (per-branch threads) — existing
2. PR merged check (single GraphQL call) — existing
3. Remote branch listing (single `ls-remote` call) — new

Dirty checks run in the build loop (local, fast, not worth threading).

`wt_list` and `wt_prune` consume the struct without re-running git commands. `wt_remove` also benefits — uses `wt.dirty` instead of a separate `is_clean` call.

Status priority in `wt_list`: `landed-dirty` > `merged` > `remote-gone` > `fresh` > `active`. A worktree that is both `remote_gone` and `merged` shows as `merged` (the merge status is more informative).

## Risks and bottlenecks

- **`ls-remote` latency.** One network call added to `list_worktrees()`. Mitigated by running in parallel with existing network calls. Offline → empty set → safe degradation.

- **Aggressive prune of remote-gone.** Could prune worktrees with unpushed commits if someone else deleted the remote branch. Mitigated by: never touching dirty worktrees, reflog retention, and future agent step (sprint 02) investigating before acting.

- **`git status --porcelain` per worktree.** N local git commands for N worktrees. Fast but linear. Not worth threading.

## What's not included

- The prune agent step (sprint 02, depends on this work)
- No new tests for `list_remote_branches` or the display changes — these are integration-level behaviors tested manually
- `wt_remove()` logic unchanged beyond using `wt.dirty`

## Wave alignment

- Advances all wave goals: new states in `wt list`, remote-gone pruning, JSON output for agent step
- All three wave risks (false positive, network, squash-merge) addressed
- Sprint 02 can now consume `wt list --format json` with `dirty` and `remote_gone` fields
