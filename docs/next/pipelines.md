---
layout: default
title: Pipelines
---

# Pipelines

*Coming soon.*

Declarative step chaining with auto-commits between steps.

## Overview

Pipelines chain steps together. Each step runs in auto mode, with automatic commits between steps. Define them in `.lf/config.yaml`:

```yaml
pipelines:
  ship:
    steps: [implement, review, polish, commit]
    push: true
    pr: true
```

Run a pipeline:

```bash
lf ship
```

## Pipeline Options

| Option | Description |
|--------|-------------|
| `steps` | List of step names to run in order |
| `push` | Override global `push` setting for this pipeline |
| `pr` | Open PR after pipeline completes |

## Examples

### Ship Pipeline

Full workflow from implementation to PR:

```yaml
pipelines:
  ship:
    steps: [implement, review, polish, commit]
    pr: true
```

### Quick Pipeline

Fast iteration without review:

```yaml
pipelines:
  quick:
    steps: [implement, commit]
    push: true
```

### Polish Pipeline

Cleanup pass on existing code:

```yaml
pipelines:
  polish:
    steps: [review, polish]
```

## Autonomous Mode

In autonomous mode:
- No interactive prompts
- Auto-commits between steps
- Pushes if `push: true` in config
- Opens PR if `pr: true`

```bash
lf ship    # runs each step, commits between steps
```

## Pipeline Files

You can also define pipelines as YAML files in `.lf/pipelines/`:

```yaml
# .lf/pipelines/ship.yaml
steps:
  - implement
  - review
  - polish
  - commit
push: true
pr: true
```

## Advanced Features

### Parallel Steps

Run steps in parallel:

```yaml
pipelines:
  test-all:
    steps:
      - parallel:
          - test-unit
          - test-integration
      - commit
```

### Model Racing

Race models against each other:

```yaml
pipelines:
  race-implement:
    steps:
      - race:
          step: implement
          models: [claude, codex]
      - commit
```
