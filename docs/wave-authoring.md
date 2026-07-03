---
layout: default
title: Wave Authoring
---

# Wave Authoring

A wave is a named agent with a goal. You author its intent and memory; it works a roadmap, dispatches workers to build each item, watches their PRs, and loops.

---

## Creating a Wave

Author `wave/<name>/GOAL.md`, then run the agent:

```bash
lfq wave run infra        # start (or attach to) the wave agent
```

In Concerto, create a wave from the dashboard — set its name and flow. Or from Python:

```python
import loopflow.api as loopflow

loopflow.create_wave("infra", repo=".", flow="build")
loopflow.run_wave("infra")
```

---

## The Wave Directory

Wave content lives in `wave/<name>/` at the root of your repo:

```
wave/infra/
├── GOAL.md                # The wave's intent and loop prompt
├── MEMORY.md              # What the agent remembers between loops
├── README.md              # Vision, strategy, goals, risks
├── 1-fix-crash-loop.md    # Roadmap — urgent
├── 2-daemon-integrity.md  # Roadmap — high
└── 3-golden-tests.md      # Roadmap — medium
```

`GOAL.md` and `MEMORY.md` are the two canonical files. `README.md` and the numbered items are the wave's roadmap — synced to a provider (Asana, Linear, Notion) when PM is connected.

### The Goal

`GOAL.md` is the wave's loop surface. Frontmatter carries durable intent; the body is the prompt the wave agent runs each loop.

```markdown
---
primary_flow: build
mode: loop
workers: 2
metrics:
  - daemon migrations are transactional
  - webhook security is enabled by default
---

Harden the daemon. Each loop: read the roadmap and memory, pick the next useful
move, dispatch a worker to build it, watch the PR, and fold what shipped into
memory.
```

### Memory

`MEMORY.md` is durable working context the wave agent writes as it goes — decisions, dead ends, what a downstream task should know. Workers inherit it as read-only context so they build with the wave's history in view; only the wave agent writes it.

### The README

Every wave README follows the same structure:

```markdown
# Infrastructure Hardening

## Vision
What this wave achieves. One paragraph.

## Strategy
How to get there. Sequence, dependencies, approach.

## Goals
- Concrete, verifiable outcomes

## Risks
- What could go wrong, and how to mitigate

## Metrics
- How to measure success
```

### Roadmap Items

Items are priority-prefixed markdown files — the wave's work queue:

```
1-fix-crash-loop.md    # Urgent — broken or blocked; fix first
2-daemon-integrity.md  # High — the clear next step
3-golden-tests.md      # Medium — "when, not if"
4-security-research.md # Low — speculative
```

Each item needs a **finish line** — one verifiable sentence, right after the title — and a **scope** that says what's in and what's out, so a worker knows when to stop:

```markdown
# Daemon Data Integrity

**Finish line:** SQLite migrations are transactional, resource leaks are
bounded, and the webhook endpoint is safe by default.

## Scope
- Transactional migrations (wrap 21 SQLite migrations in BEGIN/COMMIT)
- Resource accumulation (bound log files, file handles, lock maps)
- Webhook security (reject unauthenticated webhooks by default)
```

Keep items to one PR's worth of work — roughly 1000 LOC. If an item feels like it needs splitting, it does.

For PM-backed waves, `lf op ingest` refreshes the local mirror from the provider before the wave picks. Use `lf op ingest --item <filename-or-slug>` to target a specific item.

### Goal Frontmatter

| Field | What it does |
|-------|-------------|
| `primary_flow` | Default flow a worker runs (`build`, `garden`, `sync`, …) |
| `workers` | Parallelism for dispatched work. `0` means "don't auto-dispatch" |
| `mode` | Primary execution pattern: `manual` or `loop` |
| `metrics` | Criteria the loop re-judges each iteration |
| `agent` | Preferred agent harness/model |

Crons and triggers are live lfd state — configure them through the HTTP or Python API; they are not read from `GOAL.md`.

---

## Drafting Wave Content

**Use `lf design` to explore and draft.** Start a local design conversation and let it produce wave files. `lf design` doesn't register or run waves — think of it as drafting sheet music, not conducting.

```bash
lf design: plan infrastructure hardening for the daemon
```

The session can produce a `wave/infra/README.md` and roadmap items. Once the files exist in your repo, Concerto and lfq pick them up.

**Write by hand.** Sometimes an editor is faster. Create the files, push, done.

---

## The Loop

Run the wave agent and it works one move at a time:

1. **Read** — its `GOAL.md`, `MEMORY.md`, the roadmap, and any work already in flight.
2. **Decide** — pick the next useful move against the roadmap and metrics.
3. **Dispatch** — hand a scoped task to a worker, which runs a flow in its own worktree and opens a PR:

   ```bash
   lfq worker run infra --flow build --task "wrap SQLite migrations in a transaction"
   ```

4. **Watch** — the PR is how the worker reports back. The agent reads its diff, checks, and comments.
5. **Remember** — the agent folds what shipped into `MEMORY.md` and updates the roadmap.

The wave agent coordinates; it rarely writes code itself. Substantial work becomes a worker session you can watch and steer; only atomic fixes are done inline.

### Fold, Don't Drop

When an item ships, its context — what was learned, what changed, what downstream items should know — folds forward into memory and the remaining roadmap. Nothing is lost. When the roadmap empties, the wave directory persists as a record of what was built.

---

## Running and Monitoring

```bash
lfq wave run mywave         # start or attach to the wave agent
lfq sessions                # the wave agent and every worker it launched
lfq attach <session-id>     # jump into one over tmux
lfq list                    # all waves
lfq logs mywave             # tail agent output
lfq stop mywave             # stop a wave
```

In **Concerto**, a wave's detail view groups its live work — the wave agent session, worker runs, PR state, and anything needing your attention.

### Modes, Crons, and Triggers

Mode controls the primary execution pattern. Crons schedule supplementary flows. Triggers fire flows in response to signals.

| Mode | Behavior |
|------|----------|
| **manual** | Single run, then stop |
| **loop** | Continuously until stopped or the roadmap empties |

`workers: 0` in `GOAL.md` is valid for a wave that only runs scheduled flows. Configure crons and triggers through the API:

```python
import loopflow.api as loopflow

loopflow.create_wave("governance", repo=".", flow="garden", workers=0,
                     crons=[{"flow": "govern-coordination", "schedule": "0 0 * * *"}])

# React to another wave completing
loopflow.add_trigger("mywave", signal="wave", source_wave_id="infra")
```

| Signal | What changed | Default flow |
|--------|--------------|--------------|
| **repo** | Paths changed on main | `integrate` |
| **wave** | Another wave completed | `build` |
| **ci_failure** | CI failed on the wave's PR | `ci-fix` |

[Modes and triggers →](waves.md)

---

## Worked Example

A `wave/billing/` directory for a billing rewrite. **README.md** sets the vision — "replace the legacy billing system with a metered usage model" — and lists concrete goals:

- Usage events recorded within 5 seconds
- Invoices generate correctly for all plan types
- Legacy endpoints return the same responses during migration

**Roadmap items** are scoped to one PR each:

```
2-usage-events.md       → Event capture and storage
2-metering-api.md       → Public metering endpoint
3-invoice-generation.md → Monthly invoice calculation
3-migration-shim.md     → Legacy API compatibility layer
4-cleanup.md            → Remove old billing code
```

The wave agent picks from the highest-priority level, dispatches a worker per item, and loops — folding each shipped PR into memory — until the roadmap is empty.

---

## Reference

[Waves →](waves.md) · [Get Started →](getting-started.md) · [`lfd` commands](lfd.md) · [Configuration](config.md)
