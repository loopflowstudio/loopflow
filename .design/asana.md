# Work Coordination System

Backend-agnostic work queue that stores proposals, tracks status, and lets agents prioritize work they can make progress on. Files in `.todo/` or Asana tasks—same interface, both flow through `gather_context()`.

## Implementation Status

**Done:**
- WorkItem data model with status, claimed_by, blocked_on fields
- FileBackend: reads/writes `.todo/*.md` with YAML frontmatter
- AsanaBackend: maps to Asana tasks via API
- `lfwork` CLI: list, show, add, approve, reject, claim, release, blocked, next
- Context integration: work queue included in prompts via `gather_work_queue()`
- Maestro UI: WorkQueueView with tabs, item actions, status display
- Config: `work.backend` and `work.asana.project_id` settings

**Not yet implemented:**
- Agent loop integration (agents picking work items automatically)
- Agent branching (`<agent>-main` integration branches)
- Sync strategy (auto-rebase, conflict resolution)

## Data Structure

```python
@dataclass
class WorkItem:
    id: str                          # filename stem or Asana task ID
    title: str
    description: str
    status: Literal["proposed", "approved", "active", "done"]
    claimed_by: Literal["human", "agent"] | None
    blocked_on: str | None
    worktree: str | None
    notes: str
```

**File format** (`.todo/<id>.md`):

```yaml
---
status: proposed
claimed_by: null
blocked_on: null
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
    def update_item(self, item_id: str, **fields) -> WorkItem | None: ...
    def delete_item(self, item_id: str) -> bool: ...
```

Two implementations:
- `FileBackend`: reads/writes `.todo/*.md` with YAML frontmatter
- `AsanaBackend`: maps to Asana tasks via API

## Prioritization Logic

`get_next_work()` picks non-blocked work items:

```python
def get_next_work(items: list[WorkItem]) -> WorkItem | None:
    candidates = [
        c for c in items
        if c.status in ("approved", "active")
        and c.claimed_by != "human"
    ]
    # Sort: non-blocked first
    candidates.sort(key=lambda c: c.blocked_on is not None)
    return candidates[0] if candidates else None
```

## CLI

```bash
lfwork list                    # show all work items
lfwork list --status=proposed  # filter by status
lfwork show <id>               # show item details
lfwork add "Title"             # create new proposed item
lfwork approve <id>            # approve a proposal
lfwork reject <id>             # delete a proposal
lfwork claim <id>              # human claims work
lfwork release <id>            # release a claim
lfwork blocked                 # show items blocked on input
lfwork next                    # show what agent would pick
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

## Asana Field Mapping

| WorkItem field | Asana equivalent |
|----------------|------------------|
| id | Task GID |
| title | Task name |
| description | Task notes + subtasks (appended) |
| status | Section (Proposed / Approved / Active / Done) |
| claimed_by | Tag: "human" or untagged |
| blocked_on | Custom field (text) |
| worktree | Custom field (text) |
| notes | Task comments |

## Context Integration

Work items are formatted and included in prompts:

```python
def gather_work_queue(repo_root: Path) -> str | None:
    config = load_config(repo_root)
    backend = FileBackend(repo_root) or AsanaBackend(...)
    items = backend.list_items()
    # Format as markdown sections: Active, Approved, Proposed
    return formatted_work_queue
```

Included in prompt as `<lf:work>` section.

## Future Work

**Agent loop integration:**
- Agents call `get_next_work()` to pick items
- Update status to "active" when starting work
- Mark "done" or "blocked" on completion

**Agent branching:**
- Each agent has integration branch `<agent>-main`
- Work happens in worktrees off agent branch
- Merge to agent branch on completion, PR to main

**Sync strategy:**
- Auto-rebase agent branches onto main
- Use `lf rebase` for AI-assisted conflict resolution
- Mark work items blocked if resolution fails
