# lfq + loopflow + lfd v1 API (combined design)

## Context

Loopflow has three layers:

- **lf** (Rust) — run steps and flows with coding agents
- **lfd** (Rust) — daemon that orchestrates waves (continuous agent pipelines)
- **Concerto** (Swift) — GUI for wave management, talks to lfd

Missing: a Python API for the wave data model, and a CLI to query lfd.  
Separately, the lfd HTTP surface needs a Stripe-style v1 contract so clients converge.

## Who uses what

**`lfq`** — devops persona. Self-hosted developer checking if their server is running, viewing wave status, troubleshooting. Quick terminal feedback.  
**`import loopflow.api as loopflow`** — product/orchestration persona (humans or robots). Building systems that dynamically create and configure waves in response to incoming data. Python is the API for the wave data model; HTTP is an implementation detail.  
**Concerto** (Swift) — primary GUI. Its Swift client and this Python package are sibling clients to the same lfd API. They should converge on the same abstractions.

## Three binaries

| Binary | Language | Role |
|--------|----------|------|
| `lf` | Rust | Run steps and flows |
| `lfd` | Rust | Daemon (serve) |
| `lfq` | Python/Typer | Query lfd, manage waves |

### Port
- All clients default to port **2486**. Rust lfd, Swift Concerto, and Python loopflow all agree.

### Wave addressing
- lfd accepts **name or ID** in all wave endpoints. The handler tries ID first, falls back to name lookup. Names are unique per daemon.
- This eliminates client-side name→ID resolution in all three clients.

---

# lfq CLI (read-heavy, devops tool)

Waves are top-level — no namespace. Other resources get subcommands if they arrive.

```bash
lfq                  # status overview (daemon + all waves)
lfq list             # list waves
lfq show engbot      # wave detail + active run
lfq logs engbot      # tail agent output
lfq run engbot       # start a wave
lfq stop engbot      # stop a wave
lfq create engbot    # create a wave
lfq delete engbot    # delete a wave
lfq land engbot      # land the wave's branch
```

Priority: R >> D > C >>> U. Config updates happen through Python or Concerto. lfq prints human-friendly tables by default.

---

# loopflow Python API (full, for orchestration)

Full CRUD, all options. Sync to start. This is where dynamic orchestration lives.

```python
import loopflow.api as loopflow

# Status
loopflow.status()
loopflow.health()

# Read
loopflow.waves()
loopflow.waves(repo="/path/to/repo")
loopflow.wave("engbot")

# Create
loopflow.create_wave("engbot", repo=".", flow="ship",
                     direction=["product-engineer"],
                     area=["src/"])

# Update
loopflow.update_wave("engbot", flow="grind", status="paused")
loopflow.update_wave("engbot", direction=["designer", "infra-engineer"])

# Actions
loopflow.run_wave("engbot")
loopflow.stop_wave("engbot")
loopflow.land_wave("engbot", strict=False, local=True)

# Delete
loopflow.delete_wave("engbot")

# Remote server
from loopflow import Client
remote = Client("https://my-machine.loopflow.studio")
remote.waves()
remote.run_wave("engbot")
```

## Data model (Pydantic)

```python
from pydantic import BaseModel
from datetime import datetime

class Stimulus(BaseModel):
    kind: str  # manual, once, loop, watch, cron
    cron: str | None = None

class PullRequest(BaseModel):
    url: str
    number: int | None = None
    state: str | None = None
    title: str | None = None
    branch: str | None = None

class WaveRun(BaseModel):
    id: str
    wave_id: str
    iteration: int
    step_index: int
    status: str  # pending, running, waiting, completed, failed, cancelled
    local_worktree: str
    remote_branch: str
    pr: PullRequest | None = None
    started_at: datetime | None = None
    ended_at: datetime | None = None
    error: str | None = None
    flow_parents: list[str] = []

class Wave(BaseModel):
    id: str
    name: str
    repo: str
    flow: str
    direction: list[str]
    area: list[str]
    stimulus: Stimulus
    status: str  # idle, running, waiting, completed, error, paused
    iteration: int
    active_run: WaveRun | None = None
    created_at: datetime | None = None

    # Enriched client-side (same as Concerto does in Swift)
    branch: str | None = None
    pr_url: str | None = None
    pr_state: str | None = None  # open, merged, closed, draft
```

Protocol stays lean. Both Python and Swift clients enrich independently when needed.

## Connection

```python
# Resolution order:
# 1. LFD_URL if set (full URL, standard ports)
# 2. http://{LFD_HOST}:{LFD_PORT} (defaults: 127.0.0.1, 2486)

from loopflow import Client
client = Client()                                    # localhost:2486
client = Client("https://my-machine.loopflow.studio")  # remote, port 443
```

## Error handling

Pythonic exceptions mapped from HTTP status codes. All three clients (Rust, Swift, Python) agree on the HTTP contract.

```python
# Connection failure
loopflow.waves()              # raises ConnectionError if lfd is down

# Not found
loopflow.wave("nope")         # returns None

# Business logic errors
loopflow.run_wave("engbot")   # raises WaveAlreadyRunning (412)

# General errors
loopflow.land_wave("engbot")  # raises LoopflowError (500)
```

| HTTP | Python | Swift | Meaning |
|------|--------|-------|---------|
| 200 | return model | decoded model | success |
| 404 | return `None` | `throw` | not found |
| 412 | raise `WaveAlreadyRunning` | `throw` | precondition failed |
| 500 | raise `LoopflowError` | `throw` | server error |
| conn refused | raise `ConnectionError` | `throw` | lfd not running |

## Dependencies

```toml
dependencies = ["httpx>=0.27", "pydantic>=2.0", "typer>=0.9"]
```

FastAPI ecosystem stack. All from the same author/ecosystem.

---

# lfd HTTP API (Stripe-style v1)

## Goal
Adopt a Stripe-inspired API surface for lfd: `/v1` base path, resource-oriented URLs, consistent JSON payloads, Stripe-like list envelopes, structured errors, pagination primitives, idempotency keys, and expandable fields.

## Base path
- Single namespace lives at `/v1` (no parallel `/api` or legacy root namespace).
- Keep `/health` and `/status` at root for daemon availability checks.

## Waves
```
GET    /v1/waves
POST   /v1/waves
GET    /v1/waves/{id}
PATCH  /v1/waves/{id}
DELETE /v1/waves/{id}
POST   /v1/waves/{id}/run
POST   /v1/waves/{id}/stop
POST   /v1/waves/{id}/land
```

## Wave runs
```
GET /v1/wave_runs
GET /v1/waves/{id}/runs
```

### WaveRun fields
- `id`, `wave_id`, `iteration`, `step_index`, `status`, `local_worktree`, `remote_branch`, `pr`, `started_at`, `ended_at`, `error`, `flow_parents`
- `local_worktree`: human-friendly local path/name (may omit the fully-unique suffix).
- `remote_branch`: canonical fully-unique branch name pushed to origin.
- `pr`: optional PR object attached to the run (1:1). Waves may surface aggregates client-side.

## List responses (Stripe-style)
```
{
  "object": "list",
  "data": [ ... ],
  "has_more": false
}
```

## Single resources
```
{
  "id": "...",
  "object": "wave",
  ...
}
```

## Error payload
```
{
  "error": {
    "type": "invalid_request_error",
    "message": "...",
    "param": "repo"
  }
}
```
Use Stripe-style names when relevant; keep the set minimal.

## Pagination
- Support `limit`, `starting_after`, `ending_before` query params.
- Return `has_more` for list endpoints.

## Idempotency
- Honor `Idempotency-Key` for POST/DELETE to prevent duplicate side effects.
- Document exact replay semantics (when we retry vs return stored response).

## Expandable fields
- `expand[]=active_run` or `expand[]=recent_steps` on GET endpoints.
- Default responses should be minimal, with expansion for heavier fields.

## Core vs expanded fields (draft)
- Treat `created_at` as expandable (not core).
- Represent paused as a `status=paused` enum value (no separate `paused` boolean).
- GitHub/PR metadata handled client-side for now (mirror helpers in Python + Swift).
- PR-related expansions are TBD until worktree landing behavior is clearer.

## Version header
- Use `Loopflow-Version` for client-requested response shaping (even while v1 is unlocked).

## JSON-only
- v1 is JSON-only for request + response payloads.

---

# Cleanup / packaging changes

### What to delete
- `rust/loopflow-py/` — entire PyO3 crate
- `[tool.maturin]` in pyproject.toml
- `maturin` from build-system and dev dependencies
- `loopflow.ops` Python surface — all wave operations go through lfd

### What to add
```
python/
  loopflow/
    __init__.py      # module-level API (waves, create_wave, etc.)
    client.py        # Client class (HTTP to lfd)
    models.py        # Pydantic models (Wave, WaveRun, Stimulus)
    errors.py        # Exception types
    cli.py           # Typer CLI (lfq)
```

### pyproject.toml becomes
```
[project]
name = "loopflow"
version = "0.8.0"
dependencies = ["httpx>=0.27", "pydantic>=2.0", "typer>=0.9"]

[project.scripts]
lfq = "loopflow.cli:app"

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["python/loopflow"]
```

---

# Sequence

1. Update lfd: default port 2486, name-or-ID wave lookup, fix `--help`
2. Build `python/loopflow/` — models, client, errors, CLI
3. Delete `rust/loopflow-py/`
4. Simplify pyproject.toml (hatchling, no maturin)
5. Update `publish.py` (no more maturin wheel step)
6. Simplify Concerto Swift client (remove name→ID resolution)
