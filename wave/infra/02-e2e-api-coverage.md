# E2E test infrastructure

Live HTTP API coverage for `lfd`. Wave CRUD is covered; remaining routes need the same treatment.

## Architecture (established)

Four layers, shared between CI and manual runs:

1. **`scripts/lib/`** — reusable test primitives:
   - `lfd_runtime.py` — hermetic `lfd` lifecycle (isolated `HOME`, temp git repo, ephemeral port, health wait, session token wait, clean teardown)
   - `api_harness.py` — split client/assertion/runner utilities (`ApiClient`, `ApiAssertions`, `ScenarioRunner`)
   - `wave_scenarios.py` and `fork_scenarios.py` — reusable scenario logic

2. **`scripts/test_*.py`** — standalone script runners for humans/agents (`uv run python ...`).

3. **`tests/e2e/test_*.py`** — pytest wrappers for CI, using shared fixtures and scenario helpers.

4. **`tests/e2e/test_*.sh`** — thin CI entrypoints. `test_api_smoke.sh` now runs pytest directly.

The original plan suggested reusing `scripts/dev.py` for daemon management. Building revealed that full isolation (temp HOME, temp repo) per suite is more valuable — it eliminates flakiness from shared daemon state. `LfdRuntime` is purpose-built for this; `dev.py` remains for interactive use.

## Progress

1. ~~Build the harness against wave CRUD~~ — **done**
2. ~~Add pytest API smoke coverage (`tests/e2e/test_api_smoke.py`)~~ — **done**
3. Add coverage for chords (join, leave, nest, error cases)
4. Add coverage for stimuli (CRUD + owner constraints)
5. Add coverage for run lifecycle (run, stop, continue, land, next)
6. Add route coverage manifest so new `/v0` routes cannot land untested

Phase 3 from the original plan (migrate existing shell e2e tests to Python) was dropped. The existing shell tests (`test_smoke.sh`, `test_full_cycle.sh`, `test_rebase_conflict.sh`) exercise CLI workflows, not HTTP APIs — they serve a different purpose and don't need migration.

## Coverage targets

Every HTTP route should have at least one happy-path test and one error-path test. Priority order:

- ~~Wave CRUD (create, get, update, delete, list)~~ — **done**, 10 scenarios
- Wave lifecycle (run, stop, continue, land)
- Chords (join, leave, nest)
- Stimuli (create, update, delete)
- Runs (list, get, active run)

## Open questions

- **Runtime latency**: each suite builds `lfd` from source. As suites multiply, should we cache the binary or build once and share across suites?
- **Ephemeral port race**: reserve-then-bind gap could cause rare collisions. Not yet observed, but worth watching.
- **Fork pytest reliability**: `tests/e2e/test_fork.py` can run long and has exited with signal 143 in this environment; it needs dedicated validation where Docker + Claude creds are stable.
- **Protocol-level tests**: WebSocket/SSE routes exist but have no contract tests. Unclear whether the current harness pattern extends or needs a different approach.
