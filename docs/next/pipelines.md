---
layout: default
title: Pipelines
---

# Pipelines

*Coming soon.*

Declarative step chaining with auto-commits between steps.

## Overview

Pipelines (flows) chain steps together. Each step runs in auto mode, with automatic commits between steps. Define them in `.lf/flows/`:

```python
def flow():
    return Flow(["implement", "review", "polish", "commit"])
```

Run a pipeline:

```bash
lf ship
```

## Pipeline Options

| Option | Description |
|--------|-------------|
| `steps` | List of step names to run in order |

## Examples

### Ship Pipeline

Full workflow from implementation to PR:

```python
def flow():
    return Flow(["implement", "review", "polish", "commit"])
```

### Quick Pipeline

Fast iteration without review:

```python
def flow():
    return Flow(["implement", "commit"])
```

### Polish Pipeline

Cleanup pass on existing code:

```python
def flow():
    return Flow(["review", "polish"])
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

Pipelines live in `.lf/flows/<name>.py`.

## Advanced Features

### Parallel Steps

Run steps in parallel:

```python
def flow():
    return {
        "steps": [
            {"parallel": ["test-unit", "test-integration"]},
            "commit",
        ]
    }
```

### Model Racing

Race models against each other:

```python
def flow():
    return {
        "steps": [
            {"step": "implement", "race": ["claude", "codex"]},
            "commit",
        ]
    }
```
