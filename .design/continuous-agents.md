# Continuous Agent Loops

## What this enables

Background agents that run indefinitely, making autonomous decisions about when and what to work on.

Currently, `lf agent start` runs one iteration and stops. You have to manually restart it. This is fine for testing but falls short of the stated goal:

> "The goal is that I would be able to assign set up general areas of responsibility (maestro UI, background agents, onboarding and documentation quality, etc) and set the llms loose on those"

To "set LLMs loose" requires agents that:
1. Run continuously without manual intervention
2. Decide when there's work to do (not just run on a timer)
3. Handle failures gracefully and retry
4. Know when to stop (rate limits, errors, human intervention)

## What it takes to build

### Core change: Continuous run loop

Add a `--continuous` flag to `lf agent start` that wraps iterations in a loop:

```python
# maestro/runner.py

def run_agent_continuous(
    agent: RegisteredAgent,
    repo_root: Path,
    check_interval: int = 300,  # 5 minutes
    max_iterations: int | None = None,
) -> int:
    """Run agent continuously until stopped."""
    iterations = 0

    while True:
        # Check if agent should run
        if not _should_run(agent, repo_root):
            time.sleep(check_interval)
            continue

        # Run one iteration
        exit_code = run_agent_iteration(agent, repo_root)
        iterations += 1

        if exit_code != 0:
            # Back off on failure
            update_agent_status(DEFAULT_DB_PATH, agent.id, AgentStatus.ERROR)
            time.sleep(check_interval * 4)
            update_agent_status(DEFAULT_DB_PATH, agent.id, AgentStatus.IDLE)
            continue

        if max_iterations and iterations >= max_iterations:
            break

        # Brief pause between successful iterations
        time.sleep(60)

    return 0
```

### Trigger conditions

Agents need to know when there's work to do. Add a trigger system:

```python
@dataclass
class AgentTrigger:
    """Condition that triggers an agent iteration."""
    kind: str  # "always" | "main-changed" | "schedule" | "webhook"
    config: dict  # kind-specific config


def _should_run(agent: RegisteredAgent, repo_root: Path) -> bool:
    """Check if agent should run based on its trigger."""
    trigger = agent.spec.trigger

    if trigger.kind == "always":
        return True

    if trigger.kind == "main-changed":
        # Check if main has new commits since last run
        return _main_has_changes(repo_root, agent.last_run_at)

    if trigger.kind == "schedule":
        # Cron-like schedule
        return _schedule_matches(trigger.config, datetime.now())

    return False
```

### Graceful shutdown

Agents need to handle SIGTERM cleanly:

```python
# In run_agent_continuous

def _handle_shutdown(signum, frame):
    nonlocal should_stop
    should_stop = True
    print(f"Agent {agent.spec.name} received shutdown signal, finishing current iteration...")

signal.signal(signal.SIGTERM, _handle_shutdown)
signal.signal(signal.SIGINT, _handle_shutdown)
```

### Rate limiting

Prevent runaway agents:

```python
@dataclass
class AgentLoopSpec:
    # ... existing fields ...
    max_iterations_per_day: int | None = None
    min_interval_seconds: int = 60
```

### Status reporting

Extend agent status for continuous mode:

```python
class AgentStatus(Enum):
    IDLE = "idle"
    RUNNING = "running"
    WAITING = "waiting"  # NEW: in continuous mode, waiting for trigger
    ERROR = "error"
    STOPPED = "stopped"  # NEW: explicitly stopped by user
```

## Data structure changes

```python
@dataclass
class AgentTrigger:
    kind: str  # "always" | "main-changed" | "schedule"
    config: dict = field(default_factory=dict)


@dataclass
class AgentLoopSpec:
    # ... existing fields ...
    trigger: AgentTrigger = field(
        default_factory=lambda: AgentTrigger(kind="always")
    )
    max_iterations_per_day: int | None = None
    min_interval_seconds: int = 60
    continuous: bool = False  # Whether to run in continuous mode by default
```

## CLI changes

```bash
# Run continuously
lf agent start ui-agent --continuous

# Run with iteration limit
lf agent start ui-agent --continuous --max-iterations 10

# Run with custom trigger
lf agent register docs-agent \
    -p prompts/docs.md \
    --pipeline implement \
    --trigger main-changed

# Check agent status (shows waiting state)
lf agent list
ID       NAME        STATUS    PIPELINE              ITER  WAITING-FOR
abc123   ui-agent    waiting   design → implement    5     main-changed
def456   docs-agent  running   implement             2     -
```

## Risks and downsides

**Resource consumption**: Continuous agents consume compute indefinitely. Mitigated by:
- Rate limits (max iterations per day)
- Minimum intervals between runs
- Trigger conditions (only run when there's work)

**Runaway failures**: Agent could fail repeatedly in a loop. Mitigated by:
- Exponential backoff on failures
- Max consecutive failures before stopping
- Status reporting in maestro UI

**Context drift**: Long-running agents may drift from repo state. Mitigated by:
- Fresh worktree per iteration (already implemented)
- Always reading latest main branch
- Periodic full rebase

**Cost**: LLM API costs accumulate. Mitigated by:
- Visibility into iteration count and frequency
- Hard stop after max iterations
- Alert on unusual patterns

## Path to get there

1. **Add trigger field to AgentLoopSpec** - Database migration, update CLI
2. **Implement _should_run()** - Start with "always" and "main-changed"
3. **Add run_agent_continuous()** - Wrap existing run_agent_iteration
4. **Add --continuous flag to CLI** - Wire through to runner
5. **Add graceful shutdown handling** - SIGTERM, SIGINT
6. **Add WAITING status** - Update status display
7. **Add rate limiting** - max_iterations_per_day, min_interval

Start with steps 1-4 as MVP. The rest can follow.

## Open questions

- Should continuous mode be the default for agents? Or always explicit?
- How should "main-changed" detect changes? Compare HEAD sha? Check for new commits?
- Should agents pause when the user is actively working in main repo?
- What's the right default check interval? 5 minutes seems reasonable.

