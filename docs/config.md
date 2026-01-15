---
layout: default
title: Configuration
---

# Configuration

Loopflow configuration lives in `.lf/config.yaml` at the root of your repository.

## Example

```yaml
agent_model: claude:opus
push: true
pr: false

# Tasks that default to interactive mode
interactive:
  - design
  - iterate

# Default voice for all tasks
voice: architect

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

### agent_model

Default model for all tasks. Use `backend:variant` (e.g., `claude:opus`, `codex:o3`).

```yaml
agent_model: claude:opus
```

Override per-run with `-m`:

```bash
lf review -m codex:o3
```

### interactive

Tasks that default to interactive mode. All other tasks default to auto mode.

```yaml
interactive:
  - design
  - iterate
```

Without this setting, all tasks default to auto (non-interactive) mode. Use `-i` flag to override for any task.

### push

Auto-push after commits in auto mode.

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

Configure which IDEs loopflow installs (used by `lf ops install`).

```yaml
ide:
  warp: true           # open Warp terminal
  cursor: true         # open Cursor editor
  workspace: app.code-workspace  # optional: open this workspace file
```

### voice

Default voice(s) for all tasks. Voices are reusable personas that shape agent responses.

```yaml
voice: architect
```

Multiple voices:

```yaml
voice:
  - architect
  - concise
```

Override per-run with `--voice`:

```bash
lf review --voice concise
lf implement --voice architect,concise
```

Voice files live in `.lf/voices/` as plain markdown:

```
.lf/voices/architect.md
.lf/voices/concise.md
```

## Run Modes

By default, tasks run in **auto mode**: non-interactive with streaming output. This is ideal for most coding tasks and background execution. All runs append logs under `~/.lf/logs/<worktree>/`.

Tasks listed in the `interactive` config default to interactive mode instead. You can override the default for any task:

```bash
lf implement           # auto mode (default)
lf design              # interactive (if in config.interactive list)
lf implement -i        # force interactive
lf design -a           # force auto
lf implement &         # background (shell handles it)
```

## Pipelines

Pipelines chain tasks together. Each task runs in auto mode, with auto-commits between steps.

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

No environment variable overrides are supported yet.

## Auto-included context

Every task automatically includes:

1. All `.md` files at repository root (README, STYLE, etc.)
2. Current git diff (staged and unstaged)
3. Files specified in `context` config
4. Files passed with `-x` flag
