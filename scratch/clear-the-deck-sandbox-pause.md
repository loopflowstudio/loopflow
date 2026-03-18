# 02: Sandbox Pause and Daytona Evaluation

## Done when

- `rg -n "executor\.sandbox|ExecutorType::Sandbox|AdaptiveContainerExecutor" rust/loopflow docs deploy docker` only finds intentional migration notes or historical scratch docs.
- `mode: container` resolves to Docker with no sandbox branch.
- compose/config tests assert Docker-only behavior for container mode.
- docs no longer imply that sandbox and Docker are peers.
- the written verdict says: **Docker stays blessed; Daytona is no-go for now; sandbox is removed from the supported surface.**

## Measure

**Support-surface target**

- Blessed container executors in user-facing docs: **1**
- Experimental container executors carried in mainline config/runtime: **0**

**Baseline captured on March 17, 2026**

- Docker Sandbox docs: experimental; microVM path requires Docker Desktop 4.58+ and macOS/Windows.
- Docker supported-agents docs: all agents experimental.
- Local Docker Sandbox benchmark on this worktree:
  - create mean **12.3s**
  - exec `true` mean **104.7ms**
  - remove mean **11.5s**
- Local worktree compatibility check: `git status` inside sandbox failed because the worktree git metadata lives outside the mounted path.
- Daytona docs: open-source deployment **not safe for production**; customer-managed runners **experimental**.

## Re-run before landing

```bash
cargo test -p loopflow config_
cargo test -p loopflow compose_
cargo test -p loopflow --test land_tests --test pr_tests
rg -n "executor\.sandbox|ExecutorType::Sandbox|AdaptiveContainerExecutor" rust/loopflow docs deploy docker
```
