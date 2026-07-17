# Waves

A wave is a named agent with a goal. You author its intent and let it
coordinate; it remembers what it learns, works the next blocker, spins off
durable Task Sessions when parallelism earns it, and stays steerable the whole
time.

Two files author a wave, both living in your repo and reviewed like code:

| File | Holds |
|------|-------|
| **`wave/<name>/GOAL.md`** | The wave's intent and loop prompt — what it's for, how it judges progress |
| **`wave/<name>/MEMORY.md`** | What the wave remembers between loops — written by the wave agent |

Waves live in **Loopflow** (macOS): open the repository, select the wave, and
start it beside its conversation and work map. The same controls exist from
the CLI:

```bash
lf home start shipper                  # idempotently start the Wave on its Home
lf chat --steer "invoices first"       # steer the live body, else queue
lf status shipper                      # its Project → Task hierarchy
lf stop shipper                        # stop listener and resident gracefully
```

(`lf wave shipper` runs the resident process itself, foreground until Ctrl-C —
that is what the app and `lf home start` launch for you; run it directly when
developing a goal.)

## The planning model

Three nouns, kept distinct by kind rather than size:

| Noun | What it means | What it owns |
|------|---------------|--------------|
| **Wave** | Durable operating context | Memory, cadence, chat, and project selection |
| **Project** | Measured bet inside exactly one wave | Definition, KRs, and closure criteria |
| **Task** | Concrete work that advances a project | One implementation step, investigation, doc, or shipped change |

Keep the hierarchy shallow. Every project belongs to one wave; projects don't
contain projects and don't own memory or cadence. If a project seems to need
subprojects, split it into siblings, promote the durable context into a wave,
or demote the pieces into tasks.

```text
Product wave
  Loopflow API project
    Linear tasks
  Wave Chat project
    Linear tasks
```

## The Goal

`GOAL.md` is the loop surface: frontmatter carries machine config, the body is
the prompt the wave runs each loop.

```markdown
## Objective

Keep the runtime architecture legible. Each loop: read Linear tasks and
memory, pick the next useful move, resolve its local blocker, spin off
independent work only when parallelism earns it, and fold what shipped into
memory.

## Measures

- **Quality**: fresh-store tests cover every live persistence path.
- **Done means**: a landed PR of real product code, Linear task closed and PR-linked.

## Process

Make mechanical changes directly; write a scratch design first when the blast
radius crosses storage, auth, or public APIs.
```

Builtin goals resolve by name, so the five Viable System Model charters ship
as `s1`…`s5`:

```bash
lf wave s3            # the s3 (control) charter
```

Writing a goal well — the weight of each section, frontmatter fields, KR
craft — is covered in [Authoring → Goals](authoring.md#goals).

### Memory

`MEMORY.md` is durable working context the wave agent writes as it goes —
decisions, dead ends, what a downstream task should know. Workers inherit it
read-only; only the wave agent writes it, through the server:

```bash
lf memory show -w shipper
lf memory add "buttons: variants unified" --receipt chat_turn:turn-3
```

Never edit the file directly; it is server-owned. When a task ships, its
context folds forward into memory and the remaining Linear tasks — fold, don't
drop.

### Home

A Wave's **Home** is where its work executes — an owner plus a location:

```yaml
home: jack@local              # canonical local (the default)
home: ssh://jack@mini.local   # remote over SSH
```

Project and Task launches inherit the Home; repo/PR/release/PM commands in a
remote-home Wave forward there over `lf ssh`, carrying credentials for the
life of the process only. Probe and start with:

```bash
lf home probe <wave>   # reachable? stopped? running? — with the next action
lf home start <wave>   # idempotently start the Wave on its Home
```

See [Architecture → Homes](architecture.md#homes-and-remote-execution).

## Projects and KRs

A project is a measured bet inside a wave. Its definition and KRs live in
Linear Project content — not in a repo file, a status table, or its own memory.

```bash
lf pm project create --wave infra --title "Technical Architecture" \
  --definition "Loopflow's architecture is legible from the top down." \
  --kr "Top-down architecture documentation is complete and published."
```

KRs should read as **proof under duration**: observable end states
demonstrated on real work over a stated window, not capability checkboxes
that pass once on a demo. [Authoring → Writing KRs](authoring.md#writing-krs)
carries the craft and examples.

## Linear

Tasks live in Linear; there are no local task lists. A wave maps to an
Initiative, each project to a Linear Project, each task to an Issue. Connect
once — `lf pm init` links or creates the Initiative and wave-owned team and
writes both ids into `GOAL.md` frontmatter. Don't paste ids by hand.

```bash
lf pm init --wave infra --team-key INF     # connect or rebind
lf pm sync --wave infra                    # refresh the local SQLite snapshot
lf pm show --wave infra --no-sync          # deterministic cache-only read
lf pm task create --wave infra --project stability --title "Daemon data integrity"
lf pm task done --id 1207... --pr "https://github.com/acme/app/pull/42"
```

## Tasks

Every concrete file-writing change begins with a Linear task and runs as a
durable Task Session in its own stable sibling worktree:

```bash
lf task start "add retry to token refresh" --project <linear-project-id>
lf task run INF-123
lf task run INF-124 --stack-on INF-123     # dependent work before the parent merges
```

A Task Session advances through zero or more serial PRs to `main`. It runs
kickoff once, repeats its selected inner flow, then gates the proposed
outcome; gate repairs return the same Session to another iteration. After a
merge or abandonment, Loopflow rotates the worktree onto the next branch. The
Task inherits the wave's `GOAL.md` and `MEMORY.md` plus its Project definition
and KRs.

The wave stays steerable while several independent tasks run — task events
enter its inbox as typed observations and wake it once. Steering, receipts,
decisions, resume, and recovery are the same verbs agents use:
[The Agent API → Steer](agent-api.md#steer).

```bash
lf pr land --next parser-proof   # merge this PR, then rotate to the next
lf pr land -c                    # merge this PR, then complete the Task
lf task complete INF-124 --summary "investigation recorded"   # no PR needed
```

Keep each PR reviewable — roughly 1000 LOC. A Task may need several serial
PRs, but it still needs one concrete finish line.

## Crons

Crons schedule supplementary flows on a wave. They live in `GOAL.md`
frontmatter and are read by the resident loop: when a schedule comes due while
the loop is idle, it opens a system pass and dispatches the flow with
judgment. Edits land without a restart.

```markdown
<!-- wave/shipper/GOAL.md -->
---
crons:
  - flow: sync
    schedule: "0 0 0 1 * * *"
---
```

Schedules use 6/7-field cron syntax (seconds first). A schedule that comes due
mid-turn fires at the next turn boundary; occurrences older than 24 hours are
missed, not replayed.

## Drafting wave content

Draft with `lf design` or write the files by hand — see
[Authoring → Drafting](authoring.md#drafting). Once `wave/<name>/` exists,
the Mac app picks it up, and `lf home start <name>` starts it from the CLI.
To remove a wave, stop it, then delete `wave/<name>/`.

## Worked example

A `wave/billing/` directory for a billing rewrite. `GOAL.md` sets the intent —
"replace the legacy billing system with a metered usage model" — and the
measures the loop re-judges each iteration: usage events recorded within 5
seconds, invoices correct for all plan types, legacy endpoints unchanged
during migration.

The backing Linear project holds the concrete Tasks:

```text
Usage events       → Event capture and storage
Metering API       → Public metering endpoint
Invoice generation → Monthly invoice calculation
Migration shim     → Legacy API compatibility layer
Cleanup            → Remove old billing code
```

The wave reads Projects and Tasks with `lf pm show --no-sync`, directs the
highest-priority Project, and starts a Task Session for every independent
file-writing change. Each shipped PR folds into memory and closes its task.

## Next

[The Agent API →](agent-api.md) · [Conducting →](conducting.md) · [Get Started →](getting-started.md)

## Reference

[Configuration](config.md) · [Troubleshooting](troubleshooting.md)
