---
description: Advance a Task through its Loopflow process until landed or blocked, asking for judgment inline when interactive.
requires: a Task identity or the current checkout's tracked Task
produces: a landed or completed outcome, or an exact blocker with a command to open its Session
action_style: procedural
---
Move the Task forward as far as possible through its authored Loopflow process.
This is primarily an interactive command: resolve routine decisions autonomously
and ask for needed judgment in the present conversation, then continue. Headless,
exit when blocked or landed. An already satisfied Task needs no new execution.

## Workflow

1. **Read the work.** Resolve the explicit Task or the current checkout's Task
   with `lf task status <issue> --json`. Read its current brief, design, and
   relevant scratch history in its existing checkout. Separate accepted decisions
   from drafts and superseded plans. Inspect `lf session list --json` and the
   Task's captured Flow, cursor, driver, and pending review. Other conversations
   in the checkout are evidence, not automatically the managed Flow's Session.
   If Task identity is missing or ambiguous, ask for it; do not file duplicate work.

   Keep the current design in the Task checkout. If required material lives
   elsewhere, copy its contents and necessary references into the Task's scratch
   before launch, preserving newer destination work. The destination becomes the
   working copy. A path in another checkout does not deliver context. Do not
   launch competing implementation when existing code has no supported handoff.

2. **Preserve or choose the Flow.** Continue an unfinished captured Flow even
   when the Wave's recommendation has changed. If none exists, read the owning
   Wave's status and current Project's `flow:` default, then inspect the actual
   installed catalog with `lf list` and `lf help <flow>`. Honor an explicit Flow
   choice; otherwise use the Project default unless the design and execution
   evidence call for another entry point. For example, an accepted design with
   implementation remaining can use `pursue`; unresolved design needs the design
   review in `feature`. Read composed delivery and review steps before launching.
   Explain any departure from the default. Scratch history alone cannot approve
   a design or waive a review. A finished Flow is not evidence of unfinished work:
   stop if the Task's outcome is already satisfied. Ask about consequential scope
   or authority conflicts; when judgment is unavailable, name the exact decision.

3. **Advance the work.** Use `lf --task <issue> flow start` to continue, or
   `lf --task <issue> flow start <flow>` for the selected new Flow. Check installed
   help first: installations exposing `lf task run <issue> [--flow <flow>]` use
   that equivalent. Leave a live driver running. Recover a stopped driver through
   the same saved Flow; never restart or replace it merely to bypass a blocker.
   When judgment is needed, ask here if interactive; never open another Session
   merely to reach the person already present. At a review boundary, collect the
   required feedback and complete that exact review Session only when authorized
   and its feedback has been saved. Let the authored Flow choose the next edge.
   Headless, a required judgment or review is a blocker: report it and exit.
   Report unavailable commands or services without upgrading or changing accounts.
   Launching a Flow does not grant missing publication or merge authority.

4. **Stay with the Task.** Refresh Task status and Sessions after each action.
   An accepted launch or a healthy worker is progress, not the stopping point.
   Wait for meaningful state changes without tight polling or competing with a
   live driver. Continue through the authored steps, resolve recoverable failures,
   and use inline answers to unblock interactive work. Stop when the Task lands,
   its intended outcome is otherwise complete, or a concrete blocker remains.
   A finished Flow alone does not prove the Task is complete: inspect its outcome
   and remaining work before choosing the next supported action. Repeated calls
   must preserve existing work without duplicating Flows or conversations.

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
