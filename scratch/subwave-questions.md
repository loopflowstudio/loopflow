# Open questions (LOO-329)

Design: [Subwaves](define-durable-subwave-identity-and.md).
LOO-298 owns `scratch/questions.md`; this file is LOO-329's.

## For Jack Heart

1. May a Wave's parent ever differ from the directory it sits in?
2. How should a Task know its Wave? Jack said he is unsure of the API.
3. Should scheduled Runs stop reading `wave/` from main?
4. Does Infrastructure's objective keep "delivers verified releases" once
   release has its own?
5. The wording of the short note that rides with Wave files.
6. `GOAL.md` or `<NAME>.md`.
7. May `update-wave` at a parent read its children when asked?

## Owed to LOO-330 and LOO-331

Both drafts describe memory-only scopes, address history, and an index of
child memory in a parent's prompt. None is in this design. No message was
sent to either worker.

## Not checked

- What discovery on LOO-298 records for a nested directory.
- Whether any Home has a parent recorded for any Wave.
- The cron log path problem was read, not run.
