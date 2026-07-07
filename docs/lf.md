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
lf task 1207... --wave designer   # run one Linear task until its PR merges
```

## Steps

Names resolve in this order:

1. `.lf/steps/<step>.md` or `.lf/steps/<ns>/<step>.md` — repo-local (also overrides builtins)
2. `.claude/commands/<step>.md` — Claude Code compatible
3. `~/.lf/steps/<step>.md`, `~/.lf/steps/<ns>/<step>.md`, or `~/.claude/commands/<step>.md` — user-global
4. Core built-in steps — `build/`, `govern/`, `ops/` (run `lf --list` for the full catalog)
5. Namespaced built-in steps — e.g. `gstack/<step>`. Bare names (without `<ns>/`) resolve here only when exactly one namespace owns the name.
6. External skill namespaces — `npx/<owner>/<repo>` fetches live via `npx skills` and caches under `.agents/skills/`; cached or searchable skills can often be run as `npx/<name>`. The legacy `rams/rams` alias also resolves when `~/.claude/commands/rams.md` exists.

Namespaced steps and flows use `/`, not `:`. Run `gstack/office-hours`, not `gstack:office-hours`.

### Step Arguments

```bash
lf implement: add user authentication
```

Inside step files, `{args}` is replaced with whatever comes after the colon.

## Context Flags

### Files and Directories

| Flag | Description |
|------|-------------|
| `--docs PATH[,PATH...]` | Prefetch docs into context—files, globs, or dirs (default: none) |
| `-w, --wave NAME` | Wave name for wave/ scoping |
| `--diff-files / --no-diff-files` | Include files touched by branch (default: off) |
| `--diff / --no-diff` | Include raw `git diff` output |

### Loopflow Guidance

| Flag | Description |
|------|-------------|
| `--no-loopflow` | Omit `LOOPFLOW.md` operating guidance |

### Clipboard

| Flag | Description |
|------|-------------|
| `-c, --clipboard` | Include clipboard content in prompt |

## Run Mode Flags

| Flag | Description |
|------|-------------|
| `-i, --interactive` | Run interactively (can interrupt, redirect) |
| `-b, --batch` | Run in batch/headless mode |
| `--max-turns N` | Cap agent turns for this invocation |

## Model Flags

| Flag | Description |
|------|-------------|
| `-m, --model MODEL` | Choose model (e.g., `claude:opus`, `codex`, `gemini`, `opencode`) |
| `-d, --direction DIRECTION` | Apply direction (comma-separated for multiple) |

## Output Flags

| Flag | Description |
|------|-------------|
| `--tui` / `--ide` | Hand off to an interactive vendor session (terminal or vendor app); overrides `session.launch` |

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
| `--docs PATH[,PATH...]` | Prefetch docs into context—files, globs, or dirs (default: none) |
| `-w, --wave NAME` | Wave name for wave/ scoping |
| `-m, --model MODEL` | Model to use |
| `--tui` / `--ide` | Hand off to an interactive vendor session (terminal or vendor app); overrides `session.launch` |

Flows are defined in `.lf/flows/`. See [Configuration](config.md).

## Running Roadmap Tasks

```bash
lf task <linear-item-id> --wave designer
lf task <linear-item-id> --wave designer --max-passes 4 --wall-clock-secs 3600
```

`lf task` resolves the Linear item from the wave roadmap, creates a worker
worktree, runs bounded task passes, waits for the PR to merge, then links and
closes the Linear task.

## Speaking to Waves

```bash
lf chat "ship the button audit first"       # post into the current wave's thread
lf chat -w infra "CI is red on the PR"      # target a wave by name
lf chat --parent "blocked on schema change" # escalate to the parent wave
lf sub                                      # follow the wave's live event stream
lf sub infra --json                         # raw frames as NDJSON
lf memory                                   # print the wave's MEMORY.md
lf memory add "buttons: variants unified"   # publish one replayable fact
lf memory log                               # print facts added since the last update
lf memory update < MEMORY.md                # replace it from stdin
```

| Command | What it does |
|---------|--------------|
| `lf chat [TEXT]` | Post a message into a wave's thread; reads stdin when TEXT is omitted. Outside any wave the publish drops silently (exit 0), so the verb is safe in every prompt |
| `lf sub [WAVE] [--json]` | Follow a wave's live events (turns, mind state, memory) until killed; exits 0 with a note when no wave resolves |
| `lf memory [show\|log\|update\|add]` | Read or curate a wave's memory — `log` prints the add stream since the last update; `update` replaces the compiled `MEMORY.md`; `add` publishes a replayable fact |

All three default to the invoking context's wave (`LFD_WAVE_ID` env, else the worktree name).

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Target a wave by name |
| `--parent` | Target the invoking wave's parent (`lf chat` / `lf memory`) |

## What's Included by Default

Every step automatically includes:

| Context | Default | How to disable |
|---------|---------|----------------|
| **Agent doc** (AGENTS.md / CLAUDE.md / STYLE.md) | ✓ included | — |
| **Loopflow operating guidance** | ✓ included | `--no-loopflow` |
| **scratch/** | ✓ included | — |
| **wave/** | ✓ included | — |

## What's Opt-In

These require explicit flags or config:

| Context | How to enable |
|---------|---------------|
| **Docs** (files, globs, directories) | `--docs README.md,docs/` or `docs:` config |
| **Raw diff** (line-by-line changes) | `--diff` |
| **Branch files** (full changed file bodies) | `--diff-files` |
| **Clipboard** | `-c` / `--clipboard` |
| **Chrome automation** | `--chrome` |

See [Configuration](config.md) for setting defaults via config file.

## Examples

### Debug with clipboard

```bash
# Run tests, copy the error
lf debug -c
```

### Prefetch docs into context

```bash
lf qa --docs src/api/
```

Gathers `*.md` under `src/api/` into context before the prompt runs. Unlike
the old area scope, `--docs` only prefetches—it doesn't restrict which
files the agent touches.

### Use a different model

```bash
lf implement: add caching -m codex
```

### Apply a direction

```bash
lf gate -d ux
lf implement -d ux,clarity
```

### Disable loopflow operating guidance

```bash
lf gate --no-loopflow
```

`LOOPFLOW.md` carries loopflow-specific guidance for git, worktrees, PRs, and
delegation, and is injected by default. Use `--no-loopflow` for a leaner
prompt.

### Include clipboard content

```bash
lf debug -c    # include current clipboard text in the prompt
```

### Launch an interactive vendor session

```bash
lf design                 # interactive step → uses session.launch (default: tui)
lf gate --tui             # force a terminal handoff for a normally-headless step
lf : "fix the bug" --ide -m codex   # force the Codex app instead
```

`--tui` and `--ide` override the repo default. Set `session.launch: ide` in
`.lf/config.yaml` to make the vendor app the default for interactive steps.

### External skills

```bash
lf npx/vercel-labs/deep-research   # fetch + run from the npx skills catalog
lf npx/explain-code                # already-cached skill (no network)
```

`npx/` uses `.agents/skills/` in the current repo as a cache. Use `npx/<owner>/<repo>` when you know the package name; cached or searchable skills can often be run as `npx/<name>`. On a cache miss, Loopflow runs `npx skills add` first, then falls back to `npx skills find` when it needs a package hint. The bundled `gstack/` namespace and core `build/` / `govern/` / `ops/` catalogs are always available, and the legacy `rams/rams` alias still works when `~/.claude/commands/rams.md` is installed.

## See Also

[Get Started](getting-started.md) · [Configuration](config.md)
