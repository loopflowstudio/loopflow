# lf ops parity: worktrees + next — review

## What was implemented
- Added Rust-first worktree management: create/switch/list/prune/ci plus shell auto-cd integration.
- Implemented worktree primitives in `loopflow-engine` (list, create with naming schema, preserve move).
- Updated `lf ops next` to preserve the current worktree and create a fresh branch/worktree.
- Added tests for config parsing, context assembly, and git/worktree behavior.
- Updated `docs/lfops.md` to match new worktree paths and shell integration behavior.

## Key choices
- Use `git worktree list --porcelain` as the source of truth and enrich with merge status.
- Keep worktree paths as sibling directories (`../<short-name>`) even when branch names are schema-expanded.
- Use a shell directive file + wrapper function to handle auto-cd without spawning a daemon.
- Skip pruning dirty worktrees by checking `git status --porcelain` per worktree.

## How it fits together
`lf ops wt *` and `lf ops next` call into `loopflow-engine::worktrees`, which shells out to git for worktree discovery and creation. Shell integration writes directives to a temp file that the wrapper sources after `lf` returns.

## Risks and bottlenecks
- Merge detection depends on `origin/<default-branch>` being up to date; stale remotes can misclassify prunability.
- Shell wrapper redefines `lf`; unusual shell setups may require manual sourcing or disable auto-cd.
- Worktree pruning skips dirty trees silently; users may want explicit visibility of skipped paths.

## What's not included
- PR auto-merge, stack retargeting, or wave/worktree registry integration.
- Fish shell integration.
- Any daemon-backed worktree state or `wt` CLI event emission.
