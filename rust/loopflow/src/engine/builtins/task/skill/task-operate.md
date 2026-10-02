---
description: Continue a Task's Flow from its design and execution history, then open its Session in Loopflow Desktop.
requires: a Task identity or the current checkout's tracked Task
produces: observed Flow progress or an exact blocker, and a Desktop opening command
action_style: procedural
---
Get the Task moving through its appropriate Flow and return a command that opens
its current conversation in Loopflow Desktop.

## Workflow

1. **Read the work.** Resolve the explicit Task or the current checkout's Task
   with `lf task status <issue> --json`. Read its current brief, design, and
   relevant scratch history in its existing checkout. Separate accepted decisions
   from drafts and superseded plans. Inspect `lf session list --json` and the
   Task's captured Flow, cursor, driver, and pending review. Other conversations
   in the checkout are evidence, not automatically the managed Flow's Session.
   If Task identity is missing or ambiguous, ask for it; do not file duplicate work.

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

3. **Ensure progression.** Use `lf --task <issue> flow start` to continue, or
   `lf --task <issue> flow start <flow>` for the selected new Flow. Check installed
   help first: installations exposing `lf task run <issue> [--flow <flow>]` use
   that equivalent. Leave a live driver running. Recover a stopped driver through
   the same saved Flow; never restart or replace it merely to bypass a blocker.
   At an interactive review, preserve the boundary and provide its Session link.
   Complete that exact Session only when the present request authorizes completion
   and its feedback has been saved; let the authored Flow choose the next edge.
   Report unavailable commands or services without upgrading or changing accounts.
   Launching a Flow does not grant missing publication or merge authority.

   Keep the current design in the Task checkout. If required material lives
   elsewhere, copy its contents and necessary references into the Task's scratch
   before launch, preserving newer destination work. The destination becomes the
   working copy. A path in another checkout does not deliver context. Do not
   launch competing implementation when existing code has no supported handoff.

4. **Observe the result.** Refresh Task status and Sessions after the action.
   Establish a live driver/current step, an exact pending review, completion, or
   a concrete blocker. An accepted launch request alone is insufficient. Do not
   wait for the entire Flow or repeatedly poll a healthy worker. Repeated calls
   should return the existing work without creating another Flow or conversation.

5. **Return the Desktop handoff.** Identify the current managed Flow's exact
   AgentSession from fresh records, preferring its pending review when present.
   Return the Task, selected/saved Flow and reason, observed step or blocker, and
   a copyable command with real identifiers:

   ```sh
   open 'loopflow://task/<issue>?repo=<percent-encoded-repository>&session=<percent-encoded-session-id>'
   ```

   Encode query values and quote the URL for the shell. This opens the existing
   conversation; it does not create one or complete its review. If no Session
   exists yet, return the Task-only link and state that it opens Task details.
   Do not invent an ID, substitute an unrelated Session, or claim Desktop opened
   merely because a command was returned. No extra report file is needed.
