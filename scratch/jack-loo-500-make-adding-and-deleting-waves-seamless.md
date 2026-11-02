# LOO-500: Add and remove Waves seamlessly

Status: accepted; implemented (reconciled 2026-11-02) through the second
concept review's direction (`lf wave list` lists what Desktop lists). The third
concept review (2026-11-02) found one creation path the design deletes still
open (`lf wave connect <new-name>`); slice 11 closes it. CLI demo done; Desktop
demo awaits a display session. Jack Heart filed LOO-500; exact controls,
required fields and the meaning of deletion were left as design questions.
Choices below are the plan's; reversible ones are noted in
`scratch/questions.md`.

## Problem

Jack wants adding and removing Waves to be ordinary, discoverable actions in
Desktop and the CLI. Today:

- **Adding** is `lf wave ensure <name>`, whose help says "Ensure the configured
  Project is active". It registers the row, seeds `.lf/wave/<name>/`
  (`config.yaml`, `GOAL.md`, `MEMORY.md`) in whatever checkout ran it, and
  commits nothing. A Task worktree run strands the files on that branch.
- **Desktop's New Wave** (`NewWaveRow` in `WorkNavigator.swift`, and
  `CreateWaveSheet` in `WavesView.swift`) calls
  `PortfolioRepoState.createWave`, which writes `GOAL.md`/`MEMORY.md` with
  `FileManager` and never calls `lf`. Nothing registers Waves on read or
  launch, so the new Wave stays invisible until someone runs
  `lf wave ensure`/`connect` — the bug the `wave-lifecycle` world's own Task
  describes. The `init` and `split-wave` skills also hand-write
  `.lf/wave/<name>/GOAL.md` and then run `lf wave connect`.
- **Removing** has no command. `WaveRow` has a "Delete Wave" context item
  behind `onDelete`, which no caller passes. Removed callers of
  `delete_wave`: #1360, #1375; a `lf wave create` existed until `6c4d13269`. The store has `delete_wave` (test-only caller)
  and a retirement write that only `relocate_waves` uses for a
  registration-only collision. `wave_retirement_blockers` refuses any Wave with
  Projects — and `ensure` always creates one, so no real Wave can retire.
  Jack's own `lf wave list` shows `engbot` and `list` (an accidental
  `lf wave ensure list`) stuck as `abandoned`, and `spend` registered with no
  authored directory.
- Removing a Wave's files by hand resurrects nothing visibly but leaves the
  row; restoring a directory whose `config.yaml` id belongs to a retired row
  fails inside `planning_write::create` with an opaque error.

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

### One lifecycle, two verbs

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
4. Commit `.lf/wave/<name>/` with `commit_workflow` (`lf wave add: <name>`),
   the same path `lf wave connect` already uses. A dirty or non-branch main
   checkout leaves the files uncommitted and the receipt says so; the Wave is
   still usable.
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

Steps, each idempotent so a retry finishes partial work:

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
4. **Remove authored files** `.lf/wave/<name>/` from the main checkout and
   commit (`lf wave remove: <name>`), same rules as add.
5. **Archive the Linear Initiative** of each retired Wave that has one
   (Jack Heart, 2026-10-28), via a new `initiativeArchive` mutation beside
   `UPDATE_INITIATIVE_MUTATION` in `linear/mod.rs`. Runs after local
   retirement; a Linear failure leaves the Wave retired and the receipt says
   the Initiative is still live, and re-running `lf wave remove` retries only
   the archive. Unconnected repositories skip this step.
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
3. `git mv .lf/wave/<name> .lf/wave/<new-name>` on the main checkout and
   commit (`lf wave rename: <name> → <new-name>`), same dirty-checkout rule.
4. Idempotent: already at `<new-name>` with the same id → exit 0.
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
- id is a retired Wave → skip that directory quietly (one line in `lf.log`),
  never error, never re-create. This replaces the earlier "retired-id error".
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
  (Waves window) gain an optional Objective field and call a new
  `RegistryQuery.addWave` → `lf wave add <name> [--objective] --json`, then
  select the new Wave once the `desktop plan --all --watch` stream (which
  re-reads on `store_revisions`) delivers it. `refreshPlanning()` after the
  command stays as the immediate nudge.
- `WaveRow.onDelete` is passed in both the sidebar Wave rows and
  `WavesView`; the item is renamed **Remove Wave…** and also appears in the
  Wave workspace titlebar actions. It opens a sheet that
  renders `lf wave remove --preview --json` verbatim (counts and sentences come
  from Rust so CLI and Desktop say the same thing) and a destructive Remove
  button. No destination picker: open Tasks go to the repository, and
  moving them elsewhere uses the ordinary Task move.
- Failures show the receipt's error in the sheet and keep it open with Retry;
  a partial receipt (Tasks moved, retirement failed) says exactly that. A
  successful receipt with leftovers (an Initiative Linear kept live, files
  left uncommitted) stays open with the CLI's lines and a Done button.
- If the removed Wave is selected or open in the workspace, selection moves to
  the repository row; open Task panes stay open (their Tasks still exist).

## Risk findings

- Wave rows already have retirement columns and a `wave_addresses` view that
  excludes retired rows from every live reader (`get_wave_at`,
  `find_waves_by_slug`). Retirement is the supported mechanism.
- Implementation finding (2026-10-28): the `waves` CHECK required every retired
  row to name `superseded_by_wave_id` and a reason, so successor-less removal,
  and a peer importing only `retired_at`, both violated it. The Task's one
  migration `src/store/migrations/wave_removal.sql` rebuilds `waves` (same
  column order, indexes, triggers and `wave_addresses` view) with: live rows
  carry no successor/reason; retired rows may omit both. The upgrade test
  `wave_definitions::released_waves_upgrade_and_removal_needs_no_successor`
  seeds the released schema with a superseded retired row.
- `ops::project::ensure` creates a Project for every Wave, so
  `wave_retirement_blockers` makes retirement impossible for any real Wave —
  it must go, not be extended.
- Planning exchange only knows `WaveName`/`WaveParent`. Without a portable
  retirement edit, a peer's Git exchange would re-assert the Wave. Adding the
  `WaveRetired` edit is required. Finding: `WorkValue` is an internally tagged
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
exchange application, `ensure_wave` retired-id error, Desktop New/Remove
sheets, lf-world demo, docs/README for wave commands.

Out: navigation layout (LOO-489), subwave addressing, restoring a removed Wave (re-add mints a fresh Wave; restore is a
later Task if wanted), cleanup of other branches' stale `.lf/wave/` copies.

## Delete — do not maintain

- `WaveCommand::Ensure` and its dispatcher; `ops::project::ensure` (creation
  moves into `ops::wave_lifecycle::add`; lf-world `World::wave` calls `add`).
- Swift: `PortfolioRepoState.createWave` (both overloads) and the
  `PortfolioRepoError.emptyWaveName/duplicateWave` cases it alone uses, plus
  their `PortfolioRepoStateTests`; `RegistryQuery.ensureProject` becomes
  `addWave` (`WorkModel.activateProject` and `ProjectActivationTests` follow,
  since `add` is idempotent and ensures the first Project). Check
  `PortfolioDiscovery.authoredWaves` (filesystem `GOAL.md` scan): once Desktop
  creates through `lf`, any remaining Wave listing it feeds reads the store.
- `init` and `split-wave` builtin skills: replace hand-written
  `.lf/wave/<name>/` plus `lf wave connect` with `lf wave add` (then `connect`
  only for Linear).
- `lf wave connect <name>` minting a Wave: `connect_async` registers through
  `ensure_wave_row`, which seeds `.lf/wave/<name>/` in the invoking checkout
  with no first Project and no lifecycle commit (third concept review). The
  `ops/mod.rs` comment calling connection "a creation graph" goes with it.
- `SqliteStore::delete_wave` / `Store::delete_wave` and their test-only use.
- `wave_retirement_blockers(_in)`; relocation's collision retirement calls the
  shared retire write instead.
- `lf wave ensure` in `builtins/ops/skill/repo_session.md`,
  `builtins/wave/skill/wave_session.md`, `docs/work.md`,
  `docs/architecture/planning.md` and the `wave-lifecycle` recipe text become
  `lf wave add`. Release notes stay as shipped.

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

## Internal slices

One PR, deletions first:
1. Done: `ops::wave_lifecycle::add` + `lf wave add`; `ensure`,
   `ops::project::ensure`, `PortfolioRepoState.createWave` and its error cases
   deleted; `World::wave`, task creation, Desktop `RegistryQuery.addWave`,
   skills and docs cut over.
2. Done: `WaveRetired` edit (captured only for retired Waves), `retire_waves`,
   `lf wave remove` with preview/moves/delete-backlog/retire/archive/fold;
   `delete_wave` and `wave_retirement_blockers` deleted; `ensure_wave` matches
   authored directories by id first.
3. Done: `lf wave rename <wave> <new-name>` over relocation plus directory move,
   reference rewrite and Initiative title.
4. Done — Desktop: `RemoveWaveSheet` over `lf wave remove --preview --json`
   (Rust `WaveRemoval` DTO, fixture, Swift mirror); `RenameWaveSheet` over
   `lf wave rename <id> <new> --json`; both in the sidebar Wave row menu, the
   Waves window rows (registered Waves only) and the titlebar Wave crumb's menu.
   `NewWaveRow` and `CreateWaveSheet` take an optional Objective; the sidebar
   selects the new Wave by the id `lf wave add --json` returns.
5. Done — migration upgrade test from the released schema
   (`wave_definitions::released_waves_upgrade_and_removal_needs_no_successor`);
   Linear sync reads only live Waves (verified, no change); `lf wave show <name>`
   falls back to the most recently removed Wave at that address; rename refuses
   before any change when `.lf/wave/<new>` already exists on disk.
   `PortfolioDiscovery.authoredWaves` reviewed: it only adds placeholders for
   unregistered directories, which no longer arise from Desktop creation.
   Not built: unknown-field tolerance in the exchange reader (see questions).
6. Done (2026-11-02, after concept review) — one Wave reader. Deleted
   `PortfolioDiscovery.authoredWaves`, both `authoredWavesByRepo` copies,
   `authoredPlaceholder`, `WaveViewModel.isRegistered` and its guards, the
   write-only `plansByWaveKey`, and `WorkModelTests.authoredWavesRemainVisible`.
   `SqliteStore::register_authored_waves` walks `.lf/wave/` (descending through
   directories without their own files), skips a directory whose `config.yaml`
   id is a removed Wave, and registers the rest through `ensure_wave`; it runs
   inside `lf wave add`, before `lf wave connect --all` lists the store, and in
   `portfolio::change` for every repository the command names or the reading
   shows (`lf portfolio add` in folder mode names a repository the reading omits).
   `WaveRenamed.summary` (Rust-built leftovers, fixture `wave_renamed.json`,
   Swift mirror) keeps `RenameWaveSheet` open with those lines and a Done button.
   `WaveRemoval.has_files` stays: it is the preview's fact, `files` the receipt's.
7. Compress (2026-11-02): `remove` and `rename` share one `resolve_wave`
   (id or address, synchronous store reads); `WaveRenamed` drops the
   relocation receipt nothing read (Swift mirror and fixture follow);
   `register_authored_waves` returns nothing; Desktop's three copies of the
   Wave context menu and sheet presentation are one `WaveLifecycleMenu` and
   `waveLifecycleSheets` modifier (`Views/WaveLifecycle.swift`).
8. Realign (2026-11-02): the Remove sheet stays open on a receipt with
   leftovers (`RemoveWavePhase.leftovers`, `WaveRemoval.hasLeftovers`), as the
   Rename sheet already did; a clean removal closes at once.
9. Done (2026-11-02): `lf wave list` omits removed Waves by default and
   `--current` is deleted with its callers; `lf wave show|status` head with
   `removed at <date>` and `superseded by <id>` only with a successor.
10. Compress (2026-11-02): `retired_wave_at` takes the `WaveLocator` its
    callers already hold, like `get_wave_at`; `remove_authored` returns every
    path the commit selects; Swift's app `Wave` drops `retiredAt`,
    `supersededByWaveId`, `retirementReason` and the "Retired at" lens, which
    no listed row could reach once the list excludes removed Waves.

11. Next (third concept review, 2026-11-02): `connect` never creates a Wave.
    An explicit unregistered name runs `wave_lifecycle::add` first (main
    checkout, first Project, selective commit) and then links; connect's own
    `config.yaml` and `.lf/config.yaml` writes and their commit land on the
    main checkout through the lifecycle's `commit_authored`, not
    `commit_workflow { add: true }` on the invoking checkout. `reteam`'s
    `ensure_wave_row` loop registers authored directories through
    `register_authored`; `refresh_inventory` reads the connected Wave with
    `get_wave_at`. Proof: `wave_lifecycle` test that `lf wave connect ops`
    from a Task worktree (fake `gh`) lands `.lf/wave/ops/` committed on main
    with a first Project, and that `lf wave list` and Desktop show the same
    row. Same pass, smaller: `WaveAdded.summary` (one line when files are
    uncommitted, Rust-built like the other two receipts, fixture
    `wave_added.json`, Swift mirror replacing `WaveIdentity`); the New Wave
    popover and `CreateWaveSheet` stay open with it and a Done button, as the
    Rename and Remove sheets do. And the Remove preview names uncommitted
    edits inside `.lf/wave/<name>` on main (`git status --porcelain -- <path>`),
    since they are deleted with the directory and Git history does not keep
    them.

Remaining: slice 11; Desktop demo steps 1–3 need a display session
(demo/review). CLI demo steps 4–5 ran on `lf-world wave-lifecycle`
(2026-11-02).

## Concept review (2026-11-02, headless) — resolved

The accepted experience holds: three verbs, remove = retire with explicit
disposition, one Rust-authored receipt that CLI and Desktop both show. The
review found one acceptance gap and two smaller ones; slices 6–8 closed them.
No open product choice. Review page: `scratch/concept-review.html` (describes
the gap as found).

- **Desktop kept a second Wave reader.** `PortfolioDiscovery.authoredWaves`
  scanned `.lf/wave/*/GOAL.md` and the sidebar and Waves window merged it with
  the store's rows as unregistered placeholders, while `lf` reads
  `wave_addresses WHERE retired_at IS NULL`. They disagreed exactly where
  removal matters: a removal whose Linear archive fails keeps the directory on
  main (the Initiative id lives in `config.yaml`), so the receipt said removed
  and the sidebar showed the Wave again; a main checkout behind the planning
  store showed the same ghost. Resolved: Desktop lists only store rows, and
  `lf` registers authored directories through `ensure_wave`'s id-first match
  (which skips removed ids) on explicit writes — `lf wave add`, `lf wave
  connect --all`, a repository joining the portfolio — never on a plan read.
  A clone's authored Waves still appear once registered, the counterexample
  the deleted `WorkModelTests.authoredWavesRemainVisible` pinned; now pinned by
  `a_repository_joining_the_portfolio_registers_its_authored_waves_but_not_removed_ones`
  and `wave_definitions::registering_a_clones_directories_lists_them_and_skips_a_removed_wave`.
- **Rename leftovers were CLI-only.** `WaveRenamed.summary` (Rust-built,
  fixture `wave_renamed.json`, Swift mirror) keeps the Rename sheet open with
  the same lines and a Done button.
- **Remove leftovers were CLI-only too** (found at realign, 2026-11-02): the
  Remove sheet closed on every successful receipt, so an Initiative still live
  after an archive failure was presented as a clean removal — the "partial
  cleanup presented as success" acceptance forbids. `RemoveWavePhase.leftovers`
  keeps the sheet open with the receipt's `summary` and a Done button when
  `WaveRemoval.hasLeftovers` (an Initiative with an error, uncommitted files or
  prose mentions — the three facts Rust's `leftovers` renders); a clean removal
  still closes at once.
- `WaveRemoval.has_files` stays beside `files`: the preview's fact and the
  receipt's outcome.

Not changed: Desktop's Remove sheet exposes no `--delete-backlog` (unstarted
Tasks move and the preview says so; a checkbox is ordinary follow-up), and
`lf wave rename` still carries the pre-existing `--repo` and `--title`.

## Concept review (2026-11-02, third pass) — design changed

Run against the implemented change at `cd42ecb239`. Add, rename and remove
behave as the demo lists; the receipt is Rust-authored and shown verbatim; the
list is one list. One accepted decision was still not carried through, and
two smaller consequences of the acceptance were unbuilt. No open product
choice. Page: `scratch/concept-review.html`.

- **`lf wave connect <new-name>` is still a creation path.** The design
  deletes `ensure` so that `add` is the one way a Wave comes to exist, with
  files on main, a first Project and a commit. `connect_async` still calls
  `ensure_wave_row(store, repo_root, name)` for an explicit name, and the
  dispatcher's comment says so: "Wave connection is a creation graph: its
  positional name may select a wave not yet registered". From a Task worktree,
  `lf wave connect ops` registers the row, seeds `.lf/wave/ops/` on that
  branch, creates no Project, creates the Initiative and commits the whole
  dirty tree there (`commit_workflow { add: true }`). That is the stranded
  state the Problem section opens with, reachable through the one Wave
  command the `init` and `split-wave` skills still tell agents to run right
  after `add`. Settled by the accepted decision: connect links; an
  unregistered name adds first (slice 11).
- **Add's leftovers reach the CLI only.** `render_added` prints `.lf/wave/<name>
  is uncommitted on <main>: <reason>`; Desktop decodes `WaveIdentity { id }`
  and closes the popover. Rename and Remove keep their sheets open on the same
  kind of line. Same receipt shape for all three (slice 11).
- **The Remove preview does not name uncommitted edits.** `remove_authored`
  deletes the directory with `remove_dir_all`; the preview says "Git history
  keeps it", which is false for edits nobody committed. Jack's main checkout
  has none today (`git status -- .lf/wave` is clean), so this is a sentence
  in the preview, not a refusal (slice 11).

Looked at, holding: `lf wave rename <wave> --repo <path>` moves a Wave
between repositories under the rename verb, files not following (the receipt
says so); pre-existing relocation surface the design kept, and no accepted
decision speaks to it. `lf task create --wave <new>` adds the Wave through
`add` (it called `project::ensure` before this branch), so a mistyped Wave
name lands a committed `.lf/wave/<typo>/` on main; `lf wave remove <typo>`
is the cleanup, as the design says for strays. Desktop's Remove sheet has no
`--delete-backlog`; unstarted Tasks move and the preview says so.

## Concept review (2026-11-02, second pass) — resolved

Run against the implemented change at `0d73fe5f30` after realign. The accepted
experience holds: add, rename and remove behave as the demo lists, the receipt
is Rust-authored and shown verbatim, Desktop lists only store rows, removal is
idempotent and nothing retires before every open Task has its disposition. One
implied consequence of "removed Waves stay removed" and "Desktop and CLI
reflect the result consistently" was unbuilt; slices 9–10 closed it. No open
product choice. Page: `scratch/concept-review.html` (describes the gap as found).

- **The default `lf wave list` listed a removed Wave, as `abandoned`.**
  `retire_waves` sets `work_state = 'abandoned'` beside `retired_at`; the list
  printed `status.label()`, and only `--current` applied the live filter.
  Desktop read `lf wave list --all --current --json`, so after `lf wave remove
  desktop` the sidebar dropped the row while the documented read (`lf wave
  list`, and `--json` in the `init` and `repo_operate` skills) showed
  `desktop … abandoned` — the "stuck as abandoned" rows the Problem section
  opens with, now produced by the product's own remove. `--current` arrived in
  #1615 (2026-10-21) as a navigation filter with no decision that the
  unfiltered list is a history view; nothing in product code abandons a Wave
  except `retire_waves`, so "abandoned", "retired" and "removed" named one
  state with three words.
  Resolved (2026-11-02): the live filter is the default and `--current` is
  deleted with its callers (`RegistryQuery`, `read.rs` `Waves`, the `s1`–`s5`
  skills, `graph/mod.rs` test, `status_tests`, `wave_lifecycle`, the Swift
  README, tests and the startup benchmark). The default excludes retired rows;
  a legacy `abandoned` row that was never retired (Jack's `engbot`, `list`)
  stays listed, in Desktop too, so **Remove Wave…** can finish it. The list
  help no longer says "retained planning identities". `lf wave show|status`
  name the state with the product word: `removed at <date>`, and `superseded
  by <id>` only when a successor exists. The `WaveSnapshot` wire is unchanged
  (`status`, `retired_at`); no listing of removed Waves is added — `lf wave
  show <name|id>` reads one.
- **Stale sentence** in `swift/README.md` (Current Wave navigation) described
  the placeholder scan slice 6 deleted; it now carries the one-reader statement
  `docs/work.md` has.

Looked at, holding: `lf wave rename`'s already-renamed path answers by the
new address (Desktop renames by id, so it never takes it); the Remove sheet
stays open on prose mentions, which Jack's 2026-10-28 direction lists for a
person to edit; Desktop's sidebar selects the new Wave after `addWave`
awaits the planning refresh, so the row exists when selected.

## Checks

- 2026-11-02: `cargo test --test integration wave_lifecycle` 5/5; `--lib`
  wave_definitions, planning_peers, wave_repository_ownership, migrations pass;
  Swift `CreateWaveSheet|RemoveWaveSheet|ProjectActivation|WaveRow` pass;
  `cargo clippy --all-targets -D warnings` clean. Affected suites → gate.
- 2026-11-02 compress: `remove` resolves the Wave once (id or address) instead of
  recursing through its slug, which removed a new Wave holding a removed Wave's
  address; one directory walk serves memory folding and reference rewrites.
  `cargo test --test integration -- wave_lifecycle dto_fixtures::wave_removal` 6/6;
  clippy clean.
- 2026-11-02 realign (after the merge with main): `cargo test -p loopflow --test
  integration -- wave_lifecycle dto_fixtures::wave_removal` 6/6.
- 2026-11-02 one reader: `--lib wave_definitions::tests::registering` 1/1;
  `--test integration -- wave_lifecycle dto_fixtures::wave_re portfolio_cli_tests`
  9/9; `swift test --filter 'RenameWaveSheetTests|RemoveWaveSheetTests|DTOFixtureTests|WaveTests|MockWaveFixtureTests|WaveSurfacePrimaryHierarchyTests|WorkModelTests|CreateWaveSheetTests'`
  94/94; clippy clean. Affected suites → gate.
- 2026-11-02 compress (one reader): `--lib wave_definitions::` 5/5; `--test
  integration -- wave_lifecycle dto_fixtures::wave_re portfolio_cli_tests` 9/9;
  `swift test --filter 'RenameWaveSheetTests|RemoveWaveSheetTests|DTOFixtureTests|WaveRowTests|CreateWaveSheetTests|WaveSurfacePrimaryHierarchyTests'`
  61/61; clippy and fmt clean. Affected suites → gate.
- 2026-11-02 list default: `lf wave list` omits removed Waves without a flag
  (`--current` deleted with RegistryQuery, `read.rs`, s1–s5, benchmark); never-
  removed `abandoned` strays stay listed; `lf wave show|status` head with
  `removed`, `removed at <date>`, `superseded by <id>` only with a successor.
  `--lib status_tests::wave_list_keeps lf::tests graph::tests::vsm` 38/38;
  `--test integration -- wave_lifecycle dto_fixtures::wave_re portfolio_cli_tests`
  9/9; `swift test --filter 'RegistryQueryTests|PortfolioRepoStateTests'` 21/21;
  clippy and fmt clean. Affected suites → gate.
- 2026-11-02 realign (Remove sheet leftovers): `swift test --filter
  'RemoveWaveSheetTests|RenameWaveSheetTests|DTOFixtureTests'` 55/55. Rust untouched
  since the compress pass; affected suites → gate.
- 2026-11-02 compress (list default): `--lib wave_definitions::` 5/5; `--test
  integration -- wave_lifecycle dto_fixtures::wave_re portfolio_cli_tests` 9/9;
  `swift test --filter 'WaveLensTests|WaveTests|RenameWaveSheetTests|CreateWaveSheetTests|DTOFixtureTests|RemoveWaveSheetTests|MockWaveFixtureTests|RegistryQueryTests|PortfolioRepoStateTests'`
  98/98; clippy and fmt clean. Affected suites → gate.
- 2026-11-02 realign (after the second merge with main, `ce172bd3d0`, which only
  added `?` to `open_existing_store` calls): `cargo test -p loopflow --test
  integration -- wave_lifecycle dto_fixtures::wave_re portfolio_cli_tests` 9/9.
  Affected suites → gate.
