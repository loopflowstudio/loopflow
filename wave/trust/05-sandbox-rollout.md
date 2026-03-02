# 05: Full Sandbox Rollout

**Status (2026-02-28):** Deferred from production path. Host sandbox capability is validated, but DinD remains blocked on Linux sandbox CLI plugin distribution.

**Finish line:** All harnesses route through `SandboxExecutor`, `DockerExecutor` and Bollard dependency are deleted, and restart rehydration reattaches to running sandbox streams.

## What we're trying to learn

Is sandbox reliable enough to be the only container executor? What breaks when we remove the Bollard fallback?

## Scope

- Codex and OpenCode sandbox rollout (extend harness routing)
- DinD support contract (define and test guarantees)
- Full restart rehydration for active sandbox runs (stream reattach)
- Bollard removal (delete `DockerExecutor` and Bollard dependency)

## Context from validation (sprint 04)

### DinD decision

DinD probe rerun on 2026-02-28 — host Docker daemon reachable, but bundled lfd container reports `docker: 'sandbox' is not a docker command`. Two options identified:

- **Option A (preferred):** Add `docker-sandbox` plugin to the lfd Dockerfile. Build-time dependency, not runtime discovery.
- **Option B:** Fall back to DockerExecutor when running inside DinD. Works but defeats sandbox mode for Concerto users.

Re-run `scripts/concerto-dev.py sandbox-dind` after choosing one option and implementing it.

## Open questions

- Credential proxy: can it replace env var injection for all harnesses?
- Linux stability under sustained load with multiple concurrent sandboxes

## Done when

- All harnesses route through sandbox executor
- DockerExecutor and Bollard dependency removed
- Restart rehydration reattaches to running sandbox streams
- DinD behavior documented and tested
