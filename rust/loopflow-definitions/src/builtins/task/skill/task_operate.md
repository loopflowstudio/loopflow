---
description: Move a Task through its Graph until it's done or stuck, asking for judgment inline when interactive.
requires: a Task, or the current checkout's Task
produces: a finished Task, or exactly what it is waiting on and the command to open its Session
action_style: procedural
---
Move the Task as far forward as its Graph allows.

A paused Task stays quiet. Report its saved position and stop unless asked to unpause it.
With no live Graph run, nobody else takes this Task's next step, so it is
yours. Make routine decisions yourself, ask for judgment in this conversation,
then continue. Headless, exit when done or stuck. A Task with no owner is
nobody's to move: read and discuss it, but don't launch or ship it.

**Never start a second Graph run.** A live one is left running, and unknown
liveness is not permission. A run listed in status is not a live process: its
`running` in `lf task status <issue> --json` is read from the run's own pid.

## Method

1. **Read the work.** `lf task status <issue> --json`, then the brief, design
   and `scratch/` in its checkout, telling decisions from drafts.
   `lf session list --json` and `lf graph show <graph-id>` say what each
   Graph finished, how it ended and what it changed. If the Task is missing or
   ambiguous, ask; headless, say so and stop.

2. **Know where it stands.** A Task runs one graph (`work.graph` in status) and
   stands at a step of it. At an interactive step it waits in its conversation
   for a person; say which step.

   ```bash
   lf task run <issue>                 # continue, or rerun a stopped run
   lf task run <issue> --from <step>   # go back or skip ahead
   lf task run <issue> <graph>         # take up another graph
   lf --task <issue> graph <graph>     # run a Graph without moving the Task
   ```

   A stopped Graph is history; nothing resumes it. Read what it finished before
   choosing what to launch, and launch only what remains. Use the graph that
   was named, else the Task's default; explain any departure. Scratch history
   can't approve a design or skip a review.

3. **Advance it.** `lf task run` returns once the graph is running and stops at
   the next interactive step. Read a failed step's log before anything else,
   fix the cause if you can, and retry with `--reason "<what changed>"` only
   when something changed. Never relaunch to get past a blocker.

   `lf task auto on` turns auto on: the run stands in at the interactive steps and
   may merge the PR. `lf -b task run` stands in for its own run, and a run
   with no interactive step left ahead may merge it too. Use them only when asked to ship or land. After
   someone turns auto off (`lf task auto off`), only a requested `lf task auto on` turns
   it on again.

4. **Stay with it.** A launch is progress, not the stopping point. Reread
   status after each action, wait for real changes without tight polling,
   repair what you can, and keep going until the Task is done or a concrete
   blocker remains. A PR that conflicts with or trails its base isn't ready
   for anyone: `lf sync`, resolve, verify, push.

5. **Finish merged work.** Merging doesn't complete a Task. After a verified
   merge with no live Graph, when the outcome holds, run
   `lf task complete <issue>` and reread status. It is safe to repeat; retry
   it alone, never by rerunning gate or steps that succeeded.

## Disposition

Finish with one, and what showed it:

- **moving** — a live Graph run, left alone.
- **acted** — you continued, repaired or delivered it, and reread status.
- **available** — a review, demo, choice or merge click someone can take up,
  with its conversation link or PR.
- **dependency or capacity** — waiting on a named Task, PR, check or limit.
- **unassigned** — nobody owns it, or you were told to stop.
- **unknown** — naming the read or liveness check that is missing.

"Ready" is not a disposition. Lead with what someone can do now, then real
dependencies, then failures and their effect. People are never blockers and
never owe the agent attention.

Link the Task with `lf open --task <issue> --link --format markdown`, plus any
PR. A printed link opens nothing; run `lf open '<url>'` only when asked, and
never claim an opening it didn't report.
