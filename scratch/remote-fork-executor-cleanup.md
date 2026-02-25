# Fork Executor Cleanup

## Problem

Fork execution semantics are now correct across CLI, daemon, and Docker after 01E, but implementation details still drift:

- Fork manifest path is duplicated (`engine::fork` and `lfd::executor::wave::fork`).
- Fork worktree naming is inconsistent (`.fork-N` in CLI helpers vs `-fork-N` in daemon helpers).
- Docker branch resolution still relies on filesystem inference in places where branch identity is already known.
- CLI fork cleanup/write paths still bypass executor workspace hooks, so local and Docker cleanup can diverge again.

Who benefits: maintainers shipping remote execution, operators debugging fork failures, and users depending on protocol parity between local and remote modes.

Why now: remote deployment work (wave/remote steps 1-3) needs fork behavior to be boring and deterministic before we expand auth/API surface.

## Approach

Adopt one fork contract and thread it end-to-end.

1. **Canonical fork constants + paths in `engine::fork`**
   - Export one public fork manifest relative path constant from `engine::fork`.
   - Standardize `fork_worktree_path` to the `-fork-N` suffix convention.
   - Update both CLI (`lf/commands/flow.rs`) and daemon (`lfd/executor/helpers.rs`, `lfd/executor/wave/fork.rs`) to consume the same constant/function.
   - Keep one `is_ephemeral_worktree_path` rule aligned with that convention.

2. **Thread branch through execution context (no guessing on happy path)**
   - Add `branch: Option<&str>` to `AgentRunContext`.
   - Fill it in from orchestration:
     - normal step runs: `run.branch`
     - fork branch runs: explicit `<wave_run_id>-fork-N` branch
   - Docker workspace resolution uses context branch first; filesystem inference remains only as a recovery fallback.
   - Remove branch guessing helpers that become redundant (`infer_fork_branch_from_worktree` path-based happy-path usage).

3. **Route CLI fork manifest/cleanup through executor workspace hooks**
   - Use the executor trait hooks (`write_to_workspace`, `remove_from_workspace`, `cleanup_ephemeral_worktree`) for CLI fork manifest writes and worktree cleanup.
   - Delete direct filesystem fork helpers in `engine::fork` once no callers remain (`write_fork_manifest`, `cleanup_fork_worktrees`).
   - Do **not** introduce a new fork runner abstraction.

4. **Small follow-on cleanup in the same pass when low-risk**
   - Deduplicate host cleanup helpers (`cleanup_host_worktree`, `cleanup_ci_fix_worktree`).
   - Reduce route-level branching by moving wave lifecycle cleanup behind executor hooks where possible.

This is intentionally ambitious: eliminate drift points now instead of patching each one when it breaks.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Patch each mismatch in place (constants, parsing, cleanup) | Lowest short-term change set | Keeps multiple sources of truth; drift returns in the next fork-related change |
| Add a new `ForkRunner` abstraction for CLI + daemon | Could centralize all fork behavior | Extra abstraction layer duplicates what `AgentExecutor` already provides |
| Keep branch inference and avoid context threading | Fewer signature changes | Inference is fragile and causes wrong branch selection in Docker edge cases |

## Key decisions

- **Canonical naming is `-fork-N` everywhere.** We optimize for daemon/Docker parity and janitor safety.
- **Branch identity is explicit context, not inferred state.** Inference is fallback-only for recovery/reattach.
- **Executor hooks are the single workspace mutation surface.** Fork manifest write/remove and ephemeral cleanup must go through the same interface in CLI and daemon paths.
- **Wild success target:** fork incidents become "reproducible by reading one code path"; local and remote runs produce the same manifest/worktree behavior.
- **Wild failure to avoid:** over-broad ephemeral-path matching deletes non-ephemeral worktrees. Guard with strict suffix parsing and tests.
- **New risk introduced:** wider signature changes (`AgentRunContext`) could ripple through tests/callers. Mitigation: land in dependency order (constants/path -> context -> CLI unification) with targeted tests each step.

## Scope

- In scope:
  - Priorities 1-3 from the ingested item, plus low-risk pieces of priority 4.
  - Rust modules under `engine::fork`, `lf/commands/flow`, `lfd/executor/*`, and related tests.
- Out of scope:
  - Fork planning logic (`plan_fork_execution`, `merge_directions`).
  - Manifest schema (`ForkManifest`, `ForkManifestBranch`).
  - Scheduler slot model.
  - Docker mutation lock strategy and container rehydration.
  - Remote auth/deployment tasks from wave steps 1-3.

## Done when

- Fork manifest path constant exists in one place and both CLI/daemon import it.
- Fork worktree naming is `-fork-N` in CLI, daemon, janitor detection, and Docker branch handling.
- `AgentRunContext` carries optional branch, and Docker workspace branch selection is effectively a one-liner: context branch → fallback.
- CLI fork path uses executor workspace hooks for manifest write/remove and ephemeral cleanup; old direct fork filesystem helpers are removed.
- Tests cover naming, branch resolution, and cleanup behavior, and pass:
  - `cargo test -p loopflow fork_worktree_path`
  - `cargo test -p loopflow resolve_workspace_branch`
  - `cargo test -p loopflow run_fork`

This advances remote wave goals:
- **"Keep protocol parity"** by removing CLI/daemon fork drift.
- **"Keep orchestration ownership in loopflow"** by centralizing workspace mutations behind executor hooks.
