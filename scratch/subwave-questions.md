# Open questions (LOO-329)

Design: [Subwaves](define-durable-subwave-identity-and.md).
LOO-298 owns `scratch/questions.md`; this file is LOO-329's.

## For Jack Heart

- The wording of the short note that rides with Wave files.
- `GOAL.md` or `<NAME>.md`.
- May `realign` at a parent read its children when asked?

## Skills cleanup, delivered separately

Jack Heart requested on 2026-09-28: remove `update-wave` and
`record-learnings`; `queue` becomes compress → refresh → gate; `LOOPFLOW.md`
names only `realign`. He authorized a new worktree and landing without his
review, through the queue.

The change is on branch `jack-heart/retire-update-wave`, worktree
`/Users/jack/src/loopflow.retire-update-wave`, based on main at `c512813b5`.
It is not part of LOO-329.

No Task was filed. `lf task create --wave infrastructure` failed with "Wave
infrastructure has no chapter", and the `lf` on this session's PATH (0.12.24)
also reports "no Task exists" for LOO-329, LOO-330 and LOO-331. The store
that owns this session's Task was not found from this shell.

## Owed to LOO-330 and LOO-331

Both drafts describe memory-only scopes, address history, and an index of
child memory in a parent's prompt. None is in this design. No message was
sent to either worker.

## Not checked

- Which of Infrastructure's current KRs and Tasks are mainly release work.
- What discovery on LOO-298 records for a nested directory.
- Whether any Home has a parent recorded for any Wave.
- The cron log path problem was read, not run.
