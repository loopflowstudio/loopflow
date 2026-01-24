# Heart Model: Rework README Conceptual Model

Rename "Primitives" → "Atoms". Promote Stimulus to an Atom. Rename Voice → Goal everywhere.

## What to build

Restructure the README's conceptual model. Rename Primitives → Atoms, Voice → Goal. Promote Stimulus to an Atom. An agent becomes **area × goal × flow × stimulus**.

## Built-in Goals

**Action goals** (what to do):
- `adapt` — decide what mode to operate in based on current state
- `roadmap` — propose new work based on where we're going
- `ship` — build from approved roadmap specs

**Perspective goals** (how to approach it):
- `product-engineer` — ship working features that solve real user problems
- `designer` — create clear, actionable design documents
- `infra-engineer` — maintain a fast, reliable development pipeline
- `ceo` — make high-leverage decisions about what to build and why

Each goal has clear **success criteria** (what "done" looks like).

Voices will return later as an "advanced" use case for personas and communication style.

## Changes

### 1. README model table

**Before:**
```markdown
| Primitive | What it does |
|-----------|--------------|
| **Step** | Runs a prompt with assembled context |
| **Flow** | Chains steps together |
| **Voice** | Shapes judgment and perspective |
| **Area** | Focuses on part of the codebase |

| Stimulus | Runs when |
|----------|-----------|
| **Once** | Single run |
| **Loop** | Continuously until stopped |
| **Watch** | Area changes on main |
| **Cron** | On schedule |

An agent is **flow × area × voice**.
```

**After:**
```markdown
| Atom | What it does |
|------|--------------|
| **Step** | Runs a prompt with assembled context |
| **Flow** | Chains steps together |
| **Goal** | Shapes judgment and intent |
| **Area** | Focuses on part of the codebase |
| **Stimulus** | When to run: once, loop, watch, cron |

An agent is **area × goal × flow × stimulus**.
```

### 2. Goal stacking

Explain in README:

> Goals compose. The first sets intent. Additional goals add perspective.

```bash
lf review -g designer                     # design quality focus
lf review -g product-engineer,designer    # product focus + design perspective
```

### 3. CLI flag rename

`-v / --voice` → `-g / --goal` everywhere:
- `lf` command
- `lfd loop`, `lfd run`, `lfd subscribe`, `lfd schedule`
- Config: `voice:` → `goal:` in `.lf/config.yaml`

### 4. Directory rename

`.lf/voices/` → `.lf/goals/`
`templates/voices/` → `templates/goals/`

### 5. Docs updates

All docs that reference "voice" get updated to "goal":
- docs/config.md
- docs/agents.md
- docs/lf.md
- docs/lfd.md
- docs/index.md

## Constraints

- Must update all references atomically—no mixed terminology
- No backwards compatibility—clean break, no deprecated aliases
- All examples use actual built-in goals, not placeholder names

## Done when

```bash
grep -r "voice" docs/ README.md .lf/ --include="*.md" --include="*.yaml"
# Returns nothing (or only the word in unrelated contexts)

lf review -g designer
# Works

lfd loop ship src/ -g product-engineer
# Works
```
