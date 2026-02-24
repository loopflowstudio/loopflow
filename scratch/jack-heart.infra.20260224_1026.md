# Pytest migration + harness split for e2e API tests

## Problem

`ApiHarness` in `scripts/lib/api_harness.py` is three things jammed into one class: HTTP client, assertion library, and scenario runner. This makes it hard to reuse pieces independently. Meanwhile `scripts/test_fork.py` exists as a standalone script outside the test suite.

The current API smoke test (`scripts/test_api_smoke.py`) makes raw HTTP calls via `httpx`. It should use `loopflow.api` / `loopflow.client.Client` instead — that way we're testing the real Python client against a live daemon, making it truly end-to-end.

We want one set of test logic with two runners: **pytest for CI**, **standalone scripts for humans/agents** who want to say "spin up lfd, run this, verify it works."

## What exists today

### `scripts/lib/api_harness.py` — monolithic ApiHarness

One class doing three jobs:

1. **HTTP client**: `_authed_client`, `_anonymous_client` (httpx.Client), `request(method, path, auth, **kwargs)`
2. **Assertions**: `expect_status()`, `expect_error()`, `expect_fields()`, `expect_json_object()` — all `@staticmethod`
3. **Scenario runner**: `run_scenario(name, check)`, `print_summary()`, `has_failures()`, `_record()`

Also has `ScenarioResult` dataclass and a module-level `_json_dict()` helper.

### `scripts/lib/lfd_runtime.py` — hermetic lfd lifecycle

`LfdRuntime` dataclass. Builds lfd, creates temp HOME + temp git repo, starts daemon on ephemeral port, waits for `/health` + session token, cleans up on exit. Context manager. This is solid — keep it as-is.

### `scripts/test_api_smoke.py` — 10 wave CRUD scenarios

Uses `ApiHarness` for raw HTTP calls. Scenarios are module-level functions taking `(harness, runtime, state)`. State is a `dict[str, str]` threaded through for cross-scenario data (e.g., `primary_wave_id`). Scenarios:

- `create_wave_happy` — POST `/v0/waves`, validate shape
- `create_wave_duplicate_name_error` — POST same name twice, expect 409
- `list_waves_happy` — GET `/v0/waves`, find created wave in list
- `list_waves_auth_error` — GET without token, expect 401
- `get_wave_happy` — GET `/v0/waves/{id}`, validate shape
- `get_wave_missing_error` — GET nonexistent, expect 404
- `update_wave_happy` — PATCH flow/direction/area/status, verify response
- `update_wave_invalid_status_error` — PATCH bad status, expect 400
- `delete_wave_happy` — create then DELETE, verify deleted response
- `delete_wave_missing_error` — DELETE already-deleted, expect 404

### `scripts/test_fork.py` — Docker fork execution test

Completely standalone. Needs Docker, Postgres, Claude credentials. Uses `loopflow.api` (not raw HTTP) to create and run waves. Monitors lfd stdout for fork progress. Takes minutes. Designed for nightly/manual runs.

Key functions: `ensure_postgres()`, `ensure_agent_image()`, `build_lfd()`, `kill_existing_lfd()`, `start_lfd()`, `create_and_run_wave()`, `wait_for_completion()`.

### `loopflow.api` — Python client we should be using

`python/loopflow/api.py` exposes typed functions that wrap `Client` (in `python/loopflow/client.py`):

| Function | Returns | HTTP |
|----------|---------|------|
| `create_wave(name, repo, flow?, direction?, area?)` | `Wave` | POST `/v0/waves` |
| `waves(repo?)` | `list[Wave]` | GET `/v0/waves` |
| `wave(name_or_id)` | `Optional[Wave]` | GET `/v0/waves/{id}` |
| `update_wave(name_or_id, flow?, direction?, area?, status?)` | `Wave` | PATCH `/v0/waves/{id}` |
| `delete_wave(name_or_id)` | `None` | DELETE `/v0/waves/{id}` |

`Client` resolves auth via: explicit token → `LFD_TOKEN` env → `~/.lf/session-token` (local only). Errors raise `LoopflowError`. 404s return `None` when `allow_not_found=True`.

`Wave` model fields: `id`, `name`, `repo`, `flow`, `direction`, `area`, `status`, `iteration`, `local_worktree`, `remote_branch`, `created_at`, `wave_type`, `parent_id`, `children`, etc.

### CI

`.github/workflows/ci.yml` e2e-smoke job runs:
```
tests/e2e/test_smoke.sh
tests/e2e/test_api_smoke.sh  # → uv run python scripts/test_api_smoke.py
```

Python tests: `uv run pytest python/tests/`

No pytest markers configured. No `[tool.pytest.ini_options]` in `pyproject.toml`.

## Design

### 1. Split `scripts/lib/api_harness.py` into 3 classes

Same file, 3 classes:

**`ApiClient`** — HTTP client wrapper
```python
class ApiClient:
    def __init__(self, base_url: str, token: str, timeout_seconds: float = 10.0) -> None:
        self._authed_client = httpx.Client(base_url=base_url, timeout=timeout_seconds,
                                            headers={"Authorization": f"Bearer {token}"})
        self._anonymous_client = httpx.Client(base_url=base_url, timeout=timeout_seconds)

    def request(self, method: str, path: str, auth: bool = True, **kwargs) -> httpx.Response: ...
    def close(self) -> None: ...
    def __enter__(self) -> "ApiClient": ...
    def __exit__(...) -> None: ...
```

**`ApiAssertions`** — static assertion methods, no state
```python
class ApiAssertions:
    @staticmethod
    def expect_status(response, status_code) -> None: ...
    @staticmethod
    def expect_error(response, status_code, message_contains?, error_type?) -> dict: ...
    @staticmethod
    def expect_fields(payload, required_fields) -> None: ...
    @staticmethod
    def expect_json_object(response) -> dict: ...
```

**`ScenarioRunner`** — execution/reporting for standalone scripts
```python
class ScenarioRunner:
    def run_scenario(self, name: str, check: Callable[[], None]) -> None: ...
    def has_failures(self) -> bool: ...
    def print_summary(self) -> None: ...
```

Keep `ScenarioResult` dataclass and `_json_dict()` helper as-is.

### 2. Shared scenario logic in `scripts/lib/`

Test logic lives once. Both pytest and standalone scripts call the same functions.

**`scripts/lib/wave_scenarios.py`** — wave CRUD scenario functions

Each scenario takes a `loopflow.client.Client` (configured to talk to the hermetic daemon) + `LfdRuntime` + `state` dict. Uses `loopflow.api`-style calls via the client, plus `ApiAssertions` for error-path tests that need raw HTTP (auth errors, invalid status).

For happy paths, use `Client` directly — it returns typed `Wave` objects and raises `LoopflowError` on failures. This exercises the real Python client.

For error paths that test HTTP-level behavior (401 auth missing, 400 bad status, 404 not found, 409 duplicate), use `ApiClient` for raw HTTP + `ApiAssertions` to validate error envelope shape. The Python client intentionally swallows these into exceptions — we need to test the HTTP contract directly.

Sketch:
```python
from loopflow.client import Client
from loopflow.models import Wave
from scripts.lib.api_harness import ApiClient, ApiAssertions
from scripts.lib.lfd_runtime import LfdRuntime

def create_wave_happy(client: Client, runtime: LfdRuntime, state: dict[str, str]) -> None:
    wave = client.create_wave(name=_wave_name("smoke"), repo=str(runtime.repo_dir))
    assert isinstance(wave, Wave)
    assert wave.name.startswith("smoke-")
    state["primary_wave_id"] = wave.id

def create_wave_duplicate_error(raw: ApiClient, runtime: LfdRuntime) -> None:
    # First create via raw to set up, then try duplicate
    ...
    ApiAssertions.expect_error(response, 409, message_contains="already exists")

def list_waves_auth_error(raw: ApiClient) -> None:
    response = raw.request("GET", "/v0/waves", auth=False)
    ApiAssertions.expect_error(response, 401, message_contains="missing token")
```

**`scripts/lib/fork_scenarios.py`** — fork infrastructure helpers

Extract from `scripts/test_fork.py`: `ensure_postgres()`, `ensure_agent_image()`, `build_lfd()`, `kill_existing_lfd()`, `start_lfd_container_mode()`, `create_and_run_wave()`, `wait_for_completion()`. These become importable functions, not hardcoded in a script's `main()`.

### 3. Standalone scripts (`scripts/test_*.py`)

Keep as "spin up lfd and verify it works" entrypoints. Wrap shared logic with `ScenarioRunner`.

**`scripts/test_api_smoke.py`** — updated to use split classes:
```python
from lib.api_harness import ApiClient, ScenarioRunner
from lib.lfd_runtime import LfdRuntime
from lib.wave_scenarios import (create_wave_happy, list_waves_happy, ...)
from loopflow.client import Client

def main() -> int:
    with LfdRuntime() as runtime:
        client = Client(base_url=runtime.base_url, token=runtime.token)
        raw = ApiClient(base_url=runtime.base_url, token=runtime.token)
        runner = ScenarioRunner()
        state: dict[str, str] = {}

        runner.run_scenario("create_wave_happy", partial(create_wave_happy, client, runtime, state))
        ...
        runner.print_summary()
        return 1 if runner.has_failures() else 0
```

**`scripts/test_fork.py`** — refactored to use `fork_scenarios.py`:
```python
from lib.fork_scenarios import ensure_postgres, ensure_agent_image, start_lfd_container_mode, ...
```

### 4. Pytest fixtures in `tests/e2e/conftest.py`

```python
import pytest
from scripts.lib.lfd_runtime import LfdRuntime
from scripts.lib.api_harness import ApiClient
from loopflow.client import Client

@pytest.fixture(scope="session")
def lfd_runtime():
    with LfdRuntime() as runtime:
        yield runtime

@pytest.fixture(scope="session")
def api_client(lfd_runtime):
    with ApiClient(base_url=lfd_runtime.base_url, token=lfd_runtime.token) as client:
        yield client

@pytest.fixture(scope="session")
def lf_client(lfd_runtime):
    client = Client(base_url=lfd_runtime.base_url, token=lfd_runtime.token)
    yield client
    client.close()
```

Register markers in `pyproject.toml`:
```toml
[tool.pytest.ini_options]
markers = [
    "e2e: end-to-end tests requiring lfd",
    "docker: tests requiring Docker and Claude credentials",
]
```

Note: `tests/e2e/` needs to be able to import from `scripts/lib/`. Either add `scripts/` to `sys.path` in conftest, or add a path config in pyproject.toml `pythonpath`.

### 5. Pytest tests in `tests/e2e/`

**`tests/e2e/test_api_smoke.py`** — one test per scenario:
```python
import pytest
from scripts.lib.wave_scenarios import *

pytestmark = pytest.mark.e2e

# Module-level state shared across ordered tests
_state: dict[str, str] = {}

def test_create_wave_happy(lf_client, lfd_runtime):
    create_wave_happy(lf_client, lfd_runtime, _state)

def test_create_wave_duplicate_error(api_client, lfd_runtime):
    create_wave_duplicate_error(api_client, lfd_runtime)

def test_list_waves_happy(lf_client, _state=_state):
    list_waves_happy(lf_client, _state)

def test_list_waves_auth_error(api_client):
    list_waves_auth_error(api_client)

# ... etc
```

**`tests/e2e/test_fork.py`** — pytest wrapper:
```python
import pytest
from scripts.lib.fork_scenarios import *

pytestmark = [pytest.mark.e2e, pytest.mark.docker]

@pytest.fixture(scope="module")
def fork_infra():
    ensure_postgres()
    ensure_agent_image()
    build_lfd()
    proc = start_lfd_container_mode()
    yield proc
    proc.terminate()
    proc.wait(timeout=5)

def test_fork_execution(fork_infra):
    create_and_run_wave("wave-reduce", "product-engineer")
    success, output = wait_for_completion(fork_infra, timeout=300)
    assert success, f"fork execution failed:\n{output[-2000:]}"
```

Skip conditions:
```python
pytestmark = [
    pytest.mark.e2e,
    pytest.mark.docker,
    pytest.mark.skipif(not _docker_available(), reason="Docker not available"),
    pytest.mark.skipif(not _claude_credentials(), reason="No Claude credentials"),
]
```

### 6. Update CI and config

**`tests/e2e/test_api_smoke.sh`**:
```bash
#!/usr/bin/env bash
set -euo pipefail
uv run pytest tests/e2e/test_api_smoke.py -v
```

**`.github/workflows/ci.yml`** e2e-smoke job — no change needed if the shell wrapper calls pytest.

**`pyproject.toml`** — add:
```toml
[tool.pytest.ini_options]
markers = [
    "e2e: end-to-end tests requiring lfd",
    "docker: tests requiring Docker and Claude credentials",
]
```

Don't add test_fork to per-PR CI (needs Docker + API key — stays nightly).

## Files

| File | Action |
|------|--------|
| `scripts/lib/api_harness.py` | Split `ApiHarness` into `ApiClient`, `ApiAssertions`, `ScenarioRunner` |
| `scripts/lib/wave_scenarios.py` | New — shared wave CRUD test logic using `loopflow.client.Client` for happy paths, `ApiClient` for error paths |
| `scripts/lib/fork_scenarios.py` | New — extracted fork infra helpers from `scripts/test_fork.py` |
| `scripts/test_api_smoke.py` | Refactor to use split classes + `wave_scenarios` |
| `scripts/test_fork.py` | Refactor to use `fork_scenarios` |
| `tests/e2e/conftest.py` | New — `lfd_runtime`, `api_client`, `lf_client` fixtures |
| `tests/e2e/test_api_smoke.py` | New — pytest version calling `wave_scenarios` |
| `tests/e2e/test_fork.py` | New — pytest version calling `fork_scenarios` |
| `tests/e2e/test_api_smoke.sh` | Update to `uv run pytest tests/e2e/test_api_smoke.py -v` |
| `.github/workflows/ci.yml` | May not need changes (shell wrapper handles it) |
| `pyproject.toml` | Add `[tool.pytest.ini_options]` markers |

## Validation

```bash
# Pytest path (CI)
uv run pytest tests/e2e/test_api_smoke.py -v
uv run pytest tests/e2e/ -v -m "not docker"
tests/e2e/test_api_smoke.sh

# Standalone path (humans/agents)
uv run python scripts/test_api_smoke.py
uv run python scripts/test_fork.py --skip-build

# Fork test (needs Docker + credentials)
uv run pytest tests/e2e/test_fork.py -v
```
