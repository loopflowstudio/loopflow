# Docker fork parity review

## What was implemented

- Added Docker fork execution support in `WaveExecutor::run_fork` so fork branches execute (instead of failing up front), write a fork manifest, run `synthesize`, then fail-late if any branch failed.
- Refactored Docker workspace resolution to derive container worktree identity from `cwd`, allowing multiple branch worktrees for one wave run.
- Added `AgentExecutor::write_to_workspace` / `remove_from_workspace` / `cleanup_ephemeral_worktree` hooks so fork manifest handoff and cleanup are executor-aware.
- Implemented Docker-side workspace file writes and worktree cleanup via helper containers against the repo volume.
- Added/updated tests across fork planning, Docker workspace mapping, fork execution lifecycle, cleanup, and startup orphan recovery.
- Updated flow/docs naming from plan `review` to `research`, and added a dedicated interactive `review` step.

## Key choices

- **Reuse native fork contract** (`plan -> branch runs -> manifest -> synthesize -> cleanup -> fail-late`) instead of introducing Docker-specific semantics.
- **Serialize only git mutation windows** (shared clone/worktree operations) and keep branch agent runs parallel.
- **Use trait hooks instead of Docker downcasts** so fork orchestration stays executor-agnostic.
- **Drive prompt directions from each planned branch execution** in fork workers, keeping branch prompt context aligned with recorded branch direction metadata.
- **Document Docker capability accurately**: `docs/lfd.md` now states fork support in Docker mode rather than listing it as unsupported.

## How it fits together

`run_fork` plans branch executions, creates per-branch worktrees, launches branch agents concurrently, records `ForkRun` state, and gathers outcomes into `.lf/fork-manifest.json`. Then the normal `synthesize` step runs as a standard step using that manifest. Cleanup removes manifest/worktrees/fork records, and final run status is computed fail-late.

For Docker, workspace resolution maps host worktrees to volume-backed container worktrees so synthesize can read all branch repos directly in-volume.

## Risks and bottlenecks

- Host-side fork worktrees are still required for prompt assembly and sync behavior; Docker does not yet run purely with host placeholder dirs.
- Fork execution fan-out depends on scheduler slots; low slot counts reduce parallelism.
- Cleanup is best-effort/idempotent; repeated failures in helper-container git cleanup can leave temporary volume artifacts until later recovery.

## What's not included

- No broader Docker runtime redesign beyond fork parity requirements.
- No general host->container sync channel beyond targeted workspace file writes.
- No changes to interactive fork support (still rejected).
