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

### diff_files

Include the full content of files touched by the current branch. Default: `true`.

```yaml
diff_files: true
```

This is the primary way agents see what's changed on the branch. Instead of just seeing the diff output, the agent gets the complete file content for every modified file.

Override per-run with `--diff-files/--no-diff-files`:

```bash
lf review --no-diff-files     # skip loading changed files
```

### diff

Include raw `git diff main...HEAD` output. Default: `false`.

```yaml
diff: false
```

The raw diff shows exactly what lines changed but lacks context. Most tasks work better with `diff_files` enabled instead.

Override per-run with `--diff/--no-diff`:

```bash
lf review --diff              # include raw diff
lf review --diff --diff-files # both: files + diff
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

### summaries

Pre-generated codebase summaries included in every prompt. Useful for giving agents context about large codebases without loading all files.

```yaml
summary_tokens: 25000  # Default token budget for summaries

summaries:
  - path: src
    model: claude       # Model to use for generation
  - path: lib
    tokens: 5000        # Override default for this path
```

Generate summaries with:

```bash
lfops summarize src             # Generate summary for src/
lfops summarize -t 20000 src    # With specific token budget
lfops summarize -a              # Regenerate all configured summaries
```

Summaries are cached in `.lf/summaries/` and auto-refreshed when source files change on main.

### work

Work queue configuration for task management. Integrates with `lfwork` CLI.

```yaml
work:
  backend: file              # "file" or "asana"
  auto_rebase: true          # Rebase before starting work
  auto_land: false           # Auto-land completed work
```

With Asana backend:

```yaml
work:
  backend: asana
  asana:
    project_id: "1234567890"
```

Work items live in `.todo/` (file backend) or sync with Asana. Use `lfwork` to manage:

```bash
lfwork list                  # Show work items
lfwork add "Fix bug"         # Add item
lfwork approve <id>          # Approve for work
lfwork next                  # Show next available item
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

Tasks are markdown files in `.claude/commands/` (preferred) or `.lf/`. The filename (minus extension) is the task name.

```
.claude/commands/
  review.md
  implement.md
  test.md
  commit.md
```

Run by name:

```bash
lf review      # runs .claude/commands/review.md
lf implement   # runs .claude/commands/implement.md
```

### Task arguments

Pass arguments after a colon:

```bash
lf implement: add user authentication
```

Inside the task file, use `{args}` as a placeholder:

```markdown
# .claude/commands/implement.md

Implement the following feature:

{args}

Follow the existing code style.
```

## Environment variables

No environment variable overrides are supported yet.

## Auto-included context

Every task automatically includes:

1. All `.md` files at repository root (README, STYLE, etc.)
2. Files touched by the current branch (if `diff_files: true`, the default)
3. Files specified in `context` config
4. Files passed with `-x` flag

Raw diff output is not included by default. Enable with `diff: true` or `--diff` flag.
