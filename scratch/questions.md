# LOO-500 open questions and assumptions

Kickoff assumptions (2026-10-28), reversible; Jack Heart to confirm or redirect.

- **Remove means retire, not erase.** Chapters, completed Tasks and Wave
  Sessions stay readable in history; re-adding the name mints a fresh Wave.
  A "restore" verb is out of scope.
- **No `--move-to` or picker** (Jack Heart, 2026-10-28): moving Tasks to
  another Wave is `lf task move`.
- **Removal is recursive** (Jack Heart, 2026-10-28): child Waves retire with
  their parent, listed in the preview.
- **Open Tasks go to the removed Wave's parent, or the repository**, by
  default (Jack Heart, 2026-10-28). Started Tasks always move;
  `lf wave remove --delete-backlog` deletes unstarted ones via `task_delete`.
- **Add and remove commit `.lf/wave/<name>/` on the main checkout**, like
  `lf wave connect`. A dirty main checkout leaves the change uncommitted and
  the receipt says so.
- **Removal archives the Linear Initiative** (Jack Heart, 2026-10-28); rename
  renames it through the existing `rename_wave` client call.
- **`lf wave ensure` is deleted**, not aliased; `add` takes over creation and
  first-Project activation.
- **Planning exchange skew** (accepted by Jack Heart, 2026-10-28, not a concern now): released lf ≤ 0.15.0 fails to read a Git planning
  document containing the new `wave_retired` edit. Accepted as a loud,
  upgrade-named failure rather than a two-release rollout.

Implementation assumptions (2026-10-28, implement step), reversible:

- **Lifecycle commits select only the Wave's paths** (`commit_selected` on the
  main checkout), so a dirty main checkout still gets the commit and keeps its
  other changes; only a detached HEAD or a failing commit leaves files
  uncommitted (named in the receipt).
- **`lf task create --wave <new>`** adds the Wave through `lf wave add`, so its
  files also land committed on main.
- **`lf wave rename <wave> [<new-name>]`** reshapes the existing relocation
  command: `--name` became the positional, `--repo`/`--title` stay. A taken
  name now refuses; relocation's "retire a registration-only collision" path is
  deleted with `wave_retirement_blockers`. Without `--title`, a connected
  Initiative is renamed to the title-cased new slug (what `connect` names it).
- **Removal folds** the MEMORY.md of the removed Wave and each descendant into
  the parent; only the removed Wave's own config keys are copied.
- **A stale checkout whose directory names a retired id** is ignored for that
  id; an explicit add/connect there registers by name (a fresh Wave), it never
  revives the retired identity.
- **Archive failure keeps the files**: the Initiative id lives in
  `config.yaml`, so files stay until a re-run archives it.
- **No unknown-edit tolerance in the exchange reader** (implement, 2026-11-02):
  skipping unrecognized planning edits would let an older reader republish a
  snapshot without them, silently dropping a newer peer's removals. The loud
  failure Jack accepted is the safer skew, so the design's "skip unknown fields"
  step was not built.
- **Relocation collisions now always refuse** ("<name> already exists; remove or
  rename it first"), including registration-only shadows that relocation used
  to retire; `lf wave remove` the shadow first. Its old test was deleted.
- **Merge with main (sync, 2026-11-02):** main's in-process Desktop write path
  (`Write`/`ServedWrite`) still named `lf wave ensure`, which this branch
  deletes. `WaveEnsure` became `WaveAdd { wave, objective }` running
  `ops::wave_lifecycle::add`; Desktop's `addWave` goes through that write, so
  it records a Process as `lf wave add` would. `renameWave`/`removeWave` still
  launch `lf` (`run`); folding them into `Write` is a follow-up, not a merge
  concern. The `drafts/` migration layout is gone on main: `wave_removal.sql`
  moved to `src/store/migrations/` as a slug migration and its upgrade test
  uses `apply_before_migration`/`apply_pending_migrations`. The served-write
  equivalence fixture adds its Wave once through `lf` first, since a first
  `add` commits the seeded authored files and a repeat prints `unchanged`.
