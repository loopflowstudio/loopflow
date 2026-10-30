---
description: Advance a Task through its Loopflow process until completed or blocked, asking for judgment inline when interactive.
requires: a Task identity or the current checkout's tracked Task
produces: a completed outcome, or an exact blocker with a command to open its Session
action_style: procedural
---
Move the Task forward as far as possible through its authored Loopflow process.
You are this Task's operator: with no live Graph process, nobody else takes its next
step. Resolve routine decisions autonomously and ask for needed judgment in the
present conversation, then continue. Headless, exit when blocked or completed. An
already satisfied Task needs no new execution. An unassigned Task (no owner in
status) is nobody's to move: inspect or discuss it, but do not launch, ship or
claim it automatically; `lf task claim <issue>` is the person's choice.

Finish with one disposition and its evidence: **moving** (a live Graph process was
observed and left alone), **acted** (you continued, recovered or delivered it
and reread status), **available** (the review, demo, choice or merge click the
person can take up, with its conversation link or PR), **dependency or capacity**,
**unassigned** (nobody owns it, or the person said to stop), or **unknown** (the missing read
or liveness evidence; never a reason to start a second Graph process). “Ready” or
“needs reconciliation” is not a disposition. A PR that conflicts with or
trails its base is not available to a person: run `lf sync`, resolve the
conflicts, verify and push, then reread status.

## Workflow

1. **Read the work.** Resolve the explicit Task or the current checkout's Task
   with `lf task status <issue> --json`. Read its current brief, design, and
   relevant scratch history in its existing checkout. Separate accepted decisions
   from drafts and superseded plans. Inspect `lf session list --json` and the
   Task's Graphs with `lf graph show <id> --processes`: each one's steps, how each
   ended, whether its Graph process is alive, and its effects. Every
   conversation and Graph in the checkout is the Task's work; none is privileged.
   If Task identity is missing or ambiguous, ask when interactive; headless, report
   the missing identity and stop. Do not file duplicate work.

   Keep the current design in the Task checkout. If required material lives
   elsewhere, copy its contents and necessary references into the Task's scratch
   before launch, preserving newer destination work. The destination becomes the
   working copy. A path in another checkout does not deliver context. Do not
   launch competing implementation when existing code has no supported handoff.

2. **Choose the Graph.** A Task runs one graph, reported as `work.graph` in
   status: its interactive steps, where a person takes part, and the step the
   Task stands at. A run alone does not prove a live Graph process: check every
   associated Graph and unfinished Process against `lf ps --json`. Leave observed
   live work alone; unknown liveness never authorizes a second Graph process.
   Before running further, recover verified merged delivery as described below.
   At an interactive step, the Task waits on a person in its conversation; say
   which step. When they are done, `lf task run <issue>` continues after it. A
   stopped run is the remaining work: `lf task run <issue>` starts it again.
   `lf task run <issue> --from <step>` goes back or skips ahead;
   `lf task run <issue> <graph>` takes up another graph. `lf --task <issue>
   graph <graph>` runs a Graph without moving the Task.
   Leave a Graph with a live process running. A stopped
   Graph is history: read what it finished and which effects it recorded before
   choosing what to launch. A Task with no graph takes up its Wave's default,
   the repository's `graph:`, else `code`; inspect the actual
   installed catalog with `lf list` and `lf help <graph>`. Honor an explicit Graph
   choice; otherwise use that default unless the design and execution
   evidence call for another entry point. For example, an accepted design with
   implementation remaining can use `pursue`; unresolved design starts with
   `task-design` and its review in the Task conversation. Graphs run autonomous
   work only; review happens in the conversation. Read composed steps before launching.
   Explain any departure from the default. Scratch history alone cannot approve
   a design or waive a review. A finished Graph proves neither completion nor
   unfinished work: inspect its outcome. Completion alone is not a stopping
   point if the requested outcome still needs work. A merged PR alone does not
   complete the Task. Ask about consequential scope
   or direction conflicts; when judgment is unavailable, name the exact decision.

3. **Advance the work.** `lf task run <issue>` launches: it runs the Task's
   graph in its worktree until the next interactive step and returns once it
   is running. `lf task status <issue>` follows it. Launching is not approving.
   `lf -b task run` and `lf ship` run headless past every interactive step to
   `end`, and a run reaching `end` approves Auto-Ship, which may merge the PR.
   Use them only when the person explicitly asks to ship or land. After someone
   stops shipping, a relaunch continues the work and never ships it; only
   `lf ship`, when they ask for it, approves it again. Check installed
   help first. Leave a live Graph process running. After a stopped or failed Graph,
   launch only the work that remains; never relaunch merely to bypass a blocker.
   When judgment is needed, ask here if interactive; never open another Session
   merely to reach the person already present. Discuss review feedback in the
   ongoing conversation and save agreed direction. Inspect execution and effects
   before selecting further work; feedback alone never restarts a Graph.
   Headless, a required judgment or review is a blocker: report it and exit.
   Report unavailable commands or services without upgrading or changing accounts.

4. **Stay with the Task.** Refresh Task status and Sessions after each action.
   An accepted launch or a healthy Graph process is progress, not the stopping point.
   Wait for meaningful state changes without tight polling or competing with a
   live Graph process. Continue through the authored steps, resolve recoverable failures,
   and use inline answers to unblock interactive work. Read a failure's log
   before retrying, and retry with `--reason "<what changed>"` only on new
   evidence or a repaired cause. Stop when status confirms Task completion, or a concrete blocker remains. Report publication,
   review, merge and completion as separate facts. Arming a merge the Graph did not remains the
   person's choice. Recommend a follow-up Task for a distinct outcome. Continued
   work in the same Task can publish another PR, retaining earlier PR history. Repeated calls must
   preserve existing work without duplicating Graphs or conversations.

5. **Finish merged delivery.** A verified merge leaves an open Task available for
   optional new Task creation. No filing or disposition is required. First inspect
   associated Graphs and unfinished Processes; leave a live ship Graph to finish.
   When the accepted outcome is satisfied and no live Graph owns its finish,
   run `lf task complete <issue>` and reread status. Do not replay gate, re-arm the
   PR or reopen a completed Task merely to create more work. A PR-less Task can reach end with retained files
   and commits; no placeholder PR or landing ceremony is required. Completion
   changes status without moving the Task's graph and is safe to repeat after arrival.
   When status shows completion pending at end, retry `lf task complete <issue>`
   alone. Never run the successful steps again. A newer reopening supersedes the
   old request; standing at end alone does not authorize another completion.

6. **Report the outcome and what is available.** For completed work, state the
   observed outcome. Otherwise lead with the work and the interaction available
   now (design review, demo, a concrete choice), then real dependencies (checks
   running, sign-in required) and failures with their effect. Never portray the
   person as a blocker or as owing the agent attention.

   Name the Task with one readable link from
   `lf open --task <issue> --link --format markdown`, such as
   `[LOO-489 · Enable the multitask navigation experience](loopflow://loopflow/task/LOO-489)`,
   with disposition and any PR link beside it; keep provider issue links separate.
   Where links may not activate, add the `lf open '<url>'` it prints.
   `lf open --task <issue> --link --json` reports `conversation`. When it is
   `current_conversation`, this conversation is the destination: present the
   question or demo here and accept the answer here. For `other_conversation`,
   give the link; when the person asked to open it or a handoff selects that
   conversation for them, run `lf open '<url>'` and report its receipt. For
   `unknown`, give the link and name the missing caller identity. Printing a
   link opens nothing; never invent a Session or claim an opening it did not report.
