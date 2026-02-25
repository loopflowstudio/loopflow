# 03: Fork Executor Cleanup

Remove remaining CLI/daemon/Docker fork drift before rolling out studio auth.

## Why this phase exists

Fork behavior is functionally correct, but implementation still has drift points:

- duplicated fork manifest path constants
- inconsistent fork worktree naming conventions
- branch inference where branch identity is already known
- CLI filesystem mutations bypassing executor hooks

Auth rollout on top of this drift raises incident risk and debugging cost.

## Scope

### In scope

1. Canonical fork constants + path helpers in `engine::fork`
2. `AgentRunContext` carries optional branch; Docker resolves branch from context first
3. CLI fork manifest and cleanup use executor workspace hooks
4. Low-risk follow-on cleanup only if it does not expand scope

### Out of scope

- Fork planning algorithm changes
- Manifest schema redesign
- Scheduler model changes
- Container lock/rehydration redesign

## Contract

- `-fork-N` naming everywhere (CLI, daemon, Docker, janitor)
- one shared manifest relative path constant
- branch inference is fallback-only, not happy path
- workspace file mutation goes through `AgentExecutor` hooks

## Validation

- `cargo test -p loopflow fork_worktree_path`
- `cargo test -p loopflow resolve_workspace_branch`
- `cargo test -p loopflow run_fork`

## Done when

- Local and remote fork runs share one deterministic path for manifest + cleanup
- Docker branch selection is explicit and predictable
- Dead direct-filesystem fork helpers are removed
- Fork incidents are diagnosable by reading one contract path, not two divergent ones
