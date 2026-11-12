# Linear sync wedged on "conflicting planning facts at the same provider revision"

**State (2026-11-12):** the three wedged Tasks were cleared by hand (stored
revision moved back 1 ms). The code fix is on this branch (`scratch/plan.md`):
at an equal `updatedAt` the later read wins, and errors name
`lf task sync <issue>` / `lf wave sync`.

## What failed

`lf task sync <issue> --retry-updates` and every background delivery pass
failed for LOO-403, LOO-515 and LOO-558. Pending and uncertain updates on them
could not be sent: the pre-delivery read (`IssueOwnership`) was refused by
`put_item` (`rust/loopflow/src/linear/acceptance.rs`) before the mutation.

## Causal chain

1. **Refusal.** `put_item` raises `InvalidData` when a read has the stored
   `revision` but any compared field differs.
2. **The field that differed was `branch_name`.** Linear changed how it builds
   `branchName` (now ≤60 chars, cut at a word boundary) without bumping
   `updatedAt`. Store backup `~/.lf/incident-backups/codex-resume-20261108/loopflow-before.db`
   (Nov 8 03:31), compared with the live store after the repair:

   | Task | stored (Nov 8 backup) | Linear on Nov 12 |
   |---|---|---|
   | LOO-403 | `jack/loo-403-make-the-mac-apps-shortcuts-findable-and-put-commands-in-the` (73) | `jack/loo-403-make-the-mac-apps-shortcuts-findable-and-put` (57) |
   | LOO-515 | `…-brief-views-useful-beside-the` (72) | `…-brief-views` (54) |
   | LOO-558 | `…-hand-tasks-over-without` (73) | `…-hand-tasks` (60) |

   Every other compared column (name, url, state, completed, team, project,
   claimed project, completed_at, due_date, description length) matched. Assignee
   matched too: LOO-515 null, LOO-558 Jack's id. The assignee suspicion
   is not supported, and LOO-403's stored description equals the first 41 lines
   of Linear's current text (the rest is the local change that landed after the
   repair).

   pm_items `branch_name` length by observation date: through 2026-11-06 the max
   is 73 and most rows are over 60; from 2026-11-08 on the max is 55–60 and none
   are over 60. Rows re-read at a *newer* revision since Nov 8 show the same
   shortening with an unchanged title (LOO-402 71→60, LOO-715 69→56, EXP-13
   72→58, MTG-249 72→60…). So the change is Linear-wide and happened between
   Nov 6 and Nov 8.
3. **Why it wedged instead of healing.** `put_item` treats every column as
   authored at `updatedAt`. `branchName` (like `url`) is derived from Linear
   workspace settings and the title, so Linear can re-render it with no new
   revision. Same revision + different value was classed as invalid data.
   Nothing marks the row stale, and `needs_refresh` doesn't skip the check.
4. **Why it kept costing.** Each delivery pass re-read all three issues and
   failed again. `linear-requests.jsonl` holds **2,726 `IssueOwnership` reads of
   these three issues** between 2026-11-11 04:00 and the repair at 08:07 on
   Nov 12, 20–80 per issue per hour, always together and with no
   `DeliverTaskField` between them. The first `DeliverTaskField` for each came
   1 s after the repair. This load ran alongside the "Linear rate limit
   reached" failures on LOO-732/753/774/795/798/799, but nothing here shows
   it caused them.
5. **Why the message misled.** "refresh planning" came in with #1354
   (2026-10-01) and #1491 (2026-10-12) as prose. It was never a command.

## Exposure

711 pm_items rows (224 incomplete) still hold a >60-char branch name stored
before the change: loopflow 500, manabot 148, kata 24, etude 22, etude-studio 9,
hootro 7. With the old check, any re-read of one at its current revision
wedges: a pending field edit, ownership read, project order or inventory.
"Later read wins" covers all of them on their next read, with no migration.

## Prevention

- Fixed on this branch: the equal-revision read is accepted as the newer
  observation, for any field Linear re-renders, not just `branch_name`.
  `shortened_branch_name_at_the_same_revision_replaces_the_stored_one` replays
  a stored long branch name at Linear's revision. Pending `task_changes` are
  not touched, so a local edit stays queued.
- Not pursued: a narrower carve-out for derived columns (`branch_name`, `url`).
  It would keep the refusal for authored fields, but it only works if we know
  which fields Linear re-renders, and this incident shows we can't know that
  ahead of time.
- A refusal that would repeat every pass should not be retried forever. With
  the refusal gone, this specific loop can't recur. Watch for other persistent
  errors on the delivery pass the same way: `lf session history linear` per
  target, looking for reads with no mutation.

## Open

- Whether inventory syncs (`accept_linear_inventory_in`, one transaction) also
  failed on these rows between Nov 8 and Nov 12. The request log isn't detailed
  enough to say.
