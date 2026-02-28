# 02: Full Agent Rollout

**Finish line:** All harnesses route through `SandboxExecutor`, `DockerExecutor` and Bollard dependency are deleted, and restart rehydration reattaches to running sandbox streams.

## What we're trying to learn

Is sandbox reliable enough to be the only container executor? What breaks when we remove the Bollard fallback?

## Scope

- Codex and OpenCode sandbox rollout (extend harness routing)
- DinD support contract (define and test guarantees)
- Full restart rehydration for active sandbox runs (stream reattach)
- Bollard removal (delete `DockerExecutor` and Bollard dependency)
- Custom template strategy: default `claude` template vs shipped template with required tools

## Context from validation (sprint 01)

### DinD decision

DinD probe blocked on 2026-02-28 — host Docker daemon not reachable. Two options identified:

- **Option A (preferred):** Add `docker-sandbox` plugin to the lfd Dockerfile. Build-time dependency, not runtime discovery.
- **Option B:** Fall back to DockerExecutor when running inside DinD. Works but defeats sandbox mode for Concerto users.

Re-run `scripts/concerto-dev.py sandbox-dind` once Docker daemon is available, then commit to one option.

### Gemini template strategy

Gemini CLI probe blocked on 2026-02-28 — sandbox CLI missing `create`/`exec`. Three possible outcomes once the probe runs:

| Outcome | Response |
|---------|----------|
| Gemini CLI present in `claude` template | Done. Document minimum template version. |
| Not present, installable via `npm install -g @google/gemini-cli` inside sandbox | Add install step to sandbox executor's Gemini harness path before main exec. |
| Not present, install blocked | Investigate custom template creation (`docker sandbox create --template custom-lf`). If not supported yet, document as blocked. |

Custom template infrastructure should only be built if validation proves the default template is insufficient.

## Open questions

- Credential proxy: can it replace env var injection for all harnesses?
- Template strategy: continue default templates or ship custom with tools pre-installed? (depends on Gemini probe outcome)
- Linux stability under sustained load with multiple concurrent sandboxes

## Done when

- All harnesses route through sandbox executor
- DockerExecutor and Bollard dependency removed
- Restart rehydration reattaches to running sandbox streams
- DinD behavior documented and tested
