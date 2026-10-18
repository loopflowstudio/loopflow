---
description: Advance a Task through its Loopflow process until completed or blocked, asking for judgment inline when interactive.
requires: a Task identity or the current checkout's tracked Task
produces: a completed outcome, or an exact blocker with a command to open its Session
action_style: procedural
---
Move the Task forward as far as possible through its authored Loopflow process.
You are this Task's operator: with no live Flow process, nobody else takes its next
step. Resolve routine decisions autonomously and ask for needed judgment in the
present conversation, then continue. Headless, exit when blocked or completed. An
already satisfied Task needs no new execution. A Task whose shared status is
Paused stays held: inspect or discuss it, but do not launch work or resume it
automatically. `lf task resume <issue>` clears the hold without starting execution.

Finish with one disposition and its evidence: **moving** (a live Flow process was
observed and left alone), **acted** (you continued, recovered or delivered it
and reread status), **waiting on a person** (the named review, decision or
merge click and its Session or PR), **waiting on a dependency or capacity**,
**paused** (an explicit hold or instruction), or **unknown** (the missing read
or liveness evidence; never a reason to start a second Flow process). “Ready” or
“needs reconciliation” is not a disposition.

## Workflow

1. **Read the work.** Resolve the explicit Task or the current checkout's Task
   with `lf task status <issue> --json`. Read its current brief, design, and
   relevant scratch history in its existing checkout. Separate accepted decisions
   from drafts and superseded plans. Inspect `lf session list --json` and the
   Task's Flows with `lf flow show <id> --processes`: each one's steps, how each
   ended, whether its Flow process is alive, and its effects. Every
   conversation and Flow in the checkout is the Task's work; none is privileged.
   If Task identity is missing or ambiguous, ask when interactive; headless, report
   the missing identity and stop. Do not file duplicate work.

   Keep the current design in the Task checkout. If required material lives
   elsewhere, copy its contents and necessary references into the Task's scratch
   before launch, preserving newer destination work. The destination becomes the
   working copy. A path in another checkout does not deliver context. Do not
   launch competing implementation when existing code has no supported handoff.

2. **Choose the Flow.** A Task on a workflow reports it as
   `execution.work.workflow` in status: nodes where a person takes part,
   joined by edges that each run one Flow. An edge alone does not prove a live
   Flow process: check every associated Flow and unfinished Process against `lf ps --json`.
   Leave observed live work alone; unknown liveness never authorizes a second Flow process.
   Before selecting another edge, recover verified merged delivery as described
   below. At a node its last edge reached, the Task waits on a person in
   its conversation; say which node and which edges leave it. At a node
   because the last edge stopped or failed, that edge is the remaining work.
   Only an edge leaving the current node moves the Task:
   `lf task run <issue>` takes the only one or names them,
   `lf task run <issue> <flow>` picks one, `lf task run <issue> end` takes an
   edge that runs nothing. `lf --task <issue> flow <flow>` runs a Flow without
   moving the Task. Without a workflow, choose as follows.
   Leave a Flow with a live process running. A stopped
   Flow is history: read what it finished and which effects it recorded before
   choosing what to launch. Read the Wave default when assigned, otherwise the
   repository `workflow:`, then inspect the actual
   installed catalog with `lf list` and `lf help <flow>`. Honor an explicit Flow
   choice; otherwise use the Wave's workflow or repository default unless the design and execution
   evidence call for another entry point. For example, an accepted design with
   implementation remaining can use `pursue`; unresolved design starts with
   `task-design` and its review in the Task conversation. Flows run autonomous
   work only; review happens in the conversation. Read composed steps before launching.
   Explain any departure from the default. Scratch history alone cannot approve
   a design or waive a review. A finished Flow proves neither completion nor
   unfinished work: inspect its outcome. Completion alone is not a stopping
   point if the requested outcome still needs work. A merged PR alone does not
   complete the Task. Ask about consequential scope
   or direction conflicts; when judgment is unavailable, name the exact decision.

3. **Advance the work.** `lf task run <issue> [flow]` runs a
   fresh Flow in the Task's worktree and returns when it ends; it never continues
   an earlier one. Run it with your own background tool to keep working. Check installed
   help first. Leave a live Flow process running. After a stopped or failed Flow,
   launch only the work that remains; never relaunch merely to bypass a blocker.
   When judgment is needed, ask here if interactive; never open another Session
   merely to reach the person already present. Discuss review feedback in the
   ongoing conversation and save agreed direction. Inspect execution and effects
   before selecting further work; feedback alone never restarts a Flow.
   Headless, a required judgment or review is a blocker: report it and exit.
   Report unavailable commands or services without upgrading or changing accounts.

4. **Stay with the Task.** Refresh Task status and Sessions after each action.
   An accepted launch or a healthy Flow process is progress, not the stopping point.
   Wait for meaningful state changes without tight polling or competing with a
   live Flow process. Continue through the authored steps, resolve recoverable failures,
   and use inline answers to unblock interactive work. Read a failure's log
   before retrying, and retry with `--reason "<what changed>"` only on new
   evidence or a repaired cause. Stop when status confirms Task completion, or a concrete blocker remains. Report publication,
   review, merge and completion as separate facts. Arming a merge the Flow did not remains the
   person's choice. Recommend a follow-up Task for a distinct outcome. Continued
   work in the same Task can publish another PR, retaining earlier PR history. Repeated calls must
   preserve existing work without duplicating Flows or conversations.

5. **Finish merged delivery.** A verified merge leaves an open Task available for
   optional new Task creation. No filing or disposition is required. First inspect
   associated Flows and unfinished Processes; leave a live ship Flow to finish.
   When the accepted outcome is satisfied and no live Flow owns its finish,
   run `lf task complete <issue>` and reread status. Do not replay gate, re-arm the
   PR or reopen a completed Task merely to create more work. A PR-less Task can reach end with retained files
   and commits; no placeholder PR or landing ceremony is required. Completion
   changes status without moving the Workflow and is safe to repeat after arrival.
   When status shows completion pending at end, retry `lf task complete <issue>`
   alone. Never run the successful edge again. A newer reopening supersedes the
   old request; standing at end alone does not authorize another completion.

6. **Report the outcome or blocker.** For completed work, state the
   observed outcome. For blocked work, name what prevents progress and the exact
   decision or action needed. Identify the blocking Session from fresh Task
   and Flow records, including the pending review when that is the blocker, and
   supply a copyable command to open it in Loopflow Desktop:

   ```sh
   open 'loopflow://task/<issue>?repo=<percent-encoded-repository>&session=<percent-encoded-session-id>'
   ```

   Encode query values and quote the URL for the shell. This opens the existing
   conversation; it does not create one or complete its review. If no Session
   exists for the blocker, return the Task-only link and state that it opens Task
   details and no blocking Session is available.
   Do not invent an ID, substitute an unrelated Session, or claim Desktop opened
   merely because a command was returned. No extra report file is needed.
