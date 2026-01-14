# lfd: Agent Orchestration Daemon

**What to build:** A Unix socket daemon (`lfd`) that owns agent lifecycle, trigger evaluation, and session tracking—replacing the current SQLite-polling pattern with real-time IPC.

## Terminology

- **lfd** — The always-running daemon (like `mysqld`, `httpd`). Owns agent orchestration.
- **Maestro** — The Swift menu bar app. Connects to lfd via socket.
- **lf** — The CLI client (like `psql`, `mysql`). Talks to lfd for agent/session operations.

Analogy: postgres server + psql client. The daemon runs continuously; clients connect when needed.

User quote: "it was defined before the maestro UI swift app was built, so it needs to be rewritten"

## Data Structures

```python
@dataclass
class AgentSpec:
    name: str
    repo: Path
    pipeline: list[str]           # ["implement", "polish", "land"]
    trigger: TriggerSpec
    context: list[str]            # glob patterns
    prompt: str

@dataclass
class TriggerSpec:
    kind: Literal["manual", "main-changed", "interval"]
    interval_seconds: int | None  # for interval trigger

@dataclass
class AgentRun:
    id: str
    agent_name: str
    status: Literal["running", "completed", "error"]
    started_at: datetime
    ended_at: datetime | None
    pid: int | None
    worktree: str | None
    iteration: int
    error: str | None
    main_sha: str | None          # for main-changed tracking

@dataclass
class Session:
    """A single task run (agent iteration or manual lf command)."""
    id: str
    task: str
    repo: str
    worktree: str
    status: Literal["running", "waiting", "completed", "error"]
    started_at: datetime
    ended_at: datetime | None
    pid: int | None
    model: str
    run_mode: Literal["auto", "interactive"]
```

## Socket Protocol

lfd listens at `~/.lf/lfd.sock`. JSON-over-newline protocol:

```python
# Request format
{"method": "agents.list"}
{"method": "agents.start", "params": {"name": "my-agent"}}
{"method": "agents.stop", "params": {"name": "my-agent"}}
{"method": "sessions.list"}
{"method": "status"}

# Response format
{"ok": True, "result": [...]}
{"ok": False, "error": "Agent not found"}

# Push events (server → client, when subscribed)
{"event": "agent.started", "data": {"name": "my-agent", "pid": 1234}}
{"event": "agent.completed", "data": {"name": "my-agent", "iteration": 5}}
{"event": "session.updated", "data": {"id": "abc", "status": "completed"}}
```

Subscribe to events:
```python
{"method": "subscribe", "params": {"events": ["agent.*", "session.*"]}}
```

## Key Functions

```python
# src/loopflow/lfd/server.py
async def run_server(socket_path: Path) -> None:
    """Main daemon entry point. Runs until terminated."""

async def handle_client(reader: StreamReader, writer: StreamWriter) -> None:
    """Handle a single client connection."""

# src/loopflow/lfd/agents.py
def list_agents() -> list[AgentSpec]:
    """Load agent specs from ~/.lf/agents/*.md files."""

def should_trigger(agent: AgentSpec, last_run: AgentRun | None) -> bool:
    """Check if agent should run based on trigger config."""

async def spawn_agent(agent: AgentSpec) -> AgentRun:
    """Create worktree and spawn agent process."""

# src/loopflow/lfd/triggers.py
async def check_main_changed(repo: Path, last_sha: str | None) -> tuple[bool, str]:
    """Fetch and compare main branch SHA."""

# src/loopflow/lfd/client.py
class DaemonClient:
    """Client for connecting to lfd daemon from CLI or tests."""

    async def connect(self, socket_path: Path) -> None: ...
    async def call(self, method: str, params: dict = {}) -> Any: ...
    async def subscribe(self, events: list[str]) -> AsyncIterator[dict]: ...
```

## CLI Commands

```bash
# New top-level command
lfd start              # Start daemon (foreground, for debugging)
lfd status             # Show daemon status + running agents
lfd install            # Install launchd plist
lfd uninstall          # Remove launchd plist

# Existing lf commands talk to lfd
lf ops status            # Queries lfd socket instead of reading SQLite
lf agent start X         # Sends request to lfd socket
lf agent stop X          # Sends request to lfd socket
```

## Maestro (Swift) Changes

```swift
// Replace direct SQLite reads with socket client
class DaemonClient {
    private var connection: NWConnection?

    func connect() async throws { ... }
    func call(_ method: String, params: [String: Any] = [:]) async throws -> Any { ... }
    func subscribe(events: [String]) -> AsyncStream<Event> { ... }
}

// AgentService becomes thin wrapper
struct AgentService {
    private let client = DaemonClient()

    func list() async throws -> [Agent] {
        let result = try await client.call("agents.list")
        return parseAgents(result)
    }

    func start(name: String) async throws {
        try await client.call("agents.start", params: ["name": name])
    }
}
```

## File Layout

```
src/loopflow/lfd/
    __init__.py
    __main__.py        # entry point: python -m loopflow.lfd
    server.py          # asyncio socket server
    protocol.py        # JSON message parsing
    agents.py          # agent loading and spawning
    triggers.py        # trigger evaluation
    client.py          # Python client for CLI
    db.py              # SQLite persistence (keep for durability)
```

## Entry Point

In `pyproject.toml`:

```toml
[project.scripts]
lf = "loopflow.cli:app"
lfd = "loopflow.lfd:main"
```

`pip install loopflow` provides both `lf` and `lfd` commands.

## What to Reuse

From `src/loopflow/maestro/`:
- `db.py` — SQLite schema and queries (adapt for lfd module)
- `daemon.py` — Trigger evaluation logic (`should_trigger`, `check_main_changed`)
- `launchd.py` — Plist generation (update label to `com.loopflow.lfd`)

From `src/loopflow/cli/`:
- `agent.py` — Agent file parsing, CLI structure (rewire to use socket client)

Delete:
- `maestro/api.py` — FastAPI web UI (replaced by Maestro app)
- `cli/maestro.py` — Old PID-file daemon management (replaced by lfd)

## Constraints

- **Socket must be reliable.** If Maestro can't connect, show clear error. Don't silently fall back to SQLite.
- **Daemon owns all spawning.** CLI/Maestro never spawn agents directly—always via lfd socket.
- **Keep SQLite for persistence.** Socket is for IPC; SQLite stores durable state across daemon restarts.
- **launchd for lifecycle.** Daemon managed by launchd, not Maestro. Maestro just connects.

## Migration

1. Add socket server alongside existing code
2. Update `lf ops status` to try socket first, fall back to SQLite
3. Update Maestro to use socket client
4. Remove direct SQLite reads from Maestro
5. Remove old `lf ops maestro` commands (replaced by `lfd`)
6. Delete FastAPI web UI (`src/loopflow/maestro/api.py`) - Maestro app replaces it
7. Remove fastapi/uvicorn from dependencies

## Done When

```bash
# Daemon running via launchd
launchctl list | grep lfd
> com.loopflow.lfd

# Socket responds
echo '{"method": "status"}' | nc -U ~/.lf/lfd.sock
> {"ok": true, "result": {"agents": 0, "sessions": 0}}

# Maestro shows agents (via socket, not SQLite)
# Manual verification: agents list populates in UI

# Start/stop works via socket
lfd status
> lfd running (pid 1234)
> Agents: 2 defined, 1 running
> Sessions: 1 active
```

## Open Questions

- What's the launchd label? `com.loopflow.lfd` vs keeping `com.loopflow.agents`?
