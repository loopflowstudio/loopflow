# 01E: Docker Fork Parity

Enable fork execution in the Docker executor via a dedicated, scoped PR.

## Goal

Docker executor matches native fork semantics:

1. Run all fork branches in isolated worktrees
2. Write fork manifest
3. Run `synthesize`
4. Cleanup fork worktrees + records
5. Fail run if any branch failed (fail-late)

## Why this is separate

Fork in Docker touches workspace mapping, shared-clone locking, branch concurrency, and host/container sync behavior. Keeping this in its own PR reduces risk and makes regressions easier to isolate.

## Scope

- Branch-scoped Docker worktree mapping (no branch collisions)
- Safe shared-clone mutation locking during branch prep
- Parallel branch container execution
- Manifest + synthesize parity with CLI/native daemon behavior
- Recovery + cleanup parity (including restart safety)
- Targeted tests for success, partial failure, timeout, and cleanup

## Out of scope

- Broad Docker runtime redesign
- Cross-host worktree sync redesign
- Non-fork executor feature work

## Done when

- Docker executor no longer rejects fork runs
- Fork behavior and error semantics match native mode
- Fork-related Docker tests are green in CI/nightly
