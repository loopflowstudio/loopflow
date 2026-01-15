# Documentation Audit and Update

A documentation refresh to cover the Maestro app, lfd service layer, and internal API boundaries. Make the product vision from `.research/` accessible to users, and ensure developers have clear API reference for lf ↔ lfd ↔ Maestro communication.

## What to build

Update `docs/` with Maestro-focused user documentation, create internal API reference for lfd protocol, and distill `.research/` insights into public-facing "why" documentation.

## Current State

**Public docs (docs/):**
- `index.md` — Quick start, CLI usage
- `config.md` — `.lf/config.yaml` options
- `patterns.md` — Workflow recipes

**Missing:**
- Maestro app documentation (what it is, how to use it)
- lfd daemon documentation (agent orchestration, session tracking)
- API reference for lfd socket protocol
- Vision/philosophy content from `.research/`

**.research/ (internal):**
- `maestro-vision.md` — Core product vision, market positioning
- `target-customer.md` — User persona and values
- `landscape.md` — Competitive analysis, pain points
- `technical-reference.md` — Claude Code/Codex APIs
- `agents-future.md` — Roadmap items

## Data Structures

```python
# lfd socket protocol (from src/loopflow/lfd/protocol.py)
@dataclass
class Request:
    method: str
    params: dict[str, Any]
    id: str | None = None

@dataclass
class Response:
    ok: bool
    result: Any = None
    error: str | None = None

@dataclass
class Event:
    event: str
    data: dict[str, Any]
```

```python
# Key models (from src/loopflow/lfd/models.py)
@dataclass
class Session:
    id: str
    task: str
    repo: str
    worktree: str
    status: SessionStatus  # running, waiting, completed, error
    started_at: datetime
    model: str = "claude-code"
    run_mode: Literal["auto", "interactive"] = "auto"

@dataclass
class AgentSpec:
    name: str
    repo: Path
    pipeline: str
    trigger: TriggerSpec  # manual, main-changed, interval, loop, cron
    context: list[str]
    prompt: str
```

## Key Documentation Structure

```
docs/
├── index.md              # Update: add Maestro as primary interface
├── config.md             # Existing: config options (current)
├── patterns.md           # Existing: workflow recipes (current)
├── maestro.md            # NEW: Maestro app guide
├── lfd.md                # NEW: daemon and agents reference
├── api.md                # NEW: lfd socket protocol reference
└── vision.md             # NEW: philosophy distilled from .research/
```

### docs/maestro.md structure

```markdown
# Maestro

Visual interface for loopflow. Launch prompts, manage worktrees, track sessions.

## Getting Started
- Download/install
- Open a repo

## Prompt Launcher
- Task selector
- Args input
- Context options (docs, diff, clipboard, attached files)
- Auto vs interactive mode
- Token estimation

## Worktree Sidebar
- Create/delete worktrees
- Status badges (design, implement, review, polish)
- Open in terminal/IDE
- PR actions (create, view, land)

## Agents Panel
- View running agents
- Start/stop agents
- Iteration count, trigger status

## Keyboard Shortcuts
- Cmd+Enter: Launch prompt
- (others as implemented)
```

### docs/lfd.md structure

```markdown
# Daemon (lfd)

Background service for session tracking and agent orchestration.

## Installation
lfd install

## Session Tracking
- Auto-registered in auto mode
- Query via socket or direct DB read

## Agents
- Define agents as markdown in ~/.lf/agents/
- Trigger types: manual, main-changed, interval, loop, cron
- Pipeline execution

## Socket API
See api.md for protocol details.
```

### docs/api.md structure

```markdown
# API Reference

## Protocol
JSON-over-newline on Unix socket at ~/.lf/lfd.sock

## Methods

### status
Returns daemon health: pid, agent count, session count.

### agents.list
Returns all agent definitions with runtime status.

### agents.start
Start an agent by name.

### agents.stop
Stop a running agent.

### sessions.list
Active sessions.

### sessions.history
Session history filtered by worktree or repo.

### subscribe
Subscribe to events (session.*, agent.*, worktree.*).

### notify
Broadcast custom events.

## Events
- session.started
- session.ended
- agent.started
- agent.stopped
```

### docs/vision.md structure

```markdown
# Philosophy

Distill from .research/maestro-vision.md:

## The Problem
- Context management overwhelms agents
- Design intent lost between sessions
- Parallel work requires manual juggling
- Quality degrades without discipline

## The Loopflow Approach
- Prompts as versioned artifacts
- Pipelines with quality gates
- Backend-agnostic (Claude, Codex, Gemini)
- Worktrees for parallel isolation
- Maestro for visual orchestration

## Target User
(Brief version of target-customer.md - the "maestro" persona)
```

## Constraints

1. **Don't duplicate code** — Reference files, don't inline implementations
2. **Keep `.research/` internal** — Distill for public docs, don't copy wholesale
3. **User-facing first** — API docs are secondary to "how do I use this"
4. **Match existing tone** — Follow STYLE.md, no Args:/Returns: docstrings

## Done When

```bash
# All new docs exist and render
ls docs/maestro.md docs/lfd.md docs/api.md docs/vision.md

# Jekyll builds clean
cd docs && bundle exec jekyll build 2>&1 | grep -i error || echo "No errors"

# Links in index.md point to new pages
grep -E "maestro|lfd|api|vision" docs/index.md
```

Manual verification:
1. Run `bundle exec jekyll serve` in docs/
2. Navigate to each new page
3. Verify navigation header includes new pages
4. Check that code examples match current implementation
