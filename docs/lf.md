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

Write global flags before a built-in subcommand. Unambiguous flags also work
after it:

```bash
lf task run DES-123 --json                           # durable Task Session
lf task status DES-123 --json                        # same identity and worktree
lf pm --wave designer show                           # normalized onto `show`
lf pm task --wave designer create --title "Fix it"  # normalized onto `create`
lf commit -m "explain the change"                   # -m remains commit-local
```

Flags may cross nested subcommands to reach a selected command that owns the
spelling. If more than one level owns it, a flag already valid at its current
level stays there. Put `--` before literal arguments that look like flags.

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

## Running Waves and Tasks

```bash
lf serve designer                                  # start the named mind
lf stop designer                                   # stop its listener and resident
lf task start "fix the flaky chord-timeout test" --project <linear-project-id>
lf task run DES-123 --directive "fix the parser before the docs"
lf task steer DES-123 "rename the flag"
lf task interrupt DES-123 --message "take the smaller approach"
lf task receipt COMMAND_ID --until incorporated --timeout 30s --json
lf task acknowledge DES-123 --directive 2 --summary "the smaller parser path is active"
lf task decide DES-123 DECISION_ID approve
lf task wait DES-123
lf flow scan-pass "scan the runtime"               # one pass, no loop worktree
```

`lf serve <name>` starts the durable Wave mind and persistent playhead. A Task
Session starts only after its Linear task exists, owns one immutable worktree
and provider transcript, and remains resumable through review and merge.
`lf task attach` exposes a writable prompt that records structured commands;
terminal bytes never drive the provider directly.

## Speaking to Waves

Two wires, not one. The **thread** is the human surface: durable, replayed,
owned by a served mind. The **bus** is how agents call to each other: a table in
the shared store, ephemeral, no server in the path.

```bash
lf chat "ship the button audit first"       # post into the current wave's thread
lf chat -w infra "CI is red on the PR"      # target a wave by name
lf chat --parent "blocked on schema change" # escalate to the parent wave
lf chat --follow -w intelligence            # watch and speak from one terminal pane
lf memory                                   # print the wave's MEMORY.md
lf memory add "buttons: variants unified"   # publish one replayable fact
lf memory log                               # print facts added since the last update
lf memory update < MEMORY.md                # replace it from stdin
```

| Command | What it does |
|---------|--------------|
| `lf chat [TEXT]` | Post into a wave's thread; `--follow` replays and follows while typed lines post, `/status` reads health, and `/quit` leaves. Without `--follow`, omitted TEXT reads stdin. Outside any wave, one-shot chat prints a short drop note and exits 0 |
| `lf memory [show\|log\|update\|add]` | Read or curate a wave's memory — `log` prints the add stream since the last update; `update` replaces the compiled `MEMORY.md`; `add` publishes a replayable fact |

Both default to the invoking context's wave (`LFD_WAVE_ID` env, else the worktree name).

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Target a wave by name |
| `--parent` | Target the invoking wave's parent (`lf chat` / `lf memory`) |
| `--follow` | Replay and follow the selected thread while typed lines post into it (`lf chat`) |

## The Agent Bus

```bash
lf radio pub "landed PR #91, tests green"       # report on your own channel
lf radio pub -c infra.148e "rebase and retry"   # steer a specific hand
lf radio pub --parent "blocked on schema change" # escalate to the parent's channel
lf radio sub                                      # hear your channel and its hands
lf radio sub infra.148e --json                    # one hand's traffic as NDJSON
```

Channels are a dot tree: `infra` is the wave, `infra.148e` is one of its hands.
A subscription is a prefix, so `lf radio sub infra` hears the whole family.

| Command | What it does |
|---------|--------------|
| `lf radio pub [TEXT]` | Broadcast one frame on a channel. An INSERT into the shared store, so it works with no wave running; reads stdin when TEXT is omitted. No channel resolves, or no store on this machine — the broadcast drops with exit 0 |
| `lf radio sub [CHANNEL] [--json]` | Tune in to a channel and its descendants until killed. Never opens a socket — the served mind need not exist |

Broadcast, not delivery. `lf radio sub` tunes in at the head and hears only what is
said while it listens: nothing is replayed, and a frame published to a channel
nobody was on is gone. A frame survives one hour, then the sweeper takes it —
the bus is a wire, and `lf runs` plus the merged PR are the records of record. A
served mind is the one durable subscriber: it polls from a saved cursor, so it
catches its hands' reports across a restart, and when a frame aged out before it
woke, the miss is announced in its thread rather than passed over in silence.

| Flag | Description |
|------|-------------|
| `-c, --channel NAME` | Broadcast on any channel (`lf radio pub`) |
| `--parent` | Broadcast on the parent wave's channel (`lf radio pub`) |
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
