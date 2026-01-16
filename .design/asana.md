# Work Coordination System

**What to build**: A backend-agnostic work queue that stores proposals, tracks status, and lets agents prioritize work they can make progress on. Files in `.todo/` or Asana tasks—same interface, both flow through `gather_context()`.

## Core Concept

Work items are fractal—could be a whole feature or a small task. Agents propose work, humans review, agents (or humans) claim and execute. Agents loop continuously but bias toward items where they have confidence to proceed.

> "agents will always loop, but will bias towards things where they think they know what they need to do next and away from things where they think my feedback is essential to continue"

## Data Structure

```python
@dataclass
class WorkItem:
    id: str                          # filename stem or Asana task ID
    title: str
    description: str
    status: Literal["proposed", "approved", "active", "done"]
    claimed_by: Literal["human", "agent"] | None  # who has it, not which specific agent
    blocked_on: str | None           # freeform: "need user input on API design"
    confidence: Literal["high", "medium", "low"]  # agent's self-assessed ability to proceed
    worktree: str | None             # optional: where work is happening (e.g., "feature-xyz")
    notes: str                       # append-only log of progress
```

**File format** (`.todo/<id>.md`):

```yaml
---
status: proposed
claimed_by: null
blocked_on: null
confidence: medium
worktree: null
---
# Add OAuth login

Description of the work...

## Notes

- 2025-01-16: Created from background review
```

## Backend Protocol

```python
class WorkBackend(Protocol):
    def list_items(self, status: str | None = None) -> list[WorkItem]: ...
    def get_item(self, item_id: str) -> WorkItem | None: ...
    def create_item(self, item: WorkItem) -> WorkItem: ...
    def update_item(self, item_id: str, **fields) -> WorkItem: ...
```

Two implementations:
- `FileBackend`: reads/writes `.todo/*.md` with YAML frontmatter
- `AsanaBackend`: maps to Asana tasks via API

## Key Operations

### Agent proposes work
```python
backend.create_item(WorkItem(
    id=slugify(title),
    title="Add rate limiting",
    description="API has no rate limits...",
    status="proposed",
    confidence="medium",
))
```

### Human reviews
```python
# In Maestro or CLI
items = backend.list_items(status="proposed")
# Human approves/rejects/edits
backend.update_item(item.id, status="approved")
```

### Agent picks next work
```python
def get_next_work(backend: WorkBackend) -> WorkItem | None:
    """Pick work biased toward high-confidence, non-blocked items."""
    candidates = [
        c for c in backend.list_items()
        if c.status in ("approved", "active")
        and c.claimed_by != "human"  # skip human-claimed
    ]

    # Sort: high confidence + not blocked first
    candidates.sort(key=lambda c: (
        c.blocked_on is not None,  # blocked items last
        {"high": 0, "medium": 1, "low": 2}[c.confidence],
    ))

    return candidates[0] if candidates else None
```

### Human claims work
```python
backend.update_item(item.id, claimed_by="human")
# Agents will skip this item
```

### Agent marks blocked
```python
backend.update_item(item.id,
    blocked_on="Need user input: should we use JWT or sessions?",
    confidence="low"
)
# Agent moves to other work; item surfaces in human review queue
```

## Agent Loop Behavior

When an agent runs (via `lfd` or directly), it follows this loop:

```python
def agent_loop(backend: WorkBackend):
    while True:
        item = get_next_work(backend)

        if item:
            # Work on existing item
            backend.update_item(item.id, status="active", claimed_by="agent")
            result = do_work(item)

            if result.blocked:
                backend.update_item(item.id,
                    blocked_on=result.blocked_reason,
                    confidence="low"
                )
            elif result.done:
                backend.update_item(item.id, status="done")
            # else: still in progress, keep active

        else:
            # Nothing to work on—generate new proposal
            new_item = generate_proposal()  # agent brainstorms
            backend.create_item(new_item)
            # Loop continues; human will review the proposal
```

Key: agents never idle. If blocked everywhere, they propose new work.

## Agent Branching

Each agent has its own integration branch: `<agent-name>-main`.

```
main
  └── agent-alpha-main      # agent-alpha's changes auto-merge here
        └── feature-xyz     # worktree for current work item
```

**Flow:**
1. Agent picks work item, creates worktree off `<agent>-main`
2. Work completes → merge to `<agent>-main` (automatic)
3. `<agent>-main` → `main` via PR (or auto-land if configured)

This keeps agent work isolated while allowing continuous progress. Humans review at the PR boundary, not every commit.

## Sync Strategy

Agent branches drift as `main` advances. Sync keeps them current.

**When to sync:**
- Before starting new work item
- Periodically during long-running work (e.g., every N commits to main)
- When merge to main fails due to conflicts

**How:**
```python
def sync_agent_branch(agent_name: str):
    """Rebase agent's integration branch onto main."""
    agent_branch = f"{agent_name}-main"

    # Fetch latest main
    run(f"git fetch origin main")

    # Rebase agent branch onto main
    result = run(f"git rebase origin/main {agent_branch}")

    if result.conflicts:
        # Use lf rebase to resolve conflicts automatically
        run(f"lf rebase")  # AI-assisted conflict resolution
```

**Conflict resolution:** Default to automatic rebase via `lf rebase`. The agent resolves conflicts using its understanding of the codebase. If resolution fails, mark current work item as blocked.

```yaml
# .lf/config.yaml
work:
  auto_rebase: true          # sync agent branches automatically (default)
  # auto_rebase: false       # manual sync only
  auto_land: false           # PRs to main require human approval
  # auto_land: true          # merge to main automatically
```

**Active worktree handling:**
When rebasing `<agent>-main`, any active worktrees need updating:

```python
def sync_worktree(worktree_path: Path, agent_branch: str):
    """Rebase active worktree onto updated agent branch."""
    # In worktree directory
    run(f"git fetch origin {agent_branch}")
    result = run(f"git rebase origin/{agent_branch}")

    if result.conflicts:
        run(f"lf rebase")
```

## Work Item Lifecycle

**File backend:** Delete `.todo/<id>.md` when done. Git history preserves it if needed.

**Asana backend:** Mark task complete (stays in project for history).

## Asana Integration

**Access strategy:** Direct Asana API via `python-asana` library. No MCP or middleware.

**Context flow:** Work items flow through `gather_context()`, same path as diff files and docs. Both backends use the same pattern:

```python
def gather_context(...) -> Context:
    ...
    backend = get_work_backend(config)  # FileBackend or AsanaBackend
    work_items = backend.list_items()
    context.work_queue = format_work_items(work_items)
    ...
```

- **File backend:** Reads `.todo/*.md` via standard file loading
- **Asana backend:** Fetches tasks via API

**Auth:** `ASANA_ACCESS_TOKEN` env var (personal access token). No OAuth flow for now.

**Field mapping:**

| WorkItem field | Asana equivalent |
|----------------|------------------|
| id | Task GID |
| title | Task name |
| description | Task notes (markdown) + subtasks (appended) |
| status | Section (Proposed / Approved / Active / Done) |
| claimed_by | Tag: "human" or untagged |
| blocked_on | Custom field (text) |
| confidence | Custom field (dropdown: high/medium/low) |
| worktree | Custom field (text) |
| notes | Task comments |

## UI (Maestro)

**Review queue**: Show `status=proposed` items. Accept/reject/edit buttons.

**Active work**: Show `status=active` items, highlight human-claimed vs agent-available.

**Blocked items**: Show items where `blocked_on` is set. These need human input.

## CLI

```bash
lfwork list                    # show all work items
lfwork list --status=proposed  # show proposals needing review
lfwork approve <id>            # approve a proposal
lfwork claim <id>              # human claims work
lfwork release <id>            # release a claim
lfwork blocked                 # show items blocked on user input
```

## Configuration

```yaml
# .lf/config.yaml
work:
  backend: file                # or "asana"
  asana:
    project_id: "123456"       # Asana project GID
    # Auth via ASANA_ACCESS_TOKEN env var
```

## Constraints

- **Flat structure**: No hierarchy in loopflow's model. `.todo/` is a flat list of files. Asana subtasks don't create child WorkItems, but are included in the task description so agents can see them.
- **File backend must be simple**: Just YAML frontmatter + markdown. No database, no locks beyond git.
- **Asana backend is optional**: Don't require Asana API access for basic operation.
- **Agents must be able to run without human**: The loop continues; it just picks different work.

## Open Questions

None—ready for implementation.

## Done When

```bash
# File backend works
echo "---\nstatus: proposed\n---\n# Test" > .todo/test.md
lfwork list --status=proposed  # shows "Test"
lfwork approve test
lfwork list --status=approved  # shows "Test"

# Agent integration
lf implement  # agent calls get_next_work(), picks approved item, works on it

# Sync works
# (with commits on main that conflict with agent-main)
# Agent auto-rebases and resolves conflicts via lf rebase
```

UI verification: Maestro shows review queue with proposed items, approve button works.
