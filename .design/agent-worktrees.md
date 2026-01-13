# Agent Worktree Isolation

**Status: Implemented**

Agents run in isolated worktrees so they don't conflict with your active work or each other.

## The Problem

Currently `lf agent run` executes directly in the agent's configured repo:

```python
result = run_pipeline(
    pipeline=pipeline_config,
    repo_root=agent.repo,  # <- runs directly in main repo
    context=agent.context,
)
```

This breaks when:
1. You're editing in the main repo and an agent commits
2. Two agents target the same repo simultaneously
3. An agent fails mid-run, leaving the repo dirty

## Solution

Each agent run creates or reuses a dedicated worktree named `agent-<name>`.

```
~/src/myproject/                    # main repo (your work)
~/src/myproject.agent-docs-bot/     # worktree for docs-bot agent
~/src/myproject.agent-feature-bot/  # worktree for feature-bot agent
```

### Run Flow

1. Agent triggers (main-changed, interval, manual)
2. Daemon calls `lf agent run <name>`
3. Runner checks for existing worktree at `<repo>.agent-<name>`
4. If missing or stale, create fresh from main: `wt switch --create agent-<name>`
5. Run pipeline in that worktree
6. Record worktree path in `agent_runs` table
7. On success: worktree stays for next run (reuse speeds up iteration)
8. On failure: optionally clean up or leave for debugging

### Worktree Reuse

Agents reuse their worktree between runs for speed:

- Skip checkout/clone time
- Preserve build caches (node_modules, .venv, __pycache__)
- Keep local branches for multi-step work

Before each run, sync to latest main:

```bash
git fetch origin main
git reset --hard origin/main
```

This is safe because agent worktrees are ephemeral - any uncommitted changes are lost.

### Fresh Start Option

Sometimes an agent needs a clean slate. Add `fresh: true` to force worktree recreation:

```markdown
---
repo: /Users/jack/src/myproject
pipeline: [implement, polish, land]
trigger: main-changed
fresh: true   # always start from clean worktree
---
```

Useful when:
- Build artifacts cause problems
- Agent made bad commits that got force-pushed out
- You want guaranteed clean state

## Implementation

### Changes to markdown.py

Add `fresh` field to AgentFile:

```python
@dataclass
class AgentFile:
    name: str
    path: Path
    repo: Path
    pipeline: list[str]
    trigger: str
    context: list[str]
    prompt: str
    interval_seconds: int | None = None
    fresh: bool = False  # new: force fresh worktree each run
```

### New worktree.py module

Create `src/loopflow/maestro/worktree.py`:

```python
def ensure_agent_worktree(agent: AgentFile) -> Path:
    """Ensure agent has a worktree ready for running.

    Returns the worktree path. Creates or resyncs as needed.
    """

def remove_agent_worktree(agent: AgentFile) -> bool:
    """Remove an agent's worktree."""
```

### Changes to cli/agent.py run command

```python
@app.command("run")
def run(name: str):
    agent = get_agent_file(name)

    # Ensure worktree exists and is synced to main
    worktree_path = ensure_agent_worktree(agent)

    pipeline_config = PipelineConfig(
        name=agent.name,
        tasks=agent.pipeline,
    )

    result = run_pipeline(
        pipeline=pipeline_config,
        repo_root=worktree_path,  # <- now runs in worktree
        context=agent.context,
    )
```

### Changes to daemon.py

Record worktree in agent_runs:

```python
def _record_run_start(agent: AgentFile, pid: int, worktree: Path, main_sha: str | None) -> str:
    conn.execute(
        """INSERT INTO agent_runs (id, agent_name, status, started_at, pid, worktree, main_sha)
           VALUES (?, ?, 'running', ?, ?, ?, ?)""",
        (run_id, agent.name, datetime.now().isoformat(), pid, str(worktree), main_sha),
    )
```

### CLI cleanup command

Add `lf agent cleanup <name>` to remove stale worktrees:

```bash
lf agent cleanup docs-bot     # remove docs-bot's worktree
lf agent cleanup --all        # remove all agent worktrees
```

## Swift App Integration

The Maestro app can show worktree status per agent:

```
AGENTS
├─ docs-bot              ● running
│  └─ ~/src/myproject.agent-docs-bot
└─ feature-bot             ○ idle
   └─ (no worktree)
```

Click to reveal worktree in Finder or open in terminal.

## Dependencies

Requires `wt` (worktrunk) which is already a loopflow dependency.

## Done When

1. ✅ `lf agent run <name>` creates/reuses worktree at `<repo>.agent-<name>`
2. ✅ Worktree is synced to origin/main before each run
3. ✅ `worktree` field in agent_runs is populated
4. ✅ `fresh: true` in agent config forces fresh worktree
5. ✅ `lf agent cleanup <name>` removes agent worktree
6. ✅ `lf agent show <name>` displays worktree path if it exists

## Files Changed

- `src/loopflow/maestro/worktree.py` - New module for agent worktree management
- `src/loopflow/maestro/markdown.py` - Added `fresh` field to AgentFile
- `src/loopflow/cli/agent.py` - Updated `run`, `show`, added `cleanup` command
- `src/loopflow/maestro/daemon.py` - Records worktree path in agent_runs
- `tests/test_agent_worktree.py` - Tests for worktree functionality
