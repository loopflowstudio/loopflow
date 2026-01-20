# Builtin Goals: Roles and Modes

Simplify the agent loop API by separating **roles** (what work you do) from **modes** (how you decide what to work on).

## What to build

A goal composition system where `adaptive` mode is implicit, roles are optional, and the API defaults to "just give me an area."

## Core Concepts

**Roles** — What kind of work you do. Examples: `product-engineer`, `designer`, `infra-engineer`. Define quality bars, iteration strategies, output expectations. Optional.

**Modes** — How you decide what to work on. Builtins: `adaptive`, `roadmap`, `build`, `simplify`. The `adaptive` mode is the orchestrator—it reads context and decides which sub-mode to invoke.

**Areas** — Pathsets that scope responsibility. `Maestro/`, `src/loopflow/`, etc.

The key insight: **adaptive should be implicit**. Most agents want the smart behavior—read roadmap, build if there's work, plan if there isn't. Only override when you need deterministic behavior.

## Data Structures

```python
class GoalKind(Enum):
    ROLE = "role"       # product-engineer, designer - how to work
    MODE = "mode"       # adaptive, roadmap, build, simplify - what to decide

@dataclass
class Goal:
    name: str
    content: str
    pipeline: str
    kind: GoalKind  # NEW: inferred or explicit from frontmatter

@dataclass
class LoopSpec:
    """What gets passed to create_loop. Area is primary."""
    area: str                    # required, e.g. "Maestro/" or "."
    goals: list[str]             # from -g flags, may be empty

    # Derived at runtime
    effective_goals: list[Goal]  # resolved goals including implicit adaptive
```

The Loop model changes:
```python
@dataclass
class Loop:
    # ... existing fields ...
    area: str                    # PRIMARY identifier (was optional)
    goals: list[str]             # replaces goal_name (was singular)
```

## Key Functions

```python
def resolve_goals(repo: Path, goal_names: list[str]) -> list[Goal]:
    """Load and resolve goal names to Goal objects."""

def needs_adaptive(goals: list[Goal]) -> bool:
    """True if no mode goal present—adaptive should be injected."""
    return not any(g.kind == GoalKind.MODE for g in goals)

def build_effective_goals(repo: Path, goal_names: list[str]) -> list[Goal]:
    """Build final goal list, injecting adaptive if needed.

    - If goal_names is empty → [adaptive]
    - If only roles → [adaptive] + roles
    - If any mode present → goals as-is (no adaptive injection)
    """

def detect_goal_kind(goal: Goal) -> GoalKind:
    """Infer kind from frontmatter or content heuristics.

    Explicit: frontmatter `kind: mode` or `kind: role`
    Heuristic: references `.docs/roadmap/` decisions → mode
    Default: role
    """

def render_goals(goals: list[Goal]) -> str:
    """Combine goals into single prompt. Modes first, then roles."""
```

## CLI Changes

Current:
```bash
lfd loop product-engineer -a Maestro/
```

New — area is required positional, goals via `-g`:
```bash
# Simplest: just an area (adaptive mode implied)
lfd loop Maestro/

# With role (adaptive still implied)
lfd loop Maestro/ -g product-engineer

# Multiple goals compose
lfd loop Maestro/ -g product-engineer -g build

# Explicit mode replaces adaptive
lfd loop src/loopflow/ -g roadmap

# Custom goal
lfd loop Maestro/ -g my-custom-goal
```

**Rule:** `adaptive` is implicit unless you add a mode goal via `-g`. Adding a mode (build, roadmap, simplify) replaces adaptive. Adding only roles keeps adaptive as the orchestrator.

## Goal Detection Logic

When user provides a goal name, detect if it's:

1. **A builtin mode** (`adaptive`, `roadmap`, `build`, `simplify`) — use directly, no wrapping
2. **A builtin role** (`product-engineer`, etc.) — wrap with adaptive
3. **A user goal with `kind: mode`** — use directly, no wrapping
4. **A user goal with `kind: role`** — wrap with adaptive
5. **A user goal with no kind** — check content for mode-like patterns (references to `.docs/roadmap/`), otherwise treat as role

The heuristic: if a goal talks about *deciding what to do*, it's a mode. If it talks about *how to do work*, it's a role.

## Composition Rendering

Goals render in order: modes first, then roles.

```markdown
<lf:mode:adaptive>
{adaptive.md content}
</lf:mode:adaptive>

<lf:role:product-engineer>
{product-engineer.md content}
</lf:role:product-engineer>

<lf:role:designer>
{designer.md content}
</lf:role:designer>
```

The mode decides *what* to work on. Multiple roles provide different perspectives on *how* to work—the agent synthesizes them.

## bin/agents-start After

```bash
#!/bin/bash
cd "$(git rev-parse --show-toplevel)"

# One agent per area
uv run lfd loop Maestro/ -g product-engineer -g designer
uv run lfd loop src/loopflow/ -g product-engineer -g designer
uv run lfd loop . -g infra-engineer

uv run lfd status
```

Three agents total:
- **Maestro/** — product + design work on the Mac app
- **src/loopflow/** — product + design work on the CLI/daemon
- **.** (root) — infrastructure across the whole repo

Each gets adaptive implicitly. Roles provide quality bars and perspectives.

## Constraints

- **Area required**: Every loop needs an area. Use `.` for whole repo.
- **Adaptive implicit**: No mode goal → adaptive is injected automatically
- **Smart detection**: Don't double-inject adaptive if a mode goal is present
- **Backwards compat**: Old `lfd loop goal -a area` should still work (see below)
- **Composable**: Multiple `-g` flags combine cleanly

## Backwards Compatibility

Old syntax:
```bash
lfd loop product-engineer -a Maestro/
```

Detection: if first positional has no `/` and `-a` is present, treat as old syntax:
- First positional → `-g` goal
- `-a` value → area positional

New syntax detection: first positional contains `/` or is `.` → it's an area.

## UI Changes

None for MVP. Maestro loops panel already shows goal names—would display composed name like "adaptive + product-engineer" but that's polish.

## Done When

```bash
# Simplest form works
lfd loop Maestro/

# Goals layer correctly
lfd loop Maestro/ -g product-engineer

# Mode goal replaces adaptive
lfd loop Maestro/ -g build
# (no double-wrapping with adaptive)

# Multiple goals compose
lfd loop Maestro/ -g product-engineer -g build

# Whole repo works
lfd loop .
```

And the status shows area as primary identifier:
```
lfd status
ID        TYPE   AREA              GOALS                          STATUS
abc123    loop   Maestro/          product-engineer, designer     running
def456    loop   src/loopflow/     product-engineer, designer...  idle
```

(adaptive is implicit, not shown unless explicitly added)

---

## Open Questions

1. **Naming: role vs persona vs archetype?** "Role" feels right but open to alternatives.

2. **How does adaptive dispatch?** Current `adaptive.md` tells the agent to *decide* which mode to operate in. Should it literally invoke sub-goals, or just provide guidance? Current: guidance only (simpler).

3. **Goal file changes needed?** Existing goals in `.lf/goals/` need `kind:` in frontmatter, or we detect heuristically. Builtins get explicit `kind: mode` or `kind: role`.
