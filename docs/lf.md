---
layout: default
title: lf Command Reference
---

# lf Command Reference

`lf` is a prompt launcher. Every command launches a prompt—assembling context and passing it to Claude, Codex, Gemini, or OpenCode.

## Basic Usage

```bash
lf <step>                    # run a step file
lf <step>: args              # run with arguments
lf <prefix>:<skill>          # run external skill
lf : "inline prompt"         # no step file, just prompt
lf --list                    # show all available steps
```

## Examples

```bash
lf review                    # run .claude/commands/review.md
lf implement: add auth       # pass arguments after colon
lf sp:brainstorm             # run superpowers brainstorm skill
lf sr:gog                    # run SkillRegistry skill
lf : "fix the typo"          # inline prompt
lf debug -c                  # paste clipboard, fix the bug
```

## Steps

Steps are markdown files in these locations (searched in order):

1. External skills — `<prefix>:<skill>` format (e.g., `sp:brainstorm`, `sr:gog`)
2. `.lf/steps/<step>.md` — repo steps
3. `.claude/commands/<step>.md` — Claude Code compatible
4. Built-in steps — commit, debug, design, expand, explore, implement, init, iterate, lint, polish, rebase, reduce, refine, review

### Step Arguments

```bash
lf implement: add user authentication
```

Inside step files, `{args}` is replaced with whatever comes after the colon.

## Context Flags

### Files and Directories

| Flag | Description |
|------|-------------|
| `--area PATH` | Area scope (paths to include in context) |
| `-w, --worktree NAME` | Create worktree and run step there |
| `--lfdocs / --no-lfdocs` | Include wave/, scratch/, and root .md files (default: on) |
| `--diff-files / --no-diff-files` | Include files touched by branch (default: on) |
| `--diff / --no-diff` | Include raw `git diff` output |

### Clipboard

| Flag | Description |
|------|-------------|
| `-c, --clipboard` | Include clipboard content in prompt |

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
| `-m, --model MODEL` | Choose model (e.g., `claude:opus`, `codex`, `gemini`, `opencode`) |
| `-d, --direction DIRECTION` | Apply direction (comma-separated for multiple) |

## Output Flags

| Flag | Description |
|------|-------------|
| `--web` | Copy to clipboard and open web client (claude.ai, chatgpt.com, etc.) |

## Browser Automation

| Flag | Description |
|------|-------------|
| `--chrome / --no-chrome` | Enable Chrome browser automation |

## Running Flows

Run a named flow (chains of steps):

```bash
lf flow <name>
lf flow ship
lf flow ship -w feature-branch
```

| Flag | Description |
|------|-------------|
| `--area PATH` | Area scope (paths to include in context) |
| `-w, --worktree NAME` | Create worktree and run flow there |
| `-m, --model MODEL` | Model to use |
| `--pr` | Open PR when done |
| `--web` | Copy to clipboard and open web client |

Flows are defined in `.lf/flows/`. See [Configuration](config.md).

## What's Included by Default

Every step automatically includes:

| Context | Default | How to disable |
|---------|---------|----------------|
| **lfdocs** (wave/, scratch/, root .md files) | ✓ included | `--no-lfdocs` |
| **Branch files** (files you've changed) | ✓ included | `--no-diff-files` |

## What's Opt-In

These require explicit flags or config:

| Context | How to enable |
|---------|---------------|
| **Raw diff** (line-by-line changes) | `--diff` |
| **Clipboard** | `-c` / `--clipboard` |
| **Area scope** | `--area PATH` |
| **Summaries** | Configure in `.lf/config.yaml` |
| **Chrome automation** | `--chrome` |

See [Configuration](config.md) for setting defaults via config file.

## Examples

### Debug with clipboard

```bash
# Run tests, copy the error
lf debug -c
```

### Review with area scope

```bash
lf review --area src/api/
```

### Use a different model

```bash
lf implement -m codex: add caching
```

### Apply a direction

```bash
lf review -d designer
lf implement -d product-engineer,designer
```

### Copy prompt without running

```bash
lf review -c    # shows token breakdown, copies to clipboard
```

### Use web client instead of CLI

```bash
lf review --web    # copies to clipboard, opens claude.ai (or chatgpt.com for codex)
lf : "fix the bug" --web -m codex    # opens chatgpt.com
```

### External skills

```bash
lf sp:brainstorm    # run skill from superpowers
lf sr:gog           # run SkillRegistry skill
```

If `~/.superpowers` exists, it's auto-detected with prefix `sp`. SkillRegistry is opt-in via config. See [Configuration](config.md) for setup.

## See Also

[Get Started](getting-started.md) · [Configuration](config.md)
