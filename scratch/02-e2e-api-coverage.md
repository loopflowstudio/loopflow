# E2E HTTP API coverage

## Problem

`lfd` exposes a growing HTTP surface, but our current e2e tests only exercise CLI workflows. We are missing live coverage for:

- HTTP status codes and error payload shape
- auth + request validation at the API boundary

Who benefits: anyone changing API handlers, store logic, or client behavior. Without API contracts in CI, `/v0` regressions can ship unnoticed.

## Current baseline (already in place)

We now have a working contract-test foundation:

- `scripts/lib/lfd_runtime.py`: hermetic `lfd` runtime (isolated `HOME`, temp repo, ephemeral port, `/health` + token wait, clean teardown)
- `scripts/lib/api_harness.py`: shared HTTP assertions (`expect_status`, `expect_error`, `expect_fields`) with per-scenario pass/fail output
- `scripts/test_api_smoke.py`: live `/v0/waves` CRUD happy/error scenarios
- `tests/e2e/test_api_smoke.sh`: logic-free CI wrapper
- CI + `TESTING.md` updated to run API smoke alongside existing e2e smoke

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep shell tests and add `curl` checks inline | Fast to start, but brittle and hard to scale across many routes | Recreates ad-hoc harness logic and makes failures opaque |
| Put all API e2e in Rust integration tests | Tight coupling to internals and faster type safety | We want black-box HTTP validation and script-level ergonomics for agents/humans |
| Use pytest fixtures under `python/tests/` | Familiar Python testing workflow | This wave explicitly wants `scripts/test_*.py` as primary infra and `tests/e2e/*.sh` as wrappers |

## Key decisions

1. **Hermetic runtime per suite (isolated HOME + temp repo) is non-negotiable.**
   - Avoids flaky tests caused by shared local daemon state.

2. **Wave CRUD first, extend later.**
   - Proves the harness on the highest-traffic route family before expanding.

3. **`scripts/` owns behavior; `tests/e2e/` owns CI entrypoints only.**

## Remaining scope (next passes)

1. Expand suite coverage beyond waves:
   - Chords (`join`/`leave`/`nest`)
   - Stimuli (CRUD + owner constraints)
   - Run lifecycle (`run`/`stop`/`continue`/`land`/`next`)
2. Add route coverage manifest + CI guard so new `/v0` routes cannot land untested.
3. Add protocol-level tests where needed (WebSocket/SSE/webhooks).

## Risks to address next

- **Runtime latency**: each suite builds/checks `lfd`; may slow CI as coverage grows.
- **Ephemeral port race**: reserve/start gap could cause rare bind collisions.
- **Coverage blind spots**: only wave CRUD currently has live contract checks.

## Validation commands

```bash
uv run python scripts/test_api_smoke.py
tests/e2e/test_api_smoke.sh
```
