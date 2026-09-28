# Open questions and assumptions (LOO-329)

2026-09-28. Design:
[Subwaves: a Wave, its parent, and its address](define-durable-subwave-identity-and.md).
Review notes: [subwave-identity-review.md](subwave-identity-review.md).
LOO-298 owns `scratch/questions.md`; this file is LOO-329's.

## For Jack Heart

1. **May a parent differ from the enclosing directory.** If not, discovery
   writes the parent from the tree and the column only records it.
2. **The API for a Task's Wave.** With ids kept, rename no longer touches
   Tasks, so the choice is about reads only.
3. **Second Home.** Rows are local and ids are random. Two Homes discovering
   the same files must agree on the id.
4. **`lf wave relocate`:** shrink to one-row updates, or replace with a
   directory move that discovery records.
5. **Cron Runs:** the directory recorded at sync, or a fresh worktree per
   firing.
6. **The app's Wave page:** through Tasks only, or what has landed on the
   default branch.
7. **Release's objective text.**
8. **Explicit reads of child memory** by `update-wave` or chapter review.
9. **Wave file name:** `GOAL.md` or `<NAME>.md`.
10. **The wording of the short note** that rides with Wave files.
11. **LOO-298's scratch** in this branch's prompts, about 705 KB.
12. **Repository key:** a per-machine path today.

## Corrections owed to LOO-330 and LOO-331

Both drafts describe held scopes, address history and an index of child
memory in a parent's prompt. None of the three is in this design. No message
was sent to either worker.

## Settled in review, 2026-09-28

Several were reached after a reversal; the review notes keep the order.

- Wave UUIDs are kept.
- A Wave stores its parent's id. The slash path is input syntax.
- Renaming a parent does not change its children.
- Rename does not change history.
- A prompt reads every top-level `.md` of the Wave's directory and each
  ancestor's, memory included, from the Run's own checkout. No children, no
  siblings. `scratch/` stays recursive.
- Memory is a regular Wave file with light hinting.
- No product experience requires the main checkout's `wave/`.
- Release becomes a real Wave, as the prototype of the child Wave setup.
- This work is based on LOO-298.

## Withdrawn in review

- Wave name as identity, and with it: a reused name being the same Wave, a
  Wave name stored on Tasks to be updated on rename, and dropping
  `parent_wave_id`.

## Unresolved evidence

- Whether discovery on LOO-298 creates a row for a nested directory, and with
  what name.
- Whether any Home holds a non-null `parent_wave_id`.
- How a second Home learns a Wave's id on LOO-298.
- Cron label derivation was reported by a delegated search at `632b67ec7` and
  not re-read on LOO-298.
