# Documentation Revision

Revise docs/ to reflect current state: maestro app, evolved prompts, research positioning.

## What to build

New documentation structure inspired by Stripe (upfront caveats, quick start first), Notion (progressive disclosure), Cursor (role-based entry points), and Linear (workflow-focused).

## Current problems

1. **Missing maestro** — UI layer not documented at all
2. **Prompts live in .claude/commands/** — docs still say `.lf/`
3. **No limitations section** — Stripe-style honesty missing
4. **Buried quick start** — install comes before "try this command"
5. **Research insights not reflected** — positioning doc explains value prop better than current docs

## New structure

```
docs/
├── index.md          # Quick start at top, what it is, install, 30-second demo
├── tasks.md          # Tasks and prompts (NEW - extracted from config)
├── pipelines.md      # Pipelines and chaining (NEW - extracted from config)
├── configuration.md  # Config reference only, no conceptual content
├── maestro.md        # Session tracking + UI (NEW)
├── cli.md            # Command reference (NEW - extracted from README)
├── limitations.md    # Upfront caveats (NEW)
└── patterns.md       # Recipes (keep, but trim)
```

## index.md

```markdown
---
layout: default
title: Loopflow
---

# Loopflow

Run LLM coding tasks from reusable prompt files.

\`\`\`bash
pip install loopflow && lf ops init && lf review
\`\`\`

## 30-second demo

\`\`\`bash
# Review current branch
lf review

# Implement a feature
lf implement: add user auth

# Run full pipeline: implement → review → test → commit → PR
lf ship
\`\`\`

## What it is

Prompts as files. Pipelines chain them. Worktrees keep work isolated.

- **Tasks** — Prompt files in `.claude/commands/` or `.lf/`
- **Pipelines** — Chain tasks: `ship: [implement, review, test, commit]`
- **Session tracking** — See what's running across terminals

Supports Claude Code, OpenAI Codex, Google Gemini CLI. macOS only.

## Install

\`\`\`bash
# With uv (recommended)
uv tool install loopflow

# Or pip
pip install loopflow

# Install Claude Code + worktrunk
lf ops install
\`\`\`

Then initialize your repo:

\`\`\`bash
cd your-repo
lf ops init
\`\`\`

## Why worktrees?

Agents commit to branches. If an agent commits to the branch you're editing, you have a problem.

Worktrees give each feature its own directory. You work in main, agents work in `repo.feature-name`.

\`\`\`bash
wt switch --create my-feature
cd ../myrepo.my-feature
lf ship                        # agents work here
\`\`\`

When done: `lf ops pr create` to open PR, `lf ops land` to merge.

## Next

- [Tasks](tasks.md) — Writing and running prompts
- [Pipelines](pipelines.md) — Chaining tasks together
- [Configuration](configuration.md) — All options
- [Maestro](maestro.md) — Session tracking UI
- [Limitations](limitations.md) — What loopflow can't do
```

## tasks.md

```markdown
---
layout: default
title: Tasks
---

# Tasks

A task is a prompt file. Run it by name:

\`\`\`bash
lf review              # runs .claude/commands/review.md or .lf/review.lf
lf implement: add auth # pass arguments after the colon
lf : "fix typo"        # inline prompt, no file
\`\`\`

## Where tasks live

Loopflow searches in order:

1. `.claude/commands/{name}.md` — Claude Code standard location
2. `.lf/{name}.lf` — Loopflow-specific
3. `.lf/{name}.md` — Fallback

Use `.claude/commands/` if you want tasks to work with both loopflow and Claude Code directly.

## Task file format

Tasks are markdown. The filename (minus extension) is the task name.

\`\`\`markdown
# .claude/commands/review.md

Review the diff on the current branch against main.

Fix any issues you find. The deliverable is the fixes, not a written review.

## What to look for

- Style guide violations (see STYLE.md)
- Bugs and edge cases
- Unnecessary complexity
\`\`\`

## Arguments

Pass arguments after a colon. Use `{args}` as placeholder:

\`\`\`markdown
# .lf/implement.lf

Implement the following feature:

{args}

Follow the existing code style.
\`\`\`

\`\`\`bash
lf implement: add OAuth login
\`\`\`

## Task frontmatter

Override per-task settings with YAML frontmatter:

\`\`\`markdown
---
interactive: true
model: claude:opus
include:
  - tests/**
exclude:
  - "*.test.ts"
---

# Design

Explore how to implement {args}...
\`\`\`

Frontmatter options:
- `interactive: true` — Run in interactive mode
- `model: backend:variant` — Override default model
- `include: [patterns]` — Additional context files
- `exclude: [patterns]` — Files to exclude

## Auto-included context

Every task automatically gets:

1. All `.md` files at repo root (README, STYLE, etc.)
2. Current git diff (staged + unstaged)
3. Files in `context` config
4. Files passed with `-x` flag

## Run modes

Tasks run in **auto mode** by default: non-interactive, streaming output, logs to `~/.lf/logs/`.

\`\`\`bash
lf implement           # auto mode (default)
lf design              # interactive if configured
lf implement -i        # force interactive
lf design -a           # force auto
\`\`\`

Configure default interactive tasks in `.lf/config.yaml`:

\`\`\`yaml
interactive:
  - design
  - iterate
\`\`\`

## Built-in tasks

Loopflow includes prompts for common workflows. Run `lf ops init --prompts` to install them:

| Task | Purpose |
|------|---------|
| design | Explore approach before coding |
| implement | Build what's in .design/ |
| review | Find and fix issues |
| polish | Run tests, clean up |
| iterate | Improve code on branch |
| reduce | Simplify without changing behavior |
| draft_commit | Generate commit message |
| rebase | Rebase onto main |

See [PROMPTS.md](https://github.com/user/loopflow/blob/main/PROMPTS.md) for how they chain together.
```

## pipelines.md

```markdown
---
layout: default
title: Pipelines
---

# Pipelines

Pipelines chain tasks. Each runs in auto mode with auto-commits between.

\`\`\`yaml
# .lf/config.yaml
pipelines:
  ship:
    tasks: [implement, review, test, commit]
    pr: true
\`\`\`

\`\`\`bash
lf ship    # runs the pipeline
\`\`\`

## How pipelines work

1. Run first task
2. If changes exist, commit them
3. Run next task
4. Repeat until done
5. If `pr: true`, open PR

## Pipeline options

| Option | Description |
|--------|-------------|
| `tasks` | List of task names to run |
| `push` | Push after commits (overrides global) |
| `pr` | Open PR when done |

## Example pipelines

\`\`\`yaml
pipelines:
  # Full workflow
  ship:
    tasks: [implement, review, test, commit]
    pr: true

  # Fast iteration
  quick:
    tasks: [implement, commit]
    push: true

  # Cleanup pass
  polish:
    tasks: [review, test]
\`\`\`

## Commit behavior

Commits between tasks include:
- Task name in message
- Model used
- Prompt content hash (for reproducibility)

Set `push: true` to auto-push after each commit.
```

## maestro.md

```markdown
---
layout: default
title: Maestro
---

# Maestro

Maestro tracks running sessions and provides a visual UI for managing loopflow tasks.

## Session tracking

Every auto-mode task registers with the session database:

\`\`\`bash
lf ops status          # show running sessions
\`\`\`

Output shows task name, repo, worktree, duration, and status.

## The Maestro app (optional)

Maestro is a macOS app that provides:

- Visual session list
- Worktree sidebar
- Prompt launcher
- Log tailing

### Start the daemon

\`\`\`bash
lf ops maestro start   # start background daemon
lf ops maestro stop    # stop daemon
\`\`\`

The daemon runs on `localhost:8420` and the Maestro app connects to it.

### Requirements

- macOS 15+
- Claude Code installed
- Daemon running

## Without Maestro

Session tracking works without the app:

\`\`\`bash
# Check what's running
lf ops status

# View logs directly
tail -f ~/.lf/logs/<worktree>/<task>.log
\`\`\`

## Parallel sessions

Run multiple tasks across worktrees:

\`\`\`bash
# Terminal 1
cd ../myrepo.feature-a && lf ship &

# Terminal 2
cd ../myrepo.feature-b && lf ship &

# Check both
lf ops status
\`\`\`

Maestro shows all sessions in one view.
```

## limitations.md

```markdown
---
layout: default
title: Limitations
---

# Limitations

Be aware of these constraints before using loopflow.

## macOS only

Loopflow requires macOS. The session tracking daemon uses launchd, and the Maestro app is a native macOS application.

Linux and Windows are not supported. `lf ops install` will fail on other platforms.

## Requires Claude Code, Codex, or Gemini CLI

Loopflow wraps existing AI coding CLIs. You need at least one installed:

\`\`\`bash
lf ops install         # installs Claude Code via npm
\`\`\`

Loopflow does not provide its own AI capabilities.

## No IDE integration

Loopflow runs in the terminal. There's no VS Code extension, no Cursor plugin, no editor integration.

If you want IDE features, use Claude Code or Codex directly.

## Worktrees recommended

Without worktrees, agents commit to your current branch. This can conflict with your own work.

We strongly recommend using worktrees:

\`\`\`bash
wt switch --create feature
cd ../repo.feature
lf ship
\`\`\`

## Session tracking is local

The SQLite database lives at `~/.lf/maestro.db`. Sessions aren't shared across machines or users.

## No remote execution

Tasks run locally. There's no cloud execution, no queue system, no distributed agents.

## Model rate limits apply

Loopflow doesn't manage rate limits. If Claude or Codex throttles you, tasks will fail or slow down.

## Auto mode skips confirmation

In auto mode (the default), tasks run without asking permission. Use `yolo: false` in config if you want Claude Code to prompt before file changes.

## Pipeline failures leave state

If a pipeline fails mid-way, you'll have partial commits. Check `git log` and decide whether to continue or reset.
```

## configuration.md

Trim to reference only:

```markdown
---
layout: default
title: Configuration
---

# Configuration

Config lives in `.lf/config.yaml`.

## Full example

\`\`\`yaml
agent_model: claude:opus
push: true
pr: false
yolo: true

interactive:
  - design
  - iterate

context:
  - src/schema.py
  - docs/api.md

exclude:
  - "*.test.ts"
  - node_modules

ide:
  warp: true
  cursor: true

pipelines:
  ship:
    tasks: [implement, review, test, commit]
    pr: true
\`\`\`

## Options reference

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `agent_model` | string | `claude` | Default model (backend:variant) |
| `push` | bool | `false` | Auto-push after commits |
| `pr` | bool | `false` | Open PR after pipelines |
| `yolo` | bool | `false` | Skip permission prompts |
| `interactive` | list | `[]` | Tasks that default to interactive |
| `context` | list | `[]` | Always-included context files |
| `exclude` | list | `[]` | Glob patterns to exclude |
| `ide.warp` | bool | `false` | Install Warp terminal |
| `ide.cursor` | bool | `false` | Install Cursor editor |

## Pipelines

\`\`\`yaml
pipelines:
  name:
    tasks: [task1, task2]    # required
    push: true               # optional, overrides global
    pr: true                 # optional, open PR when done
\`\`\`

## Per-task overrides

Use frontmatter in task files. See [Tasks](tasks.md#task-frontmatter).

## CLI overrides

| Flag | Description |
|------|-------------|
| `-i, --interactive` | Force interactive mode |
| `-a, --auto` | Force auto mode |
| `-m, --model` | Override model |
| `-x, --context` | Add context files |
```

## cli.md

```markdown
---
layout: default
title: CLI Reference
---

# CLI Reference

## Running tasks

\`\`\`bash
lf <task>              # run task from .claude/commands/ or .lf/
lf <task>: args        # pass arguments
lf : "prompt"          # inline prompt
lf <pipeline>          # run pipeline from config
\`\`\`

## Options

| Option | Description |
|--------|-------------|
| `-i, --interactive` | Run interactively |
| `-a, --auto` | Run in auto mode |
| `-x, --context FILE` | Add context file |
| `-m, --model MODEL` | Choose model |
| `-c, --copy` | Copy prompt to clipboard |
| `-v, --paste` | Include clipboard in prompt |
| `--parallel a,b` | Run with multiple models |

## Operations

\`\`\`bash
lf ops init            # initialize repo
lf ops install         # install dependencies
lf ops doctor          # check installation
lf ops status          # show running sessions
\`\`\`

## Maestro

\`\`\`bash
lf ops maestro start   # start daemon
lf ops maestro stop    # stop daemon
\`\`\`

## Pull requests

\`\`\`bash
lf ops pr create       # open PR from branch
lf ops pr land         # merge PR
lf ops pr land -a      # auto-merge when checks pass
lf ops land            # local merge via worktrunk
\`\`\`

## Worktrees

Loopflow uses worktrunk for worktree management:

\`\`\`bash
wt list                # show worktrees
wt switch --create X   # create or switch
wt remove X            # remove worktree + branch
\`\`\`
```

## patterns.md

Keep but trim to essentials:

```markdown
---
layout: default
title: Patterns
---

# Patterns

## Design-first

\`\`\`bash
wt switch --create auth
cd ../myrepo.auth
lf design: add OAuth
lf implement
lf review
\`\`\`

## Parallel features

\`\`\`bash
cd ../myrepo.feature-a && lf ship &
cd ../myrepo.feature-b && lf ship &
lf ops status
\`\`\`

## Model comparison

\`\`\`bash
lf implement --parallel claude,codex: add caching
# Creates two worktrees, runs both
\`\`\`

## Review-only

\`\`\`bash
lf review              # find issues
lf review -a           # find and fix
\`\`\`

## Inline prompts

\`\`\`bash
lf : "fix the typo"
lf : "add type hints to utils.py"
\`\`\`
```

## Constraints

- Don't duplicate info between pages—reference instead
- Quick start must work with copy-paste (no placeholders)
- Limitations before features (Stripe pattern)
- Task file location is `.claude/commands/` first, not `.lf/`
- Maestro is optional, CLI works without it
- macOS only—be explicit

## Done when

```bash
# Docs render locally
cd docs && bundle exec jekyll serve

# Quick start works from scratch
pip install loopflow
lf ops init
lf review
# Should work

# Limitations page exists and loads
open http://localhost:4000/limitations

# All internal links resolve
# No broken references to old structure
```

## Open questions

None—structure is clear from research.
