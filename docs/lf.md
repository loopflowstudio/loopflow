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
lf : "inline prompt"         # no task file, just prompt
```

## Examples

```bash
lf review                    # run .claude/commands/review.md
lf implement: add auth       # pass arguments after colon
lf : "fix the typo"          # inline prompt
lf debug -v                  # paste clipboard, fix the bug
```

## Task Files

Tasks are markdown files in these locations (searched in order):

1. `.claude/commands/<task>.md` — preferred, portable
2. `.lf/<task>.md` — local override
3. Built-in tasks — debug, design, implement, polish, review

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

## Context Assembly

Every task automatically includes:

1. **Repo docs** — all `.md` files at repository root (README, STYLE, etc.)
2. **Branch files** — full content of files touched by the current branch
3. **Config context** — files listed in `.lf/config.yaml` context section
4. **Extra files** — files passed with `-x` flag
5. **Clipboard** — if `-v` flag is set
6. **Summaries** — pre-generated LLM summaries (if configured)

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

## See Also

- [Built-in tasks](builtins.md) — debug, design, implement, polish, review
- [Configuration](config.md) — `.lf/config.yaml` options
- [Patterns](patterns.md) — workflows and recipes
