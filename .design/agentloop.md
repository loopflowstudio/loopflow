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

## Goals vs Areas: Structured vs Convention

Two ways to define agent scope:

| Concept | Type | Purpose |
|---------|------|---------|
| **Area of Responsibility** | Structured (pathset) | WHERE the agent works |
| **Goal** | Convention (prompt file) | WHAT the agent does |

**Area** is structured because tooling can use it:
- Scope context gathering to those paths
- Restrict file edits to that area (safety)
- Show ownership in UI ("who owns auth/?")

**Goal** is convention because intent is nuanced:
- "Improve test coverage" vs "Find security vulnerabilities"
- Prose captures intent better than fields
- Like voices—prompt files written for a purpose

**Key insight**: The area implies responsibility ("you are responsible for auth" is redundant if `area: [src/auth/]`). Goals should be concrete objectives, not job descriptions.

### Incremental climbing

Goals are destinations. Each PR is one step toward the goal, not an attempt to achieve everything at once.

```
Goal: 80% test coverage (currently 45%)
  └── PR #1: Add tests for auth module (+5%)
  └── PR #2: Add tests for api endpoints (+8%)
  └── PR #3: Add tests for utils (+3%)
  └── ... keeps climbing until 80%
```

Each iteration the agent:
1. **Assesses** - where are we vs the goal?
2. **Plans** - what's one achievable step?
3. **Executes** - do that step, commit, PR
4. **Repeats** - until goal reached or indefinitely for maintenance

### First agents

Role-based goals combined with area scope:

| Agent | Goal | Area |
|-------|------|------|
| maestro-designer | designer | Maestro/ |
| maestro-engineer | product-engineer | Maestro/ |
| loopflow-designer | designer | src/loopflow/, docs/ |
| loopflow-engineer | product-engineer | src/loopflow/ |
| infra-engineer | infra-engineer | src/ |

### Goal files

Goal files live in `.lf/goals/`. For role-based agents, they define how someone in that role works each iteration:

```markdown
# .lf/goals/designer.md

Improve user experience and interface quality.

## Each iteration
1. Pick ONE screen, flow, or interaction
2. Identify friction, inconsistency, or confusion
3. Improve it with minimal scope creep
4. Verify it looks/works right

## Focus areas
- Visual consistency
- User flow friction
- Error states and edge cases
- Accessibility

## Done when
Never—continuous improvement
```

```markdown
# .lf/goals/product-engineer.md

Build features and ship working code.

## Each iteration
1. Pick ONE feature gap, bug, or improvement
2. Design minimally, implement fully
3. Add tests for new code
4. PR with clear description

## Focus areas
- Feature completeness
- User-facing functionality
- Test coverage on changes
- No regressions

## Done when
Never—continuous improvement
```

```markdown
# .lf/goals/infra-engineer.md

Improve code quality, tooling, and developer experience.

## Each iteration
1. Pick ONE pain point (slow, flaky, complex, outdated)
2. Fix it properly
3. Document if non-obvious

## Focus areas
- Build/test speed
- Code complexity
- Dependency health
- Developer ergonomics

## Done when
Never—continuous improvement
```

Referenced by name in agent definition, injected into every task prompt.

## Data structures

Extend existing `AgentSpec`:

```python
@dataclass
class AgentSpec:
    name: str
    repo: Path
    pipeline: str                # "design,implement,polish"
    trigger: TriggerSpec         # kind=LOOP for continuous
    area: list[str]              # NEW: pathset - where agent works
    goal: str                    # NEW: reference to .lf/goals/{goal}.md
    prompt: str                  # Optional inline prompt (legacy, prefer goal file)
    # ... existing fields ...
```

Add step tracking to `AgentRun`:

```python
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
    goal: str                # name of goal file
    goal_content: str | None # loaded content for display
    area: list[str]          # pathset
    pipeline: list[str]
    last_pr_url: str | None
```

## Agent definition format

Agent files in `~/.lf/agents/` reference goals and areas:

```yaml
# ~/.lf/agents/security-reviewer.md
---
name: security-reviewer
repo: ~/src/myproject
area: [src/auth/, src/api/middleware/, src/utils/crypto.py]
goal: security
pipeline: design,implement,polish
trigger: loop
merge: auto
---

Optional inline notes or overrides to the goal file.
```

The `goal: security` references `.lf/goals/security.md` in the repo.

## CLI interface

Add to existing `lf` command:

```bash
# Start a loop (creates agent file + starts it)
lf loop start security-review \
  --goal security \
  --area "src/auth/,src/api/middleware/" \
  --pipeline "design,implement,polish"

# List all loops
lf loop status
# NAME              STATUS   ITER  STEP        AREA
# security-review   running  3     implement   src/auth/...
# test-coverage     idle     7     -           tests/...

# Detailed status
lf loop status security-review
# Name: security-review
# Status: running (iteration 3, step: implement)
# Goal: security (.lf/goals/security.md)
# Area: src/auth/, src/api/middleware/
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
2. **Context scoping**: If `area` is set, only include those paths in context
3. **After each task**: Emit step.completed event
4. **After PR creation**: Save to `loop_prs` table, emit iteration.done
5. **On error**: Emit error event (existing behavior stops the loop)

### Area scoping

When `area` is set, it affects context gathering:

```python
def gather_prompt_components(..., area: list[str] | None = None):
    if area:
        # Only include files matching the area pathset
        # Similar to existing context: [...] but exclusive
        diff_files = [f for f in diff_files if matches_area(f, area)]
```

Future: could also restrict edits to area (safety), but start with context scoping only.

### Goal injection

Load goal file and inject (extends existing `_inject_agent_prompt`):

```python
def _load_goal(repo: Path, goal_name: str) -> str | None:
    """Load goal file from .lf/goals/{name}.md"""
    path = repo / ".lf" / "goals" / f"{goal_name}.md"
    return path.read_text() if path.exists() else None

def _inject_agent_prompt(components, agent):
    """Inject goal + inline prompt into task."""
    parts = []

    # Load goal file
    if agent.goal:
        goal_content = _load_goal(agent.repo, agent.goal)
        if goal_content:
            parts.append(f"<lf:goal>\n{goal_content}\n</lf:goal>")

    # Add inline prompt if present
    if agent.prompt:
        parts.append(agent.prompt)

    # Prepend to task content
    ...
```

## UI changes

Maestro additions:

1. **Loop badge** on AgentSidebar items showing current step
2. **Loop detail** in AgentDetailPanel:
   - Pipeline progress: `design ✓ → implement ● → polish ○`
   - Area visualization (file tree with highlighted paths)
   - Goal preview (collapsible)
   - PR history with links
3. **"Start Loop"** in New Agent sheet:
   - Goal selector (dropdown of `.lf/goals/*.md`)
   - Area picker (file tree with checkboxes, progressively disclosed)
   - Pipeline builder (drag-drop tasks)

## Constraints

- Loop names = agent names (unique per `~/.lf/agents/`)
- One loop per repo (existing worktree constraint)
- Pipeline tasks must exist (validated at start)
- Goal file must exist if specified (validated at start)
- Area paths are relative to repo root
- Stop waits for current step (existing SIGTERM behavior)

## Implementation order

1. Add `area` and `goal` fields to AgentSpec parsing
2. Create `.lf/goals/` directory convention and loader
3. Add `current_step` column to agent_runs
4. Update `runner.py` to:
   - Track current_step
   - Load and inject goal files
   - Scope context by area
5. Create `src/loopflow/lf/loop.py` with CLI wrappers
6. Add `lf loop` commands to CLI
7. Add loop_prs table and PR tracking
8. Emit new events from runner
9. Update Maestro UI (area picker, goal selector)

## Done when

```bash
# Create a goal file (destination, not job description)
cat > .lf/goals/test-coverage-80.md << 'EOF'
Reach 80% test coverage.

## Measure
`pytest --cov` - target 80% on src/

## Each iteration
Pick ONE untested module. Add comprehensive tests.
Small PRs that compound.

## Done when
Coverage >= 80%
EOF

# Start a loop
lf loop start test-loop \
  --goal test-coverage-80 \
  --area "src/,tests/" \
  --pipeline "design,implement,polish"

# Verify it's running
lf loop status
# NAME       STATUS   ITER  STEP      AREA
# test-loop  running  1     design    src/...

# Watch progress
lf loop status test-loop
# Shows:
#   Goal: test-coverage-80 (Reach 80% test coverage)
#   Area: src/, tests/
#   Pipeline: design → implement → polish
#   Current: design (iteration 1)

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
- Goal content visible in detail panel
- Area paths highlighted
- Current step shows in detail panel
- PR links appear after iteration
