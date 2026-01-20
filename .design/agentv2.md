# Roadmapping

Agents should propose substantial work when the roadmap is thin, build when it's full.

## What's implemented

This branch adds:
- `src/loopflow/lf/roadmap.py` — `RoadmapItem`, `Roadmap`, and functions for loading, creating, approving, starting, and completing items
- `src/loopflow/lf/goals.py` — extended to support builtin goals with fallback to `templates/goals/`
- `src/loopflow/templates/goals/` — four builtin goals: `adaptive`, `build`, `roadmap`, `simplify`
- `src/loopflow/templates/commands/roadmap.md` — task for proposing new roadmap items

## What's left

- CLI command `lf roadmap --list` for viewing roadmap
- CLI command `lf roadmap --approve <item>` for approving items
- Integration into `lfd` loop runner to use roadmap for work selection

## Data structures

```python
@dataclass
class RoadmapItem:
    """A design spec for substantial work."""
    path: Path                    # .docs/roadmap/<area>/<name>.md
    area: str                     # e.g., "api", "ui", "core"
    status: str                   # proposed | approved | in-progress | done
    title: str                    # from markdown h1
    created_at: datetime
    approved_at: datetime | None

@dataclass
class Roadmap:
    """All roadmap items for a repo."""
    items: list[RoadmapItem]

    def for_area(self, area: str) -> list[RoadmapItem]:
        """Items matching an area."""

    def by_status(self, status: str) -> list[RoadmapItem]:
        """Items with a specific status."""

    def depth(self, area: str | None = None) -> int:
        """Count of approved items ready for building."""
```

## File structure

```
.docs/
  roadmap/
    api/
      add-rate-limiting.md      # status: approved
      graphql-subscriptions.md  # status: proposed
    ui/
      dark-mode.md              # status: in-progress
    _done/                      # archived after building
      user-auth.md
```

## Key functions (implemented)

```python
def load_roadmap(repo: Path) -> Roadmap:
    """Load all roadmap items from .docs/roadmap/."""

def create_item(repo: Path, area: str, name: str, title: str, content: str) -> RoadmapItem:
    """Create a new roadmap item."""

def approve_item(item: RoadmapItem) -> None:
    """Mark item as approved."""

def start_item(item: RoadmapItem, branch: str, repo: Path) -> None:
    """Mark item in-progress, copy to .design/<branch>.md."""

def complete_item(item: RoadmapItem, repo: Path) -> None:
    """Move to _done/, clean up .design/."""

def format_roadmap_list(roadmap: Roadmap) -> str:
    """Format roadmap for display."""
```

## Builtin goals

Four builtin goals in `templates/goals/`:

| Goal | Pipeline | Purpose |
|------|----------|---------|
| `adaptive` | `@ship` | Switch between build/roadmap/simplify based on context |
| `build` | `@ship` | Build from approved roadmap specs |
| `roadmap` | `@design` | Propose new roadmap items |
| `simplify` | `@polish` | Make codebase cleaner, easier, smaller |

Goals are loaded via `load_goal(repo, name)` which checks user-defined first, then falls back to builtins.

## Roadmap item format

```markdown
---
status: proposed | approved | in-progress | done
area: api
created_at: 2026-01-20T10:00:00
---

# Add rate limiting

One paragraph describing what and why.

## Scope

- What's included
- What's explicitly not included

## Approach

Technical direction. Not a full design doc—just enough to unblock building.
```

## Constraints

- Roadmap items are markdown files, not a database
- Status lives in frontmatter, not filename
- `_done/` is the archive, kept for reference
- Agents can read roadmap, goals determine what they can write
