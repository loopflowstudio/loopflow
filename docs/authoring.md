# Authoring

The prompt library lives in your repo and is reviewed like code: skills,
flows, and directions under `.lf/`, goals under `wave/`. This page is how to
write each one well. Where they resolve and what ships builtin is reference —
see [`lf` → Skills](lf.md#skills).

## Skills

A skill is a markdown file that tells the coding agent what to do:

```markdown
# .lf/skills/audit.md

Audit auth changes on this branch.
Check for:
- Missing validation
- Confusing errors
- Gaps in tests

Fix any issues you find.
```

```bash
lf audit                      # run it
lf audit: focus on auth       # {args} in the file receives "focus on auth"
```

Write skills direct and imperative — state what to do, not what the skill
is. One skill, one job: `design` writes the spec, `implement` builds from
it, `gate` judges ship-readiness. Chain them rather than writing one skill
that does everything.

Frontmatter can make a directly invoked skill interactive; the body stays prose:

```markdown
---
interactive: true
---
Explore the problem with the human before writing anything.
```

Skills chain through `scratch/`: a step writes `scratch/<branch>.md`, the
next step reads it. That contract is what makes flows work — if your skill
produces something a later step needs, write it to `scratch/`, not to chat.

## Flows

A flow is a YAML list of steps — each step names a skill, an op, or another
flow — with commits between them:

```yaml
# .lf/flows/ship-api.yaml
- implement
- compress
- gate
```

Skills that need judgment run `lf ask "<question>"`. The command records the
exchange under the current Turn, waits for its routed Answer, then returns that
Answer to the same skill process. Flow YAML needs no interaction flag.

Mechanical git/PR operations ride along as `op:` steps:

```yaml
- implement
- gate
- op: pr land --create-pr
```

### Branching (xor)

Branches route a flow on an agent's assessment of the current state. Exactly
one path runs:

```yaml
# flow: garden
- scan
- assess
- xor:
    router: assess
    paths:
      act:
        flow: garden-act
        description: "Adjustments needed — mutate waves, then review"
      silence:
        description: "Everything is healthy"
```

The `router:` skill reads `scratch/` and chooses a path; routing
instructions are appended to its prompt automatically, so the skill author
focuses on *what to think about*, not how to express the choice. A path
with no `flow:` or `skill:` (like `silence`) is a clean no-op exit. With no
`router:`, a generic routing agent picks from `scratch/` contents.

Keep flows bounded. A flow is one pass — repetition belongs to Wave,
Project, and Task runtimes, not to loops inside a flow.

## Directions

A direction shapes judgment — what "good" means for this run:

```markdown
# .lf/directions/ux.md

Optimize for user experience quality: visibility, feedback, consistency.

## Success

A design doc in scratch/ that another engineer could implement from.
```

```bash
lf gate -d ux
lf gate -d ux,clarity     # directions compose; stack intents
```

Write a direction as values plus a success condition, not a task list. A
`ux` direction sets user-facing intent; a `clarity` direction adds
code-model rigor; stacking gets both. Builtin groups: `infra`, `ux`,
`craft`, `creativity`, `ceo`.

## Goals

`GOAL.md` is a wave's loop surface: frontmatter carries machine config, the
body is the prompt the wave runs each wake.

```markdown
<!-- wave/infra/GOAL.md -->
---
crons:
  - flow: sync
    schedule: "0 0 0 1 * * *"
---

## Objective

Keep the runtime architecture legible. Each loop: read Linear tasks and
memory, pick the next useful move, resolve its local blocker, spin off
independent work only when parallelism earns it, and fold what shipped into
memory.

## Measures

- **Quality**: fresh-store tests cover every live persistence path.
- **Done means**: a landed PR of real product code, Linear task closed and PR-linked.

## Process

Make mechanical changes directly; write a scratch design first when the
blast radius crosses storage, auth, or public APIs.
```

The three sections carry different weight. **Objective** is identity — what
this wave is for and how it moves. **Measures** is judgment — what the wave
re-reads each wake to decide if the bet holds. **Process** is constraint —
when to design first, what never to touch.

### Frontmatter

| Field | What it does |
|-------|-------------|
| `owner` | Optional OS user allowed to start the Wave automatically |
| `home` | Optional HomeId, hostname, or IP allowed to start the Wave automatically |
| `agent` | Preferred agent harness/model |
| `crons` | Supplementary flow schedules, fired by the wave's resident loop |
| `pm.linear_initiative` | Linear Initiative id backing the wave (written by `lf pm init`) |
| `pm.linear_team` | Linear team id owning the Wave's Project and Task prefixes |

`owner` and `home` say where automatic startup is wanted. Both are optional and
independent. They are policy, not authorization or observed runtime state.
Execution placement remains durable state: use
`lf work place wave <wave-id> <home-id>`. Bare `lf start` and `lfd` require both
the authored policy and recorded placement to match; named `lf start <wave>` is
an explicit local override.

### Writing KRs

Project KRs live in Linear, but writing them is goal craft. A KR should read
as **proof under duration**: an observable end state demonstrated on real
work over a stated window, not a capability checkbox that passes once on a
demo.

- **Endurance over capability.** Not "the loop can fix a failing build" but
  "over one week, every dispatched loop lands or stops with an actionable
  record — zero silent stalls."
- **Counted.** Streaks, N/N trials: "four consecutive weekly releases with
  zero manual repair," "5/5 restarts lose nothing."
- **Unattended.** The window counts only if no human repaired anything
  inside it. A rescue resets the streak.
- **Falsifiable on real load.** Measured against the living workspace, never
  a fresh demo state.

```markdown
# Weak: capability checkboxes
- The agent can fix a failing build.
- Reports are visible in the app.

# Strong: proof under duration
- Over one week of real work, every dispatched loop lands its PR unattended
  or stops with an actionable record — zero silent stalls, zero rescues.
- Four consecutive weekly releases complete with no manual repair.
```

Avoid backlog bullets, implementation receipts, and issue ids in KRs; put
concrete work in tasks.

### Drafting

Use `lf design` to explore and draft — it can produce a wave's `GOAL.md` and
`MEMORY.md` without registering or running anything. Or write the files by
hand.

```bash
lf design: plan infrastructure hardening for the runtime
```

Seed `MEMORY.md` with the load-bearing context a first run needs. After that,
agents edit the same reviewed file through the ordinary repository workflow;
`update-wave` owns deliberate end-of-work curation.

## Adaptation

When an agent learns something repo-specific, the durable home for that
learning is `.lf/`: adapt the skill, add a direction, or set config — and
commit it with the work so the change stays reviewable. Prompts that live in
the repo improve the way code does: by diff.

## See Also

[Waves](waves.md) · [`lf` reference](lf.md) · [Configuration](config.md)
