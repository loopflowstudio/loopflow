---
layout: default
title: Concepts
---

# Concepts

Loopflow is built around four primitives. Everything else is tooling around these.

## The four files

| Primitive | File | Question it answers |
|-----------|------|---------------------|
| **Config** | `.lf/config.yaml` | What does the agent see? |
| **Step** | `.lf/*.md` | What does the agent do? |
| **Voice** | `.lf/voices/*.md` | How does the agent judge? |
| **Flow** | `.lf/flows/*.py` | What order do steps run? |

---

## Config

The raw material. What files, docs, and context the agent sees.

```yaml
# .lf/config.yaml
context:
  - src/schema.py
  - docs/api.md

summaries:
  - path: src
    tokens: 10000
```

Config shapes the agent's view of your codebase. It doesn't tell the agent what to do — it determines what information is available when the agent runs.

See [Configuration](config.md) for the full reference.

---

## Step

The action. A markdown file that tells the agent what to do.

```markdown
# .lf/review.md

Review the code on this branch. Check for:
- Correctness
- Test coverage
- Style guide compliance

Fix any issues you find.
```

Steps run to completion. They have **exit conditions** — they finish when the work is done.

Built-in steps: `debug`, `design`, `implement`, `polish`, `review`, `lint`

Create your own in `.lf/` or `.claude/commands/`. Override built-ins by using the same name.

---

## Voice

The perspective. A voice does two things:

1. **Guides** — defines what success looks like
2. **Multiplexes** — stack as many as you want on any step or flow

```markdown
# .lf/voices/product-engineer.md

You are a product engineer. You care about:
- User value over technical elegance
- Shipping over perfection
- Pragmatic tradeoffs

When reviewing code, ask: does this solve the user's problem?
When implementing, ask: what's the simplest thing that works?

Communicate directly. Skip the caveats.
```

Voices don't run on their own — they combine with steps. The same step runs differently under different voices.

```bash
lf review --voice product-engineer              # one voice
lf review --voice product-engineer,concise      # stack multiple
lf implement --voice designer,thorough          # different combination
```

Voices compose. A `product-engineer` voice defines success criteria. A `concise` voice shapes communication style. Stack them to get both.

---

## Flow

The sequence. Chains steps together with commits between them.

```python
# .lf/flows/ship.py
def flow():
    return Flow(["implement", "polish", "review"])
```

Flows orchestrate. Run a flow and it executes each step in order, committing after each one succeeds.

```bash
lf --flow ship
```

Flows answer: what order do steps run? When does one step hand off to the next?

---

## How they combine

```
Config     →  assembles context
Step       →  executes with that context
Voice      →  shapes interpretation and success criteria
Flow       →  chains multiple steps
```

A typical session:

```bash
lfd loop src/ --flow ship --voice product-engineer
```

This runs the `ship` flow (implement → polish → review) through the `product-engineer` voice, using whatever context is configured.

---

## What goes where

```
.lf/
├── config.yaml           # context configuration
├── design.md             # step: design features
├── implement.md          # step: build from design
├── polish.md             # step: run tests, fix issues
├── review.md             # step: assess and fix
├── voices/
│   ├── product-engineer.md
│   ├── designer.md
│   └── infra-engineer.md
└── flows/
    ├── ship.py
    └── submit.py
```

Built-in steps and voices ship with loopflow. Override them by creating files with the same name in your repo.

---

## Next

- [Quick fix](quick-fix.md) — One-command debugging
- [Feature workflow](workflow.md) — Design, implement, ship
- [Background agents](agents.md) — Autonomous work
