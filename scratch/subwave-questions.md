# Open questions and assumptions (LOO-329)

2026-09-28. Design:
[Subwaves: Wave name as identity](define-durable-subwave-identity-and.md).
Review notes: [subwave-identity-review.md](subwave-identity-review.md).

## For Jack Heart

1. **What Run history does on rename.** On LOO-298 Runs are rows that
   reference their Wave. History stays with the retired old name, or follows
   the Wave to the new one.
3. **What crosses the subwave boundary besides memory ancestry and
   navigation.** Home placement, budget and parent chapter review are
   undecided.
4. **Release's objective text.** Release becomes a Wave; its Wave file needs
   an objective Jack authors or approves.
5. **Does `lf wave relocate` survive** as a convenience over move, rediscover,
   move Project, retire.
6. **Repository key.** `waves.repo` is a path and differs per Home. The key
   beside the name could be the provider repository identity.
7. **Wave file name.** `GOAL.md` or `<NAME>.md`, and whether a structured file
   absorbs `MEMORY.md`.
8. **Explicit reads of child memory.** Jack answered that prompts include
   ancestors' Wave files and memory, and no children or siblings. Whether
   `update-wave` or chapter review at a parent may read children on request
   is still open.

11. **What Wave-level surfaces read when there is no Task.** Cron, chat,
    discovery and the app's Wave page have no Task checkout. Jack's rule is
    that no experience should require main's copy.

12. **The API for a Task's Wave.** Jack is unsure of the right one. LOO-298
    fills ancestors in constructors and validates with triggers.
13. **LOO-298's scratch in this branch's prompts.** About 705 KB rides every
    Run here.

## Unreconciled between LOO-330 and LOO-331

- Both proposed an index of child memory in a parent's prompt. Jack's answer
  is no children.
- How deep an explicit union for curation reads, if one exists.

Both drafts also still describe held scopes and address history.

## Assumptions made to keep moving

- "No surviving renames" was read as: historical records stay under the old
  name and nothing resolves an old name to a new one.
- "Treated as separate" was read as: a subwave has its own Initiative, chapter
  Project, crons and chat.
- No message was sent to the LOO-330 or LOO-331 workers.
- Nothing was run against the installed Home. No branch binary was built.

## Unresolved evidence

- The complete list of tables referencing `waves(id)`.
- Whether any store holds a retired row and a live row with the same name.
- Whether any Home other than the installed one holds a non-null
  `parent_wave_id`.
- Cron, journal and Run-subject derivations were reported by a delegated
  search and not re-read.

## Settled in review, 2026-09-28

- Wave name is the identity; no Wave ids are tracked.
- Parentage is implicit in the name; `parent_wave_id` is dropped.
- A prompt reads every top-level `.md` of the Wave's directory and each
  ancestor's, memory included; no children, no siblings.
- `scratch/` stays recursive.
- A reused name is the same Wave and inherits the history recorded there.
- Wave files are read from the Run's own checkout; there is no single Wave
  memory.
- No product experience requires the main checkout's `wave/`.
- Memory is a regular Wave file with light hinting. The exact note is proposed,
  not confirmed.
- Release becomes a real Wave, as the prototype of the child Wave setup.
- A Wave rename updates the Wave name on its Project; the Project survives.
  Merging into an occupied Wave is not designed.
- A Task records its Wave's name directly, validated against its Project.
