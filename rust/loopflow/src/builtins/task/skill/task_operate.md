---
description: Move a Task through its Graph until it's done or stuck, asking for judgment inline when interactive.
requires: a Task, or the current checkout's Task
produces: a finished Task, or exactly what it is waiting on and the command to open its Session
action_style: procedural
---
Move the Task as far forward as its Graph allows.

You are this Task's operator: with no live Graph process, nobody else takes its
next step. Make routine decisions yourself, ask for judgment in this
conversation, then continue. Headless, exit when stuck or done. A Task already
done needs no new run. An unassigned
Task (no owner in status) is nobody's to move: read or discuss it, but don't
launch, ship or claim it; `lf task claim <issue>` is someone's own choice.

Finish with one disposition and what showed it:

- **moving**: a live Graph process, left alone.
- **acted**: you continued, repaired or delivered it, and reread status.
- **available**: a review, demo, choice or merge click someone can take up, with
  its conversation link or PR.
- **dependency or capacity**
- **unassigned**: nobody owns it, or you were told to stop.
- **unknown**: which read or liveness check is missing.

"Ready" is not a disposition. A PR that conflicts with or trails its base isn't
ready for anyone: `lf sync`, resolve conflicts, verify, push, reread status.

**Never start a second Graph process.** A live one is left running, and unknown
liveness is not permission. A run in status doesn't mean a live process: check
each Graph and unfinished Process against `lf ps --json`.

## Workflow

1. **Read the work.** `lf task status <issue> --json`, then the brief, design and
   `scratch/` history in its checkout; tell accepted decisions from drafts.
   `lf session list --json` and `lf graph show <id> --processes` for each Graph:
   its steps, how each ended, whether it's alive, what it changed. Every
   conversation and Graph in the checkout is the Task's work. If the Task is
   missing or ambiguous, ask when interactive; headless, say so and stop.

   Keep the design in the Task's checkout. If needed material lives elsewhere,
   copy its contents into the Task's `scratch/` before launch, without
   overwriting newer work; a path in another checkout carries nothing. Don't
   file duplicate work, or launch a competing implementation of code that was
   never handed off.

2. **Choose the Graph.** A Task runs one graph, shown as `work.graph` in status,
   with its interactive steps and the step the Task stands at. At an interactive
   step the Task waits in its conversation; say which step.

   ```bash
   lf task run <issue>                 # continue, or rerun a stopped run
   lf task run <issue> --from <step>   # go back or skip ahead
   lf task run <issue> <graph>         # take up another graph
   lf --task <issue> graph <graph>     # run a Graph without moving the Task
   ```

   A Task with no graph takes up its Wave's default, the repository's `graph:`,
   else `code` (`lf list`, `lf help <graph>`). Use a Graph that was named;
   otherwise the default, unless the design and past runs call for another entry
   point: `pursue` for an accepted design with implementation left, `task-design`
   for an open design. Graphs run autonomous work; review happens in the
   conversation. Read composed steps before launching and explain any departure
   from the default. Scratch history can't approve a design or skip a review.

   A stopped Graph is history: read what it finished and changed before choosing
   what to launch. A finished Graph or a merged PR doesn't mean the Task is done;
   read the outcome. Ask about scope or direction conflicts that matter; when
   nobody can answer, name the exact decision.

3. **Advance the work.** `lf task run <issue>` runs the graph to the next
   interactive step and returns once it's running; `lf task status <issue>`
   follows it. `lf -b task run` and `lf ship` run past every interactive step
   to `end`, and reaching `end` approves Auto-Ship, which may merge the PR. A
   plain `lf task run` with no interactive step left ahead, such as one run
   from the last, approves it too. Use them only when asked to ship or land. After
   shipping is stopped, a relaunch continues the work and never ships it; only a
   requested `lf ship` approves again.

   After a stopped or failed Graph, launch only what remains; never relaunch to
   get past a blocker. Read a failure's log first, and retry with
   `--reason "<what changed>"` only when something changed. When judgment is
   needed, ask here; never open another Session to reach someone already
   present. Save agreed review direction; feedback alone never restarts a Graph.
   Headless, a needed judgment or review is a blocker: report it and exit.
   Report missing commands or services without upgrading or switching accounts.

4. **Stay with the Task.** Refresh status and Sessions after each action. A
   launch is progress, not the stopping point. Wait for real state changes
   without tight polling, repair what you can, and keep going until status shows
   the Task done or a concrete blocker remains. Publishing, review, merge and
   completion are separate facts. Arming a merge the Graph didn't arm is
   someone's own choice. A distinct outcome gets a follow-up Task; continued
   work in this Task can publish another PR. Repeated calls never duplicate
   Graphs or conversations.

5. **Finish merged delivery.** After a verified merge with no live ship Graph
   finishing it, and the accepted outcome holds, run `lf task complete <issue>`
   and reread status. A Task without a PR can reach end with its files and
   commits. Completion doesn't move the graph and is safe to repeat. Don't replay gate, re-arm the PR, or reopen a completed Task to make
   more work. When status shows completion pending at end, retry
   `lf task complete <issue>` alone; never rerun steps that succeeded. A newer
   reopening supersedes the old request; standing at end alone doesn't justify
   another completion.

6. **Report what's done and what's available.** For finished work, state the
   outcome. Otherwise lead with what someone can do now (design review, demo, a
   concrete choice), then real dependencies (checks running, sign-in needed) and
   failures with their effect. Never portray a person as a blocker or as owing
   the agent attention.

   Link the Task with `lf open --task <issue> --link --format markdown`, such as
   `[LOO-489 · Enable the multitask navigation experience](loopflow://loopflow/task/LOO-489)`,
   with its disposition and any PR link; keep provider issue links separate.
   Where links may not open, add the `lf open '<url>'` it prints.
   `lf open --task <issue> --link --json` reports `conversation`: for
   `current_conversation`, ask or demo here; for `other_conversation`, give the
   link, and run `lf open '<url>'` only when asked to open it or a handoff
   selects that conversation; for `unknown`, give the link and say the caller's
   identity is missing. A printed link opens nothing; never claim an opening
   `lf open` didn't report.
