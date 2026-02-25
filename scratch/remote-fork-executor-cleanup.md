# Fork Executor Cleanup

## Problem

Fork execution now works in CLI, daemon, and Docker, but the implementation is still split across parallel code paths with different assumptions:

- Fork manifest path is duplicated (`engine::fork` vs `lfd::executor::wave::fork`).
- Fork worktree naming differs (`.fork-N` in CLI path builder vs `-fork-N` elsewhere).
- Docker still infers branch names from filesystem paths for fork runs.
- CLI fork cleanup/write still bypasses executor trait hooks.

That drift is exactly how parity regresses. This cleanup advances the remote wave goal to **“Keep protocol parity”** and the post-01E invariant that fork semantics stay identical across local + Docker execution.

## Approach

Make one execution contract and route all fork workspace mutations through executor hooks.

1. **Canonicalize fork paths/constants in `engine::fork`**
   - Export a single manifest constant (`.lf/fork-manifest.json`) from `engine::fork`.
   - Make `engine::fork::fork_worktree_path` the only fork worktree builder and standardize on `-fork-N` suffix.
   - Delete duplicate fork path helpers/constants in daemon code and switch call sites to shared functions.
   - Update `is_ephemeral_worktree_path` to match the same suffix contract.

2. **Thread explicit branch identity through execution**
   - Add `branch: Option<&str>` to `AgentRunContext`.
   - Add branch to `AgentLaunchRequest`; fill it from run/fork orchestration (`run.branch` and `branch_name`).
   - In Docker executor, resolve workspace branch as:
     1) explicit context branch, 2) current git branch, 3) fallback.
   - Remove `infer_fork_branch_from_worktree` and `wave_run_id`-based branch guessing from normal execution paths.
   - Keep filesystem inference only where we truly reattach/recover without full run context.

3. **Unify CLI fork file ops through `AgentExecutor` hooks**
   - Switch CLI fork manifest write/remove and fork worktree cleanup to `write_to_workspace`, `remove_from_workspace`, and `cleanup_ephemeral_worktree`.
   - Remove `write_fork_manifest` / `cleanup_fork_worktrees` once no caller remains.
   - Do not introduce a new runner abstraction; reuse the existing executor trait contract.

4. **Finish small structural cleanup**
   - Deduplicate `cleanup_host_worktree` and `cleanup_ci_fix_worktree` behind one shared helper.
   - Move wave lifecycle executor-specific behavior out of HTTP route `executor_type` branching and behind trait methods.
   - Simplify Docker workspace resolution helpers after explicit branch threading lands.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep current split, patch only obvious bugs | Lowest short-term effort | Drift remains; parity bugs return when one path changes first |
| Add a dedicated `ForkRunner` abstraction for CLI + daemon | Could centralize all fork flow logic | Extra abstraction layer duplicates `AgentExecutor` responsibilities |
| Push all fork execution into daemon and make CLI proxy it | Strong unification | Big behavior shift, out of scope for cleanup follow-up |

## Key decisions

- **Use explicit branch context over path inference.** Branch identity is execution data, not a filename heuristic.
- **One fork naming rule (`-fork-N`) everywhere.** Janitor, cleanup, Docker mapping, and tests all key off the same suffix.
- **Executor trait is the only workspace mutation boundary.** File writes/cleanup for fork artifacts must use trait hooks, not ad-hoc filesystem calls.
- **Accept one controlled breaking change:** legacy `.fork-N` naming is not kept as an active compatibility mode.

**Wild success check:** adding a new executor backend requires implementing trait hooks, with zero fork-specific conditionals in orchestration or routes.

**Wild failure check:** if context branch plumbing is incomplete, Docker writes/syncs the wrong workspace branch. Mitigation: explicit branch-precedence tests and fork end-to-end tests in both local and Docker paths.

## Scope

- In scope:
  - Priority 1–4 refactors above
  - Targeted test updates for path naming, branch resolution, and cleanup behavior
  - Route cleanup that removes executor-type branching for wave lifecycle hooks
- Out of scope:
  - Fork planning (`plan_fork_execution`, `merge_directions`)
  - Manifest schema (`ForkManifest`, `ForkManifestBranch`)
  - Scheduler slot behavior
  - Docker container rehydration/mutation lock model

## Done when

- There is one fork manifest path constant and one fork worktree path builder used by both CLI and daemon code.
- Docker normal run path no longer infers fork branch from worktree name when branch context is available.
- CLI fork path uses executor workspace hooks for manifest + cleanup (no direct `engine::fork` fs helpers left).
- Wave HTTP routes no longer branch on `executor_type` for wave lifecycle side effects.
- Validation passes:
  - `cargo test -p loopflow fork_worktree_path`
  - `cargo test -p loopflow resolve_workspace_branch`
  - `cargo test -p loopflow execute_fork_`
