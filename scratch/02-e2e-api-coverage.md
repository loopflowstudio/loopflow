# E2E HTTP API coverage

## Scope

Live HTTP API contract coverage for `lfd` using shared scenario logic with two runners:

- **Pytest** for CI (`tests/e2e/test_api_smoke.py`)
- **Standalone scripts** for humans/agents (`scripts/test_api_smoke.py`, `scripts/test_fork.py`)

## Current state

### Test architecture

1. **`scripts/lib/` reusable layers**
   - `lfd_runtime.py`: hermetic daemon lifecycle (isolated `HOME`, temp repo, ephemeral port, health/token wait, teardown)
   - `api_harness.py`: split utilities (`ApiClient`, `ApiAssertions`, `ScenarioRunner`)
   - `wave_scenarios.py`: shared wave CRUD scenario logic
   - `fork_scenarios.py`: shared Docker fork infrastructure helpers

2. **Standalone runners (`scripts/test_*.py`)**
   - Manual/agent validation path
   - Uses `ScenarioRunner` plus shared scenarios

3. **Pytest wrappers (`tests/e2e/test_*.py`)**
   - CI path using shared fixtures in `tests/e2e/conftest.py`
   - `e2e` and `docker` markers configured in `pyproject.toml`

4. **CI entrypoints**
   - `tests/e2e/test_smoke.sh` for CLI smoke
   - `uv run pytest tests/e2e/test_api_smoke.py -v` for HTTP API smoke

### Covered now

- `/v0/waves` CRUD happy + error paths (including auth and validation error envelopes)
- Docker fork smoke via pytest (`tests/e2e/test_fork.py`) that verifies fork branches launch
- Shared scenario implementation reused by pytest and standalone scripts

### Validation commands

```bash
uv run pytest tests/e2e/test_api_smoke.py -v
uv run pytest tests/e2e/ -v -m "not docker"
uv run python scripts/test_api_smoke.py
```

## Decisions to keep

- Use `loopflow.client.Client` for happy-path e2e coverage.
- Use raw HTTP (`ApiClient`) for explicit API contract/error-envelope checks.
- Keep Docker-backed fork coverage out of per-PR CI; run manually/nightly via `docker` marker.

## Remaining work

1. Expand HTTP coverage beyond waves:
   - Chords (`join`/`leave`/`nest`)
   - Stimuli (CRUD + owner constraints)
   - Run lifecycle (`run`/`stop`/`continue`/`land`/`next`)
2. Add route coverage manifest/guard so new `/v0` routes require tests.
3. Add protocol-level contract tests where needed (WebSocket/SSE/webhooks).

## Known risks

- Runtime cost grows as more suites build/start `lfd`.
- Reserve-then-bind ephemeral port pattern can race under high parallelism.
- Docker fork pytest remains environment-sensitive and needs stable infra validation.
