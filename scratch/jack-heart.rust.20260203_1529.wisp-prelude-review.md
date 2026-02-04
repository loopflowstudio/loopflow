# Review: rust ops parity polish

## What was implemented
- Added the `loopflow-ops` crate and wired `lf ops` to use it for commit/pr/land/next/abandon/rebase workflows.
- Implemented agent-backed commit/PR message generation, lint integration, rebase recovery, and PR lifecycle updates.
- Added a README for the new crate and cleaned up workflow edge cases (local merge strategy, lint skip behavior, sync failure handling).

## Key choices
- Keep `lf` as a thin wrapper that delegates to `loopflow-ops` so `lfd` can reuse the same workflows.
- Use `Progress` callbacks for all UX output/confirmations to keep CLI and daemon flows consistent.
- Skip lint entirely when no checker is configured, rather than launching a fixer with no signal.

## How it fits together
`lf ops` routes to `loopflow-ops` workflows, which orchestrate `loopflow-engine` git/agent primitives and the GitHub CLI, while surfacing status via `Progress`.

## Risks and bottlenecks
- GitHub CLI availability and behavior differences can block PR flows; errors surface but are not retried.
- Rebase recovery relies on agent output; if the agent fails, the operation aborts.
- Local merge strategy uses `git land` primitives; behavior should be revalidated against Python parity.

## What's not included
- Wave-based branch naming and metadata updates in `next`.
- Fish shell integration and ops doctor/add/cp/version commands.
- PR base detection beyond worktree metadata (still defaults to main when ambiguous).
