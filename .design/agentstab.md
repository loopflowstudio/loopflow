# Agents & Pipelines

**What to build:** Background agents with DAG-based pipelines that continuously generate and merge work, with emoji-tagged worktrees/PRs for tracking.

## Scope

**V1 (this design):**
- Pipeline DAG format and execution engine
- Agent goal files (new prompt type)
- Emoji tracking in worktrees, branches, PR titles
- Triggers: manual, loop, cron with grace period

Future work (state sync, conflict handling, rate limiting) captured in `.research/agents-future.md`.

## Overview

Agents are long-running background processes that:
1. Follow a **goal** - a high-level strategic direction (e.g., "user growth", "security", "internationalization")
2. Execute an "inner loop" **pipeline** that produces a single diff per iteration
3. Merge results via configurable strategy (auto-land or PR)

**One worktree = one diff = one pipeline run.** Parallel steps within a pipeline share the worktree.

Pipelines are DAGs of tasks. Tasks are tactical ("implement", "review"); goals are strategic.

> "Agents will continually create and clear worktrees, but they are bigger than worktrees."

## Data Structures

### Agent Definition

```python
@dataclass
class AgentDef:
    name: str
    emoji: str                              # Visual identifier, e.g. "🔧"
    goal: Path                              # Path to goal.md prompt
    pipeline: str                           # Pipeline name to run
    merge_strategy: Literal["auto", "pr"]   # How to land work
    trigger: TriggerDef                     # When to run

@dataclass
class TriggerDef:
    kind: Literal["manual", "loop", "cron"]
    cron: str | None = None                 # Cron expression if kind=cron
    grace_minutes: int = 60                 # Window for missed schedules
```

Agent files live in `~/.lf/agents/{name}.md` (current location). Format:

```markdown
---
emoji: 🔧
goal: .lf/goals/refactor.md
pipeline: ship
merge: auto
trigger: loop
# or: trigger: cron("0 9 * * *", grace: 120)
---

Optional inline prompt that extends the goal file.
```

### Pipeline Definition

Pipelines live in `.lf/pipelines/{name}.yaml` (in repo, version controlled).

```yaml
# .lf/pipelines/ship.yaml
steps:
  - implement
  - review
  - parallel:
      - test
      - lint
  - land

# Can reference other pipelines
# .lf/pipelines/full-ci.yaml
steps:
  - pipeline: ship
  - deploy
```

```python
@dataclass
class PipelineDef:
    name: str
    steps: list[PipelineStep]

@dataclass
class PipelineStep:
    task: str | None = None                 # Task name
    pipeline: str | None = None             # Or nested pipeline
    parallel: list[PipelineStep] | None = None  # Or parallel group
    config: StepConfig | None = None        # Per-step overrides

@dataclass
class StepConfig:
    model: str | None = None                # Override backend
    voice: str | None = None                # Voice for this step (future)
```

### State

Agent runtime state lives in the existing SQLite database (`~/.lf/lfd.db`). Extended schema:

```python
@dataclass
class AgentRun:
    """Existing, extended with emoji"""
    id: str
    agent_name: str
    status: AgentStatus
    started_at: datetime
    ended_at: datetime | None
    pid: int | None
    worktree: str | None
    iteration: int
    error: str | None
    main_sha: str | None
    emoji: str                              # NEW: from agent def
```

Agent definitions remain in `~/.lf/agents/{name}.md`. State sync to repo is a future feature.

### Worktree & PR Tracking

Emoji appears in branches, worktrees, and PR titles for visual tracking.

```python
@dataclass
class AgentWorktree:
    agent: str                              # Agent name
    emoji: str                              # From agent def
    branch: str                             # e.g. "🔧/refactor-bot/001"
    iteration: int
    created_at: datetime
```

**Naming conventions:**
- Worktree: `{repo}.{emoji}-{agent}-{iteration:03d}` → `loopflow.🔧-refactor-bot-001`
- Branch: `{emoji}/{agent}/{iteration:03d}` → `🔧/refactor-bot/001`
- PR title: `{emoji} {summary}` → `🔧 Refactor auth module`

The emoji makes it easy to:
- `wt list | grep 🔧` to see all worktrees for an agent
- Visually scan GitHub PR list by agent
- Know which background work belongs to which agent

## Key Functions

```python
# Pipeline loading
def load_pipeline(name: str, repo: Path) -> PipelineDef:
    """Load from .lf/pipelines/{name}.yaml or inline in config.yaml"""

def resolve_pipeline(pipeline: PipelineDef, repo: Path) -> list[ResolvedStep]:
    """Expand nested pipelines, return flat DAG with dependencies"""

# Pipeline execution
def execute_pipeline(pipeline: PipelineDef, worktree: Path, goal: str) -> PipelineResult:
    """Run DAG in worktree, injecting goal as context for each step"""

def run_parallel_steps(steps: list[PipelineStep], worktree: Path) -> list[StepResult]:
    """Run steps concurrently using asyncio, same worktree"""

# Agent lifecycle
def agent_iteration(agent: AgentDef) -> IterationResult:
    """One iteration: create worktree → run pipeline → merge → cleanup"""

def agent_loop(agent: AgentDef) -> None:
    """Main loop for continuous agents"""

# Scheduling
def parse_cron(expr: str) -> CronSchedule:
    """Parse cron expression (standard 5-field format)"""

def should_run_cron(schedule: CronSchedule, last_run: datetime, grace: int) -> bool:
    """Check if scheduled time passed (with grace period for laptop sleep)"""

def next_run_time(schedule: CronSchedule, after: datetime) -> datetime:
    """Calculate next scheduled run time"""

# Naming
def agent_branch_name(agent: AgentDef, iteration: int) -> str:
    """Generate branch name: {emoji}/{agent}/{iteration:03d}"""

def agent_worktree_path(repo: Path, agent: AgentDef, iteration: int) -> Path:
    """Generate worktree path: {repo}.{emoji}-{agent}-{iteration:03d}"""
```

## Constraints

- **Parallel steps share a worktree.** Tasks running in parallel must not conflict (e.g., both read-only, or touch different files). If they conflict, make them sequential.
- **Cron grace period requires persistent state.** Must survive daemon restart to know when last schedule was due.
- **Emoji in branch names may hit git limitations.** Test with common git hosts (GitHub, GitLab). May need fallback to ASCII prefix like `[bot]` or agent initials.
- **Pipeline files are in-repo but agents are user-local.** Clear separation: pipelines are "how to do work", agents are "who runs what where".
- **Goals are strategic, not tactical.** Goal files describe direction ("improve security", "grow user base"), not specific features. The agent + pipeline figures out what to build.

### Work Selection

The agent doesn't pre-plan a backlog. Each iteration:
1. Reviews its configured fileset (the parts of the codebase relevant to its goal)
2. Considers the current state of main
3. The `design` step asks: "given this goal and this codebase, what's the best next change?"
4. Pipeline continues with the designed change

This means the goal file is direction, not a task list. The agent adapts to whatever main looks like when it starts each iteration.

### Internal Documentation

Three-tier documentation structure:

| Folder | Purpose | Lifecycle |
|--------|---------|-----------|
| `.design/` | Current work specs | Deleted on `lf ops pr land` |
| `.research/` | Future work, cross-agent notes | Persists, version controlled |
| `docs/` | Public-facing docs | Persists, version controlled |

`.research/` is where any agent can write notes for others:
- Future feature ideas
- Technical debt observations
- "Someone with a security focus should look at X"
- Observations that don't fit current scope

This enables cross-agent communication. A refactoring agent might notice a security issue and leave a note for the security agent to pick up on its next iteration.

```
.research/
  future-work.md      # Ideas for later
  observations.md     # Things noticed while working
  security-notes.md   # Left by agents with security goals
```

## Open Questions

1. **Goal file format.** Pure markdown prompt, or structured sections (objectives, constraints, out-of-scope)?
2. **`.research/` structure.** Free-form files, or convention like `{agent-emoji}-notes.md`?

## Done When

```bash
# 1. Create a DAG pipeline in repo
cat > .lf/pipelines/ship.yaml << 'EOF'
steps:
  - design
  - implement
  - parallel:
      - test
      - lint
  - land
EOF

# 2. Create a goal file
cat > .lf/goals/security.md << 'EOF'
Improve the security posture of this codebase.

Focus areas:
- Input validation
- Authentication hardening
- Dependency vulnerabilities
EOF

# 3. Create an agent with emoji
lfd new security-bot --emoji 🔒 --goal .lf/goals/security.md --pipeline ship --trigger loop

# 4. Start agent, verify emoji in worktree
lfd start security-bot
wt list | grep 🔒
# Output includes: loopflow.🔒-security-bot-001

# 5. Verify parallel execution in logs
lfd logs security-bot
# Shows [test] and [lint] running concurrently after implement

# 6. Verify PR has emoji prefix (when merge=pr)
gh pr list | grep 🔒
# Shows: 🔒 Add input validation to user endpoints
```
