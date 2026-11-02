# LOO-500: Add and remove Waves seamlessly

Status: accepted experience; retry repair extended on 2026-11-02 to reconcile
the index after a successful commit and failed reset; focused regressions passed.
Verification after integrating main reproduced removal and rename retry failures;
retained operation evidence now replaces those recovery paths. Three concept reviews closed the alternate
creation path (`lf wave connect <new-name>` adds first). Merged with main at
`4e9ade2d04`. Happy-path CLI demo done; Desktop demo awaits a display session.
Jack Heart filed LOO-500; exact controls,
required fields and the meaning of deletion were left as design questions.
Choices below are the plan's; reversible ones are noted in
`scratch/questions.md`.

## Problem and current implementation

Jack wants adding and removing Waves to be ordinary, discoverable actions in
Desktop and the CLI, preserving existing work and making interrupted operations
retryable. The branch now shares `ops::wave_lifecycle`, registers new Waves and
their first Projects, commits authored files on main, and exposes add, rename
and removal in Desktop. The retired `ensure` command and Desktop filesystem
creation/placeholder paths are gone. Original diagnosis remains at
`fe5e2fab42776f23678bdc13eb8d1c01ead6e22e:scratch/jack-loo-500-make-adding-and-deleting-waves-seamless.md`.

Recovery now retains original identities, provider obligations and exact authored
changes before effects. The dated counterexamples below explain the repair;
broader acceptance, live Linear recovery and native Desktop interaction remain.

## Demo

On `lf-world wave-lifecycle` (LOO-535 built it for this Task: `research`
empty, `desktop` with open/started/completed Tasks and a Wave conversation,
`archive` with only completed work):

1. Desktop sidebar → **New Wave…** → name `ops`, optional objective → the `ops`
   row appears selected, its workspace opens on the Wave conversation; back to
   the repository row, no refresh.
2. `research` → **Remove Wave…** → sheet says "research has no Tasks or
   Sessions." → Remove → row disappears.
3. `desktop` → **Remove Wave…** → sheet: "3 open Tasks move to the repository
   (1 started keeps its worktree, PR and running Graph). 1 completed Task and
   1 chapter stay readable in history. The Wave conversation stays in
   history." → Remove → the three Tasks appear under the repository; the
   started one is still running.
4. CLI: `lf wave list` agrees; `lf wave show desktop` shows the removed Wave
   with its chapter history.
5. Re-run `lf wave remove desktop` → "desktop was already removed" exit 0.
   `lf wave add ops` again → "ops already exists" exit 0, no duplicate.

## Chosen approach

### One shared lifecycle

```
lf wave add <name> [--objective <text>] [--json]
lf wave remove <name> [--preview] [--delete-backlog] [--json]
lf wave rename <name> <new-name> [--json]
```

- `add` replaces `lf wave ensure` (deleted, not aliased). `ensure`'s
  Project-activation step stays inside `add`: a new Wave gets its first
  Project so planning works immediately.
- Both live in `ops::wave_lifecycle` (new module, replacing the creation half
  of `ops::project::ensure`). The CLI and Desktop call the same op: Desktop's
  add runs through the in-process `Write::WaveAdd` (recorded as `lf wave add`
  would be); rename and remove launch `lf wave rename|remove … --json`.
  Folding those two into `Write` is ordinary follow-up work, not part of this
  Task.

### Add

1. Resolve the repository's **main checkout** (`CanonicalRepo`), never the
   invoking worktree, so the authored files land where every later launch reads
   them.
2. `SqliteStore::ensure_wave` registers the row and seeds files (matching an
   authored directory by its `config.yaml` id first; see "Other checkouts lag
   the store"); `--objective` replaces the seeded `GOAL.md` objective text.
3. Ensure the first Project (existing `ensure_project`).
4. Commit `.lf/wave/<name>/` with `commit_selected` (`lf wave add: <name>`),
   through the lifecycle's `commit_authored`. Unrelated dirty files stay intact.
   A non-branch checkout or failed commit leaves the files uncommitted and the
   receipt says so; the Wave is still usable.
5. Idempotent: an existing live Wave at that address returns
   `{created: false}` and exit 0.

Name validation reuses `normalize_wave_name` plus `ensure_wave`'s component
check; no new name rules. Stray registrations like Jack's `list` are cleaned
up with `remove` itself.

### Remove = retire, with explicit disposition of open Tasks

Removal **retires** the Wave (`retired_at`, `retirement_reason = "removed"`,
`work_state = abandoned`); it never deletes rows. Chapters (Projects), KRs,
completed Tasks and Wave Sessions stay readable through `lf wave show <name>`
and the Wave's history — GOAL.md requires historical chapters stay readable.

Required sequence, with durable retry evidence described below:

1. **Preview** (also `--preview`, and what Desktop's sheet renders): open Tasks
   split into unstarted vs started (with live Graph counts), completed Tasks,
   Projects, Wave Sessions, child Waves, Linear Initiative binding, and whether
   the main checkout has the authored directory.
2. **Dispose of open Tasks.** Jack Heart (2026-10-28) decided:
   - Removal is recursive: the Wave and every child Wave retire together.
   - Default: open Tasks of the removed subtree move to the removed Wave's
     parent Wave, or to the repository (no Wave) when it is top-level, through
     the existing `ops::task::task_move_membership`.
   - Started Tasks (worktree, PR or Graph) always move; no flag deletes them.
   - `--delete-backlog`: unstarted Tasks in the subtree are deleted through
     the existing `ops::task::lifecycle::task_delete` (same as `lf task
     delete`, trashing the Linear issue) instead of moving. The preview lists
     each Task that will be deleted.
   - No `--move-to`: moving Tasks to another Wave belongs to `lf task move
     --wave`, before or after removal.
   On the default path: Identity, worktree,
   PR, Workflow position and execution are untouched (AGENTS.md: started
   unfinished Tasks move intact). This is the explicit disposition the backlog
   rule asks for.
3. **Retire** the Wave and its child Waves (recursive) in one transaction through a new
   portable planning edit `WaveRetired(Option<i64>)` (sibling of
   `TaskDeletedAt`), so Git planning exchange (LOO-412) carries removal to
   peers instead of them re-creating it.
4. **Archive the Linear Initiative** of each retired Wave that has one
   (Jack Heart, 2026-10-28), via a new `initiativeArchive` mutation beside
   `UPDATE_INITIATIVE_MUTATION` in `linear/mod.rs`. Runs after local
   retirement; a Linear failure leaves the Wave retired and the receipt says
   the Initiative is still live. Re-running `lf wave remove` retries outstanding
   archives before finishing authored cleanup. Files stay until archival succeeds;
   the saved operation retains every original descendant binding independently
   of those files. Unconnected repositories skip this step.
5. **Remove authored files** `.lf/wave/<name>/` from the main checkout and
   commit (`lf wave remove: <name>`), same rules as add.
6. Receipt: what moved or was deleted, what was retired, what stayed, which
   Initiatives were archived, and anything left (uncommitted files, archive
   failures).

`wave_retirement_blockers` is replaced by the preview: blockers become
consequences. Only one condition still refuses: a Task that cannot move
(its move errors), which the receipt names; nothing is retired until all moves
succeed, so a retry resumes from step 2.

### Rename

Jack Heart (2026-10-28): `lf wave rename` should exist. Same op module and
commit-on-main path as add/remove:

1. Validate `<new-name>` with `normalize_wave_name`; an existing live Wave at
   that address refuses with its name.
2. Write the existing portable `WaveName` planning edit (exchange already
   carries it) — identity, Projects, Tasks, Sessions and history are
   untouched; only the address changes.
3. Apply the saved file move and reference rewrites on the main checkout and
   commit (`lf wave rename: <name>`) through the saved-image path below.
   The private Git index excludes unrelated changes and preserves later edits.
4. Required idempotency: already at `<new-name>` with the same id → exit 0
   after reporting or finishing pending effects. Saved operations establish
   the old identity; a live target alone never proves a prior rename.
5. A connected Wave's Linear Initiative is renamed through the existing
   `LinearClient::rename_wave` (already used by `ops::linear`), so the name
   stays consistent; a Linear failure is reported and retried like removal's
   archive.

Desktop: **Rename Wave…** beside Remove in the same context menu and titlebar
actions, calling `lf wave rename --json`. Selection follows the Wave's id.

### Repository files follow the lifecycle (Jack Heart, 2026-10-28)

Rename and remove also update the repository's authored files on the main
checkout, in the same commit as the directory change:

- **References.** Rewrite structured references to the old address:
  child Wave directories (path-nested, so `git mv` carries them), and
  `.lf/wave/<name>` paths in other Waves' `GOAL.md`/`MEMORY.md`. Prose that
  merely mentions the bare word (e.g. "product") is not rewritten — Jack's
  written text stays word for word; the receipt lists remaining prose
  mentions with file:line so a person or agent can edit them.
- **Remove folds up.** A removed child Wave's `MEMORY.md` is appended to its
  parent's `MEMORY.md` under `## From <name> (removed <date>)`; its
  `config.yaml` keys the parent lacks (`graph`, `chat`, …) are copied, never
  overwriting the parent's; `id` and `linear` are not copied. A top-level
  Wave has no parent: its files are deleted and stay in Git history and the
  retired Wave's history.

### Other checkouts lag the store (Jack Heart, 2026-10-28)

The planning store changes immediately for every checkout; the authored files
change only on main, so Task worktrees and branches keep the old
`.lf/wave/<name>/` until they rebase. Today `ensure_wave` matches by path:
after `product → prod`, a stale checkout's `.lf/wave/product/config.yaml`
(id X) finds no live `product`, then tries to create X again and fails (or,
after removal, would resurrect the address). Fix: authored directories are
matched **by `config.yaml` id first**:

- id is a live Wave → use it; the store's name and parent win, no edit is
  written (a stale path never renames back).
- id is a retired Wave → automatic registration skips that directory and its
  subtree quietly. Explicit `add` registers a fresh identity at the address;
  neither path revives the retired identity.
- no id / unknown id → today's registration.

Branch edits to a renamed Wave's files follow `git mv` on rebase; edits to a
removed Wave's files surface as ordinary rebase conflicts.

### Removed Waves stay removed

- `ensure_wave` skips an authored directory whose id is retired (see above).
- `lf wave add <name>` after removal mints a fresh identity (the address is
  free because readers already exclude `retired_at`).
- Linear: removal archives (never deletes) the Initiative; Linear sync stops
  reading retired Waves.
- `lf wave list` lists live Waves, the same rows Desktop shows; a removed Wave
  is read by address or id with `lf wave show` (second concept review,
  2026-11-02). Desktop's `Wave` row therefore carries no retirement fields and
  no "Retired at" lens; the wire `WaveSnapshot` keeps them for `lf wave show`.

### Desktop

Entry points reuse the controls that already exist (LOO-489 owns the layout;
this Task makes them work and adds nothing beside them):

- `NewWaveRow` (sidebar `WorkNavigator.waveSection`) and `CreateWaveSheet`
  (Waves window) expose an optional Objective field and call
  `RegistryQuery.addWave` → `Write::WaveAdd`, then
  select the new Wave once the `desktop plan --all --watch` stream (which
  re-reads on `store_revisions`) delivers it. `refreshPlanning()` after the
  command stays as the immediate nudge.
- `WaveRow.onDelete` is passed in both the sidebar Wave rows and
  `WavesView`; the item is renamed **Remove Wave…** and also appears in the
  Wave workspace titlebar actions. It opens a sheet that
  renders `lf wave remove --preview --json` verbatim (counts and sentences come
  from Rust so CLI and Desktop say the same thing) and a destructive Remove
  button. No destination picker: open Tasks go to the parent or repository;
  moving them elsewhere uses the ordinary Task move. Desktop exposes no
  `--delete-backlog`; that option stays in the CLI.
- Failures show the receipt's error in the sheet and keep it open with Retry;
  a partial receipt (Tasks moved, retirement failed) says exactly that. A
  successful receipt with leftovers (an Initiative Linear kept live, files
  left uncommitted) stays open with the CLI's lines and a Done button.
- If the removed Wave is selected or open in the workspace, selection moves to
  the repository row; open Task panes stay open (their Tasks still exist).

## Risk findings

- Wave rows already have retirement columns; the `wave_addresses`
  live readers exclude retired rows (`get_wave_at`,
  `find_waves_by_slug`). Retirement is the supported mechanism.
- Implementation finding (2026-10-28): the `waves` CHECK required every retired
  row to name `superseded_by_wave_id` and a reason, so successor-less removal,
  and a peer importing only `retired_at`, both violated it. The Task's one
  migration `src/store/migrations/wave_removal.sql` rebuilds `waves` (same
  column order, indexes, triggers and `wave_addresses` view) with: live rows
  carry no successor/reason; retired rows may omit both. The upgrade test
  `wave_definitions::released_waves_upgrade_and_removal_needs_no_successor`
  seeds the released schema with a superseded retired row.
- The former `ops::project::ensure` created a Project for every Wave, making
  `wave_retirement_blockers` refuse every real Wave. Both have been replaced.
- Planning exchange now carries `WaveRetired` alongside `WaveName`/`WaveParent`,
  preventing peers from re-asserting a removed Wave. Finding: `WorkValue` is an internally tagged
  enum with no `#[serde(other)]`, and `PlanningSnapshot` deserializes every
  change, so a released lf (≤ 0.15.0) reading a document that contains
  `wave_retired` fails the whole Git planning read. Decision: add the variant;
  peers on 0.15.0 fail loudly until upgraded (exchange users are Jack's own
  Machines; solo-dev rule: detect skew and name the fix). The kickoff idea of a
  reader that skips unknown edits was dropped during implementation: an older
  reader that republished a snapshot without them would silently drop a newer
  peer's removals. Recorded in `scratch/questions.md`.
- The authored directory is shared Git content. Committing on the main
  checkout is what `lf wave connect` already does; Desktop targets the main
  repo (project memory), so this matches where Desktop runs.

## Alternatives considered

- **Hard delete rows** (`delete_wave`). Loses chapter history GOAL.md requires
  and breaks Task `project_id` references; exchange peers would re-create the
  Wave. Rejected.
- **Refuse removal while Tasks exist** (today's blockers). Pushes the user to
  move Tasks one by one — exactly the manual cleanup LOO-500 removes.
- **Abandon open Tasks with the Wave by default.** Destroys unreviewed
  backlog without disposition. Deletion is the explicit `--delete-backlog`
  choice instead; moving to another Wave stays `lf task move`.
- **Keep `ensure` and add `remove` beside it.** Leaves the misleading
  Project-activation name as the creation path. Rejected.

## Scope

In: `lf wave add|remove|rename`, the lifecycle op, `WaveRetired` edit and its
exchange application, id-first authored registration, Desktop New/Remove
sheets, lf-world demo, docs/README for wave commands.

Out: navigation layout (LOO-489), subwave addressing, restoring a removed Wave (re-add mints a fresh Wave; restore is a
later Task if wanted), cleanup of other branches' stale `.lf/wave/` copies.

## Delete — do not maintain

Retry repair deletes live-subtree reconstruction after retirement, target-only
rename success, transient authored-path vectors as recovery evidence, and
`LinearRenameOptions`/`LinearRenameResult` plus the address-resolving provider
rename. The surviving provider mutation accepts the captured Initiative ID.
The durable operation below owns those effects; no second Wave state is added.

Compression removes `commit_authored`'s deleted-path discovery (only add/connect
still call it), duplicate live-subtree walks and duplicate Git blob readers.
Removal/rename keep their saved-image commit path; its base/index reads select
literal filenames and read the exact object returned by Git. The split-wave
recipe uses the child's full address throughout and the first Project add creates.
No obsolete lifecycle implementation remains to delete.

The post-commit repair removes index reconciliation from `commit_images`'s
commit-only branch. The operation's saved index/after images now reconcile all
selected paths even when HEAD already matches, under the same mutation lock.
Later staged edits and paths owned by replacement Waves remain untouched.
Recovery selects files and pending commits with direct loops; retries no longer
rewrite the saved `applied` state. The reset-failure helper only asserts the
failure receipt, since neither caller consumes it. Removed the obsolete
`scratch/concept-review.html`; its exact historical reference remains below.

Previously removed with their exclusive tests:

- `WaveCommand::Ensure`, `ops::project::ensure`, `WaveEnsure`/`ensureProject`;
  `add` owns creation and first-Project activation in CLI, Desktop and lf-world.
- `PortfolioRepoState.createWave`, `PortfolioRepoError`,
  `PortfolioDiscovery.authoredWaves`, both `authoredWavesByRepo` caches,
  `authoredPlaceholder`, `isRegistered` and Swift retirement-only rows/lenses.
- `delete_wave`, `wave_retirement_blockers` and relocation's collision retirement.
- Connect's direct `ensure_wave_row` creation and whole-tree commit;
  `--current` and all its callers; skills' manual Wave-file setup.
- `lf/commands/wave_rename.rs`: rename dispatch sits beside add/remove in `lf.rs`.
- `wavePlanKey`, `waveSelectionId`, the placeholder-target dictionary and repeat
  sorting in `WavesView`: selection and plans use the registered Wave ID.

The surviving store list already excludes retired rows; its callers need no
second filter. Folded files select parent `MEMORY.md` and `config.yaml` for
commit, never the whole parent directory (which includes unrelated sibling work).
Removal, rename and add share the uncommitted-files sentence.

## Forbidden outcomes

- An alias from `ensure` to `add`, or both existing.
- A Desktop-only creation path that writes `GOAL.md` itself.
- A second "removed" flag beside `retired_at`.
- A remove that reports success while Tasks remain attached to a retired Wave.

## Done when

- Gate: `cargo test -p loopflow --test integration wave_lifecycle` passes,
  covering: add is idempotent and lands files on main; remove of an empty Wave
  and a repeat remove (by name or old id) reporting `already_removed`; remove
  of a Wave with unstarted, started and completed Tasks moves the open ones
  (started keeps worktree and Graph), retires the Wave, keeps chapters readable
  via `lf wave show`; a partially completed removal (a Task already moved)
  finishes on re-run; a child Wave's Tasks go to its parent and its memory
  folds up; `--delete-backlog` deletes only unstarted Tasks; rename keeps the
  Wave id, Tasks and chapters, moves the authored directory on main, and
  refuses a taken name or an existing directory. Library tests cover the rest:
  `wave_definitions::stale_checkouts_match_by_id_and_never_resurrect_a_removed_wave`
  (`ensure_wave` from a stale checkout neither errors nor renames/resurrects)
  and `planning_peers::a_removed_wave_reaches_a_peer_as_retired_…` (exchange
  carries `WaveRetired`).
- Gate: Swift view tests for the New Wave and Remove Wave sheets render the
  preview DTO (fixture under `tests/fixtures/dto/`), the failure/Retry state
  and the leftovers/Done state, headless.
- Demo: the five steps above on `lf-world wave-lifecycle`, in Desktop and CLI.

## Remaining work

Gate owns affected suites, migration upgrade verification, lint and the design's
broader acceptance checks. The retry implementation now retains Machine-local
operation evidence and has public CLI regressions for the four counterexamples,
replacement identities, changed provider bindings and later staged/working edits.
Provider failure tests use missing repository configuration, so they prove retained
obligations, not successful recovery against live Linear.
The focused suite exercises failed commits and post-commit reset failures, not process termination at every
journal/file/provider boundary; source inspection alone is not that coverage.
Desktop demo steps 1–3 need a display session (demo/review); CLI
steps 4–5 ran on `lf-world wave-lifecycle` (2026-11-02). Headless view tests
prove sheet content and actions, not the complete native interaction.

Completed implementation slices and three concept reviews are preserved at
`fe5e2fab42776f23678bdc13eb8d1c01ead6e22e:scratch/jack-loo-500-make-adding-and-deleting-waves-seamless.md`.
The third review's HTML is historical too: `fe5e2fab42776f23678bdc13eb8d1c01ead6e22e:scratch/concept-review.html`.
Those reviews closed the filesystem placeholder reader, live-list mismatch,
connect's alternate creation path, missing Desktop leftovers and the
uncommitted-edit preview; their happy-path results do not establish retry safety.

## Retry recovery design (2026-11-02)

Engineering decision: retain lifecycle operations in the existing Machine SQLite
store, in the Task's `wave_removal` migration. Each operation pins its root and
subtree IDs, original addresses, destination identity, backlog disposition,
provider IDs/title, and before/after authored file images before any effect.
This is a local effect journal, not portable Wave state or another planning owner.
Retirement/name edits still use the existing planning writer and exchange.

A repository lifecycle lock serializes add/remove/rename. Pending requests resolve
by saved identity/address before live address lookup. Completed rename evidence
requires either an unresolved old address or the original ID still at its saved
target; an unrelated target never proves a prior rename. A fresh explicit ID addresses
a replacement. Completed effects never replay. Provider updates target captured
IDs, never a later config binding. Authored edits are prepared off-checkout, then
persisted before applying, so folds and reference rewrites survive interruption.
Retries apply only unchanged before-images or recognize after-images; replacement
Wave paths are excluded by identity. Atomic per-file replacement avoids an
interruption between deleting and rewriting a file. Files added later to the old
directory remain untouched and keep cleanup pending. Commits use the saved after-images in an
independent index, preserving later working/index edits. Changed committed content
is reported as pending rather than overwritten. Completed records retain identity
evidence for repeated commands. Cross-repository file relocation remains manual.
Index reconciliation is a separate unfinished effect: matching HEAD after-images
skip commit creation, but still reconcile unchanged saved index entries. The
operation becomes file-complete only after reconciliation and directory cleanup.
No new journal field is needed; retained index/after images identify the residue.

## Retry design counterexamples (2026-11-02)

Verified against `fe5e2fab42` with the checkout's `target/debug/lf`, temporary
Git repositories and isolated Machines; no configured provider or live Work
was changed. These contradict Jack Heart's acceptance that failed/interrupted
operations permit retry without presenting partial cleanup as success.

- **Recursive archive retry loses the child.** Add `parent` and
  `parent/child`, give only the child's `config.yaml` a synthetic
  `linear.initiative`, and leave repository `planning.team` absent. The first
  `lf wave remove parent --json` retires both Waves and reports the child's
  archive error, retaining files. Repeat the same command: `initiatives` is
  empty, `files.state` is `committed`, and the child's config is deleted.
  `remove` traverses children only before retirement, and `list_child_waves`
  excludes retired rows. The retry therefore forgets the archive obligation
  and deletes its only remaining Initiative locator. A real archive outage
  follows the same error branch; no provider call was needed to reproduce it.
- **Commit retry hides pending deletions.** Add `research`, detach main's
  HEAD, and remove it. The receipt correctly reports `uncommitted` after
  deleting the directory. Switch back to `main` and repeat removal: the
  receipt reports `unchanged`, but `git status --porcelain --
  .lf/wave/research` still lists all three deletions. `has_files == false`
  bypasses `commit_authored`. For a removed child, the pending commit can
  also include folded parent memory/config and rewritten references outside
  the deleted directory; those paths exist only in the first call's local
  vector.

- **Rename retry hides pending moves.** An isolated CLI probe at `601c5a5b25`
  added `research`, detached main's HEAD, and ran
  `lf wave rename research ops --json`. The first receipt reported `uncommitted`. After switching back to
  `main`, repeating the rename returned `files.state = unchanged` and an empty
  summary while Git still showed three deleted `research` files and an
  untracked `ops` directory. The old-name lookup misses, so rename returns
  before file commit or provider reconciliation. This probe used the existing
  `target/debug/lf`; source inspection confirmed the early return at that revision.
- **An occupied target does not establish a previous rename.** In that same
  fixture, `lf wave rename never-existed ops --json` succeeded with the `ops`
  identity and `renamed = false`. No previous identity links those names.
  A retry must distinguish the original Wave from an unrelated or replacement
  Wave at the target; a live target alone cannot establish completion.
- **Post-commit reset failure leaves staged residue.** The follow-up isolated CLI
  probe against `6cb4adb010` failed `git reset` after removal committed. Retrying
  returned `files.state = unchanged` and “already removed,” leaving all three
  deleted files staged as additions (`AD`). Matching HEAD after-images bypassed
  `commit_images` and its index reconciliation. The repair reconciles selected
  index entries independently; public CLI regressions inject repeated reset
  failures for removal and rename, preserving later staged/working edits and
  replacement Waves without creating another commit.

The accepted outcome stays unchanged. These failures required retry evidence
that survives retirement, rename and directory deletion: retained
original/descendant identities, provider bindings and exact pending authored paths.
This is not another Wave state flag. The Machine-local operation journal now
retains this evidence in the Task's existing migration. Retry tests exercise
both interrupted commits and replacement identities; incomplete provider effects
stay visible. The original counterexamples above retain the evidence that shaped
the repair. Live Linear recovery and native Desktop interaction remain
unproved; isolated Git/CLI failures establish the local recovery gap only.

## Checks

Earlier happy-path and Desktop evidence remains in
`601c5a5b25:scratch/jack-loo-500-make-adding-and-deleting-waves-seamless.md`.
The four original retry probes are retained above; native Desktop and live
Linear recovery still belong to demo/review. This pass's affected owners are
`wave_lifecycle` integration, `store::sqlite::wave_definitions` migration upgrade,
and lifecycle callers in `global_commands`, `served_reads` and
`linear::local_planning`; no Swift or DTO shape changed in the retry repair.

2026-11-02 compress: `TMPDIR=/private/tmp uv run python scripts/test_network.py cargo test -p loopflow --test integration wave_lifecycle:: < /dev/null` passed 14/14; `cargo fmt --all`, `git diff --check` and `lf context` pass; broader affected suites/migration upgrade/lint → gate, native interaction/live Linear recovery → demo/review.
