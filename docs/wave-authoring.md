---
layout: default
title: Wave Authoring
---

# Wave Authoring

A wave is a program of work that agents process autonomously. You define what to build, and the wave picks tasks, runs flows, opens PRs, and loops until the backlog is empty.

---

## Creating a Wave

In Concerto, create a wave from the dashboard — set its name, flow, area, and direction.

CLI equivalent:

```bash
lfq create mywave .
```

Python API:

```python
import loopflow.api as loopflow

loopflow.create_wave("mywave", repo=".", flow="build", direction=["clarity"], area=["src/"])
```

This creates the wave in lfd. To give it work, you write wave content on disk.

---

## The Wave Directory

Wave content lives in `wave/<name>/` at the root of your repo:

```
wave/infra/
├── README.md               # Vision, strategy, goals, risks
├── p0-fix-crash-loop.md   # Broken / unblock-now work
├── p1-daemon-integrity.md # Clear next step
├── p2-golden-tests.md     # Big "when not if" work
└── p3-security-research.md
```

The `wave/` directory is the source of truth for what to build. `lfd` reads it; `update-wave` writes it.

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
- What "done" looks like

## Risks
- What could go wrong
- How to mitigate

## Metrics
- How to measure success
```

The README is the wave's north star. Agents read it before every iteration to stay aligned.

### Writing Items

Items are bucketed markdown files. The prefix sets shared priority:

```
p0-fix-crash-loop.md    # broken or blocked; fix first
p1-daemon-integrity.md  # clear next step
p2-golden-tests.md      # committed later
p3-security-research.md # speculative
```

Bucket meanings:

- **`p0-*`** — the current codebase is broken and needs to be fixed before forward progress
- **`p1-*`** — the clear next step
- **`p2-*`** — a big idea that is "when, not if"
- **`p3-*`** — speculative work

`ingest` picks from the highest-priority non-empty bucket first. Within a bucket, exact ordering is intentionally loose.

Each item needs:

**A finish line.** One sentence, verifiable. Put it right after the title:

```markdown
# Daemon Data Integrity

**Finish line:** SQLite migrations are transactional, resource leaks
are bounded, and the webhook endpoint is safe by default.
```

**Scope.** What's in, what's out. The agent needs to know when to stop:

```markdown
## Scope

The lfd daemon has three data integrity issues:
- Transactional migrations (wrap 21 SQLite migrations in BEGIN/COMMIT)
- Resource accumulation (bound log files, file handles, lock maps)
- Webhook security (reject unauthenticated webhooks by default)
```

Keep items focused. One PR's worth of work — roughly 1000 LOC. If an item feels like it needs splitting, it probably does.

### The Config YAML

Optional. Mirrors the wave's fields in lfd:

```yaml
# wave/infra/infra.yaml
flow: ship-wave
mode: loop
area:
  - rust/loopflow/src/lfd/
  - rust/loopflow/src/lfd/store/
direction:
  - security
  - reliability
triggers:
  - signal: wave
    source_wave_id: backend
    flow: build
```

| Field | What it does |
|-------|-------------|
| `flow` | Which flow to run (`build`, `ship-wave`, `grind`, etc.) |
| `mode` | Execution pattern: `manual`, `loop`, or `cron` |
| `area` | Paths in scope for this wave |
| `direction` | Quality lenses applied to every step |
| `triggers` | Signal + flow pairs (repo, wave, ci_failure). Defaults don't need declaring |

If omitted, the wave uses whatever was set via `lfq create` or the Python API.

---

## Drafting Wave Content

Two paths to wave content:

**Use `lf design` to explore and draft.** Start a local design conversation, let it produce wave files. `lf design` doesn't register or run waves — think of it as drafting sheet music, not conducting.

```bash
lf design: plan infrastructure hardening for the daemon
```

The design session can produce a `wave/infra/README.md` and bucketed items. Once these files exist in your repo, Concerto and lfq pick them up.

**Write by hand.** Sometimes a text editor is faster. Create `wave/<name>/README.md`, add bucketed items, push. The structure is simple enough to write directly.

---

## The Auto-Loop

When a wave runs, it cycles through four phases:

```
ingest → kickoff → build → update-wave → [loop]
```

**ingest** picks the highest-priority item from `wave/<name>/` and moves it to `scratch/`.

**kickoff** elaborates the item into an actionable design — alternatives considered, research done, success and failure imagined.

**build** implements the design: implement → compress → lint → gate. Each sub-step commits. The result is a PR.

**update-wave** removes shipped items from `wave/<name>/` and folds context from completed work into remaining items. This is the only step that writes to the wave directory.

The loop terminates when: the backlog is empty, `max_iterations` is reached, or the wave is stopped.

### Fold, Don't Drop

When an item ships, `update-wave` doesn't just delete it. Context from the shipped work — what was learned, what changed, what downstream items should know — folds forward into the remaining items. Nothing is lost.

When the backlog empties, the wave is complete. The wave directory persists — the README stays as documentation of what was built.

---

## Running and Monitoring

### Concerto

Visual wave dashboard — create waves, watch progress, review PRs, stop and restart. The native experience on macOS.

### lfq (CLI)

Same `lfd` backend, terminal interface:

```bash
lfq run mywave              # start running
lfq list                    # show all waves
lfq logs mywave             # tail agent output
lfq stop mywave             # stop a wave
```

### Python API

```python
import loopflow.api as loopflow

loopflow.run_wave("mywave")
loopflow.waves()             # list all waves
```

### Modes and Triggers

Mode controls execution pattern. Triggers fire flows in response to signals.

| Mode | Behavior |
|------|----------|
| **manual** | Single run, then stop |
| **loop** | Continuously until stopped or backlog empty |
| **cron** | On schedule (`0 9 * * *`) |

| Signal | What changed | Default flow |
|--------|--------------|--------------|
| **repo** | Paths changed on main | `integrate` |
| **wave** | Another wave completed | `build` |
| **ci_failure** | CI failed on the wave's PR | `ci-fix` |

```python
import loopflow.api as loopflow

# Set mode at creation
loopflow.create_wave("mywave", repo=".", flow="build", mode="loop", area=["src/"])

# Add a trigger — react to another wave
loopflow.add_trigger("mywave", signal="wave", source_wave_id="infra")
```

[Modes and triggers →](waves.md)

---

## Worked Example

A `wave/billing/` directory for a billing rewrite:

**README.md** sets the vision:
> Replace the legacy billing system with a metered usage model.

**Goals** are concrete:
- Usage events are recorded within 5 seconds of occurrence
- Invoices generate correctly for all plan types
- Legacy API endpoints return the same responses during migration

**Items** are scoped to one PR each:

```
p1-usage-events.md       → Event capture and storage
p1-metering-api.md       → Public metering endpoint
p2-invoice-generation.md → Monthly invoice calculation
p2-migration-shim.md     → Legacy API compatibility layer
p3-cleanup.md            → Remove old billing code
```

The two `p1-*` items are both legitimate next steps. `ingest` picks from the highest-priority non-empty bucket first, then keeps looping until the backlog is empty.

---

## Reference

[Waves →](waves.md) · [Get Started →](getting-started.md) · [`lfd` commands](lfd.md) · [Configuration](config.md)
