# Authoring

Read this when the steps behind the Mac app's work need to change. Edit
[skills](glossary.md#loopflows-words) to change the AI's instructions,
[flows](glossary.md#loopflows-words) to change their order, and Wave goals to
change what the work is for. These are files in the repository, reviewed with
the code: skills and flows under `.lf/`, goals under `wave/`.

Start with one skill. Add a Flow when several steps need to run together.
[`lf` → Skills](lf.md#skills) lists where Loopflow looks for files: a repository
skill overrides a personal or built-in skill with the same name, so the
repository can carry its own way of working.

## Skills

A skill is a [Markdown](glossary.md#borrowed-from-software-engineering) text
file that tells the coding agent what to do. This example audits
[authentication](glossary.md#borrowed-from-software-engineering), the code
that checks who is signing in, on the current Git branch:

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

A direct launch from a terminal (a [TTY](glossary.md#borrowed-from-software-engineering))
is interactive: someone can answer while it runs. `--batch` and automated Flow
steps are headless: nobody is there to type a reply. State how the skill should
finish in either mode, especially when it needs judgment or conversation:

```markdown
## Reviewer mode

- **Interactive reviewer:** explore the problem in the current conversation.
- **Parent reviewer:** answer the assigned question from supplied evidence and
  return without waiting for a person.
```

Skills pass working notes through [scratch](glossary.md#loopflows-words):
a step writes `scratch/<branch>.md`, using the branch name in the filename; the
next step reads it. That contract is what makes flows work — if your skill
produces something a later step needs, write it to `scratch/`, not to chat.

Use `concept-review` to reconsider the product during a Task with a person, or after
`review-slice` for an autonomous assessment. Draft the affected usage docs and
skill guidance first, then follow the simpler interaction through data types,
[APIs](glossary.md#borrowed-from-software-engineering) (interfaces programs call),
and infrastructure.
Keep proposed alternatives distinct from accepted requirements and verified
behavior. The review supplies evidence; `loop-decide` owns navigation when the
Flow declares a decision step.

## Flows

A [commit](glossary.md#borrowed-from-software-engineering) saves changes in
Git history. A Flow is a [YAML](glossary.md#borrowed-from-software-engineering) list. Each
step names a skill, a mechanical operation (`op`), or another Flow. Commits
between steps save checkpoints, so later work has a recorded starting point:

```yaml
# .lf/flows/ship-api.yaml
- implement
- compress
- gate
```

[Work](glossary.md#loopflows-words) is a saved planning record, such as a Task
or Wave. Replace `<work>` with one such as `wave:infra`. Skills that need
another Work's perspective launch a separate [Run](glossary.md#loopflows-words) with
`lf --as <work> : "<prompt>"`. Skills that genuinely need a decision from the user use
`lf ask "<request>"`; the Run blocks while a durable session works in the
same checkout, then resumes when the user completes that conversation.

In the Mac app, an interactive review appears in Sessions. A
[Session](glossary.md#loopflows-words) is a conversation that can be reopened.
To require one at a particular step, set `human: true` and give the step an
`id` unique within the expanded Flow (including any nested Flows):

```yaml
- kickoff
- step:
    id: review_design
    name: review-design
    human: true
```

The reviewer and agent clarify the design together. The agent saves feedback
with `lf session ready "feedback and remaining work"`. The reviewer chooses
**Complete** in the app, or uses `lf session complete <session-id>` with the
identifier from `lf session list`. The Flow carries that
feedback to its next step. Provider exit or readiness alone leaves it waiting.
Human steps have no navigation verdict or backward edge. Put a deciding step
after the review when its feedback should choose between continuing and more work.

Declare delivery explicitly. An `op:` step runs a mechanical command;
`pr land` below requests [auto-merge](glossary.md#borrowed-from-software-engineering)
and waits for the pull request to merge after its checks pass:

```yaml
- implement
- gate
- op: pr land
```

### Working notes and feedback

```text
scratch/search-design.md
scratch/search-feedback.md
scratch/search-recovery-proof.md
```

Use topic-named Markdown notes for designs, research, demos and review findings.
Write for someone who did not attend the conversation: context and date,
observations, attributed reviewer feedback, agreed changes, unresolved questions, and the next
useful action with its proof. Link related notes. Update the relevant account
and mark superseded conclusions while preserving useful evidence. Notes remain
available across steps, regardless of which skill wrote them or runs next.

A review's ready summary points to that material:

```sh
lf session ready "See scratch/search-feedback.md: implement the agreed empty state; verify recovery after clearing the query"
```

The `loop-decide` skill reads those notes alongside the current design.
Notes recommend what to do; the deciding step records whether the Flow should
continue or repeat. No special handoff filename is required. Markdown in
`scratch/`, including subfolders, becomes [context](glossary.md#loopflows-words)
for each fresh Run—the information the AI receives with its instructions.
An already-running agent can reread files changed after it started.

### Branching (xor)

Use [XOR branching](glossary.md#loopflows-words) when the next steps depend on
what an agent finds. XOR means exactly one path runs. A router is the skill
that chooses the path; these Flow branches are separate from Git branches:

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

The `router:` skill reads the available evidence and records one choice with
`lf flow route PATH`. Routing instructions and path descriptions are appended
to its captured prompt. The choice belongs to the active Run and takes effect
when it succeeds; failed Runs discard their candidates. A path
with no `flow:`, `skill:`, or inline `steps:` (like `silence`) does no further
work on that path. With no `router:`, a generic routing agent picks from
`scratch/` contents.

### Loopflows

A [loopflow](glossary.md#loopflows-words) is a Flow with one or more backward
edges: declared routes back to earlier steps. Use the same Flow
commands; Task binding adds context and Task authority. Task and ordinary Flow
execution interpret backward edges through the same transition rules.

```bash
lf feature
lf task run DES-123 --flow feature
```

A loop is a backward edge in that Flow. It returns from a deciding step to an
earlier step; the intervening steps form its body. Give the destination and
deciding step stable ids:

```yaml
- step:
    id: implement
    name: implement
- compress
- review-slice
- step:
    id: review_concepts
    name: concept-review
- step:
    id: decide
    name: loop-decide
    repeat:
      from: implement
- step:
    id: review_delivery
    name: demo
    human: true
- step:
    id: decide_delivery
    name: loop-decide
    repeat:
      from: implement
```

One pass runs implement, compress, review-slice, concept-review, and loop-decide.
The reviews supply evidence; loop-decide chooses Advance or Iterate through the
[decision protocol](lf.md#flow-decisions-and-recovery). Iterate returns to `from`
with direction; Advance reaches the human demo. Complete returns the demo's
feedback and revised design to the second loop-decide. Its own explicit edge
also targets implement: the outer loop repeats implementation, both reviews,
the inner decision loop, and demo. Review completion itself chooses no edge.

When the deciding step runs, use `lf flow decide advance "evidence"` or
`lf flow decide iterate "next action and proof"`. The current decision Run owns
that choice; its candidate takes effect only after the Run succeeds. A review's
final prose or a successful process exit cannot substitute for the decision.

Backward edges have no pass limit. Iterate follows the edge as long as the
decision calls for more work; a requested revision needs no budget reset. Pass counts
describe history. Missing decisions stop execution. Blocked is a stopped
execution outcome: report it with
`lf flow blocked "reason, attempted direction, evidence, and question"`.
An [invocation](glossary.md#loopflows-words) is one execution of a Flow;
an occurrence identifies a step within it, and a pass counts visits through
a loop. Loopflow records one Ask for that invocation, occurrence, and pass. Retries
join that Ask or recover its saved completion. Its Session runs `unblock`, using
concept-review with the reviewer by default. Completion returns evidence to
loop-decide for reassessment without choosing a navigation decision. If the blocker
remains unresolved, report it; do not open identical Asks automatically.

Resume the saved invocation to preserve its captured definition, position,
direction, and accepted decisions. Edits to the source apply to new invocations.
Finishing a Flow grants no implicit merge or Task-completion authority and
does not choose another Flow; author delivery explicitly.

`feature` combines design review with this loop; `pursue` starts at
implementation. Ordinary and Task invocations capture every XOR router and path
before execution and track the saved position through nested paths and backward edges.
Recovery reads that captured definition, including paths not yet selected.
Local tests cover saved definitions and recovery. They do not prove that a
live coding-tool conversation can move through every review and decision;
that needs a demonstration with the configured app and coding tool.

## Goals

Use a [Wave](glossary.md#loopflows-words) for an objective that outlasts one
Task. Its `GOAL.md` body supplies the instructions for each turn.
[Frontmatter](glossary.md#borrowed-from-software-engineering), the YAML block
at the top, supplies settings. A [cron](glossary.md#borrowed-from-software-engineering)
entry schedules a Flow; the seven fields specify seconds, minutes, hours,
day of month, month, day of week, and year.

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
change affects storage, account access, or interfaces other programs use.
```

**Objective** names who benefits and what improves. **Process** says how to
work: when to design first and what to leave alone. The current chapter's
plan lives in an internal Linear Project. It holds
[KRs](glossary.md#loopflows-words), the outcomes and evidence to judge, and
metric targets. Repeated measurements are defined in
`wave/<wave>/metrics/*.md`; keeping their definitions with the Wave preserves
measurement history across chapters. See [Live metrics](waves.md#live-metrics).

### Frontmatter

| Field | What it does |
|-------|-------------|
| `owner` | Optional OS user allowed to start the Wave automatically |
| `home` | Optional HomeId, hostname, or IP allowed to start the Wave automatically |
| `agent` | Preferred agent harness/model |
| `crons` | Supplementary flow schedules, fired by the wave's resident loop |
| `pm.linear_initiative` | Linear Initiative id backing the wave (written by `lf pm init`) |

PM means planning management. [Linear](glossary.md#borrowed-from-software-engineering)
organizes a Wave as an Initiative, its chapters as Projects, and Tasks as
issues. A Team owns those issues. The repository's `.lf/config.yaml` selects
the planning provider and Team:

```yaml
pm:
  provider: linear
  linear_team: "stable-team-uuid"
```

Do not copy provider or Team bindings into Wave frontmatter. Every Wave reuses
the repository Team and owns only its Initiative.

`owner` and `home` say where automatic startup is wanted. Both are optional and
independent. They are policy, not authorization or observed runtime state.
Execution placement remains durable state: use
`lf work place wave <wave-id> <home-id>`. Bare `lf start` and `lfd` require both
the authored policy and recorded placement to match; named `lf start <wave>` is
an explicit local override.
Whether this machine may pursue Work is Home-local registry state. Change it
with `lf work enable|disable <wave|project|task> <id>`; these commands never edit
the goal or another repository file.

### Writing KRs

Chapter KRs live in Linear. Begin with the Wave objective: who benefits and
what becomes easier, safer, faster, clearer, or newly possible in their work.
A chapter supplies evidence toward that objective, rather than a second
objective for its internal Project.

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
# Wave objective: Operators can dispatch work without babysitting the loop.
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

Seed `MEMORY.md` with the decisions and background a first Run needs. After that,
agents edit the same reviewed file through the ordinary repository workflow;
`update-wave` owns deliberate end-of-work curation.

## Adaptation

Curate durable lessons and decisions in the owning Wave's
`wave/<name>/MEMORY.md`. Identify the owner from the work's context and Wave
objectives. A repository with no Waves gets one named for the repository, with
a repo-wide GOAL grounded in its purpose and a MEMORY for durable lessons.
This local setup needs no PM binding or running Wave. If existing Waves leave
ownership unclear, ask the requester and keep the question in scratch until resolved.

Put executable changes where they apply: task instructions in the relevant
skill, repo conventions in the agent guide, and configuration in
`.lf/config.yaml`. Do not create standalone learning or memory files in `.lf/`.
Commit these changes with the work so they stay reviewable.

## See Also

[Waves](waves.md) · [`lf` reference](lf.md) · [Configuration](config.md)
