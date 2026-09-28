# Open questions (LOO-329)

Design: [Subwaves](define-durable-subwave-identity-and.md).
LOO-298 owns `scratch/questions.md`; this file is LOO-329's.

## For Jack Heart

5. The wording of the short note that rides with Wave files.
6. `GOAL.md` or `<NAME>.md`.
7. May `update-wave` at a parent read its children when asked?

## Settled

- A parent's objective may include subwave goals. Its KRs and Tasks do not
  mention a subwave's work.
- Scheduled and ad hoc Runs read the same way: the Wave from the Task or
  `--wave`, the files from the checkout the Run is in.
- A Task reaches its Wave through its Project, by id, as LOO-298 has it.
  Jack Heart asked only that it be done right. A Wave stored on the Task is
  added when a read needs it, with LOO-298's check.
- A Wave's parent is the Wave whose directory it sits in.

## Owed to LOO-330 and LOO-331

Both drafts describe memory-only scopes, address history, and an index of
child memory in a parent's prompt. None is in this design. No message was
sent to either worker.

## Not checked

- Which of Infrastructure's current KRs and Tasks are release work.
- What discovery on LOO-298 records for a nested directory.
- Whether any Home has a parent recorded for any Wave.
- The cron log path problem was read, not run.
