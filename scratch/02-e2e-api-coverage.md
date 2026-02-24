# E2E HTTP API coverage

## Problem

`lfd` exposes a growing HTTP surface, but our current e2e tests only exercise CLI workflows. We are missing live coverage for:

- HTTP status codes and error payload shape
- auth + request validation at the API boundary

Who benefits: anyone changing API handlers, store logic, or client behavior. Today they can break `/v0` without CI noticing.

## Approach

Build a **hermetic API contract test system** in `scripts/` and make shell e2e files thin CI wrappers. Start with wave CRUD — the most-used endpoints and the cleanest test of the harness.

1. **Extract reusable daemon lifecycle into `scripts/lib/lfd_runtime.py`.**
   - Provide `LfdRuntime` context manager: build `lfd` (cargo handles dirty check), start on an ephemeral port, wait for `/health`, expose base URL + token, tear down on exit.
   - Isolated `HOME` and temp repo per suite so tests don't touch `~/.lf` or depend on local state.
   - Reusable by `scripts/dev.py` and future test suites.

2. **Add `scripts/lib/api_harness.py` for route-level assertions.**
   - `httpx` client with auth header wiring from runtime token.
   - Assertion helpers for:
     - exact status code
     - standard error envelope (`error.type`, `error.message`)
     - required JSON fields per route
   - Structured output: one line per scenario + summary + non-zero exit on any failure.

3. **Implement wave CRUD smoke suite.**
   - `scripts/test_api_smoke.py`: create, list, get, update, delete — happy paths and error cases (missing wave, bad input, auth failures).

4. **Wire CI through a shell wrapper.**
   - `tests/e2e/test_api_smoke.sh` — one-line `uv run python ...` entry point.
   - Shell script is logic-free: scripts contain behavior, `tests/e2e` is CI glue.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep shell tests and add `curl` checks inline | Fast to start, but brittle and hard to scale across many routes | Recreates ad-hoc harness logic and makes failures opaque |
| Put all API e2e in Rust integration tests | Tight coupling to internals and faster type safety | We want black-box HTTP validation and script-level ergonomics for agents/humans |
| Use pytest fixtures under `python/tests/` | Familiar Python testing workflow | This wave explicitly wants `scripts/test_*.py` as primary infra and `tests/e2e/*.sh` as wrappers |

## Key decisions

1. **Hermetic runtime per suite (isolated HOME + temp repo) is non-negotiable.**
   - Wild failure to avoid: six months of flaky tests caused by shared `~/.lf` state and zombie daemons.

2. **Wave CRUD first, extend later.**
   - Proves the harness works on the simplest endpoints. Chords, stimuli, run lifecycle, and coverage guard come in follow-up passes once the foundation is solid.

3. **`scripts/` owns behavior; `tests/e2e/` owns CI entrypoints only.**

## Scope

- In scope:
  - `scripts/lib/lfd_runtime.py` and `scripts/lib/api_harness.py`
  - `scripts/test_api_smoke.py` — wave CRUD happy + error paths
  - `tests/e2e/test_api_smoke.sh` — CI wrapper

- Out of scope (future passes):
  - Chords (join/leave/nest)
  - Stimuli (create/list/delete, owner constraints)
  - Run lifecycle (run/stop/continue/land/next)
  - Coverage manifest + guard
  - WebSocket / SSE / webhook coverage

## Done when

```bash
uv run python scripts/test_api_smoke.py
tests/e2e/test_api_smoke.sh
```

Wave create, list, get, update, delete each have at least one happy-path and one error-path scenario passing against a live `lfd` instance.
