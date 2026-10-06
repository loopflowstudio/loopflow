# Authoring

The prompt library lives in your repo and is reviewed like code: skills
and flows under `.lf/`, goals under `wave/`. This page is how to
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
lf audit                               # run it
lf audit: focus on auth                # {args} in the file receives "focus on auth"
```

Write skills direct and imperative — state what to do, not what the skill
is. One skill, one job: `design` writes the spec, `implement` builds from
it, `gate` judges ship-readiness. Chain them rather than writing one skill
that does everything.

Direct launch from a TTY runs interactively. `-b` and automated
flow execution run the same skill headlessly. Define the inputs and useful
change independently of the caller. When judgment is unavailable, make supported
corrections and leave consequential choices explicit. Use a reviewer protocol
only when the caller supplies one.

Pass work between skills through its existing owner: code, a working plan,
or findings. Use `scratch/` for plans and evidence that later steps need;
update them in place rather than creating a report per pass. Test skills
standalone, midstream, and on repeated use.

Use `concept-review` to reconsider the product with someone present, on request
or in the ongoing Wave conversation. Realign reconciles the plan, code, and identified Wave memory.
Draft the affected usage docs and skill guidance first, then follow the simpler
interaction through types, APIs, and infrastructure. Product clarity is valuable
even without deleting code.
Keep proposed alternatives distinct from accepted requirements and verified
behavior. The work and updated plan supply evidence; `loop-or-next` owns navigation
when the Flow declares a decision step.

## Flows

A flow is a YAML list of steps — each step names a skill, an op, or another
flow — with commits between them:

```yaml
# .lf/flows/ship-api.yaml
- implement
- compress
- gate
```

Bare names and `flow: NAME` prefer an authored flow, then a skill. Adding a
same-named flow changes those references. Use `step: NAME` to select the skill
explicitly; this also lets a flow call its own same-named skill without a cycle.
An invalid flow reports its error instead of falling back to the skill.

Skills that need another Work's perspective launch it directly with
`lf --task <task> : "<prompt>"`. Headless skills missing required input
explain the failure in ordinary output and stop. The Wave operator
reads existing logs and discusses unresolved judgment in its own chat.

Keep design and review in the Task conversation. Operational Flows reject
`human: true` steps, including inside composed Flows and XOR alternatives, before
launching work. Save agreed feedback and remaining work in the conversation;
Session completion controls are retired. Inspect the exact
Flow and its effects before selecting further work.

Mechanical git/PR operations ride along as `cmd:` steps:

```yaml
- implement
- gate
- cmd: pr land
```

`cmd:` invokes a builtin command with its arguments. Named skills and flows
remain separate targets; `lf run NAME` selects only those definitions. Use
`cmd:` in authored YAML; the former `op:` spelling is no longer accepted.

### Working notes and feedback

```text
scratch/search-design.md
scratch/search-feedback.md
scratch/search-recovery-proof.md
```

Use topic-named Markdown notes for designs, research, demos and review findings.
Write for someone who did not attend the conversation: context and date,
observations, human feedback, agreed changes, unresolved questions, and the next
useful action with its proof. Link related notes. Update the relevant account
and mark superseded conclusions while preserving useful evidence. Notes remain
available across steps, regardless of which skill wrote them or runs next.

Discuss the review's note paths and takeaway in the ongoing conversation. A note
recommends work; the caller selects the next action after inspecting its evidence.
There is no required handoff filename or control file. Recursive scratch Markdown
is assembled into fresh context; a running agent can reread updated files.

### Branching (xor)

Branches route a flow on an agent's assessment of the current state. Exactly
one path runs:

```yaml
# .lf/flows/assess-change.yaml
- qa
- xor:
    router: triage
    paths:
      repair:
        flow: code
        description: "Reproduced defects within the change need repair"
      silence:
        description: "No actionable defect in the supplied change"
```

The `router:` skill reads the available evidence and returns `{"path":"NAME"}`
as its message asks. Routing instructions and path descriptions are appended
to its captured prompt. The choice belongs to the selected native turn and takes effect
when it succeeds; a retry must supply its own candidate. A path
with no `flow:`, `skill:`, or inline `steps:` (like `silence`) is a clean no-op
exit. With no `router:`, a generic routing agent picks from `scratch/` contents.

### Loopflows

A **loopflow** is a Flow with one or more backward edges. Use the same Flow
commands; Task binding adds context and Task authority. Task and ordinary Flow
execution interpret backward edges through the same transition rules.

```bash
lf pursue
lf task run DES-123 pursue
```

A loop is a backward edge in that Flow. `loop: <target>` runs a deciding step
last and returns to an earlier step; the steps between form its body:

```yaml
- implement
- compress
- flow: refresh
- loop: implement
- pr-publish
```

`loop` names a preceding step by its skill. When that skill runs more than once,
give one occurrence an `id` and loop to that. `step:` replaces the default
`loop-or-next` decider:

```yaml
- step: {name: implement, id: first}
- implement
- loop: first
  step: my-decider
```

One pass runs implement, compress, refresh (sync → realign), and loop-or-next.
The work and updated plan supply evidence; loop-or-next chooses Advance or Iterate through the
[decision protocol](lf-reference.md#flow-decisions-and-recovery). Iterate returns to the target
with direction; Advance leaves the loop and publishes. This is `pursue`: one
implementation loop with no human review or outer return edge.

At the deciding occurrence, return `{"decision":"advance","summary":"evidence","reason":null}`
or `{"decision":"iterate","summary":"next action and proof","reason":null}`, or
`{"decision":"blocked","summary":null,"reason":"question and evidence"}`. The contract
travels in the step's message, not as a provider schema. The Flow reads the final answer of the turn its
step captured; invalid output gets at most two corrective turns in the same
conversation, then the Flow fails. A failed turn cannot navigate.

Backward edges have no pass limit. Iterate follows the edge as long as the
decision calls for more work; human revision needs no budget reset. Pass counts
describe history. Missing decisions stop execution. Blocked is a stopped
decision: return it with a required reason in the final structured result.
Existing logs and outcomes provide the evidence. The Wave operator resolves
impediments or discusses missing judgment in its ongoing chat. A stopped Flow
is never retried or resumed; its caller launches fresh work explicitly.

Inspect a Flow's Execs and effects before launching further work.
Edits to the source apply to new Flows.
Finishing a Flow grants no implicit merge or Task-completion authority and
does not choose another Flow; author delivery explicitly.

`pursue` starts at implementation. Ordinary and Task Flows compile every XOR router and path
before execution and use the same cursor for nested paths and backward edges.
The implementation and recovery fixtures do not establish live provider/Session
handoff parity; that still requires a configured end-to-end demonstration.

## Workflows

A workflow is a Task's outer shape: stages where you take part in the Task
conversation, joined by edges that each run one Flow.

```yaml
# .lf/workflows/feature.yaml
stages:
  design: review-design      # the skill the conversation uses at this stage
  demo: demo
edges:
  - { from: start,  to: design, flow: task-design }
  - { from: design, to: design, flow: task-design }   # revise the design
  - { from: design, to: demo,   flow: pursue }
  - { from: demo,   to: demo,   flow: pursue }        # another pass
  - { from: demo,   to: end,    flow: ship }          # land
```

```bash
lf task run DES-123            # start: the only edge, task-design
lf task run DES-123 pursue     # at design: build it
lf task run DES-123 ship       # at demo: land it
lf task status DES-123         # stage or running edge
```

`start` and `end` are implicit. An edge runs anything `lf run` accepts, a
skill included. Only an edge into `end` may omit `flow:`; take it with
`lf task run ISSUE end`. A Task with no PR is a workflow with no landing edge,
like builtin `research`. Two edges leaving one stage cannot run the same Flow.

A Task takes up a workflow on its first `lf task run` when its Project's
default names one, or when you name one. It keeps the graph as it was then.
The Task is on an edge while that Flow runs, at the edge's target when it
succeeds, and back where it left when it stops or fails. Nothing else moves
it: there is no approve or complete command. Give feedback in the Task
conversation, which is told its stage's skill and the command for each edge
leaving it. `lf --task ISSUE run FLOW` runs a Flow without moving the Task.

Builtins are `feature` (design, demo, land), `code` (demo, land) and
`research` (findings, no PR). `lf flow list` shows workflows beside Flows with
their source; `lf flow customize NAME` writes a builtin to `.lf/workflows/` or
`.lf/flows/` and prints the path. `.lf/workflows/NAME.yaml` wins over a Flow of
the same name. An invalid file stays listed with its reason.

A Flow that repeats a step passes it `--steers-after STEER`, so the step
receives only Task direction newer than its last run. The option works on any
`lf` run.

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

## Process

Make mechanical changes directly; write a scratch design first when the
blast radius crosses storage, auth, or public APIs.
```

The two sections carry different weight. **Objective** is identity — what this
wave is for and how it moves. **Process** is constraint — when to design first,
what never to touch. Project definitions and KRs live in Linear. Official live
measurement lives in reviewed `wave/<wave>/metrics/*.md` contracts, not a
`GOAL.md` Measures section.

### Frontmatter

| Field | What it does |
|-------|-------------|
| `agent` | Preferred agent harness/model |
| `crons` | Flow schedules installed through `lf cron sync` |
| `pm.linear_initiative` | Linear Initiative id backing the wave (written by `lf repo connect`) |

The repository owns PM provider and Team authority in `.lf/config.yaml`:

```yaml
pm:
  provider: linear
  linear_team: "stable-team-uuid"
```

Do not copy provider or Team bindings into Wave frontmatter. Every Wave reuses
the repository Team and owns only its Initiative.

Execution placement is durable state: use `lf wave place <wave-id> <home-id>`.
Placement does not edit the goal, launch work or stop existing conversations.

### Writing KRs

Project KRs live in Linear, but writing them begins one level above the
measurement. State who the Project serves and what becomes easier, safer,
faster, clearer, or newly possible in their real work. For internal Projects,
the beneficiary may be an operator, maintainer, or agent; still name the
downstream experience instead of treating the mechanism as self-justifying.

Then choose evidence. A KR should read
as **proof under duration**: an observable end state demonstrated on real
work over a stated window, not a capability checkbox that passes once on a
demo.

- **Connected to the bet.** Be able to finish the sentence: “If this holds,
  the intended user improvement is credible because …” A convenient metric
  with no causal link is telemetry, not a KR.
- **Endurance over capability.** Not "the loop can fix a failing build" but
  "over one week, every dispatched loop lands or stops with an actionable
  record — zero silent stalls."
- **Counted.** Streaks, N/N trials: "four consecutive weekly releases with
  zero manual repair," "5/5 restarts lose nothing."
- **Unattended.** The window counts only if no one intervened
  inside it. A rescue resets the streak.
- **Falsifiable on real load.** Measured against the living workspace, never
  a fresh demo state.

```markdown
# Weak: capability checkboxes
- The agent can fix a failing build.
- Reports are visible in the app.

# Strong: user promise with proof under duration
# Project: Operators can dispatch work without babysitting the loop.
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
`realign` reconciles memory with the plan and code.

Before curating a parent, realign discovers immediate-child `MEMORY.md` files
with filesystem tools, reads relevant sections, and promotes shared lessons
while keeping local detail in the child. Unread coverage stays explicit.

## Adaptation

Curate durable lessons and decisions in the owning Wave's
`wave/<name>/MEMORY.md`. Identify the owner from the work's context and Wave
objectives. A repository with no Waves gets one named for the repository, with
a repo-wide GOAL grounded in its purpose and a MEMORY for durable lessons.
This local setup needs no PM binding or running Wave. If existing Waves leave
ownership unclear, ask the human and keep the question in scratch until resolved.

Put executable changes where they apply: task instructions in the relevant
skill, repo conventions in the agent guide, and configuration in
`.lf/config.yaml`. Do not create standalone learning or memory files in `.lf/`.
Commit these changes with the work so they stay reviewable.

## See Also

[Waves](waves.md) · [`lf` reference](lf.md) · [Configuration](config.md)
