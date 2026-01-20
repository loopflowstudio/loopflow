# Database Schema: Loops

## Overview

Two new tables track loop configuration and execution history.

## Tables

### loops

Stores loop/subscription configuration. One row per (goal, repo) pair.

```sql
CREATE TABLE loops (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,              -- loop, flow, subscribe, schedule
    goal TEXT NOT NULL,              -- goal name (references .lf/goals/{goal}.md)
    repo TEXT NOT NULL,              -- absolute path to repo
    personal_main TEXT NOT NULL,     -- branch name (e.g., "designer-main")
    status TEXT NOT NULL,            -- idle, running, waiting, error
    iteration INTEGER DEFAULT 0,     -- current iteration count
    pr_limit INTEGER DEFAULT 5,      -- max outstanding PRs before waiting
    merge_mode TEXT DEFAULT 'auto',  -- auto, pr, land

    -- Type-specific config
    project_file TEXT,               -- for flow: path to project spec
    pathset TEXT,                    -- for subscribe: comma-separated paths
    cron TEXT,                       -- for schedule: cron expression
    area TEXT,                       -- area override (comma-separated paths)

    pid INTEGER,                     -- process ID when running
    created_at TEXT NOT NULL,

    UNIQUE(type, goal, repo)
);

CREATE INDEX idx_loops_status ON loops(status);
CREATE INDEX idx_loops_repo ON loops(repo);
```

### loop_runs

Stores each iteration attempt. Many rows per loop.

```sql
CREATE TABLE loop_runs (
    id TEXT PRIMARY KEY,
    loop_id TEXT NOT NULL,
    iteration INTEGER NOT NULL,
    status TEXT NOT NULL,            -- running, completed, error
    started_at TEXT NOT NULL,
    ended_at TEXT,
    worktree TEXT,                   -- path to iteration worktree
    current_step TEXT,               -- which task is running
    error TEXT,                      -- error message if failed
    pr_url TEXT,                     -- PR created this iteration
    pr_number INTEGER,               -- PR number for API access

    FOREIGN KEY (loop_id) REFERENCES loops(id)
);

CREATE INDEX idx_loop_runs_loop ON loop_runs(loop_id);
CREATE INDEX idx_loop_runs_status ON loop_runs(status);
```

## Enums

### Loop Type

| Value | Description |
|-------|-------------|
| `loop` | Continuous homeostasis |
| `flow` | One-off project execution |
| `subscribe` | Triggered by file changes |
| `schedule` | Triggered by cron |

### Loop Status

| Value | Description |
|-------|-------------|
| `idle` | Not running |
| `running` | Currently executing an iteration |
| `waiting` | Paused (outstanding >= pr_limit) |
| `error` | Last iteration failed |

### Merge Mode

| Value | Description |
|-------|-------------|
| `auto` | PR auto-merges to personal-main |
| `pr` | PR waits for manual approval |
| `land` | Auto-merge to personal-main AND to main |

### Run Status

| Value | Description |
|-------|-------------|
| `running` | Iteration in progress |
| `completed` | Iteration succeeded, PR created |
| `error` | Iteration failed |

## Migration

From current schema, need to:

1. Create `loops` table
2. Create `loop_runs` table
3. Migrate data from `agent_runs` if any exists
4. Eventually drop `agent_runs` table

## Key Operations

```python
# Create or get loop
def get_or_create_loop(goal: str, repo: Path, type: str) -> Loop

# Update loop status
def update_loop_status(loop_id: str, status: LoopStatus) -> bool

# Record iteration
def save_loop_run(loop_id: str, iteration: int, ...) -> str

# Count outstanding PRs
def count_outstanding(loop_id: str) -> int

# List active loops
def list_loops(repo: Path | None = None, active_only: bool = False) -> list[Loop]
```

## Questions

- Should `loops` table store the full goal content or just the name?
  - **Decision:** Just the name. Goal file is source of truth.

- Should we track which commits are on personal-main?
  - **Decision:** No. Use git directly to count outstanding.
