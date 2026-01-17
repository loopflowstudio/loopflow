---
layout: default
title: Patterns
---

# Patterns

Workflows and recipes for common scenarios.

## Quick Debug

Copy an error, paste it to loopflow:

```bash
# Run something, get an error, copy it
lf debug -v    # -v pastes clipboard
```

The debug task reads the stacktrace, finds the file, fixes the bug. No context-gathering required.

## Design-First Development

Start with a design task that explores the problem before writing code.

```bash
wt switch --create auth-feature
lf design: add OAuth login    # interactive: discuss approach
lf implement                  # builds what you designed
lf polish                     # runs tests, fixes issues
lf review                     # produces verdict
```

The built-in design task writes to `.design/<branch>.md`. Implement reads it automatically.

## Parallel Features

Run multiple features simultaneously in separate worktrees:

```bash
# Terminal 1
wt switch --create feature-a
lf ship &

# Terminal 2
wt switch --create feature-b
lf ship &

# Check status
lfops status
```

## Model Racing

Race models against each other, pick the winner:

```bash
lf implement --race claude,codex: add caching layer
```

This runs both models in parallel worktrees, then uses a judge prompt to pick the better implementation.

For parallel without judging:

```bash
lf implement --parallel claude,codex: add caching
```

Compare results manually with `lfwt compare`.

## Summarization

For large codebases, pre-generate summaries so agents have context without loading all files:

```bash
lfops summarize src/           # Generate summary for src/
lfops summarize -a             # Regenerate all configured summaries
```

Configure in `.lf/config.yaml`:

```yaml
summary_tokens: 25000
summaries:
  - path: src
  - path: lib
    tokens: 5000
```

Summaries auto-refresh when source files change on main.

## Autonomous Pipeline

Run a full pipeline non-interactively:

```bash
lf ship
```

In autonomous mode:
- No interactive prompts
- Auto-commits between tasks
- Pushes if `push: true` in config
- Opens PR if `pr: true`

## Context Options

Add specific files to context:

```bash
lf implement -x src/models.py -x src/api.py: add user endpoints
```

Paste clipboard content:

```bash
lf debug -v              # -v pastes clipboard
```

Set default context in config:

```yaml
context:
  - src/schema.py
  - docs/api.md
```

## Inline Prompts

Quick one-off tasks without a task file:

```bash
lf : "fix the typo in the README"
lf : "add type hints to utils.py"
lf : "rename getUserById to findUserById everywhere"
```

## Custom Pipelines

Define pipelines for different workflows:

```yaml
pipelines:
  ship:
    tasks: [implement, review, polish, commit]
    pr: true

  quick:
    tasks: [implement, commit]
    push: true

  polish:
    tasks: [review, polish]
```

```bash
lf ship      # full workflow
lf quick     # fast iteration
lf polish    # cleanup pass
```

## PR Workflow

```bash
lfops pr      # create or update PR, open in browser
lfops land    # merge and cleanup worktree
```

The `pr` command is idempotent: run it to create, or again to update after more commits.

## Work Queue

Manage tasks with the work queue:

```bash
lfwork add "Implement dark mode"    # propose work
lfwork approve <id>                  # approve for work
lfwork next                          # show next item for agents
```

Configure backend in `.lf/config.yaml`:

```yaml
work:
  backend: file      # or "asana"
  auto_rebase: true
```

## Yolo Mode

For trusted pipelines, skip all permission prompts:

```yaml
yolo: true
```

Use when:
- Running in an isolated worktree
- Pipeline is well-tested
- You want fully autonomous operation
