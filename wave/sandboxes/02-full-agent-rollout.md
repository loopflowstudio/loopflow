# 03: Full Agent Rollout

**Finish line:** All harnesses route through `SandboxExecutor`, `DockerExecutor` and Bollard dependency are deleted, and restart rehydration reattaches to running sandbox streams.

## What we're trying to learn

Is sandbox reliable enough to be the only container executor? What breaks when we remove the Bollard fallback?

## Scope

- Codex and OpenCode sandbox rollout (extend harness routing)
- DinD support contract (define and test guarantees)
- Full restart rehydration for active sandbox runs (stream reattach)
- Bollard removal (delete `DockerExecutor` and Bollard dependency)
- Custom template strategy: default `claude` template vs shipped template with required tools

## Open questions

- Credential proxy: can it replace env var injection for all harnesses?
- Template strategy: continue default templates or ship custom with tools pre-installed?
- Linux stability under sustained load with multiple concurrent sandboxes

## Done when

- All harnesses route through sandbox executor
- DockerExecutor and Bollard dependency removed
- Restart rehydration reattaches to running sandbox streams
- DinD behavior documented and tested
