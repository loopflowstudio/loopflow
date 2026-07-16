# Task review points

> For each Task, where does the human interact or review?

> Can unattended work follow the same process instead of deleting the review
> steps?

> After a headless wave, how does one human demo/code-review pass catch up?

## Product contract

Each Project defines the default flow for its Task Sessions. A Task may override
that flow at launch. Interactive skills inside the resolved flow are review
exercises, not optional decorations. `review-design`, `demo`, and `code-review`
remain in the flow in both attended and headless execution.

The interaction policy changes the reviewer:

```text
require  interactive exercise is conducted with the human and blocks there
defer    the same exercise is conducted by an AI reviewer; human review is owed
```

Use `--headless` for the launch affordance. Do not expose
`--skip-interactive`: no flow step is skipped.

For a headless Task, route review to its resident Project worker. For a
headless Project, route review to its Wave worker. A root Wave uses a dedicated
reviewer body so the worker that produced the work is not silently approving
its own result. If the selected reviewer cannot run, the exercise remains
pending; execution does not manufacture a pass.

Manual Wave catch-up shows the human work that already received AI review. It
includes the prior exercise, evidence, feedback, and resulting changes. It
clears only the human-review obligations the catch-up actually covers.

## Names and inheritance

```text
Project.task_kickoff_flow  one-time opening flow inherited by new Tasks
Project.task_flow          repeated inner flow inherited by new Tasks
Project.task_gate_flow     finish-attempt flow inherited by new Tasks
Task.*_flow                optional authored phase overrides
TaskSession.resolved_*     immutable launch-time phase plan
InteractionPolicy          require | defer | inherit
CLI                        --headless
```

## Task lifecycle

One Task Session plays three ordinary flows:

```text
kickoff once → { iterate one or more times → gate } until approved
```

The durable Session pins a flow and interaction policy for each phase. The
standard plan is a human-guided sandwich:

```text
kickoff  attended   clarify, design, and review-design
iterate  headless   implement, compress, lint; repeat as needed
gate     attended   prove every Done when, demo, and code-review
```

`--headless` overrides reviewer routing for the whole lifecycle, so kickoff and
gate still run but their interactive exercises go to the parent worker.
Interactive steps inside iterate are parent-reviewed under the standard plan as
well. The flag never changes which phases or steps execute.

Persist phase, phase cursor, iterate count, gate cycle, and pending terminal outcome. A
review waitpoint is keyed by phase + that phase's invocation + step, so kickoff
and gate can use the same skill without colliding.

Gate begins whenever iterate proposes a terminal outcome. It receives that
pending outcome as context. `approved` settles the Task with that outcome;
`changes_requested` records the findings as direction, clears the pending
outcome, and returns the same Task Session to iterate. A gate that cannot reach
a judgment remains at the gate. Clean completion, failure, and ordinary
abandonment all pass through it; a future force-cancel may waive it only with an
explicit durable reason. Multi-PR rotation stays inside iterate—the gate
belongs to a Task finish attempt, not automatically to each PR.

Resolution at Task launch:

```text
phase flow:   launch override > Task override > Project phase flow > system default
interaction:  --headless > Task phase policy > Project phase policy > standard
```

Capture both resolved values on the durable Task Session. Later Project edits
affect new Sessions, not work already running.

The standard phase policies are `require / defer / require`. `defer` remains an
accurate internal policy name: it defers human participation
and creates human-review debt. It does not defer execution of the skill.

## One review exercise, two reviewers

Every interactive skill must describe both ways to conduct its exercise:

- **With the human:** orient them, expose the right proof, accept live
  reactions, and block until completion or hand-back.
- **As a headless reviewer:** inspect the same proof, make the judgments the
  skill asks for, return explicit findings and disposition, and never pretend
  to report a human reaction.

The authored purpose and completion criteria stay the same. Do not fork
`demo`/`demo-headless` or delete interactive steps from an alternate flow.

`demo` turns every `Done when` criterion in the design doc into a checkpoint.
For each criterion it names the proof surface—product, command/API, code,
logs/admin state, stored data, or measurements—runs or inspects the proof, and
connects the outcome to the implementation. A product path plus its resulting
log/admin/metric evidence is often stronger than either surface alone.

## Durable interaction review

Represent the exercise itself, not only its presentation or eventual debt:

```text
InteractionReview
  id
  Wave / Project / Task / Task Session ownership
  resolved flow, iteration, step path, and skill
  policy: require | defer
  reviewer: human | Project Session | Wave | dedicated reviewer body
  status: requested | active | completed | handed_back | failed
  request prompt and reason
  worktree, base/head commit, fingerprint, PR/delivery receipt
  reviewer outcome, findings, and disposition
  originating and completing body generations/times
```

One row is idempotent per flow waitpoint. It is the shared source of truth for
both paths:

### Required human review

```text
Task reaches interactive skill
  -> create InteractionReview(reviewer=human)
  -> open one W2-175 handoff for that review
  -> Task waits at the replay-safe cursor
  -> human attaches to the existing provider conversation
  -> completion/hand-back closes the review
  -> the same Task resumes exactly once and advances
```

The W2-175 row remains the attach/presentation contract. It references the
review; it is not a second review lifecycle.

### Headless parent review

```text
Task reaches the identical interactive skill
  -> create InteractionReview(reviewer=owning Project Session)
  -> Project observes a typed review request with skill prompt + evidence
  -> Project worker conducts the exercise against the Task worktree
  -> Project records findings/disposition through an authorized completion command
  -> Task incorporates the response, resumes exactly once, and advances
  -> completed review remains outstanding for human catch-up
```

The Task may wait on its parent agent, but it never waits for a human in
headless mode. Parent review is real work in the same flow, not a synthetic
`Skipped` playhead body.

Project review requests use the existing typed observation boundary. Add a
specific request/resolution protocol rather than encoding review as a generic
decision or free-form steer. Only the owning parent (or its Wave escalation)
may complete the request.

## Human-review debt and catch-up

Outstanding debt is the projection:

```text
completed reviews with policy=defer
minus reviews covered by a human catch-up receipt
```

Keep it out of Linear. It is execution evidence, not backlog intent. Headless
review can find and fix issues before delivery, but it does not claim the human
experienced the result.

Catch-up is manual for now:

```bash
lf --wave product review
```

One Wave-scoped catch-up:

1. Reads outstanding reviews, their findings, delivery receipts, open PRs, and
   the current integration base.
2. Groups work by coherent user journey or architectural seam, not Task count.
3. Surfaces evidence that cannot coexist: missing branches, unlanded stack
   parents, conflicts, stale commits, or unavailable environments.
4. Runs an overall `demo` and/or `code-review`, using every relevant design
   doc's `Done when` criteria as the agenda.
5. Records exactly which review ids the human covered. Concrete feedback
   becomes Tasks; uncovered ids stay open.

No cron, age/count threshold, automatic catch-up, or automatic red status.
Headless review debt is visible but is not a live blocked Session. A currently
waiting human handoff is live attention and remains red/attachable.

## Prototype ladder

Use one throwaway repo, temporary SQLite registry, and fake provider. Preserve
the same scenario while replacing only the fake seam.

### Slice 1 — interaction routing

The same expanded flow reaches `review-design`, `demo`, or `code-review` under
both policies.

- `require` yields a human review request and preserves the cursor.
- `defer` yields a parent-agent review request and preserves the cursor.
- Completing either review advances the cursor once.
- Neither path records `StepOutcome::Skipped`.

### Slice 2 — Task → Project walking skeleton

Add the durable `InteractionReview` lifecycle and typed Task observation. Teach
`project_pursue` to conduct the requested skill against the exact Task worktree
and call the authorized completion command. Prove findings arrive in the Task
conversation before its next flow step.

### Slice 3 — human W2-175 rendezvous

Open one handoff for required reviews. Attach to the Task provider, kill and
restart the parent body, complete the review, and observe one resume.

### Slice 4 — inheritance and manual catch-up

Add Project `task_flow`, tri-state policy inheritance, headless propagation,
and one Wave catch-up receipt covering multiple completed reviews.

### Slice 5 — product surfaces

Expose active human handoffs, active parent reviews, outstanding human-review
debt, stale/unavailable evidence, and catch-up coverage through the shared CLI
and Swift DTOs.

## Demo script

Create `scripts/demo_task_review.py` with deterministic and live modes:

```bash
uv run python scripts/demo_task_review.py --smoke
uv run python scripts/demo_task_review.py
uv run python scripts/demo_task_review.py --live
```

The scenario:

```text
1. launch Task A with a flow containing review-design and demo
2. show Task A block at review-design; attach, complete, and resume once
3. launch Task B with the same flow and --headless
4. show its Project worker conduct review-design and return findings
5. show Task B incorporate the findings, then reach demo
6. show the Project worker prove every Done when criterion and complete demo
7. show Task B continue with no skipped playhead steps
8. list two completed AI reviews still owed to the human
9. invoke one Wave catch-up and clear exactly the reviews it covers
```

## Test matrix

| Layer | Proof |
|---|---|
| Flow engine | Same flow requests human vs parent reviewer; neither skips; nested/loop cursors resume once |
| Registry | One review per waitpoint; reviewer authorization; restart/replay idempotence; completion advances once |
| Parent protocol | Project receives exact skill/evidence, records findings, and Task incorporates them before continuing |
| Human handoff | One W2-175 child per review; repeated attach/completion and parent replacement cannot double-resume |
| Policy | Launch > Task > Project > system; `--headless` propagates; resolved values are pinned |
| Debt | Completed deferred reviews minus catch-up coverage; uncovered ids remain; no automatic trigger |
| CLI demo | Fake provider runs all nine acts in a temporary repo/database |
| DTO/Swift | Rust and Swift agree on reviewer, status, evidence, next owner, actions, and debt counts |
| Live smoke | Real parent provider and tmux/Ghostty survive review, attach, restart, and hand-back |

## Done when

On one Project, two Tasks use the same flow containing `review-design` and
`demo`. The attended Task blocks once at each exercise and resumes exactly once
after the human completes it. The headless Task has its Project worker conduct
both exercises, incorporates that feedback, and completes with no skipped flow
steps. One manual Wave catch-up shows both prior AI reviews and the resulting
work, covers every relevant design-doc `Done when` criterion using the best
proof surface, and clears only the review ids the human actually covered.
