# Review: lfq + loopflow Python API + lfd v1 API

## What was implemented

Three-layer client convergence for lfd:

1. **lfd v1 HTTP API** (Rust) — Stripe-style REST surface at `/v1/waves`, `/v1/wave_runs` with list envelopes, cursor pagination, expand params, and structured error responses.

2. **Python loopflow package** (`python/loopflow/`) — Pure-Python client (httpx) with Pydantic models, module-level API (`import loopflow.api as loopflow`), and Typer CLI (`lfq`). Replaces the PyO3 `loopflow-py` crate.

3. **Swift/Concerto client updates** — `LocalWaveService` now talks to `/v1/` endpoints, parses Stripe-style list responses, extracts PR metadata from `active_run.pr`, uses shared port 2486 via `LoopflowCore`.

4. **lf flow dispatch** (Rust) — `lf flow <name>` now runs flows directly from the CLI, expanding steps and executing them sequentially.

5. **Cleanup** — Deleted `rust/loopflow-py/` (PyO3 crate), removed maturin from build system, simplified `pyproject.toml` to hatchling, simplified `publish.py`.

## Key choices

- **httpx + pydantic + typer** over PyO3: Same FastAPI ecosystem, eliminates the Rust-Python build step entirely. The Python package is now a pure wheel — simpler to install, debug, and maintain.

- **Name-or-ID wave addressing** in lfd: All wave endpoints accept either an ID or a name. The handler tries ID parse first, falls back to name lookup. This eliminates client-side name→ID resolution across all three clients (Rust, Swift, Python).

- **Stripe-style API conventions**: `{"object": "list", "data": [...], "has_more": false}` envelopes, `expand[]=active_run` for opt-in field inclusion, structured `{"error": {"type": "...", "message": "..."}}` responses. Provides a familiar contract for all clients.

- **`api.py` as the public API** (not `__init__.py`): Users write `import loopflow.api as loopflow` to get module-level functions. `__init__.py` stays empty per CLAUDE.md style.

- **Stimulus removed from `run_wave` params**: The Rust `RunWaveRequest` only accepts `flow`, `direction`, `area` overrides. Stimulus configuration belongs in `update_wave`, not in the run action. Python client aligned to match.

## How it fits together

```
User code ──► loopflow.api ──► loopflow.Client ──► lfd /v1/* ──► store + executor
                                                         ▲
Concerto UI ──► LocalWaveService ─────────────────────────┘
                                                         ▲
lfq CLI ──────► loopflow.api ──► loopflow.Client ────────┘
```

All three clients (Python API, lfq CLI, Concerto Swift) hit the same lfd v1 endpoints at port 2486. PR metadata flows through `active_run.pr` in wave responses. Client-side enrichment (branch, PR URL, staleness) stays in each client.

## Risks and bottlenecks

- **PR lookup in request path**: `pr_for_run()` spawns `gh pr view` via `spawn_blocking` for each wave that has an active run. If lfd manages many waves with open PRs, the list-waves response could be slow. Currently mitigated by the `expand[]=active_run` opt-in.

- **Wave name uniqueness**: Name-or-ID lookup does a linear scan of all waves to match by name. Fine for dozens of waves, but O(N) per request.

- **Pagination is in-memory**: Both `paginate_waves` and `paginate_wave_runs` load all records from the store, then slice. Works at current scale but won't scale to large run histories.

- **`WaveRunDto.created_at` duplicates `started_at`**: `dto.rs:193` sets `created_at` to `run.started_at`. If runs get a distinct `created_at` field later, this will need updating.

## What's not included

- **Idempotency keys**: `Idempotency-Key` header not yet honored on POST/DELETE.
- **`expand[]=recent_steps`**: Only `active_run` expansion is implemented.
- **Async Python client**: The `Client` class uses synchronous httpx. Async support deferred.
- **Wave run streaming** in Python: `wave_logs()` works but there's no SSE/WebSocket client for real-time events.
- **Tests for Python package**: No pytest test files yet. Module imports verified clean; behavior tested through e2e flows.

## Gate fixes applied

- `model_dump(mode="json")` for datetime serialization in CLI JSON output
- Return type hint added to `Client.wave_logs()`
- Removed `stimulus` param from `run_wave` (server doesn't accept it)
- Added error message to `lfq show` when wave not found
- Removed dead `api_error_with_param` function (clippy)
- Removed stale comment in `routes/mod.rs`
- `cargo fmt` applied
- Fixed `lfq create` CLI examples in README and docs (positional args, not `--repo`)
