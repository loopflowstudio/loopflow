# 02: Sandbox Pause and Daytona Evaluation

## Problem

Loopflow still carries a misleading second container story inside `mode: container`. User-facing docs already bless Docker, but the code, compose renderer, and tests still treat sandbox as a near-peer executor. That violates the wave vision to keep `lfd`'s deployment surface honest and small.

The people who pay for this ambiguity are the operators and developers who have to debug it:

- self-hosters see a config knob that looks supported end to end when it is not
- maintainers carry `ExecutorType::Sandbox`, `AdaptiveContainerExecutor`, compose branches, and tests for a path the docs already downplay
- harness authors inherit behavior differences around worktrees, credentials, cleanup, and failure recovery that are hard to reason about

This item advances the clear-the-deck goals:

- **"Docker is the only blessed container executor unless a measured replacement beats it."**
- **"Sandbox has one explicit status instead of an adaptive half-product."**
- **"Deploy docs, compose generation, and executor tests all describe the same support story."**

## Approach

Choose the hard cut.

1. **Make Docker the only supported executor in `mode: container`.**
   - `Mode::Container` always resolves to `ExecutorType::Docker`.
   - Remove `ExecutorType::Sandbox` from the mainline config/runtime path.
   - Delete `AdaptiveContainerExecutor` and the sandbox-specific executor path from `WaveExecutor`.

2. **Delete `executor.sandbox` from the supported config surface.**
   - Remove it from docs, examples, compose generation, and config tests.
   - If the key is still present in YAML, fail fast with a migration error telling the user that container mode is Docker-only now.
   - Do not silently ignore it. Hidden compatibility is how this surface grows back.

3. **Do not integrate Daytona in this wave.**
   - Record a no-go verdict for now: Daytona is interesting, but not ready to become Loopflow’s blessed replacement path.
   - Keep Daytona as a follow-up experiment only after the Docker-only cleanup lands, with its own spike branch or item.
   - The bar for reopening executor plurality is one end-to-end wave run on a self-hosted target with worktrees, credentials, recovery, and cleanup all proven.

4. **Write the verdict into the codebase and docs, not just scratch.**
   - `docs/lfd.md`, deploy docs, compose templates, and config reference must all say the same thing: container mode means Docker.
   - Tests must enforce that story.

### Why this is the right call

As of **March 17, 2026**, the evidence says “shrink,” not “explore more in production surface area.”

- **Docker Sandboxes are still explicitly experimental** in Docker’s docs, require Docker Desktop 4.58+, and their microVM path is for **macOS or Windows**, while Loopflow’s container mode is aimed at remote/shared hosts and self-hosted Linux deployments.
- Docker’s supported-agents page marks **all sandboxed agents experimental**, including Claude, Codex, Gemini, and OpenCode.
- A local benchmark on this machine showed:
  - `docker sandbox create --name lf-bench shell <worktree>`: **12.3s mean** across 5 runs (min 9.8s, max 18.7s)
  - `docker sandbox exec lf-bench true`: **104.7ms mean**
  - `docker sandbox rm lf-bench`: **11.5s mean**
- More importantly, `docker sandbox exec -w <worktree> git status --short` failed in this Loopflow worktree with `fatal: not a git repository`, because the worktree’s `.git` file points at the sibling main repo’s `.git/worktrees/...` metadata that was not available inside the sandbox.
- Daytona’s docs show stronger long-term shape for agent infrastructure — Git APIs, process/session APIs, volumes, snapshots, Docker-in-Docker support — but its official open-source deployment is still documented as **not safe for production**, and customer-managed runners are still **experimental**.

The likely wild success here is boring in the best way: operators stop thinking about executor choice inside container mode, and maintainers debug one container path. The likely wild failure is keeping the adaptive path alive because it is “almost useful,” then spending six more months paying for a dual runtime with no fully supported second executor.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep `AdaptiveContainerExecutor` and continue documenting sandbox as experimental | Lowest immediate churn; preserves current experiments | Fails the wave. We still ship two runtime branches, two mental models, and a config knob that looks more supported than it is. |
| Keep sandbox behind a narrower experimental flag | Smaller public story, but code/tests still carry the branch | Still leaves compose/config/test complexity, and the worktree + credential gaps remain real. This is an accounting trick, not simplification. |
| Replace sandbox with Daytona now | Better conceptual fit for long-lived sandboxes, Git operations, sessions, and future agent tooling | Too much new surface area right now. Official OSS deployment is still marked unsafe for production, custom runners are experimental, and integrating it would expand the matrix before we have one proven self-hosted wave run. |

## Key decisions

- **Delete, don’t demote.** Mainline Loopflow will not carry an experimental container executor after this wave.
- **Fail fast on stale config.** Reject `executor.sandbox` instead of tolerating it.
- **Judge replacements by wave reality, not startup demos.** A candidate must prove worktree correctness, credential sync, harness compatibility, crash recovery, and cleanup on a real self-hosted wave run.
- **Do not reopen the deployment matrix.** This item is about collapsing executor choice inside container mode, not adding another supported deployment flavor.
- **Accept that Docker remains the blessed path even if it is less elegant.** The goal is one support story, not maximal optionality.

## Scope

- In scope:
  - remove `executor.sandbox` from supported config/docs
  - remove `ExecutorType::Sandbox`, `AdaptiveContainerExecutor`, and sandbox-only compose acceptance from the mainline runtime
  - add migration/error coverage for stale sandbox config
  - update docs and tests so container mode clearly means Docker
  - record the no-go verdict for Daytona in this wave item
- Out of scope:
  - shipping Daytona integration
  - preserving backwards compatibility for sandbox configs beyond a clear migration error
  - new auth modes, service-manager modes, or deployment shapes
  - broader agent-runtime redesign beyond this executor decision

## Done when

- `rg -n "executor\.sandbox|ExecutorType::Sandbox|AdaptiveContainerExecutor" rust/loopflow docs deploy docker` only finds intentional migration notes or historical scratch docs.
- `mode: container` resolves to Docker with no sandbox branch.
- compose/config tests assert Docker-only behavior for container mode.
- docs no longer imply that sandbox and Docker are peers.
- the written verdict says: **Docker stays blessed; Daytona is no-go for now; sandbox is removed from the supported surface.**

## Measure

Capture the decision with both a support-surface metric and a reality check.

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

**Re-run before landing**

```bash
cargo test -p loopflow config_tests compose_ land_tests pr_tests
rg -n "executor\.sandbox|ExecutorType::Sandbox|AdaptiveContainerExecutor" rust/loopflow docs deploy docker
```
