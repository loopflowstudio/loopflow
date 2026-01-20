# Agent Looping API

## What to build

Simple CLI for starting/stopping/monitoring agent loops, built on top of the existing `lfd` infrastructure. A "loop" is an agent that cycles through a pipeline continuously, producing one PR per iteration.

## User's words

> "Agent looping goes along with pipelines. You would give the pipeline some primary prompt that explains the *goals* and *responsibilities* of the agent. The agent then gets a pipeline something like design→implement→polish and it just loops that pipeline, generating one new PR each time."

> "The technical architecture should assume DAG, not pipelines, but the public API for now only needs to expose simple pipelines."

## Existing infrastructure (reuse, don't rebuild)

The `lfd` daemon already has most of what we need:

```python
# ~/.lf/agents/*.md - agent definitions with frontmatter
# lfd.db agent_runs table - tracks running agents
# lfd.db sessions table - tracks individual task executions
# AgentSpec - agent definition (repo, pipeline, trigger, prompt)
# AgentRun - runtime state (status, iteration, pid, worktree)
# TriggerKind.LOOP - "restart immediately after completing"
```

The existing `runner.py:run_agent_iteration()` already:
- Creates worktrees from personal-main
- Runs pipeline tasks sequentially
- Creates PRs via merge modes
- Tracks sessions for each task

**What's missing**: A simpler CLI surface and better status visibility.

## Data structures

No new data structures. Extend existing ones:

```python
# Add to AgentSpec (already exists in models.py)
@dataclass
class AgentSpec:
    name: str
    repo: Path
    pipeline: str                # "design,implement,polish"
    trigger: TriggerSpec         # kind=LOOP for continuous
    prompt: str                  # The "goal" - goals and responsibilities
    # ... existing fields ...

# Add step tracking to AgentRun
@dataclass
class AgentRun:
    # ... existing fields ...
    current_step: str | None = None  # NEW: which task is running
```

Internal DAG structure (for future, not exposed yet):

```python
@dataclass
class PipelineNode:
    """Internal: node in pipeline DAG."""
    task: str
    next: list[str] = field(default_factory=list)
```

## Key functions

```python
# New file: src/loopflow/lf/loop.py
# Thin wrapper around existing lfd agent infrastructure

def start_loop(
    name: str,
    goal: str,
    pipeline: str,
    repo: Path | None = None,
) -> None:
    """Create agent file and start it running.

    Creates ~/.lf/agents/{name}.md with trigger=loop,
    then calls lfd to start it.
    """

def stop_loop(name: str) -> None:
    """Stop a running loop gracefully."""

def list_loops() -> list[LoopStatus]:
    """Return status of all loop-triggered agents."""

def get_loop_status(name: str) -> LoopStatus | None:
    """Detailed status for one loop."""
```

```python
@dataclass
class LoopStatus:
    """View model for loop status (assembled from AgentSpec + AgentRun)."""
    name: str
    status: str              # idle, running, error
    iteration: int
    current_step: str | None
    worktree: Path | None
    goal: str
    pipeline: list[str]
    last_pr_url: str | None
```

## CLI interface

Add to existing `lf` command (not a new `lf loop` subcommand):

```bash
# Start a loop (creates agent file + starts it)
lf loop start security-review \
  --goal "Find and fix security vulnerabilities" \
  --pipeline "design,implement,polish"

# List all loops
lf loop status
# NAME              STATUS   ITER  STEP        PR
# security-review   running  3     implement   github.com/...
# test-coverage     idle     7     -           github.com/...

# Detailed status
lf loop status security-review
# Name: security-review
# Status: running (iteration 3, step: implement)
# Goal: Find and fix security vulnerabilities
# Pipeline: design → implement → polish
# Worktree: /Users/.../repo.security-review
# Recent PRs:
#   #42 [security-review] Fix SQL injection (merged)
#   #41 [security-review] Add input validation (merged)

# Stop gracefully
lf loop stop security-review

# One-shot: run loop once, don't continue
lf loop once security-review
```

## Daemon integration

Reuse existing `lfd` server methods. Add current_step tracking:

```python
# Modify runner.py to update current_step before each task
conn.execute(
    "UPDATE agent_runs SET current_step = ? WHERE id = ?",
    (task_name, run_id),
)
```

New events (broadcast to existing subscribers):

```python
"loop.step.started"   # {"name": str, "step": str, "iteration": int}
"loop.step.completed" # {"name": str, "step": str, "status": str}
"loop.iteration.done" # {"name": str, "iteration": int, "pr_url": str}
```

## Database changes

Add column to existing table:

```sql
ALTER TABLE agent_runs ADD COLUMN current_step TEXT;
```

Add table for PR tracking:

```sql
CREATE TABLE loop_prs (
    id TEXT PRIMARY KEY,
    loop_name TEXT NOT NULL,
    iteration INTEGER NOT NULL,
    pr_url TEXT NOT NULL,
    pr_number INTEGER,
    status TEXT NOT NULL,  -- open, merged, closed
    created_at TEXT NOT NULL,
    FOREIGN KEY (loop_name) REFERENCES agent_runs(agent_name)
);
```

## Iteration workflow

The existing `run_agent_iteration()` handles most of this. Modifications:

1. **Before each task**: Update `current_step` in DB, emit event
2. **After each task**: Emit step.completed event
3. **After PR creation**: Save to `loop_prs` table, emit iteration.done
4. **On error**: Emit error event (existing behavior stops the loop)

Goal injection (already happens via `_inject_agent_prompt`):

```python
# Existing code in runner.py
def _inject_agent_prompt(components, agent):
    """Inject agent prompt into the prompt components."""
    if not agent.prompt:
        return components
    # Prepends agent.prompt to task content
```

## UI changes

Maestro additions:

1. **Loop badge** on AgentSidebar items showing current step
2. **Loop detail** in AgentDetailPanel:
   - Pipeline progress: `design ✓ → implement ● → polish ○`
   - PR history with links
3. **"Start Loop"** in New Agent sheet (sets trigger=loop)

## Constraints

- Loop names = agent names (unique per `~/.lf/agents/`)
- One loop per repo (existing worktree constraint)
- Pipeline tasks must exist (validated at start)
- Goal = agent prompt (required, non-empty)
- Stop waits for current step (existing SIGTERM behavior)

## Implementation order

1. Add `current_step` column to agent_runs
2. Update `runner.py` to track current_step
3. Create `src/loopflow/lf/loop.py` with CLI wrappers
4. Add `lf loop` commands to CLI
5. Add loop_prs table and PR tracking
6. Emit new events from runner
7. Update Maestro UI

## Done when

```bash
# Start a loop
lf loop start test-loop \
  --goal "Add comprehensive test coverage" \
  --pipeline "design,implement,polish"

# Verify it's running
lf loop status
# NAME       STATUS   ITER  STEP
# test-loop  running  1     design

# Watch progress
lf loop status test-loop
# Shows current step updating: design → implement → polish

# After iteration completes, verify PR
gh pr list | grep test-loop
# Shows PR created

# Stop the loop
lf loop stop test-loop

# Verify stopped
lf loop status
# NAME       STATUS  ITER
# test-loop  idle    1
```

Maestro verification:
- Loop appears in agent sidebar
- Current step shows in detail panel
- PR links appear after iteration
