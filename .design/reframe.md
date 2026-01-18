# Reframe: Prompts + Git Workflow

## What to build

Rewrite docs to position loopflow as a CLI tool for prompt reuse. The core is simpler than current docs suggest:

- Store prompts as files
- Run them with `lf <task>`
- Ship with `lfops pr`

Everything else—pipelines, daemon, Maestro—moves to "advanced" or "coming soon."

---

## The new story

```bash
# Debug an error
lf debug -v                      # paste error, watch it fix

# Build a feature
wt switch --create my-feature    # worktree for isolation
lf design: add auth              # interactive design
lf implement                     # build it
lf polish                        # run tests, fix issues
lfops pr                         # open PR

# Land it
lfops land                       # squash-merge, cleanup
```

No pipelines. No daemon. Just tasks and git workflow.

---

## Tagline

**Keep:** "Arrange agents to code in harmony" (poetic, aspirational)

**Add subtitle:** "Reusable prompts. Composable workflows."

---

## Docs structure

**Level 1: Core**
```
docs/
  index.md          # Quick start: install, run a task, ship with lfops
  tasks.md          # Writing and organizing prompts
  workflow.md       # lfops pr, lfops land, lfops commit
  config.md         # .lf/config.yaml (trimmed to L1 features)
  patterns.md       # Recipes (debug, design-first, manual chaining)
```

**Level 2: Advanced**
```
docs/advanced/
  pipelines.md      # Declarative task chaining, auto-commit
  agents.md         # lfd daemon, background agents
  multi-model.md    # Racing, parallel execution
  api.md            # Socket protocol
```

**Coming Soon**
```
docs/
  roadmap.md        # Maestro GUI, future vision
```

---

## What moves where

| Feature | Current | New |
|---------|---------|-----|
| `lf <task>` | Level 1 | Level 1 |
| `lfops pr/land/commit` | Mentioned | Level 1 (prominent) |
| Worktrees (`wt`) | Level 1 | Level 1 |
| Pipelines | Level 1 | Level 2 |
| `lfd` daemon | Level 1 | Level 2 |
| Multi-model | Level 1 | Level 2 |
| Session tracking | Level 1 | Internal detail |
| Maestro | Level 1 | Coming soon |

---

## Key quotes to preserve

> "Store prompts in git."

> "Tasks are markdown files."

> "Tight loops. Do one thing, hand off cleanly."

---

## Done when

```bash
# Main docs focus on tasks + lfops workflow
grep -r "lfd\|daemon" docs/*.md   # only in docs/advanced/
grep -r "pipeline" docs/*.md      # only in docs/advanced/
grep -r "Maestro" docs/*.md       # only in docs/roadmap.md

# README has new subtitle
head -5 README.md | grep "Reusable prompts"

# Quick start shows manual chaining, not pipelines
grep "lf implement" docs/index.md | grep -v "lf ship"
```
