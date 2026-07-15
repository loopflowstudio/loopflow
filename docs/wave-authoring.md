---
layout: default
title: Wave Authoring
---

# Wave Authoring

A wave is a named agent with a goal. You author its intent, memory, and project
bets; it works the next blocker inline, spins off independent loops when they
earn a separate lifecycle, and remembers what ships.

---

## The Planning Model

Use three nouns:

| Noun | What it means | What it owns |
|------|---------------|--------------|
| **Wave** | Durable operating context | Memory, cadence, budget, chat, and project selection |
| **Project** | Measured bet inside exactly one wave | Definition, KRs, and closure criteria |
| **Task** | Concrete work that advances a project | One implementation step, investigation, doc, or shipped change |

Keep the hierarchy shallow. Every project belongs to one wave. Projects do not
contain projects, and projects do not own their own memory or cadence. If a
project seems to need subprojects, split it into sibling projects, promote the
durable operating context into a wave, or demote the pieces into tasks.

Good projects are either completable behavioral improvements or standing quality
frontiers. Individual cleanup work is a task; a recurring debt frontier can be a
project.

```text
Product wave
  Loopflow API project
    Linear tasks
  Wave Chat project
    Linear tasks

Infrastructure wave
  Technical Architecture project
    Linear tasks
  Release Stability project
    Linear tasks
```

---

## Creating a Wave

Author `wave/<name>/GOAL.md` — the body is the goal prompt; optional frontmatter sets machine config such as `crons:` and `pm:`. Then run the agent:

```bash
lf wave infra              # start the Wave
```

---

## The Wave Directory

Wave content lives in `wave/<name>/` at the root of your repo:

```
wave/infra/
├── GOAL.md               # The wave's intent and loop prompt
└── MEMORY.md             # What the agent remembers between loops
```

`GOAL.md` and `MEMORY.md` are the authored wave surface. Projects, KRs, and
tasks live in Linear and sync into SQLite — read and edit them with `lf pm`
(see [Linear Tasks](#linear-tasks)).

### The Goal

`GOAL.md` is the wave's loop surface. Frontmatter carries machine config; the body is the prompt the wave agent runs each loop.

```markdown
## Objective

Keep the runtime architecture legible. Each loop: read Linear tasks and memory, pick the next useful
move, resolve its local blocker, spin off independent work only when parallelism
earns it, and fold what shipped into memory.

## Measures

- **Quality**: fresh-store tests cover every live persistence path.
- **Quality**: each public command maps to one product concept.
- **Done means**: a landed PR of real product code, Linear task closed and PR-linked.

## Process

Make mechanical changes directly; write a scratch design first when the blast
radius crosses storage, auth, or public APIs.
```

Run `lf wave <wave>` to start that Wave directly. Builtin goals resolve by name the same way, so the five VSM system charters ship as `s1`…`s5`:

```bash
lf wave s3            # the s3 (control) charter
```

### Memory

`MEMORY.md` is durable working context the wave agent writes as it goes — decisions, dead ends, what a downstream task should know. Workers inherit it as read-only context so they build with the wave's history in view; only the wave agent writes it.

### Projects

A project is a measured bet inside a wave. Store its definition and KRs in
Linear Project content. It does not own a task list, status table, independent
memory, or a repo file.

```bash
lf pm project create --wave infra --title "Technical Architecture" \
  --definition "Loopflow's architecture is legible from the top down." \
  --kr "Top-down architecture documentation is complete and published." \
  --kr "Every public API is ratified as minimally simple for its purpose."
```

KRs should read as proof under duration: observable end states that show the
bet holds, demonstrated on real work over a stated window — not capability
checkboxes that pass once on a demo. The strongest KRs share four properties:

- **Endurance over capability.** Not "the loop can fix a failing build" but
  "over one week, every dispatched loop lands or stops with an actionable
  record — zero silent stalls."
- **Counted.** Streaks, N/N trials, consecutive cycles: "four consecutive
  weekly releases with zero manual repair," "5/5 restarts lose nothing."
- **Unattended.** The window counts only if no human repaired anything
  inside it. A rescue resets the streak.
- **Falsifiable on real load.** Measured against the living workspace as
  history accumulates, never a fresh demo state; a miss produces a visible
  failure event, not a shrug.

Avoid backlog bullets, implementation receipts, status, and issue ids. Put
concrete work in tasks.

```markdown
# Weak: capability checkboxes
## KRs
- The agent can fix a failing build.
- Reports are visible in the app.
- Memory is saved.

# Strong: proof under duration
## KRs
- Over one week of real work, every dispatched loop lands its PR unattended
  or stops with an actionable record — zero silent stalls, zero rescues.
- The thread survives every restart it meets in a week of daily use, 5/5,
  with zero learnings lost.
- Four consecutive weekly releases complete with no manual repair.
```

### Linear Tasks

Tasks live in Linear. There are no local task lists. A wave maps to a Linear
Initiative, each project maps to a Linear Project, and each task maps to an
Issue. `lf pm sync` refreshes a machine-local SQLite snapshot; no Project files
are generated in the repository.

Connect a wave to Linear once. `lf pm init` links or creates the Initiative,
then writes its id into `GOAL.md` frontmatter:

```yaml
# wave/infra/GOAL.md frontmatter
pm:
  provider: linear
  linear_initiative: 8c4ba3f9-cf23-4136-87ed-37847aa7dc82
```

Do not look up or paste this id by hand. With no pinned id, `lf pm init` links
one exact Initiative-title match or creates it when absent, then persists the
id. Duplicate titles fail loudly.

Then read and edit tasks:

```bash
lf pm init --wave infra                                             # connect the Linear Initiative
lf pm sync --wave infra                                             # refresh SQLite
lf pm status                                                        # show linked waves and task counts
lf pm show --wave infra                                             # read; refresh when stale
lf pm show --wave infra --no-sync                                   # deterministic cache-only read
lf pm show --wave infra --project stability                         # filter to one Project
lf pm task create --wave infra --project stability --title "Daemon data integrity"
lf pm task done --id 1207... --pr "https://github.com/acme/app/pull/42"
```

Keep each PR reviewable — roughly 1000 LOC. A Task may need several serial PRs,
but it still needs one concrete finish line. Split independent outcomes into
separate Tasks and name the Project each advances.

### Goal Frontmatter

| Field | What it does |
|-------|-------------|
| `agent` | Preferred agent harness/model |
| `crons` | Supplementary flow schedules (`flow:` + `schedule:`), fired by the wave's resident loop |
| `pm.linear_initiative` | Linear Initiative id backing the wave (written by `lf pm init`) |
| `home` | The Wave's execution Home — a user-owned address (default: the current user's local machine) |

The resident loop reads `crons:` directly from this frontmatter and opens a system pass when a schedule comes due; edits land without a restart. See [Crons](waves.md#crons).

### Home

A Wave's **Home** is where its work executes — an *owner* plus a *location*, not a
host alias or a local/remote flag. Author it in `GOAL.md` frontmatter:

```yaml
home: jack@local              # canonical local (the default when omitted)
home: ssh://jack@mini.local   # remote over SSH
home: ssh://jack@10.0.0.5:22  # IPv4 + explicit port
home: ssh://jack@[2001:db8::1] # bracketed IPv6
home: jack@mini.local         # shorthand → ssh://jack@mini.local
```

The owner is required and is distinct from credentials — those keep riding SSH
and Doppler. A public IP, a private/Tailscale DNS name, and a public host all use
the same `ssh://` location; whether it answers is *operational evidence*, not part
of the address. Project and Task launches inherit the Home, and repo/PR/release/PM
commands run in a remote-home Wave forward there over `lf ssh`.

Resolve and act on a Wave's Home:

```bash
lf home probe <wave>   # reachable? stopped? running? — with the reason and next action
lf home start <wave>   # idempotently start (or attach to) the Wave on its Home
lf status <wave>       # includes the Home address and probed runtime evidence
```

---

## Drafting Wave Content

**Use `lf design` to explore and draft.** Start a local design conversation and let it produce wave files. `lf design` doesn't register or run waves — think of it as drafting sheet music, not conducting.

```bash
lf design: plan infrastructure hardening for the runtime
```

The session can produce a wave's `GOAL.md` and `MEMORY.md`. Once the files exist in your repo, `lf wave <name>` runs them and Loopflow picks them up; connect Linear with `lf pm init` and add tasks with `lf pm task create`.

**Write by hand.** Sometimes an editor is faster. Create the files, push, done.

---

## The Loop

Run the wave agent and it works one move at a time:

1. **Read** — its `GOAL.md`, `MEMORY.md`, Linear tasks, and any work already in flight.
2. **Decide** — pick the next useful move against the task list and metrics.
3. **Execute** — create or select the concrete Linear task, then start its Task
   Session:

   ```bash
   lf task run INF-123
   ```

4. **Watch** — use Task status and linked events; steer or interrupt the same
   session when review changes course.
5. **Remember** — the agent folds what shipped into `MEMORY.md` and updates Linear tasks.

The Wave coordinates. A durable Project Session pursues a Project's KRs and
sleeps while its Task Sessions run. Concrete changes ship through Task
Sessions; Projects never own worktrees, branches, or PRs.

### Fold, Don't Drop

When a task ships, its context — what was learned, what changed, what downstream tasks should know — folds forward into memory and the remaining Linear tasks. Nothing is lost. When the open task list empties, the wave directory persists as a record of what was built.

---

## Running and Monitoring

```bash
lf wave mywave               # start the Wave (Ctrl-C to stop)
lf project attach <id>      # audited Project control prompt
lf task attach INF-123      # audited Task control prompt
```

In **Loopflow**, a wave's detail view renders its native Project → Task work
map, including current direction, next-move ownership, Task PR history, and
anything needing attention.

### Crons

Crons live in `GOAL.md` frontmatter; the Wave's resident opens each due
schedule as a system turn:

```markdown
<!-- wave/governance/GOAL.md -->
---
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

The backing **Linear project** holds concrete Tasks. Most finish in one PR;
larger coherent outcomes can advance through several serial PRs:

```
Usage events       → Event capture and storage
Metering API       → Public metering endpoint
Invoice generation → Monthly invoice calculation
Migration shim     → Legacy API compatibility layer
Cleanup            → Remove old billing code
```

The wave agent reads Projects and Tasks with `lf pm show --no-sync`, then
directs the highest-priority Project. Every independent file-writing change
starts a Task Session under that Project. Each shipped PR folds into memory and
the Task closes with `lf pm task done`.

---

## Reference

[Waves →](waves.md) · [Get Started →](getting-started.md) · [Configuration](config.md)
