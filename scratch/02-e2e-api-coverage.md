# E2E HTTP API coverage

## Problem

`lfd` exposes a large HTTP surface, but our current e2e tests only exercise CLI workflows. We are missing live coverage for:

- HTTP status codes and error payload shape
- auth + request validation at the API boundary
- cross-resource constraints (join/leave/nest invariants, run lifecycle preconditions)

Who benefits: anyone changing API handlers, store logic, or client behavior. Today they can break `/v0` without CI noticing. We need this now because route count is growing and drift between handler intent and real behavior is already expensive to debug.

## Approach

Build a **hermetic API contract test system** in `scripts/` and make shell e2e files thin CI wrappers.

1. **Extract reusable daemon lifecycle into `scripts/lib/lfd_runtime.py`.**
   - Move process lifecycle helpers out of ad-hoc scripts into one module used by both `scripts/dev.py` and API tests.
   - Provide `LfdRuntime` context manager: build `lfd`, start on an ephemeral port, wait for `/health`, expose base URL + token, and always tear down.
   - Run each suite with isolated `HOME` and temp repo so tests do not touch `~/.lf` or depend on local state.

2. **Add `scripts/lib/api_harness.py` for route-level assertions.**
   - `httpx` client with auth header wiring from runtime token.
   - Assertion helpers for:
     - exact status code
     - standard error envelope (`error.type`, `error.message`)
     - required JSON fields per route
   - Structured output: one line per scenario + summary + non-zero exit on any failure.

3. **Implement scenario suites by domain (priority-first).**
   - `scripts/test_api_smoke.py`: wave CRUD + list
   - `scripts/test_api_chords.py`: join/leave/nest happy and invariant failures
   - `scripts/test_api_stimuli.py`: create/list/delete stimulus + owner constraint failures
   - `scripts/test_api_runs.py`: list/get active run semantics
   - `scripts/test_api_lifecycle.py`: run/stop/continue/land/next using temp git repos and local-only settings

4. **Add a coverage guard so route additions cannot skip tests.**
   - `scripts/lib/api_coverage_manifest.py` maps each required route to: owning suite, happy case id, error case id.
   - `scripts/test_api_coverage_guard.py` verifies every priority route is present in the manifest and has both case types.
   - CI fails if a new/changed route lacks coverage metadata.

5. **Wire CI through shell wrappers only.**
   - `tests/e2e/test_api_smoke.sh` and `tests/e2e/test_api_contract.sh` become one-line `uv run python ...` entry points.
   - Keep shell scripts logic-free to match wave direction: scripts contain behavior; `tests/e2e` is CI glue.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep shell tests and add `curl` checks inline | Fast to start, but brittle and hard to scale across many routes | Recreates ad-hoc harness logic and makes failures opaque |
| Put all API e2e in Rust integration tests | Tight coupling to internals and faster type safety | We want black-box HTTP validation and script-level ergonomics for agents/humans |
| Use pytest fixtures under `python/tests/` | Familiar Python testing workflow | This wave explicitly wants `scripts/test_*.py` as primary infra and `tests/e2e/*.sh` as wrappers |

## Key decisions

1. **Hermetic runtime per suite (isolated HOME + temp repo) is non-negotiable.**
   - Wild failure to avoid: six months of flaky tests caused by shared `~/.lf` state and zombie daemons.

2. **Route coverage is enforced with a manifest + guard, not convention.**
   - Wild success target: adding a new route immediately fails CI with a precise “missing happy/error case” message.

3. **Priority routes first, then full-route expansion.**
   - Start with wave CRUD/lifecycle/chords/stimuli/runs, then extend the same harness to sessions/chat/hooks.

4. **`scripts/` owns behavior; `tests/e2e/` owns CI entrypoints only.**
   - This follows Infra wave intent: “Dev scripts in `scripts/` are the primary test infrastructure; `tests/e2e/` is a thin CI wrapper.”

## Scope

- In scope:
  - `scripts/lib/lfd_runtime.py` and `scripts/lib/api_harness.py`
  - Domain API suites for priority routes
  - Coverage manifest + guard script
  - CI shell wrappers that only invoke Python scripts

- Out of scope:
  - WebSocket streaming deep coverage (`/ws`, SSE chat events)
  - GitHub webhook end-to-end with real external callbacks
  - Rewriting existing unit tests in `python/tests/`

## Done when

```bash
uv run python scripts/test_api_smoke.py
uv run python scripts/test_api_chords.py
uv run python scripts/test_api_stimuli.py
uv run python scripts/test_api_runs.py
uv run python scripts/test_api_lifecycle.py
uv run python scripts/test_api_coverage_guard.py
tests/e2e/test_api_smoke.sh
tests/e2e/test_api_contract.sh
```

And the observable outcome is: every priority HTTP route has at least one happy-path and one error-path scenario, enforced by the coverage guard in CI.
