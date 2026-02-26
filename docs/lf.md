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
lf npx:explain-code          # fetch from npx skills and run
lf sp:brainstorm             # run superpowers brainstorm skill
lf sr:gog                    # run SkillRegistry skill
lf : "fix the typo"          # inline prompt
lf debug -c                  # paste clipboard, fix the bug
```

## Steps

Steps are markdown files in these locations (searched in order):

1. External skills — `<prefix>:<skill>` format (e.g., `npx:explain-code`, `sr:gog`)
2. `.lf/steps/<step>.md` — repo steps
3. `.claude/commands/<step>.md` — Claude Code compatible
4. Built-in steps — run `lf --list` for the current built-in catalog (e.g., `debug`, `review`, `implement`)
5. `.agents/skills/<step>/SKILL.md` — user-installed agent skills (e.g., via `npx skills add`)

### Step Arguments

```bash
lf implement: add user authentication
```

Inside step files, `{args}` is replaced with whatever comes after the colon.

## Context Flags

### Files and Directories

| Flag | Description |
|------|-------------|
| `-a, --area PATH` | Area scope (paths to include in context) |
| `-w, --wave NAME` | Wave name for wave/ scoping |
| `--lfdocs / --no-lfdocs` | Include wave/, scratch/, and root `.md` files |
| `--diff-files / --no-diff-files` | Include files touched by branch (default: on) |
| `--diff / --no-diff` | Include raw `git diff` output |

### Clipboard

| Flag | Description |
|------|-------------|
| `-c, --clipboard` | Include clipboard content in prompt |

## Run Mode Flags

| Flag | Description |
|------|-------------|
| `-i, --interactive` | Run interactively (can interrupt, redirect) |
| `-b, --batch` | Run in batch/headless mode |

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
lf <flow>
lf ship -w feature-branch
```

| Flag | Description |
|------|-------------|
| `-a, --area PATH` | Area scope (paths to include in context) |
| `-w, --wave NAME` | Wave name for wave/ scoping |
| `-m, --model MODEL` | Model to use |
| `--web` | Copy to clipboard and open web client |

Flows are defined in `.lf/flows/`. See [Configuration](config.md).

## What's Included by Default

Every step automatically includes:

| Context | Default | How to disable |
|---------|---------|----------------|
| **lfdocs** (wave/, scratch/, root .md files) | ✓ included | `--no-lfdocs` or `lfdocs: false` in `.lf/config.yaml` |
| **Branch files** (files you've changed) | ✓ included | `--no-diff-files` |

## What's Opt-In

These require explicit flags or config:

| Context | How to enable |
|---------|---------------|
| **Raw diff** (line-by-line changes) | `--diff` |
| **Clipboard** | `-c` / `--clipboard` |
| **Area scope** | `--area PATH` |
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
lf implement: add caching -m codex
```

### Apply a direction

```bash
lf review -d ux
lf implement -d ux,clarity
```

### Include clipboard content

```bash
lf debug -c    # include current clipboard text in the prompt
```

### Use web client instead of CLI

```bash
lf review --web    # copies to clipboard, opens claude.ai (or chatgpt.com for codex)
lf : "fix the bug" --web -m codex    # opens chatgpt.com
```

### External skills

```bash
lf npx:explain-code # fetch + run from npx skills ecosystem
lf sp:brainstorm    # run skill from superpowers
lf sr:gog           # run SkillRegistry skill
```

`npx:` uses `.agents/skills/` as a cache. If `~/.superpowers` exists, it's auto-detected with prefix `sp`. SkillRegistry is opt-in via config. See [Configuration](config.md) for setup.

## See Also

[Get Started](getting-started.md) · [Configuration](config.md)
