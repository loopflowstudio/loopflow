# Runs and Triggers

## Problem

The current "Loop → Job" rename conflated concepts. We renamed everything to `Job` when we actually need distinct entities:

- **Run**: an execution instance of a Flow schema
- **Triggers**: entities that spawn Runs (Loop, Subscription, Schedule)

The term "Job" was meant to be an umbrella, but the implementation collapsed everything into one model and lost the meaningful distinctions.

## Concepts

**Flow** = a schema/definition (what to do)

**Run** = an execution instance (doing it)

**Trigger** = something that spawns Runs:
- **Loop**: continuously spawns Runs until stopped
- **Subscription**: spawns a Run when pathset changes on main
- **Schedule**: spawns a Run on cron

A Run with no trigger parent is a **direct run** - invoked explicitly via CLI or API.

## Data Model

### Run

The execution instance. Has a polymorphic parent encoded as a simple string.

```python
class RunStatus(Enum):
    PENDING = "pending"    # Created, not yet started
    RUNNING = "running"    # Currently executing
    COMPLETED = "completed"  # Finished successfully
    FAILED = "failed"      # Finished with error
    CANCELLED = "cancelled"  # Stopped before completion

@dataclass
class Run:
    id: str
    parent: str | None  # "loop:<id>" | "subscription:<id>" | "schedule:<id>" | None

    flow: str           # Flow name (from .lf/flows/)
    area: str           # Area of responsibility
    goals: list[str]    # Goal names

    status: RunStatus
    iteration: int      # Which iteration of parent (0 for direct runs)

    worktree: str | None      # Git worktree path
    branch: str | None        # Branch name
    current_step: str | None  # Current step in flow
    error: str | None         # Error message if failed
    pr_url: str | None        # PR URL if created

    started_at: datetime | None
    ended_at: datetime | None
    created_at: datetime
```

Parent encoding:
- `"loop:abc123"` - spawned by Loop abc123
- `"subscription:def456"` - spawned by Subscription def456
- `"schedule:ghi789"` - spawned by Schedule ghi789
- `None` - direct run (no parent)

```python
def parse_parent(parent: str | None) -> tuple[str | None, str | None]:
    if not parent:
        return None, None
    kind, id = parent.split(":", 1)
    return kind, id
```

### Loop

Continuously spawns Runs until stopped.

```python
class LoopStatus(Enum):
    IDLE = "idle"        # Not running
    RUNNING = "running"  # Currently has an active Run
    WAITING = "waiting"  # Paused (PR limit reached)
    ERROR = "error"      # Last Run failed

@dataclass
class Loop:
    id: str
    flow: str
    area: str
    goals: list[str]
    repo: Path

    status: LoopStatus
    iteration: int       # Total iterations completed

    main_branch: str     # Branch for accumulating work (e.g., "myarea-swift-main")
    pr_limit: int        # Max outstanding PRs before waiting
    merge_mode: MergeMode  # PR or LAND

    pid: int | None      # Process ID when running
    created_at: datetime
```

### Subscription

Watches pathset on main, spawns Run when changes detected.

```python
@dataclass
class Subscription:
    id: str
    flow: str
    area: str
    goals: list[str]
    repo: Path

    pathset: str         # Comma-separated paths to watch
    last_main_sha: str | None  # Last seen main SHA

    status: LoopStatus   # Reuse - idle/running/waiting/error
    iteration: int

    main_branch: str
    pr_limit: int
    merge_mode: MergeMode

    pid: int | None
    created_at: datetime
```

### Schedule

Spawns Run on cron schedule.

```python
@dataclass
class Schedule:
    id: str
    flow: str
    area: str
    goals: list[str]
    repo: Path

    cron: str            # Cron expression

    status: LoopStatus
    iteration: int

    main_branch: str
    pr_limit: int
    merge_mode: MergeMode

    pid: int | None
    created_at: datetime
```

## Database Schema

Four tables. Simple, portable SQL.

```sql
CREATE TABLE runs (
    id TEXT PRIMARY KEY,
    parent TEXT,  -- "loop:<id>" | "subscription:<id>" | "schedule:<id>" | NULL

    flow TEXT NOT NULL,
    area TEXT NOT NULL,
    goals TEXT,  -- JSON array

    status TEXT NOT NULL,
    iteration INTEGER NOT NULL DEFAULT 0,

    worktree TEXT,
    branch TEXT,
    current_step TEXT,
    error TEXT,
    pr_url TEXT,

    started_at TEXT,
    ended_at TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE loops (
    id TEXT PRIMARY KEY,
    flow TEXT NOT NULL,
    area TEXT NOT NULL,
    goals TEXT,
    repo TEXT NOT NULL,

    status TEXT NOT NULL DEFAULT 'idle',
    iteration INTEGER NOT NULL DEFAULT 0,

    main_branch TEXT NOT NULL,
    pr_limit INTEGER NOT NULL DEFAULT 5,
    merge_mode TEXT NOT NULL DEFAULT 'pr',

    pid INTEGER,
    created_at TEXT NOT NULL
);

CREATE TABLE subscriptions (
    id TEXT PRIMARY KEY,
    flow TEXT NOT NULL,
    area TEXT NOT NULL,
    goals TEXT,
    repo TEXT NOT NULL,

    pathset TEXT NOT NULL,
    last_main_sha TEXT,

    status TEXT NOT NULL DEFAULT 'idle',
    iteration INTEGER NOT NULL DEFAULT 0,

    main_branch TEXT NOT NULL,
    pr_limit INTEGER NOT NULL DEFAULT 5,
    merge_mode TEXT NOT NULL DEFAULT 'pr',

    pid INTEGER,
    created_at TEXT NOT NULL
);

CREATE TABLE schedules (
    id TEXT PRIMARY KEY,
    flow TEXT NOT NULL,
    area TEXT NOT NULL,
    goals TEXT,
    repo TEXT NOT NULL,

    cron TEXT NOT NULL,

    status TEXT NOT NULL DEFAULT 'idle',
    iteration INTEGER NOT NULL DEFAULT 0,

    main_branch TEXT NOT NULL,
    pr_limit INTEGER NOT NULL DEFAULT 5,
    merge_mode TEXT NOT NULL DEFAULT 'pr',

    pid INTEGER,
    created_at TEXT NOT NULL
);

-- Index for finding runs by parent
CREATE INDEX idx_runs_parent ON runs(parent);
```

## CLI

```bash
# Direct run (no parent trigger)
lfd run <flow> <area> [-g goal]...
# Creates Run with parent=None, executes once

# Continuous loop
lfd loop <flow> <area> [-g goal]...
# Creates Loop, starts spawning Runs

# Subscription (pathset trigger)
lfd subscribe <flow> <area> -p <path>... [-g goal]...
# Creates Subscription, watches for changes

# Schedule (cron trigger)
lfd schedule <flow> <area> -c "<cron>" [-g goal]...
# Creates Schedule, triggers on cron

# Management
lfd status              # Show all triggers and recent runs
lfd stop <id>           # Stop a trigger or cancel a run
lfd logs <id>           # Show logs for a run
```

Examples:
```bash
lfd run ship swift/                    # Run ship flow once on swift/
lfd loop ship swift/ -g designer       # Continuous loop with designer goal
lfd subscribe ship src/ -p "*.py"      # Run when Python files change
lfd schedule ship . -c "0 9 * * *"     # Run daily at 9am
```

## Migration

The current branch has:
- `Job` model (should become: Loop, Subscription, Schedule based on type)
- `JobRun` model (should become: Run)
- `jobs` table (should split into: loops, subscriptions, schedules)
- `job_runs` table (should become: runs)

Migration steps:
1. Create new tables (runs, loops, subscriptions, schedules)
2. Migrate data based on JobType:
   - LOOP → loops table
   - SUBSCRIBE → subscriptions table
   - SCHEDULE → schedules table
   - FLOW → direct run (no trigger, just a Run with parent=None)
3. Migrate job_runs → runs, encoding parent as "loop:<id>" etc.
4. Drop old tables

## What About "Job"?

We don't use "Job" anymore. The concepts are:
- **Run**: execution instance
- **Loop/Subscription/Schedule**: trigger types

"Job" was an umbrella term that didn't add clarity. The specific trigger names are more descriptive.

## Open Questions

1. **Flow type**: Currently `JobType.FLOW` means "one-shot manual". In the new model, this is just `lfd run` - a direct Run with no parent. Do we need a stored "Flow" trigger entity, or is `lfd run` sufficient?

2. **Shared config**: Loop, Subscription, Schedule share many fields (flow, area, goals, repo, status, iteration, main_branch, pr_limit, merge_mode). Extract a base Trigger class/table, or keep them separate for simplicity?

3. **Server events**: Current server uses `job.*` events. Rename to `run.*`, `loop.*`, `subscription.*`, `schedule.*`?
