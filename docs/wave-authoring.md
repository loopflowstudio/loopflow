---
layout: default
title: Wave Authoring
---

# Wave Authoring

A wave is a named agent with a goal. You author its intent and memory; it works a roadmap, dispatches workers to build each item, watches their PRs, and loops.

---

## Creating a Wave

Author `wave/<name>/GOAL.md` — the body is the goal prompt; optional frontmatter sets machine config such as `workers:`, `crons:`, and `pm:`. Then run the agent:

```bash
lf wave infra             # start the wave agent
```

---

## The Wave Directory

Wave content lives in `wave/<name>/` at the root of your repo:

```
wave/infra/
├── GOAL.md    # The wave's intent and loop prompt
└── MEMORY.md  # What the agent remembers between loops
```

`GOAL.md` and `MEMORY.md` are the two files a wave authors. The roadmap itself lives in Linear, not in the repo — read and edit it with `lf op pm` (see [Roadmap](#the-roadmap)).

### The Goal

`GOAL.md` is the wave's loop surface. Frontmatter carries machine config; the body is the prompt the wave agent runs each loop.

```markdown
---
workers: 2
---

## Objective

Harden the daemon. Each loop: read the roadmap and memory, pick the next useful
move, dispatch a worker to build it, watch the PR, and fold what shipped into
memory.

## Measures

- **Quality**: daemon migrations are transactional.
- **Quality**: webhook security is enabled by default.
- **Done means**: a landed PR of real product code, roadmap item closed and PR-linked.

## Process

Use a direct worker for mechanical changes; write a scratch design first when the
blast radius crosses storage, auth, or public APIs.
```

Run `lf wave <wave>` to start that wave directly. Builtin goals resolve by name the same way, so the five VSM system charters ship as `s1`…`s5`:

```bash
lf wave s3           # the s3 (control) charter
```

### Memory

`MEMORY.md` is durable working context the wave agent writes as it goes — decisions, dead ends, what a downstream task should know. Workers inherit it as read-only context so they build with the wave's history in view; only the wave agent writes it.

### The Roadmap

The roadmap lives in Linear. There are no local roadmap files and nothing to sync — `lf op pm` reads and edits the wave's Linear project directly.

Connect a wave to Linear once. `lf op pm init` creates (or links) the project and writes its id into `GOAL.md` frontmatter:

```yaml
# wave/infra/GOAL.md frontmatter
pm:
  linear_project: 8c4ba3f9-cf23-4136-87ed-37847aa7dc82
```

Then read and edit the roadmap:

```bash
lf op pm init --wave infra                              # connect/create the Linear project
lf op pm show --wave infra                              # print the live roadmap
lf op pm update --wave infra --title "Daemon data integrity" --notes "..."   # add a task
lf op pm update --wave infra --id 1207... --status done # close a task
lf op pm status                                         # show linked waves
```

Keep each task to one PR's worth of work — roughly 1000 LOC. If a task feels like it needs splitting, it does. Give it a clear finish line so a worker knows when to stop.

### Goal Frontmatter

| Field | What it does |
|-------|-------------|
| `workers` | Parallelism for dispatched work. `0` means "don't auto-dispatch" |
| `agent` | Preferred agent harness/model |
| `crons` | Supplementary flow schedules (`flow:` + `schedule:`), fired by the wave's resident mind |
| `pm.linear_project` | Linear project id backing the wave's roadmap (written by `lf op pm init`) |

The resident mind reads `crons:` directly from this frontmatter and opens a system turn when a schedule comes due; edits land without a restart. See [Crons](waves.md#crons).

---

## Drafting Wave Content

**Use `lf design` to explore and draft.** Start a local design conversation and let it produce wave files. `lf design` doesn't register or run waves — think of it as drafting sheet music, not conducting.

```bash
lf design: plan infrastructure hardening for the daemon
```

The session can produce a wave's `GOAL.md` and `MEMORY.md`. Once the files exist in your repo, `lf wave <name>` runs them and Concerto picks them up; connect the roadmap with `lf op pm init` and add tasks with `lf op pm update`.

**Write by hand.** Sometimes an editor is faster. Create the files, push, done.

---

## The Loop

Run the wave agent and it works one move at a time:

1. **Read** — its `GOAL.md`, `MEMORY.md`, the roadmap, and any work already in flight.
2. **Decide** — pick the next useful move against the roadmap and metrics.
3. **Dispatch** — hand a scoped task to a worker, which runs a flow in its own worktree and opens a PR:

   ```bash
   lf build "wrap SQLite migrations in a transaction" --wave infra --dispatch
   ```

4. **Watch** — the PR is how the worker reports back. The agent reads its diff, checks, and comments.
5. **Remember** — the agent folds what shipped into `MEMORY.md` and updates the roadmap.

The wave agent coordinates; it rarely writes code itself. Substantial work becomes a worker session you can watch and steer; only atomic fixes are done inline.

### Fold, Don't Drop

When an item ships, its context — what was learned, what changed, what downstream items should know — folds forward into memory and the remaining roadmap. Nothing is lost. When the roadmap empties, the wave directory persists as a record of what was built.

---

## Running and Monitoring

```bash
lf wave mywave              # start the wave agent (Ctrl-C to stop)
tmux ls                     # the wave agent and every worker it launched
tmux attach -t <name>       # jump into one; agent output lives here
```

In **Concerto**, a wave's detail view groups its live work — the wave agent session, worker runs, PR state, and anything needing your attention.

### Crons

Crons live in `GOAL.md` frontmatter; the wave's resident mind fires each due schedule as a system turn and dispatches the flow with judgment. `workers: 0` is valid for a wave that only runs scheduled flows:

```markdown
<!-- wave/governance/GOAL.md -->
---
workers: 0
crons:
  - flow: govern-coordination
    schedule: "0 0 0 * * * *"
---
```

[Crons →](waves.md#crons)

---

## Worked Example

A `wave/billing/` directory for a billing rewrite. **`GOAL.md`** sets the intent — "replace the legacy billing system with a metered usage model" — and the metrics the loop re-judges each iteration:

- Usage events recorded within 5 seconds
- Invoices generate correctly for all plan types
- Legacy endpoints return the same responses during migration

The **Linear roadmap** holds the tasks, each scoped to one PR:

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
