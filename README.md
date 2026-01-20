# Loopflow

Reusable prompts for AI coding agents. Store them in git, chain them into pipelines, run them across isolated worktrees.

![Demo](docs/demo.gif)

## Install

```bash
pip install loopflow
lfops install
```

## Quick Start

**Debug an error:**
```bash
# Copy an error to clipboard, then:
lf debug -v
```

**Build a feature:**
```bash
wt switch --create my-feature
lf design: add user authentication
lf implement && lf polish && lf review
lfops pr
```

**Run a pipeline:**
```bash
lf ship    # implement → polish → review → PR
```

## How It Works

Tasks are markdown files in `.claude/commands/`:

```markdown
# .claude/commands/review.md

Review the diff on this branch. Fix any issues.

## What to look for
- Bugs and edge cases
- Style guide violations
```

Run by name:

```bash
lf review                     # run the task
lf review -x src/api.py       # add context
lf : "fix the typo"           # inline prompt
```

Loopflow assembles context (repo docs, branch diff, files you specify) and passes it to Claude Code, Codex, or Gemini.

[Learn more about tasks](docs/builtins.md)

## Worktrees

Agents work in isolated branches so you can keep working on something else:

```bash
wt switch --create auth       # create worktree
wt list                       # show all worktrees
```

[Learn more about worktrees](docs/patterns.md#worktrees)

## Documentation

- [Getting Started](docs/getting-started.md) — install, first task, ship your work
- [Built-in Tasks](docs/builtins.md) — debug, design, implement, polish, review
- [Configuration](docs/config.md) — models, context, pipelines
- [Patterns](docs/patterns.md) — workflows and recipes
- [Command Reference](docs/lf.md) — all flags and options

## Works With

Loopflow plays nicely with:

- **[worktrunk](https://github.com/loopflowstudio/worktrunk)** — git worktree management (`wt` commands)
- **[superpowers](https://github.com/obra/superpowers)** — skill library for AI agents (run via `lf sp:<skill>`)

## Requirements

macOS. Works with [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex CLI](https://github.com/openai/codex), or [Gemini CLI](https://github.com/google-gemini/gemini-cli).

## License

MIT
