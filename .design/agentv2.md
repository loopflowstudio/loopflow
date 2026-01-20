# Roadmapping

Agents should propose substantial work when the roadmap is thin, build when it's full.

## What to build

A roadmap system where humans and agents both contribute design specs, and agents bias their work selection based on roadmap depth.

## The problem

Background agents default to safe, small fixes when they don't have clear direction. This produces churn instead of progress. We want agents to either:
1. Implement from a roadmap of pre-designed work
2. Propose new roadmap items when the backlog is thin

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

    def depth(self, area: str) -> int:
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

## Roadmap item format

```markdown
---
status: proposed | approved | in-progress | done
area: api
---

# Add rate limiting

One paragraph describing what and why.

## Scope

- What's included
- What's explicitly not included

## Approach

Technical direction. Not a full design doc—just enough to unblock building.
```

## Adaptive goal

Adaptive is just a prompt — the LLM decides:

```markdown
# .lf/goals/adaptive.md

Look at the roadmap in .docs/roadmap/ and recent git history. Decide:
- Substantial approved items waiting? Build one.
- Major recent changes or bug churn? Simplify first.
- Direction unclear? Roadmap.

Bias toward building if good work is queued. Only roadmap when you need direction.
```

No hardcoded heuristics. The agent judges based on context.

## Roadmap goal

```markdown
# .lf/goals/roadmap.md

Read .docs/ to understand where we're going.

Be honest about what's working and what isn't. Double down on what's working. Stop doing what isn't. Then figure out what's next.

Propose concrete items to .docs/roadmap/<area>/.
```

## Concepts

**Steps** — granular operations (tasks):
- `grease`, `polish`, `simplify`, `review`, `implement`, `design`, etc.
- Single-purpose, can be chained in pipelines

**Goals** — what a looping agent works toward:
- `roadmap` — propose substantial work to `.docs/roadmap/`
- `build` — build from approved roadmap specs
- `simplify` — make cleaner, easier, smaller
- `adaptive` — switch between goals based on context (extensible)

**Loops** — run a goal continuously
**Flows** — run a goal (or chain) once

Goals internally run pipelines of steps:
```
goal: build
  └── steps: implement → review → polish → commit

goal: roadmap
  └── steps: design → commit
```

Loops/flows can chain goals:
```bash
lfd loop adaptive              # single goal, switches internally
lfd flow "roadmap → build"     # explicit goal chain
```

## Goal configuration

```yaml
# .lf/goals/product-engineer.md frontmatter
---
area: api
goal: adaptive                # switches between roadmap/build/simplify
merge_mode: pr                # conservative: human reviews
---

# .lf/goals/simplifier.md frontmatter
---
area: api
goal: simplify
merge_mode: land              # aggressive: auto-land cleanups
---
```

## Autonomy via merge_mode

`merge_mode` controls how aggressively work proceeds:

| merge_mode | When roadmap thin | When roadmap full |
|------------|-------------------|-------------------|
| `pr` | Propose → human approves → build | Build → PR → human reviews |
| `land` | Propose → self-approve → build immediately | Build → auto-land |

With `land`, propose + build collapse — agent identifies opportunity, specs briefly, builds, ships. This is opportunistic expansion without a separate goal.

## Key functions

```python
def load_roadmap(repo: Path) -> Roadmap:
    """Load all roadmap items from .docs/roadmap/."""

def propose_item(area: str, prompt: str) -> RoadmapItem:
    """Run design flow, write to .docs/roadmap/<area>/."""

def start_item(item: RoadmapItem) -> None:
    """Mark item in-progress, copy to .design/<branch>.md."""

def complete_item(item: RoadmapItem) -> None:
    """Move to _done/, clean up .design/."""
```

## Commands

```bash
# Human-driven
lf roadmap: add rate limiting to api    # interactive design → proposed item
lf roadmap --approve api/rate-limiting  # mark approved

# Agent-driven (via goal)
lfd loop api-engineer                   # auto-selects propose vs build
```

## How agents find their work

Roadmap is fully public — everyone sees all of `.docs/roadmap/`. But each goal owns an area:

```
.docs/roadmap/
  api/           ← api-engineer OWNS these
  ui/            ← ui-engineer OWNS these
  core/          ← core-engineer OWNS these
```

Context includes the whole roadmap — you see what other agents are planning. Spot dependencies, avoid duplicate work, understand the big picture. Your `area` determines what you're responsible for building.

## Resolved questions

**Cross-area items** — You plan for your area. If it spans areas, break it up or pick the primary owner.

**Priority** — Always evaluate: where's the gap between vision and implementation? Where's the most leverage? Agent judges.

**Maestro UI** — No special UI. Roadmap items are files in the repo. Maestro already shows files.

## Migration note

This roadmap system replaces the existing `lfwork` command. The key differences:
- Roadmap is file-based in `.docs/roadmap/<area>/`, not a generic work queue
- Area organization ties directly into goal ownership
- No claimed_by/blocked_on concepts (agent judgment handles this)

The Asana backend from lfwork could be added to roadmap later if needed.

## Constraints

- Roadmap items are markdown files, not a database
- Status lives in frontmatter, not filename
- `_done/` is the archive, kept for reference
- Agents can read roadmap, goals determine what they can write

## Done when

```bash
# Roadmap loading works
lf roadmap --list                       # shows items by area and status

# Propose flow works
lf roadmap: add caching to api          # creates .docs/roadmap/api/caching.md

# Agent work selection works
lfd loop api-engineer                   # proposes if thin, builds if full
```
