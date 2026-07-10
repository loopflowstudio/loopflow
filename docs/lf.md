---
layout: default
title: lf Command Reference
---

# lf Command Reference

`lf` is a prompt launcher. Every command launches a prompt—assembling context and passing it to Claude, Codex, Gemini, or OpenCode.

## Basic Usage

```bash
lf <skill>                        # run a skill file
lf <skill>: args                  # run with arguments
lf <namespace>/<skill>            # run a namespaced skill (e.g. gstack/office-hours)
lf npx/<owner>/<repo>            # fetch any Claude Skill live via npx skills
lf : "inline prompt"             # no skill file, just prompt
lf --list                        # show all available skills
```

## Examples

```bash
lf gate                           # run the gate skill
lf implement: add auth            # pass arguments after colon
lf gstack/office-hours            # run a built-in gstack skill
lf office-hours                   # bare name works when unambiguous
lf npx/vercel-labs/deep-research  # fetch a skill from the npx skills catalog
lf : "fix the typo"               # inline prompt
lf debug -c                       # paste clipboard, fix the bug
lf --wave designer loop task "fix the flaky test"   # loop task until the PR merges
```

## Skills

Names resolve in this order:

1. `.lf/skills/<skill>.md` or `.lf/skills/<ns>/<skill>.md` — repo-local (also overrides builtins)
2. `.claude/commands/<skill>.md` — Claude Code compatible
3. `~/.lf/skills/<skill>.md`, `~/.lf/skills/<ns>/<skill>.md`, or `~/.claude/commands/<skill>.md` — user-global
4. Core built-in skills — `build/`, `govern/`, `ops/` (run `lf --list` for the full catalog)
5. Namespaced built-in skills — e.g. `gstack/<skill>`. Bare names (without `<ns>/`) resolve here only when exactly one namespace owns the name.
6. External skill namespaces — `npx/<owner>/<repo>` fetches live via `npx skills` and caches under `.agents/skills/`; cached or searchable skills can often be run as `npx/<name>`. The legacy `rams/rams` alias also resolves when `~/.claude/commands/rams.md` exists.

Namespaced skills and flows use `/`, not `:`. Run `gstack/office-hours`, not `gstack:office-hours`.

### Skill Arguments

```bash
lf implement: add user authentication
```

Inside skill files, `{args}` is replaced with whatever comes after the colon.

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

Run a named flow (chains of skills):

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

## Running Loops

```bash
lf serve designer                                  # start the named mind
lf --wave designer loop task "fix the flaky chord-timeout test"
lf --wave designer loop task "…" --max-passes 4 --wall-clock-secs 3600
lf --wave designer loop scan-pass "scan the runtime" --detach
lf flow scan-pass "scan the runtime"               # one pass, no loop worktree
```

With one name, `lf serve <name>` starts that mind and its persistent playhead.
With a flow plus free text, it creates a child worktree and loops until the
flow's skills write `done` to `scratch/loop.yaml` (`task`: when the PR merges).
Loops allow at least two passes; run the flow directly for one-shot work.
Without `--detach` the caller owns the loop and blocks; with `--detach` the
already-running wave server owns it and returns a read-only tmux inspection
session. Detach only when the parent has another useful move while the loop
runs. Linear holds filed tasks,
`lf runs` shows active hands, and merged PRs record done.

## Speaking to Waves

Two wires, not one. The **thread** is the human surface: durable, replayed,
owned by a served mind. The **bus** is how agents call to each other: a table in
the shared store, ephemeral, no server in the path.

```bash
lf chat "ship the button audit first"       # post into the current wave's thread
lf chat -w infra "CI is red on the PR"      # target a wave by name
lf chat --parent "blocked on schema change" # escalate to the parent wave
lf wavechat intelligence                    # watch and speak from one terminal pane
lf memory                                   # print the wave's MEMORY.md
lf memory add "buttons: variants unified"   # publish one replayable fact
lf memory log                               # print facts added since the last update
lf memory update < MEMORY.md                # replace it from stdin
```

| Command | What it does |
|---------|--------------|
| `lf chat [TEXT]` | Post a message into a wave's thread; reads stdin when TEXT is omitted. Outside any wave it prints a short drop note and exits 0, so the verb is safe in every prompt |
| `lf wavechat [WAVE]` | Replay and follow a served mind's thread while typed lines post into it; `/status` reads health and `/quit` leaves |
| `lf memory [show\|log\|update\|add]` | Read or curate a wave's memory — `log` prints the add stream since the last update; `update` replaces the compiled `MEMORY.md`; `add` publishes a replayable fact |

All three default to the invoking context's wave (`LFD_WAVE_ID` env, else the worktree name).

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Target a wave by name |
| `--parent` | Target the invoking wave's parent (`lf chat` / `lf memory`) |

## The Agent Bus

```bash
lf radio "landed PR #91, tests green"        # report on your own channel
lf radio -c infra.148e "rebase and retry"    # steer a specific hand
lf radio --parent "blocked on schema change" # escalate to the parent's channel
lf sub                                       # hear your channel and its hands
lf sub infra.148e --json                     # one hand's traffic as NDJSON
```

Channels are a dot tree: `infra` is the wave, `infra.148e` is one of its hands.
A subscription is a prefix, so `lf sub infra` hears the whole family.

| Command | What it does |
|---------|--------------|
| `lf radio [TEXT]` | Broadcast one frame on a channel. An INSERT into the shared store, so it works with no wave running; reads stdin when TEXT is omitted. No channel resolves, or no store on this machine — the broadcast drops with exit 0 |
| `lf sub [CHANNEL] [--json]` | Tune in to a channel and its descendants until killed. Never opens a socket — the served mind need not exist |

Broadcast, not delivery. `lf sub` tunes in at the head and hears only what is
said while it listens: nothing is replayed, and a frame published to a channel
nobody was on is gone. A frame survives one hour, then the sweeper takes it —
the bus is a wire, and `lf runs` plus the merged PR are the records of record. A
served mind is the one durable subscriber: it polls from a saved cursor, so it
catches its hands' reports across a restart, and when a frame aged out before it
woke, the miss is announced in its thread rather than passed over in silence.

| Flag | Description |
|------|-------------|
| `-c, --channel NAME` | Broadcast on any channel (`lf radio`) |
| `--parent` | Broadcast on the parent wave's channel (`lf radio`) |
| `--from LABEL` | Byline for machine speech (`--from ci`). Testimony, not proof: the row records it beside the channel the frame arrived on |

## Reading the Local Ledger

```bash
lf runs                         # one row per process, with trace/span ids
lf trace 66863649               # render the nested process tree
lf trace 66863649 --json        # feed the Telemetry dashboard
lf usage                        # additive spend by repo and provider
lf usage --json --days 30       # additive skill/run boundary rows
lf doctor                       # audit continuity, identity, lineage, coverage
lf doctor --json                # machine-readable audit
```

`run_id` identifies the whole trace. Each nested `lf` process gets its own
`process_id`, and terminal rows carry their own command, tokens, cost, provider,
and model. `lf trace` leaves killed processes open instead of hiding them.

## Measuring Codebase Weight

```bash
lf tokens                       # lines and model tokens by tracked path
lf tokens --days 365            # daily history, grouped by file extension
lf tokens --json                # token-weighted tree for other tools
```

`lf tokens` counts with the same tokenizer used by the context budget. It skips
untracked and non-UTF-8 files; a symlink counts its tracked link text instead of
duplicating its target; history walks git blobs without checking them out.

## What's Included by Default

Every skill automatically includes:

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

`LOOPFLOW.md` carries loopflow-specific guidance for inline execution and
mechanical git/PR operations. Tier skills add scoped delegation. Use
`--no-loopflow` for a leaner prompt.

### Include clipboard content

```bash
lf debug -c    # include current clipboard text in the prompt
```

### Launch an interactive vendor session

```bash
lf design                 # interactive skill → uses session.launch (default: tui)
lf gate --tui             # force a terminal handoff for a normally-headless skill
lf : "fix the bug" --ide -m codex   # force the Codex app instead
```

`--tui` and `--ide` override the repo default. Set `session.launch: ide` in
`.lf/config.yaml` to make the vendor app the default for interactive skills.

### External skills

```bash
lf npx/vercel-labs/deep-research   # fetch + run from the npx skills catalog
lf npx/explain-code                # already-cached skill (no network)
```

`npx/` uses `.agents/skills/` in the current repo as a cache. Use `npx/<owner>/<repo>` when you know the package name; cached or searchable skills can often be run as `npx/<name>`. On a cache miss, Loopflow runs `npx skills add` first, then falls back to `npx skills find` when it needs a package hint. The bundled `gstack/` namespace and core `build/` / `govern/` / `ops/` catalogs are always available, and the legacy `rams/rams` alias still works when `~/.claude/commands/rams.md` is installed.

## See Also

[Get Started](getting-started.md) · [Configuration](config.md)
