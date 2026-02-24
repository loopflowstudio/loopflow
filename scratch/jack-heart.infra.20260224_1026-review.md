# API smoke harness review

## What was implemented

- Added a hermetic daemon runtime at `scripts/lib/lfd_runtime.py`:
  - builds `lfd`
  - starts `lfd serve` on an ephemeral localhost port
  - isolates `HOME` and repo state in a temp directory
  - waits for `/health` and session token before tests run
  - tears down process and temp files on exit
- Added a reusable HTTP assertion harness at `scripts/lib/api_harness.py` with scenario execution, status assertions, error-envelope assertions, field checks, and summary output.
- Added a live wave CRUD API suite at `scripts/test_api_smoke.py` covering:
  - create/list/get/update/delete happy paths
  - duplicate create, missing wave, invalid status, and auth error paths
- Added CI wrapper `tests/e2e/test_api_smoke.sh` that delegates to the Python suite.
- Wired CI to run the new API smoke wrapper in `.github/workflows/ci.yml` (`e2e-smoke` job).
- Updated `TESTING.md` so local/CI commands include the new API smoke test.

## Key choices

- **Hermetic runtime per suite** instead of shared local daemon state, to avoid flaky tests and local machine coupling.
- **Reusable harness helpers** (`expect_status`, `expect_error`, `expect_fields`) instead of inline assertions in each scenario.
- **Single smoke domain first (wave CRUD)** to validate runtime + harness architecture before expanding to chords/stimuli/lifecycle routes.
- **Shell wrapper stays logic-free** so behavior lives in `scripts/` and CI entrypoints remain thin.

## How it fits together

`tests/e2e/test_api_smoke.sh` calls `scripts/test_api_smoke.py`. The script starts `LfdRuntime`, gets an auth token, then runs named scenarios through `ApiHarness` against live `/v0/waves` routes. Each scenario prints PASS/FAIL; the harness prints a summary and exits non-zero on failures. CI runs this wrapper alongside existing smoke tests.

## Risks and bottlenecks

- Startup/build cost: each run builds or checks `lfd`, so API smoke adds latency to local and CI loops.
- Port reservation race: runtime reserves an ephemeral port before spawn; a rare collision between reserve/start is still possible.
- Coverage depth: this suite only covers wave CRUD; other `/v0` domains are still untested live.

## What's not included

- Chords API coverage
- Stimuli API coverage
- Run lifecycle API coverage
- Route coverage manifest/guard enforcement
- WebSocket/SSE/webhook contract tests
