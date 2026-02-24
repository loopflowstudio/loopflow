# E2E test infrastructure

No existing e2e test starts the `lfd` daemon or exercises the HTTP API. Current e2e tests (`test_smoke.sh`, `test_full_cycle.sh`, `test_rebase_conflict.sh`) exercise the `lf` CLI only. Python tests mock the HTTP client. This means the HTTP validation layer — status codes, error messages, cross-resource constraints — has no integration coverage.

## Architecture

Three layers:

1. **`scripts/test_*.py`** — Python scripts that do the real work. Start `lfd`, exercise APIs, validate responses, tear down. Self-contained: an LLM agent should be able to run one, read the output, and know if things work.

2. **`tests/e2e/test_*.sh`** — Trivial shell one-liners (`uv run python scripts/test_...`) that exist as CI entry points. No logic in shell.

3. **`scripts/dev.py`** — Already handles `lfd` lifecycle (build, start, kill). Test scripts should reuse this for daemon management rather than reimplementing it.

## Shared test harness

Extract a `scripts/lib/test_harness.py` (or similar) that provides:
- Start `lfd` from source, wait for ready, return base URL
- HTTP helpers with response validation (assert status, assert shape)
- Cleanup on exit (kill daemon, remove temp data)
- Structured output: pass/fail per test case, summary at end

Individual test scripts import the harness and focus on the domain logic.

## Migration plan

1. Build the harness against one new test (`test_api_smoke.py` — create wave, get wave, update wave, delete wave)
2. Add coverage for chords (join, leave, nest, error cases)
3. Migrate existing shell e2e tests to Python scripts with shell wrappers
4. Add to CI alongside existing tests, then remove the old shell versions

## Coverage targets

Every HTTP route should have at least one happy-path test and one error-path test. Priority order:
- Wave CRUD (create, get, update, delete, list)
- Wave lifecycle (run, stop, continue, land)
- Chords (join, leave, nest)
- Stimuli (create, update, delete)
- Runs (list, get, active run)
