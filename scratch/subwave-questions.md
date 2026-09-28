# Open questions (LOO-329)

Design: [Subwaves](define-durable-subwave-identity-and.md).
LOO-298 owns `scratch/questions.md`; this file is LOO-329's.

## For Jack Heart

5. The wording of the short note that rides with Wave files.
6. `GOAL.md` or `<NAME>.md`.
7. May `realign` at a parent read its children when asked?

## Settled

- A parent's objective may include subwave goals. Its KRs and Tasks should
  not focus on a subwave's work; mentioning it is fine.
- Scheduled and ad hoc Runs read the same way: the Wave from the Task or
  `--wave`, the files from the checkout the Run is in.
- A Task reaches its Wave through its Project, by id, as LOO-298 has it.
  Jack Heart asked only that it be done right. A Wave stored on the Task is
  added when a read needs it, with LOO-298's check.
- A Wave's parent is the Wave whose directory it sits in.

## Requested by Jack Heart, outside this Task

2026-09-28, in this review:

- `update-wave` goes away.
- `record-learnings` probably goes away too.
- The `queue` Flow becomes compress → refresh. Today it is compress →
  update-wave → gate. Whether gate stays was not said.
- `LOOPFLOW.md` names only `realign` for curating Wave memory.

This cannot be done on this branch: `realign` and the `refresh` Flow exist
only on main. On main, `update-wave` is referenced in 10 source and doc files
and 6 goldens; `record-learnings` in 5 files and 5 goldens, including the
`ship` and `ship-demo` Flows. No Task has been filed.

## Owed to LOO-330 and LOO-331

Both drafts describe memory-only scopes, address history, and an index of
child memory in a parent's prompt. None is in this design. No message was
sent to either worker.

## Not checked

- Which of Infrastructure's current KRs and Tasks are mainly release work.
- What discovery on LOO-298 records for a nested directory.
- Whether any Home has a parent recorded for any Wave.
- The cron log path problem was read, not run.
