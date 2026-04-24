---
layout: default
title: lf Command Reference
---

# lf Command Reference

`lf` is a prompt launcher. Every command launches a prompt—assembling context and passing it to Claude, Codex, Gemini, or OpenCode.

## Basic Usage

```bash
lf <step>                        # run a step file
lf <step>: args                  # run with arguments
lf <namespace>/<step>            # run a namespaced step (e.g. gstack/office-hours)
lf npx/<owner>/<repo>            # fetch any Claude Skill live via npx skills
lf : "inline prompt"             # no step file, just prompt
lf --list                        # show all available steps
```

## Examples

```bash
lf gate                           # run the gate step
lf implement: add auth            # pass arguments after colon
lf gstack/office-hours            # run a built-in gstack step
lf office-hours                   # bare name works when unambiguous
lf npx/vercel-labs/deep-research  # fetch a skill from the npx skills catalog
lf : "fix the typo"               # inline prompt
lf debug -c                       # paste clipboard, fix the bug
```

## Steps

Names resolve in this order:

1. `.lf/steps/<step>.md` or `.lf/steps/<ns>/<step>.md` — repo-local (also overrides builtins)
2. `.claude/commands/<step>.md` — Claude Code compatible
3. `~/.lf/steps/<step>.md` or `~/.lf/steps/<ns>/<step>.md` — user-global
4. Core built-in steps — `build/`, `govern/`, `ops/` (run `lf --list` for the full catalog)
5. Namespaced built-in steps — e.g. `gstack/<step>`. Bare names (without `<ns>/`) resolve here only when exactly one namespace owns the name.
6. `npx/<owner>/<repo>` — fetched live via `npx skills`, cached at `.agents/skills/`

The colon form `gstack:office-hours` is still accepted and normalized to `gstack/office-hours`; prefer the slash form in new code.

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
lf qa --area src/api/
```

### Use a different model

```bash
lf implement: add caching -m codex
```

### Apply a direction

```bash
lf gate -d ux
lf implement -d ux,clarity
```

### Include clipboard content

```bash
lf debug -c    # include current clipboard text in the prompt
```

### Use web client instead of CLI

```bash
lf gate --web      # copies to clipboard, opens claude.ai (or chatgpt.com for codex)
lf : "fix the bug" --web -m codex    # opens chatgpt.com
```

### External skills

```bash
lf npx/vercel-labs/deep-research   # fetch + run from the npx skills catalog
lf npx/explain-code                # already-cached skill (no network)
```

`npx/` uses `.agents/skills/` in the current repo as a cache. On a cache miss, `npx skills add <name>` runs to fetch the skill. For everything else, the bundled `gstack/` namespace and the core `build/` / `govern/` / `ops/` catalogs are always available — no setup needed.

## See Also

[Get Started](getting-started.md) · [Configuration](config.md)
