---
requires: a Task identity and authoritative merge evidence when it has a PR
produces: linked follow-up Tasks or a recorded reason none is needed, then confirmed Task completion
---
Finish delivered work by giving each accepted remaining obligation its own Task.

1. **Read the delivery.** Resolve the Task with `lf task status <issue> --json`.
   Read its accepted brief, merged PR copy and head, delivery evidence, acceptance
   limits and existing follow-up receipts. Scratch may already be deleted; it
   must not be the only record of an obligation. If there is no Task, report the
   PR delivery without inventing one. If already done, report the recorded
   disposition without filing again. A Task with a PR needs authoritative merge
   evidence; an open, closed-unmerged or unreadable PR leaves it pending.

2. **Identify remaining obligations.** File only concrete accepted work whose
   evidence comes later: an installed release, production behavior, real usage
   or an agreed measurement. Do not invent monitoring after every refactor or
   turn suggested enhancements into accepted scope. Preserve unresolved legacy
   remaining-work records until their scope is converted; never treat missing
   evidence as satisfaction. If the accepted scope is ambiguous, report the
   exact decision in the existing conversation and stop without completing.

3. **File or link, then resolve the disposition.** Use the source Task's command:

   ```bash
   lf task follow-up <source> --title '<outcome>' --notes '<complete brief>' --due YYYY-MM-DD --wave <owner>
   lf task follow-up <source> --existing <follow-up>
   lf task follow-up <source> --finish '<why these Tasks cover the remaining obligations>'
   ```

   `--due` and `--wave` are optional. An explicit owner wins; otherwise filing
   selects the source Wave's current Project. Each brief states the source Task
   and PR, observation, environment/version prerequisite, expected evidence and
   what to do on failure. Put timezone or time-of-day requirements in the brief;
   a due date is only a day. Links are related Tasks, not prerequisites that keep
   the source open until the children finish.

   Inspect existing receipts before every retry. The command retains the child
   identity, destination and payload across an uncertain response or chapter
   rotation. Use a stable `--key` for each obligation (default: `follow-up`),
   and a different key for each additional obligation. Retry the same filing;
   do not reword it, select a new destination,
   or bypass it with ordinary Task creation to escape an error. Preserve successful
   filings when another fails. Confirm every issue and relation before `--finish`;
   failed or uncertain readback leaves follow-through pending.

   When no accepted obligation remains, use
   `lf task follow-up <source> --none '<reason no follow-up is needed>'` instead.
   A PR-less Task may file useful follow-ups but needs no mandatory landing
   ceremony. Newly imagined improvements remain suggestions.

4. **Complete and verify.** Run `lf task complete <source>` and reread status.
   It shares the checks of `lf task move <source> end`; neither `--force` nor an
   agent's successful turn bypasses merge or required filing. Retry a recorded
   disposition and pending planning writeback without filing again. Report done
   only when status confirms end/completion, with the follow-up links or recorded
   none reason. A failed completion remains pending with its concrete cause.

A dated follow-up returns on its owning Wave's next operation. Filing installs
no timer and authorizes no arbitrary future execution. Run an unattended check
only when its brief already authorizes that concrete check. Inspect installed
Wave schedule coverage; without it, report “next Wave pass; no automatic check
scheduled.” Unknown schedule evidence stays unknown. Time passing never proves
the follow-up succeeded. The source completes after filing, without waiting for
that later evidence.
