---
interactive: true
requires: scratch/<branch>.md (design doc)
produces: live Linear Projects and tasks | docs/ commits
action_style: procedural
---
Take a finished design and make it live in the planning system—projects and tasks launched immediately, no code solved here.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — `scratch/<branch>.md` is the design this session encodes;
  `scratch/questions.md` holds open questions and assumptions.
- Read `wave/*/GOAL.md` for the active roster—everything you file lands under
  one of these waves.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

## Reviewer mode

The launch prompt identifies the reviewer for this exercise.

- **Human reviewer:** walk them through the proposed encoding—wave, projects,
  task list—before filing. Their edits reshape the plan, not the design.
- **Parent reviewer:** treat the design doc and Task directive as intent.
  Make the encoding calls from the evidence, record genuine ambiguity in
  `scratch/questions.md`, file everything, and send the resulting decisions
  through the review protocol.

## The job

Decide, don't solve. This session turns a design into live planning
structure; it never writes product code, sketches implementations, or fills
design gaps. A gap in the design becomes an open question in the task that
owns it, not a problem to work here.

The associated PR carries only planning artifacts: the design doc under
`scratch/` and any durable context committed to `docs/`. If you're tempted to
touch product code, you're in the wrong skill—run `lf implement` on a task
instead.

## Workflow

1. **Place the design.** Match it against each wave's Objective and Bounds in
   `wave/*/GOAL.md`. Almost every design serves exactly one wave; if it
   genuinely spans two, split the encoding by wave rather than forcing one.
2. **Map to projects.** Read the wave's bets (`lf pm show --wave <name>`).
   Most designs advance an existing project—prefer that. Create a new one
   (`lf pm project create`) only when the design is a genuinely new measured
   bet: a completable behavioral improvement or a standing quality frontier,
   with a definition and proof-shaped KRs. Never create a wave here.
3. **File the tasks.** One task per independently shippable piece:
   `lf pm task create --project <project> --title "…" --notes "…"`. Titles
   state outcomes, not activities. Notes must be self-contained—quote the
   design's intent, constraints, and done-when verbatim, since `scratch/`
   dies when this PR lands and the worker won't have it. Slice before
   technical design, not after: notes carry what the increment must do, not
   how—each task gets its own design session when a worker picks it up.
   Pre-designing the whole series here is the same mistake as solving it,
   and the detail goes stale before it's used. Filing is launching: tasks
   are live in Linear the moment they exist, and the wave loop picks them
   up. Don't stage a plan that waits for a second approval.
4. **Preserve durable context.** Anything future sessions need beyond the
   tasks—architecture decisions, constraints, rejected alternatives—goes in
   `docs/` as a commit in this PR, or into wave memory via `lf memory add`.
   Never edit `wave/<name>/MEMORY.md` directly; it is server-owned.
5. **Commit and close.** `git add scratch/ docs/ && git commit -m "plan:
   <branch>"`, then tell the user what went live: which wave, which projects,
   how many tasks, and which task to run first.

## What good encoding looks like

- Every task is independently shippable—its own branch, commit, and PR.
- Task notes let a worker start cold: intent, constraints, done-when, and the
  design excerpts that anchor them.
- The plan's structure survives the death of `scratch/`: Linear plus `docs/`
  carry everything load-bearing.
- Nothing filed is speculative. If a piece of the design is still an open
  question, it becomes a question in the notes of the task that will answer
  it—not a task of its own.
