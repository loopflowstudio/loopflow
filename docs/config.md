---
layout: default
title: Configuration
---

# Configuration

Loopflow configuration lives in `.lf/config.yaml` at the root of your repository.

## Example

```yaml
model: claude
push: true
pr: false

context:
  - src/schema.py
  - docs/api.md

exclude:
  - "*.test.ts"
  - node_modules

ide:
  warp: true
  cursor: true
  workspace: myproject.code-workspace

pipelines:
  ship:
    tasks: [implement, review, test, commit]
    pr: true
```

## Options

### model

Default model for all tasks. Currently supported: `claude`, `codex`.

```yaml
model: claude
```

Override per-run with `-m`:

```bash
lf review -m codex
```

### push

Auto-push after commits in batch mode (`-p`).

```yaml
push: true
```

### pr

Open a PR after pipelines complete.

```yaml
pr: true
```

### yolo

Skip all permission prompts. Passes `--dangerously-skip-permissions` to Claude Code.

```yaml
yolo: true
```

Use with caution. Best for trusted pipelines in worktrees.

### context

Files always included as context for every task.

```yaml
context:
  - src/schema.py
  - ARCHITECTURE.md
```

These are added alongside the automatic context (all `.md` files at repo root).

### exclude

Glob patterns to exclude from file listings shown to the agent.

```yaml
exclude:
  - "*.test.ts"
  - node_modules
  - dist
```

### ide

Configure which IDEs open when creating worktrees.

```yaml
ide:
  warp: true           # open Warp terminal
  cursor: true         # open Cursor editor
  workspace: app.code-workspace  # optional: open this workspace file
```

## Pipelines

Pipelines chain tasks together. Each task runs in batch mode (`-p`), with auto-commits between steps.

```yaml
pipelines:
  ship:
    tasks: [implement, review, test, commit]
    push: true   # override global push setting
    pr: true     # open PR when done
```

Run a pipeline:

```bash
lf ship
```

### Pipeline options

| Option | Description |
|--------|-------------|
| `tasks` | List of task names to run in order |
| `push` | Override global `push` setting for this pipeline |
| `pr` | Open PR after pipeline completes |

## Task files

Tasks are markdown files in `.lf/`. The filename (minus extension) is the task name.

```
.lf/
  config.yaml
  review.lf
  implement.lf
  test.lf
  commit.lf
```

Run by name:

```bash
lf review      # runs .lf/review.lf
lf implement   # runs .lf/implement.lf
```

### Task arguments

Pass arguments after a colon:

```bash
lf implement: add user authentication
```

Inside the task file, use `{args}` as a placeholder:

```markdown
# .lf/implement.lf

Implement the following feature:

{args}

Follow the existing code style.
```

## Environment variables

Override config with environment variables:

| Variable | Description |
|----------|-------------|
| `LF_MODEL` | Default model (`claude`, `codex`) |

## Auto-included context

Every task automatically includes:

1. All `.md` files at repository root (README, STYLE, etc.)
2. Current git diff (staged and unstaged)
3. Files specified in `context` config
4. Files passed with `-x` flag
