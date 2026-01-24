---
layout: default
title: Loopflow
---

# Loopflow

Arrange and conduct an agent orchestra.

## Try it

```bash
uv tool install loopflow
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug
# copy error to clipboard
lf debug -c                       # fix it
```

---

## The Model

| Atom | What it does | File |
|------|--------------|------|
| **Step** | Runs a prompt with assembled context | `.lf/steps/*.md` |
| **Flow** | Chains steps together | `.lf/flows/*.py` |
| **Goal** | Shapes judgment and intent | `.lf/goals/*.md` |
| **Area** | Focuses on part of the codebase | path argument |
| **Stimulus** | When to run: once, loop, watch, cron | command |

An agent is **area × goal × flow × stimulus**.

Area is the path you pass—not a file. It scopes what the agent sees and changes.

| Stimulus | Runs when |
|----------|-----------|
| **Once** | Single run (one-shot) |
| **Loop** | Continuously until stopped |
| **Watch** | Area changes on main |
| **Cron** | On schedule |

---

## Step

A markdown file that tells the agent what to do.

```markdown
# .lf/steps/review.md

Review the code on this branch. Check for:
- Correctness
- Test coverage
- Style guide compliance

Fix any issues you find.
```

```bash
lf review                     # run the step
lf review: focus on auth      # pass arguments
```

Steps run to completion. Built-ins: `debug`, `design`, `implement`, `polish`, `review`, `lint`.

**Where to put steps:** `.lf/steps/` is canonical. Symlink for Claude Code compatibility:

```bash
ln -s ../.lf/steps .claude/commands
```

---

## Flow

Chains steps together with commits between them.

```python
# .lf/flows/ship.py
def flow():
    return Flow(["implement", "polish", "review"])
```

```bash
lf --flow ship
```

Or chain manually:

```bash
lf design: add auth && lf implement && lf polish
```

---

## Goal

Shapes how the agent judges and responds.

```markdown
# .lf/goals/architect.md

You are a software architect. You care about:
- System boundaries and interfaces
- Long-term maintainability
- Avoiding premature abstraction

When reviewing, ask: will this scale?
When implementing, ask: what's the right seam?
```

```bash
lf review --goal architect
lf review --goal architect,concise    # stack multiple
```

Goals compose. An `architect` goal defines success criteria. A `concise` goal shapes communication style. Stack them to get both.

---

## Area

The path you pass to `lfd`. Scopes what the agent works on.

```bash
lfd loop ship src/api/        # work on src/api/
lfd loop ship .               # work on everything
```

Combined with flow and goal, area defines the agent's mission:

```bash
lfd loop ship src/api/ --goal architect
```

---

## Where Files Live

```
.lf/                      # Repo config and extensions
  config.yaml             # Model, context defaults
  steps/                  # Step prompts (preferred)
  goals/                  # Judgment and intent
  flows/                  # Flow definitions
.claude/commands/         # Steps (Claude Code compatible)
scratch/                  # PR scratchpad (cleared on merge)
roadmap/                  # Internal docs (persists)
~/.lf/                    # Global config and steps
```

### scratch/ vs roadmap/

| | scratch/ | roadmap/ |
|---|---|---|
| **Lifespan** | Dies with the PR | Lives forever |
| **Purpose** | Current work | Forward-looking plans |
| **Location** | Root only | Root + per-folder |
| **Example** | "Add auth" spec | "How auth should work long-term" |

`roadmap/` can exist at any level:
- Root `roadmap/` is auto-included (part of lfdocs)
- `src/api/roadmap/` holds API-specific plans
- Including a path via `-p` auto-includes its nested `roadmap/` folders

### What's Auto-Included

Every step sees: `README.md`, `STYLE.md`, `CLAUDE.md`, `scratch/`, `roadmap/`, and files touched by your branch.

---

## Next

[Background Agents →](agents.md)

## Reference

[`lf` commands](lf.md) · [`lfops` commands](lfops.md) · [`lfd` commands](lfd.md) · [Configuration](config.md)
