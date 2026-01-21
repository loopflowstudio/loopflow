---
layout: default
title: lf Command Reference
---

# lf Command Reference

`lf` runs steps against AI coding agents. It assembles context (repo docs, diff, files) and passes everything to Claude, Codex, or Gemini.

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
lf debug -v                  # paste clipboard, fix the bug
```

## Task Files

Tasks are markdown files in these locations (searched in order):

1. External skills — `<prefix>:<skill>` format (e.g., `sp:brainstorm`, `sr:gog`)
2. `.claude/commands/<step>.md` — preferred, portable
3. `.lf/<step>.md` — local override
4. Built-in steps — commit, debug, design, expand, explore, implement, init, iterate, lint, polish, rebase, reduce, refine, review, roadmap

### Task Arguments

```bash
lf implement: add user authentication
```

Inside step files, `{args}` is replaced with whatever comes after the colon.

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

Every step automatically includes:

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
lf sr:gog           # run SkillRegistry skill
```

If `~/.superpowers` exists, it's auto-detected with prefix `sp`. SkillRegistry is opt-in via config. See [Configuration](config.md) for setup.

## See Also

[Feature Workflow](workflow.md) · [Configuration](config.md) · [Quick Fix](quick-fix.md)
