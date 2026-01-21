---
status: proposed
area: infra
---

# CI smoke suite for core CLI flows

Loopflow relies on fast, deterministic CLI behavior; regressions in `lf`, `lfd`, or `lfops` only show up when users run tasks. Add a hermetic CI smoke suite that validates the most common flows without hitting real LLM APIs so we catch breakages early and keep releases reliable.

## Scope

- Add a CI workflow that runs on PRs and main using `uv`.
- Create a minimal smoke test harness that exercises `lf`, `lfd loop`, and `lfops land` against a temporary repo fixture.
- Stub agent execution so tests run offline and deterministically.
- Publish a short README in the test module describing how to run the suite locally.
- Exclude real model API calls, long-running agent loops, or GUI integrations.

## Approach

- Introduce a `LF_TEST_MODE=1` path (or a dedicated `--agent stub`) that routes agent invocations to a lightweight stub binary returning predictable outputs.
- Build a temp repo fixture during tests with minimal `.lf/` and `.claude/commands/` content, then run CLI commands via subprocess in a sandboxed temp directory.
- Add timeouts and structured logs so failures show which step failed (context build, prompt assembly, agent invocation, or post-step hooks).
- Wire the CI workflow to cache `uv` dependencies and run only the smoke suite plus existing unit tests.
