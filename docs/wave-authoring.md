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
├── GOAL.md    # The wave's intent and loop prompt
└── MEMORY.md  # What the agent remembers between loops
```

`GOAL.md` and `MEMORY.md` are the two files a wave authors. The roadmap itself lives in Asana, not in the repo — read and edit it with `lf op pm` (see [Roadmap](#the-roadmap)).

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

Run `lf wave <wave>` to start that wave directly. Builtin goals resolve by name the same way, so the five VSM system charters ship as `s1`…`s5`:

```bash
lf wave s3           # the s3 (control) charter
```

### Memory

`MEMORY.md` is durable working context the wave agent writes as it goes — decisions, dead ends, what a downstream task should know. Workers inherit it as read-only context so they build with the wave's history in view; only the wave agent writes it.

### The Roadmap

The roadmap lives in Asana. There are no local roadmap files and nothing to sync — `lf op pm` reads and edits the wave's Asana project directly.

Connect a wave to Asana once. `lf op pm init` creates (or links) the project and writes its id into `GOAL.md` frontmatter:

```yaml
# wave/infra/GOAL.md frontmatter
pm:
  asana_project: 1207xxxxxxxxxxxx
```

Then read and edit the roadmap:

```bash
lf op pm init --wave infra                              # connect/create the Asana project
lf op pm show --wave infra                              # print the live roadmap
lf op pm update --wave infra --title "Daemon data integrity" --notes "..."   # add a task
lf op pm update --wave infra --id 1207... --status done # close a task
lf op pm status                                         # show linked waves
```

Keep each task to one PR's worth of work — roughly 1000 LOC. If a task feels like it needs splitting, it does. Give it a clear finish line so a worker knows when to stop.

### Goal Frontmatter

| Field | What it does |
|-------|-------------|
| `primary_flow` | Default flow a worker runs (`build`, `garden`, `sync`, …) |
| `workers` | Parallelism for dispatched work. `0` means "don't auto-dispatch" |
| `mode` | Primary execution pattern: `manual` or `loop` |
| `metrics` | Criteria the loop re-judges each iteration |
| `agent` | Preferred agent harness/model |
| `pm.asana_project` | Asana project id backing the wave's roadmap (written by `lf op pm init`) |

Crons and triggers are live lfd state — configure them through the HTTP or Python API; they are not read from `GOAL.md`.

---

## Drafting Wave Content

**Use `lf design` to explore and draft.** Start a local design conversation and let it produce wave files. `lf design` doesn't register or run waves — think of it as drafting sheet music, not conducting.

```bash
lf design: plan infrastructure hardening for the daemon
```

The session can produce a wave's `GOAL.md` and `MEMORY.md`. Once the files exist in your repo, Concerto and lfq pick them up; connect the roadmap with `lf op pm init` and add tasks with `lf op pm update`.

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

A `wave/billing/` directory for a billing rewrite. **`GOAL.md`** sets the intent — "replace the legacy billing system with a metered usage model" — and the metrics the loop re-judges each iteration:

- Usage events recorded within 5 seconds
- Invoices generate correctly for all plan types
- Legacy endpoints return the same responses during migration

The **Asana roadmap** holds the tasks, each scoped to one PR:

```
Usage events       → Event capture and storage
Metering API       → Public metering endpoint
Invoice generation → Monthly invoice calculation
Migration shim     → Legacy API compatibility layer
Cleanup            → Remove old billing code
```

The wave agent reads the roadmap with `lf op pm show`, picks the highest-priority task, dispatches a worker per task, and loops — folding each shipped PR into memory and closing the task with `lf op pm update --status done` — until the roadmap is empty.

---

## Reference

[Waves →](waves.md) · [Get Started →](getting-started.md) · [`lfd` commands](lfd.md) · [Configuration](config.md)
