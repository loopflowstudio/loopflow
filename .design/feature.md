# Agent Loops

Background agents that run configurable pipelines with persistent prompts and optional context. Registered through Maestro, stored in SQLite.

## Data Structures

```python
class OuterLoopMode(Enum):
    PR_CHAIN = "pr-chain"      # Create PRs that chain on each other
    LAND_COMMITS = "land-commits"  # Land directly via wt merge

class AgentStatus(Enum):
    IDLE = "idle"
    RUNNING = "running"
    WAITING = "waiting"       # In continuous mode, waiting for trigger
    ERROR = "error"
    STOPPED = "stopped"       # Explicitly stopped by user

class TriggerKind(Enum):
    ALWAYS = "always"
    MAIN_CHANGED = "main-changed"

@dataclass
class AgentTrigger:
    kind: TriggerKind = TriggerKind.ALWAYS
    config: dict = field(default_factory=dict)

@dataclass
class OuterLoopConfig:
    mode: OuterLoopMode

@dataclass
class AgentLoopSpec:
    name: str
    prompt_path: Path
    pipeline: list[str]
    context: list[str] = field(default_factory=list)
    outer_loop: OuterLoopConfig = field(
        default_factory=lambda: OuterLoopConfig(mode=OuterLoopMode.LAND_COMMITS)
    )
    trigger: AgentTrigger = field(default_factory=AgentTrigger)
    continuous: bool = False
    min_interval_seconds: int = 60
    max_iterations_per_day: int | None = None

@dataclass
class RegisteredAgent:
    id: str
    spec: AgentLoopSpec
    status: AgentStatus = AgentStatus.IDLE
    last_run_at: Optional[datetime] = None
    current_worktree: Optional[Path] = None
    current_branch: Optional[str] = None
    iteration: int = 0
    pid: Optional[int] = None
```

## CLI

```bash
# Register an agent
lf ops agent register my-agent \
    -p prompts/my-agent.md \
    --pipeline design,implement,review \
    --context src/ \
    -o land-commits \
    -t always

# List agents
lf ops agent list

# Start agent (single iteration, background)
lf ops agent start my-agent

# Start agent (continuous mode, background)
lf ops agent start my-agent -c

# Start agent (foreground, for debugging)
lf ops agent start my-agent -f

# Stop agent
lf ops agent stop my-agent

# Show agent details
lf ops agent show my-agent

# Remove agent
lf ops agent remove my-agent
```

## How It Works

1. **Registration**: Agent spec (prompt, pipeline, context, outer loop config) is saved to SQLite.

2. **Starting**: Agent subprocess is spawned, runs the pipeline in a fresh worktree.

3. **Pipeline execution**: Each task in the pipeline runs with the agent's prompt injected as a prefix.

4. **Outer loop handling**:
   - `land-commits`: Uses `wt merge --no-squash` to land to main.
   - `pr-chain`: Pushes branch, creates PR via `gh pr create`, chains PRs when multiple iterations exist.

5. **Continuous mode**: Loops until stopped, checking trigger conditions and respecting rate limits.

## Files

- `src/loopflow/maestro/agent.py` - Data structures
- `src/loopflow/maestro/agents.py` - Public API
- `src/loopflow/maestro/runner.py` - Iteration and continuous loop logic
- `src/loopflow/maestro/db.py` - Agent CRUD operations
- `src/loopflow/cli/agent.py` - CLI commands
