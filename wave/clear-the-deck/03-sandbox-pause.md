# 03: Sandbox Pause and Daytona Evaluation

**Finish line:** Custom sandbox code removed or feature-gated. Daytona evaluated with clear go/no-go criteria. If go: spike integration. If no-go: document what's missing and what we need.

## Context

SandboxExecutor is built but blocked on Docker Sandbox CLI plugin availability. Rather than wait, pause the custom work and evaluate Daytona — purpose-built AI agent sandbox infra, sub-90ms container creation, first-class git API, self-hosted via Docker Compose.

## What to build

1. **Evaluate Daytona** against concrete criteria:
   - Container creation latency (<200ms acceptable?)
   - Git integration (clone, branch, commit, push — does their API cover our needs?)
   - Context file sync (can we mount/sync area files into the sandbox?)
   - Agent harness compatibility (can Claude Code / Codex / OpenCode run inside?)
   - Self-hosted reliability (Docker Compose, no external dependencies)
   - Licensing (AGPL-3.0 — implications for our MIT codebase?)

2. **If go:** Spike a DaytonaExecutor that replaces SandboxExecutor. Same interface, Daytona backend. Prove it works for one wave run end-to-end.

3. **If no-go:** Document gaps. Feature-gate SandboxExecutor behind `--experimental-sandbox`. Keep DockerExecutor as default. Revisit when Daytona or Docker Sandbox matures.

4. **Clean up** regardless: remove dead sandbox code paths, simplify executor selection logic.

## Done when

- Daytona evaluation document in scratch/ with go/no-go verdict
- Custom sandbox code either replaced (go) or feature-gated (no-go)
- Executor selection logic simplified
