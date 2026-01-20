---
layout: default
title: lf Command Reference
---

# lf Command Reference

`lf` runs tasks against AI coding agents. It assembles context (repo docs, diff, files) and passes everything to Claude, Codex, or Gemini.

## Basic Usage

```bash
lf <task>                    # run a task file
lf <task>: args              # run with arguments
lf <prefix>:<skill>          # run external skill
lf : "inline prompt"         # no task file, just prompt
lf --list                    # show all available tasks
```

## Examples

```bash
lf review                    # run .claude/commands/review.md
lf implement: add auth       # pass arguments after colon
lf sp:brainstorm             # run superpowers brainstorm skill
lf : "fix the typo"          # inline prompt
lf debug -v                  # paste clipboard, fix the bug
```

## Task Files

Tasks are markdown files in these locations (searched in order):

1. External skills — `<prefix>:<skill>` format (e.g., `sp:brainstorm`)
2. `.claude/commands/<task>.md` — preferred, portable
3. `.lf/<task>.md` — local override
4. Built-in tasks — debug, design, implement, polish, review

### Task Arguments

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

## Context Flags

### Files and Directories

| Flag | Description |
|------|-------------|
| `-x FILE` | Add a file or directory to context |
| `--no-docs` | Skip repo docs (README, STYLE, etc.) |
| `--no-diff-files` | Skip files changed on this branch |
| `--diff` | Include raw `git diff` output |

### Clipboard

| Flag | Description |
|------|-------------|
| `-v, --paste` | Include clipboard content in prompt |

### Summaries

| Flag | Description |
|------|-------------|
| `--summaries` | Include pre-generated codebase summaries |
| `--no-summaries` | Skip summaries |

## Run Mode Flags

| Flag | Description |
|------|-------------|
| `-i, --interactive` | Run interactively (can interrupt, redirect) |
| `-a, --auto` | Run in auto mode (default, runs to completion) |

## Model Flags

| Flag | Description |
|------|-------------|
| `-m, --model MODEL` | Choose model (e.g., `claude:opus`, `codex`, `gemini`) |
| `--voice VOICE` | Apply voice/persona (comma-separated for multiple) |

## Output Flags

| Flag | Description |
|------|-------------|
| `-c, --copy` | Copy assembled prompt to clipboard, show token breakdown |

## Browser Automation

| Flag | Description |
|------|-------------|
| `--chrome` | Enable Chrome browser automation |
| `--no-chrome` | Disable Chrome automation |

## What's Included by Default

Every task automatically includes:

| Context | Default | How to disable |
|---------|---------|----------------|
| **Repo docs** (README, STYLE, etc.) | ✓ included | `--no-docs` |
| **Branch files** (files you've changed) | ✓ included | `--no-diff-files` |

## What's Opt-In

These require explicit flags or config:

| Context | How to enable |
|---------|---------------|
| **Raw diff** (line-by-line changes) | `--diff` |
| **Clipboard** | `-v` / `--paste` |
| **Extra files** | `-x FILE` |
| **Summaries** | Configure in `.lf/config.yaml` |
| **Chrome automation** | `--chrome` |

See [Configuration](config.md) for setting defaults via config file.

## Examples

### Debug with clipboard

```bash
# Run tests, copy the error
lf debug -v
```

### Review with extra context

```bash
lf review -x docs/api.md -x tests/
```

### Use a different model

```bash
lf implement -m codex: add caching
```

### Apply a voice

```bash
lf review --voice concise
lf implement --voice architect,concise
```

### Copy prompt without running

```bash
lf review -c    # shows token breakdown, copies to clipboard
```

### External skills

Run skills from external libraries like [superpowers](https://github.com/obra/superpowers):

```bash
lf sp:brainstorm              # run brainstorm skill from superpowers
lf sp:write-plan -m codex     # with a different model
lf sp:execute-plan -i         # interactive mode
```

External skills use loopflow's context assembly, so they get your repo docs, branch files, and other context that wouldn't be available running them directly.

Configure skill sources in `.lf/config.yaml`:

```yaml
skill_sources:
  - name: superpowers
    prefix: sp
    path: ~/.superpowers
```

If `~/.superpowers` exists, it's auto-detected with prefix `sp`.

## See Also

- [Built-in tasks](builtins.md) — debug, design, implement, polish, review
- [Configuration](config.md) — `.lf/config.yaml` options
- [Patterns](patterns.md) — workflows and recipes
