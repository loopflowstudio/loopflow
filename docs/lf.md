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

```bash
lf implement: add user authentication
```

Inside task files, `{args}` is replaced with whatever comes after the colon.

## Context Flags

### Files and Directories

| Flag | Description |
|------|-------------|
| `-x FILE` | Add a file or directory to context |
| `--no-lfdocs` | Skip repo docs (README, STYLE, etc.) |
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
| **Repo docs** (README, STYLE, etc.) | ✓ included | `--no-lfdocs` |
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

```bash
lf sp:brainstorm    # run skill from superpowers
```

If `~/.superpowers` exists, it's auto-detected with prefix `sp`. See [Configuration](config.md) for custom skill sources.

## See Also

[Built-in Tasks](builtins.md) · [Configuration](config.md) · [Patterns](patterns.md)
