---
description: Advance a Task through its Loopflow process until landed or blocked, asking for judgment inline when interactive.
requires: a Task identity or the current checkout's tracked Task
produces: a landed or completed outcome, or an exact blocker with a command to open its Session
action_style: procedural
---
Move the Task forward as far as possible through its authored Loopflow process.
You are this Task's operator: with no live worker, nobody else takes its next
step. Resolve routine decisions autonomously and ask for needed judgment in the
present conversation, then continue. Headless, exit when blocked or landed. An
already satisfied Task needs no new execution.

Finish with one disposition and its evidence: **moving** (a live driver was
observed and left alone), **acted** (you continued, recovered or delivered it
and reread status), **waiting on a person** (the named review, decision or
merge click and its Session or PR), **waiting on a dependency or capacity**,
**paused** (an explicit hold or instruction), or **unknown** (the missing read
or liveness evidence; never a reason to start a second driver). “Ready” or
“needs reconciliation” is not a disposition.

## Workflow

1. **Read the work.** Resolve the explicit Task or the current checkout's Task
   with `lf task status <issue> --json`. Read its current brief, design, and
   relevant scratch history in its existing checkout. Separate accepted decisions
   from drafts and superseded plans. Inspect `lf session list --json` and the
   Task's captured Flow, cursor, driver, and pending review. Other conversations
   in the checkout are evidence, not automatically the managed Flow's Session.
   Look for a live driver before acting: check every Flow in
   `execution.work.flows`, including those with `managed: false`, and every
   unfinished Exec in `execution.work.execs`, against `lf ps --json`. A live
   process on any of them makes the Task moving, whoever launched it.
   If Task identity is missing or ambiguous, ask for it; do not file duplicate work.

   Keep the current design in the Task checkout. If required material lives
   elsewhere, copy its contents and necessary references into the Task's scratch
   before launch, preserving newer destination work. The destination becomes the
   working copy. A path in another checkout does not deliver context. Do not
   launch competing implementation when existing code has no supported handoff.

2. **Preserve or choose the Flow.** Continue an unfinished captured Flow even
   when the Wave's recommendation has changed; it needs no permission. A Flow
   ends where it is authored to end: when it finished and the work has not
   landed, name the PR and what remains, and leave the choice of a further
   Flow to the person. If the Task has never had a Flow, read the owning
   Wave's status and current Project's `flow:` default, then inspect the actual
   installed catalog with `lf list` and `lf help <flow>`. Honor an explicit Flow
   choice; otherwise use the Project default unless the design and execution
   evidence call for another entry point. For example, an accepted design with
   implementation remaining can use `pursue`; unresolved design needs the design
   review in `feature`. Read composed delivery and review steps before launching.
   Explain any departure from the default. Scratch history alone cannot approve
   a design or waive a review. A finished Flow proves neither completion nor
   unfinished work: inspect its outcome, and stop if the Task's outcome is
   already satisfied. Ask about consequential scope
   or direction conflicts; when judgment is unavailable, name the exact decision.

3. **Advance the work.** Use `lf --task <issue> flow start` to continue, or
   `lf --task <issue> flow start <flow>` for the selected new Flow. Check installed
   help first. Recover a stopped driver through
   the same saved Flow; never restart or replace it merely to bypass a blocker.
   When judgment is needed, ask here if interactive; never open another Session
   merely to reach the person already present. A review belongs to its own
   Session: complete that exact Session with `lf session complete <id>` only
   when the participant decides the review with you and its feedback is saved.
   Never approve on their behalf. Let the authored Flow choose the next edge.
   Headless, a required judgment or review is a waiting disposition: report it
   and exit.
   Report unavailable commands or services without upgrading or changing accounts.

4. **Stay with the Task.** Refresh Task status and Sessions after each action.
   An accepted launch or a healthy worker is progress, not the stopping point.
   Wait for meaningful state changes without tight polling or competing with a
   live driver. Continue through the authored steps, resolve recoverable failures,
   and use inline answers to unblock interactive work. Read a failure's log
   before retrying, and retry with `--reason "<what changed>"` only on new
   evidence or a repaired cause. Stop when the Task lands,
   its intended outcome is otherwise complete, or a concrete blocker remains.
   Report publication, review, landing, Task completion and remaining scope as
   separate facts. Arming a merge the Flow did not, and
   rotating to a next PR with `lf pr next`, are the person's to choose. Repeated
   calls must preserve existing work without duplicating Flows or conversations.

5. **Report the outcome or blocker.** For landed or completed work, state the
   observed outcome. For blocked work, name what prevents progress and the exact
   decision or action needed. Identify the blocking AgentSession from fresh Task
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
