# Review: pytest migration + harness split for e2e API tests

## What was implemented

- Split `scripts/lib/api_harness.py` into three focused components:
  - `ApiClient` for raw authenticated/anonymous HTTP requests
  - `ApiAssertions` for API contract assertions
  - `ScenarioRunner` for standalone scenario execution and reporting
- Added shared scenario libraries:
  - `scripts/lib/wave_scenarios.py` for wave CRUD smoke scenarios
  - `scripts/lib/fork_scenarios.py` for Docker fork infrastructure and execution helpers
- Refactored standalone scripts:
  - `scripts/test_api_smoke.py` now runs shared wave scenarios using `Client` for happy paths and raw HTTP for contract/error paths
  - `scripts/test_fork.py` now delegates to shared fork helpers
- Added pytest e2e coverage:
  - `tests/e2e/conftest.py` with shared `LfdRuntime`, `ApiClient`, and `Client` fixtures
  - `tests/e2e/test_api_smoke.py` with one test per wave scenario
  - `tests/e2e/test_fork.py` as Docker/credentials-gated pytest wrapper
- Updated CI/test configuration:
  - `tests/e2e/test_api_smoke.sh` now runs `uv run pytest tests/e2e/test_api_smoke.py -v`
  - `pyproject.toml` now registers `e2e` and `docker` pytest markers and sets root python path
  - `TESTING.md` and wave docs updated to reflect API smoke coverage and runner structure

## Key choices

- Reused one scenario implementation for both runners instead of duplicating logic between script and pytest paths.
- Tested happy paths through `loopflow.client.Client` to validate real client behavior end-to-end.
- Kept raw HTTP checks for explicit contract/error assertions (401/400/404/409 payload shape) where client exceptions would hide envelope details.
- Kept fork coverage out of per-PR CI (`docker` marker), but made it pytest-addressable for nightly/manual environments.

## How it fits together

`LfdRuntime` provides an isolated daemon + repo environment. Shared scenario modules run against that environment using either `Client` (typed client behavior) or `ApiClient` + `ApiAssertions` (wire-level contract behavior). Standalone scripts and pytest wrappers both call the same scenarios, so local manual validation and CI smoke validation execute the same core checks.

## Risks and bottlenecks

- `LfdRuntime` still uses reserve-then-bind port selection, which can theoretically race under heavy parallelism.
- API smoke builds/starts `lfd`, so runtime cost scales with suite growth.
- Docker fork pytest is environment-sensitive and long-running; in this environment `uv run pytest tests/e2e/test_fork.py -v` exited with signal 143.

## What's not included

- No expansion yet to chords/stimuli/run-lifecycle HTTP routes.
- No route-manifest/coverage guard to enforce `/v0` route test coverage.
- No protocol-level contract tests (SSE/WebSocket/webhooks).
