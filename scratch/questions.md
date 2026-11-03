# LOO-500 decisions and assumptions

Jack Heart's accepted direction (2026-10-28):

- Remove retires recursively. Open Tasks move to the removed subtree's parent
  or repository; started Tasks always move. `--delete-backlog` deletes only
  unstarted Tasks through `task_delete`. Other destinations use `lf task move`.
- Rename/remove update authored files on main, rewrite `.lf/wave/<name>` path
  references, and leave bare-word prose unchanged. Removal folds child memory
  and missing config keys into the parent. Rename renames the Initiative;
  removal archives it.
- Git planning readers on released lf ≤ 0.15.0 may fail loudly on the new
  `wave_retired` edit until upgraded. Skipping unknown edits was rejected during
  implementation: republishing would silently discard a newer peer's removal.

Reversible implementation choices, retained for demo/review:

- Remove keeps chapters, completed Tasks and Wave Sessions readable; re-add
  mints a fresh Wave. No restore verb. `ensure` is deleted, not aliased.
- `commit_selected` commits only lifecycle paths on main and preserves unrelated
  dirty files. This supersedes kickoff's dirty-main/no-commit assumption. A
  detached HEAD or failed commit leaves files uncommitted in the receipt.
- `task create --wave <new>` and `connect <new>` add first through the same
  lifecycle. Connect's binding files also commit selectively on main; refresh
  inventory reads an existing Wave. No second creation path.
- Removal folds the subtree's memories but only the root removed Wave's config
  keys, excluding `id`/`linear`. Archive failure keeps files containing the
  Initiative binding. The retry journal also retains original descendant IDs
  and bindings; later config changes never redirect an outstanding effect.
  Machine-local operations retain exact authored images before applying effects.
  The design's dated counterexamples record why live rows/paths were insufficient.
- Automatic authored registration skips a retired config id and its subtree;
  explicit add/connect may create a fresh identity by name. A live config id
  wins over a stale directory name.
- Desktop lists store rows only. Authored directories register on explicit
  writes (`add`, `connect --all`, portfolio selection), never on plan reads.
  Folder-mode selection registers its discovered repositories then; a later
  clone registers on its first explicit write or the next portfolio change.
- Known pre-existing limitation: id-first matching does not check repository;
  a second clone on the same Machine attaches to the first clone's Wave rows,
  which path-keyed listing hides in the second clone. Separate work if needed.
- Default `wave list` hides retired rows, keeping legacy abandoned rows visible
  for removal. No `--current` or removed-Wave inventory flag; `show <name|id>`
  reads history. A reused name selects the new Wave; the old id still reads history.
- `rename <wave> [<new-name>]` preserves `--repo`/`--title`; cross-repository
  relocation still leaves files to move manually. Taken names always refuse,
  including registration-only shadows. Default Initiative title is the
  title-cased new slug. Changing the relocation surface is separate work.
- Remove previews name uncommitted edits inside the directory, which removal
  deletes; only committed files remain in Git. No additional refusal.
- Desktop closes clean receipts immediately. Add/rename summaries and remove's
  typed leftovers (archive error, uncommitted files) keep the
  sheet open with Rust's sentences and Done. Remove offers no delete-backlog UI.
- Desktop add uses `Write::WaveAdd`, recording the same Process as the CLI.
  Rename/remove still launch `lf`; moving them into `Write` is follow-up work.
  `WaveAdded.project` mirrors only id/name; the planning stream owns full Projects.
- Main's migration layout is `src/store/migrations/wave_removal.sql` (one Task
  migration); its upgrade test uses `apply_before_migration`/`apply_pending_migrations`.
  Served-write equivalence seeds/commits the Wave once before comparing repeat adds.

The detailed implementation/review chronology remains at
`fe5e2fab42776f23678bdc13eb8d1c01ead6e22e:scratch/questions.md`.

2026-11-02 retry implementation assumption: an unfinished matching remove/rename
request names its saved operation even if its original address has been reused.
Use the replacement's explicit ID for a new operation. Machine SQLite owns the
local effect journal; completed records prove prior identity but never replay effects.

2026-11-03 native review: Jack Heart rejected prose-mention warnings entirely.
Remove the scan, DTO field and receipts; preserve ordinary prose without
reporting it. Explicit authored path rewrites remain. Jack also reported New
Wave disappearing on repository selection and requested comparison with Linear,
Figma and Slack for long lists. Jack Heart accepted a permanent New Wave footer below the scrolling list
and removal of prose warnings (“ok sounds good”). No merge instruction was given.
