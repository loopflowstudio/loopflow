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
lf audit                      # run it
lf audit: focus on auth       # {args} in the file receives "focus on auth"
```

Write skills direct and imperative — state what to do, not what the skill
is. One skill, one job: `design` writes the spec, `implement` builds from
it, `gate` judges ship-readiness. Chain them rather than writing one skill
that does everything.

Direct launch from a TTY runs interactively. `--batch` and automated
flow execution run the same skill headlessly, so write a bounded contract for
both surfaces when the work involves judgment or conversation:

```markdown
## Reviewer mode

- **Interactive reviewer:** explore the problem in the current conversation.
- **Parent reviewer:** answer the assigned question from supplied evidence and
  return without waiting for a person.
```

Skills chain through `scratch/`: a step writes `scratch/<branch>.md`, the
next step reads it. That contract is what makes flows work — if your skill
produces something a later step needs, write it to `scratch/`, not to chat.

Use `concept-review` to reconsider the product mid-task with a human, or after
`review-slice` for an autonomous assessment. Draft the affected usage docs and
skill guidance first, then follow the simpler interaction through types, APIs,
and infrastructure. Product clarity is valuable even without deleting code.
Keep proposed alternatives distinct from accepted requirements and verified
behavior. The review supplies evidence; `loop-decide` owns navigation when the
Flow declares a decision step.

## Flows

A flow is a YAML list of steps — each step names a skill, an op, or another
flow — with commits between them:

```yaml
# .lf/flows/ship-api.yaml
- implement
- compress
- gate
```

Skills that need another Work's perspective launch it directly with
`lf --as <work> : "<prompt>"`. Skills that genuinely need a decision from the user use
`lf ask "<request>"`; the Run blocks while a durable session works in the
same checkout, then resumes when the user completes that conversation.

Run a step interactively with `human: true`. Give it an `id` stable within
its expanded Flow so the conversation can be reopened:

```yaml
- kickoff
- step:
    id: review_design
    name: review-design
    human: true
```

The human and agent clarify the design in that conversation. The agent saves
feedback with `lf session ready "feedback and remaining work"`; the human ends
the review with `lf session complete <session-id>`. The Flow carries that
feedback to its next step. Provider exit or readiness alone leaves it waiting.
Human steps have no navigation verdict or backward edge. Put a deciding step
after the review when its feedback should choose between continuing and more work.

Mechanical git/PR operations ride along as `op:` steps:

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
observations, human feedback, agreed changes, unresolved questions, and the next
useful action with its proof. Link related notes. Update the relevant account
and mark superseded conclusions while preserving useful evidence. Notes remain
available across steps, regardless of which skill wrote them or runs next.

A review's ready summary points to that material:

```sh
lf session ready "See scratch/search-feedback.md: implement the agreed empty state; verify recovery after clearing the query"
```

Loop-decide starts at those paths, then reconciles the current design and other
relevant scratch evidence. A note recommends work; the deciding occurrence
records navigation through the Flow protocol. There is no required handoff
filename or control file. Recursive scratch Markdown is assembled into fresh
Run context; a running agent can reread files updated since its launch.

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

The `router:` skill reads the available evidence and records one choice with
`lf flow route PATH`. Routing instructions and path descriptions are appended
to its captured prompt. The choice belongs to the active Run and takes effect
when it succeeds; failed Runs discard their candidates. A path
with no `flow:`, `skill:`, or inline `steps:` (like `silence`) is a clean no-op
exit. With no `router:`, a generic routing agent picks from `scratch/` contents.

### Loopflows

A **loopflow** is a Flow with one or more backward edges. Use the same Flow
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

At the deciding occurrence, use `lf flow decide advance "evidence"` or
`lf flow decide iterate "next action and proof"`. The current decision Run owns
that choice; its candidate takes effect only after the Run succeeds. A review's
final prose or a successful process exit cannot substitute for the decision.

Backward edges have no pass limit. Iterate follows the edge as long as the
decision calls for more work; human revision needs no budget reset. Pass counts
describe history. Missing decisions stop execution. Blocked is a stopped
execution outcome: report it with
`lf flow blocked "reason, attempted direction, evidence, and question"`.
The runtime keys one Ask to the exact invocation, occurrence, and pass. Retries
join that Ask or recover its saved completion. Its Session runs `unblock`, using
concept-review with the human by default. Completion returns evidence to
loop-decide for reassessment without choosing a navigation decision. If the blocker
remains unresolved, report it; do not open identical Asks automatically.

Resume the saved invocation to preserve its captured definition, position,
direction, and accepted decisions. Edits to the source apply to new invocations.
Finishing a Flow grants no implicit merge or Task-completion authority and
does not choose another Flow; author delivery explicitly.

`feature` combines design review with this loop; `pursue` starts at
implementation. Ordinary and Task invocations capture every XOR router and path
before execution and use the same cursor for nested paths and backward edges.
Recovery reads that captured definition, including paths not yet selected.
The implementation and recovery fixtures do not establish live provider/Session
handoff parity; that still requires a configured end-to-end demonstration.

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
| `owner` | Optional OS user allowed to start the Wave automatically |
| `home` | Optional HomeId, hostname, or IP allowed to start the Wave automatically |
| `agent` | Preferred agent harness/model |
| `crons` | Supplementary flow schedules, fired by the wave's resident loop |
| `pm.linear_initiative` | Linear Initiative id backing the wave (written by `lf pm init`) |

The repository owns PM provider and Team authority in `.lf/config.yaml`:

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
`update-wave` owns deliberate end-of-work curation.

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
