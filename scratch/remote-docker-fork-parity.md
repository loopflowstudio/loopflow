# 01E: Docker Fork Parity (Current State)

## Goal

Fork flows (`wave-reduce`, `wave-polish`, `wave-expand`) now run under the Docker executor with native-equivalent semantics:

- parallel branch execution
- manifest handoff into `synthesize`
- fail-late final status
- cleanup of fork artifacts

## What ships now

### Fork execution parity

`WaveExecutor::run_fork` no longer rejects Docker runs. Docker forks now follow the same high-level contract as native:

1. Plan branches once with existing fork planning logic.
2. Run branch agents concurrently.
3. Persist branch outcomes and write `.lf/fork-manifest.json`.
4. Run `synthesize` after branches complete.
5. Mark run failed at the end if any branch or synthesize failed.
6. Cleanup worktrees + fork-run records.

### Docker workspace model

Container worktree resolution is based on `cwd` (agent workspace identity), not only wave run identity. This allows multiple concurrent branch worktrees for a single wave run.

Volume layout uses sibling worktrees:

```text
/workspace/repos/<repo>/
  main/
  worktrees/
    <wave>
    <wave>-fork-0
    <wave>-fork-1
```

### Manifest and workspace file handoff

`AgentExecutor` now provides executor-aware workspace file operations (including manifest writes), so Docker can place files where containerized follow-up steps read them.

### Cleanup and recovery

Fork cleanup is idempotent and covers:

- volume-side worktree removal in Docker
- fork-run record deletion
- orphan cleanup on restart recovery

## Key implementation decisions retained

- Reused native fork contract instead of Docker-specific semantics.
- Serialized only shared-clone git mutation windows; kept branch agent execution parallel.
- Used executor trait hooks instead of Docker downcasts in fork orchestration.
- Kept scope to Docker fork parity (no broad runtime redesign).

## Current limitations / follow-up

- Docker fork branches still rely on host-side git worktrees for prompt assembly (`build_step_prompt` needs local step/context files before container launch).
- Moving to true host placeholder dirs would require either:
  - pre-prompt container→host sync, or
  - a prompt build path that does not depend on host worktree materialization.
