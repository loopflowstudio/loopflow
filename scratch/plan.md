# LOO-995 — a Wave is exactly one Project

PR #1997 removed the chapter machinery (current Project, activation,
rollover, KRs, metric targets, instruments, observations, live metrics) and
is held, unmerged. This slice replaces the Wave–Project model the branch
built after that.

Jack Heart, 2026-11-22, after the concept review (`scratch/concept-review.html`):
"wave is at most 1 project for now", then "i think we should just have a wave
= exactly 1 project for now, and move to 0-1 later", "would that simplify
tings?", "the old ones just dont matter", "we can drop the history or drop
the notion of there being history".

Superseded by those words, and to be removed from the branch: the climb rule
(a Task joins its Wave's Project, else the nearest ancestor's, else none,
from 2026-11-20 "(b) the Task joins the Wave directly with no Project, which
adds a wave_id on the Task"), and "a Wave holding several Projects in order"
(2026-11-19 scope note). The rest of both scope notes stands: Projects are
real records with a name, summary and status that start and end.

## The model

- A Wave has exactly one Project, made with the Wave. A Project belongs to
  exactly one Wave. `projects.wave_id` is not null; its unique index waits for LOO-1089 (slice 10).
- A Task in a Wave is in that Wave's Project. A Task in no Wave has no
  Project. A Task's Wave is read through its Project, as on main:
  `tasks.wave_id`, the `task_wave` plan field, its writer rule, its DTO and
  fixture changes, and `task_project_in`'s climb all go. A branch-only
  migration column costs nothing to drop: edit `remove_chapters.sql` in place.
- Moving a Task to another Wave moves it to that Wave's Project. One rule,
  for create and move; `task_move_project`'s keep-if-below rule goes.
- A Project's status is its own (started, completed, …, mirrored from the
  tracker). An ended Project stays the Wave's Project; new Tasks still join
  it. No history: nothing lists a Wave's earlier Projects, and nothing
  orders them (`tracker_wave_projects.position` goes; mark it leaving).
- No choice to explain: the "in Project <name>" line on `lf task create`
  and the "newest started" tie-break go.

## Built (slice 1, storage and commands)

- `tasks.wave_id`, `task_wave`, `with_task_waves`, `task_project_in`,
  `wave_within_in`, `task_move_project`, `joined_project_name` and the
  "in Project" suffix are gone; readers join `projects` for a Task's Wave again.
- `upsert_wave_prepared` makes a new live Wave's Project; a Wave without one
  (arrived from a released build) gets it on its first Task
  (`wave_project_in`). Its id is `ProjectId::for_wave`, the Wave's UUID, so
  Machines making it separately make one record, and its Linear `create`
  carries that UUID.
- Migration collapses and fills as below; `tracker_wave_projects.position`
  leaving. `idx_projects_wave` stays plain (slice 10).
- Linear import binds the first tracker Project it meets in the Wave's
  Initiative to the Wave's unbound Project (unless the Project's own `create`
  is out and this is not it); once bound, others are skipped, so the unique
  index never fails an import. Slice 2 replaces the Initiative with connect.
- Docs: `docs/work.md`, `docs/dev/planning.md`, `reference.md`,
  `reference-models.md`, glossary, `AGENTS.md`, `split-wave`.

Checks run (2026-11-22): storage lib suite after fixture fixes; execution
`explain`, ownership, admission, membership, Linear planning rows; CLI
`local_planning::`, `status_tests::`; `documented_commands::`,
`golden_prompt`; clippy clean. On a copy of the real store, the migration's
one-Project tail left 0 live Waves without a Project, 0 Waves with two, 1,071
of 1,071 Tasks still in a Project (`coverage` 81 Tasks in its one Project),
and a clean `foreign_key_check`.

After merging main (55c12ad9c6, 544 files): `cargo check --workspace
--all-targets` clean.

## Built (slice 2, trackers)

- The link is `projects.external_project_id` on the Wave's Project; nothing
  else says which Wave a tracker Project is. Acceptance binds a tracker
  Project to a Wave only when it is linking one: the Project's own `create`
  landing, `connect <wave> <project>`, and the Wave's inventory refresh
  (which reads only the Project it links). `tracker_wave_projects` is no longer
  read or written (marked leaving). An issue belongs to the Wave whose Project
  links its Project; an issue in a Project no Wave links is not imported.
- `lf wave plan connect <wave> [<project>]` (URL, id or name; `named_project`).
  Without one, the Wave's Project's `create` goes out. With one, the queued
  `create` retires; a Project Loopflow already made is replaced while none of
  the Wave's Tasks is in Linear (otherwise an error says to move them). The
  receipt is `{wave, project_id, project_name, created, team…, files}`.
- Gone: Wave `linear: initiative:` reading (the key stays a Wave-only key so
  released files load), `tracker_wave_sync.initiative` writes, Initiative
  create/rename/archive/list, Project-to-Initiative attachment and its queued
  `link` actions (retired in the migration), the held-until-moved-back rule,
  `membership_unresolved` writes, the blocked-send gate (it only existed for
  Initiative-less Waves), `lf wave rename --title`, the `initiative(s)` fields of
  the rename and removal receipts (Rust, Swift, fixtures, server schema).
- `lf wave status` prints `Project   <name> (local|linked) · <status>`.
- Fixtures: `planning_repo()` links `project-1`; `local_planning_repo()` does
  not. `insert_project` (fixture-only) writes with tracker origin so seeding the
  Wave's own Project queues nothing. Execution store fixtures use
  `ProjectId::for_wave`; two-Project and Initiative-move cases went.

Learned: in a connected repository `lf wave add` now sends the Wave's Project
`create` at once, so `connect <wave> <project>` usually meets a Project
Loopflow already made; that is why connect replaces an empty link.

Checks run (2026-11-22, this slice): execution `ops::linear::`, `linear::`,
`store::tests::`, `ops::tracker`, `ops::run::`, `ops::task::`,
`work::wave::config::` pass except
`oauth_tests::pm_read_linear_oauth_sqlite_contention_has_bounded_failure_and_recovers`
(fails deterministically here at its refresh-lock deadline; this change does not
touch that path, unverified on the base) and
`ops::task::lifecycle_tests::ordinary_task_comments_and_completion_preserve_live_graph_and_arrival`
(timed out at 300 s under load once). CLI: `local_planning::`,
`wave_lifecycle::`, `status_tests::`, `repository_team_matrix::`,
`wave_repository_ownership::`, `lf::commands::waves::tests::`, `lf::tests::`,
`dto_fixtures::`, `server::schema::tests::` (schema regenerated),
`documented_commands::`, `task_adoption_tests::`, `planning_lookup_tests::`,
`golden_prompt` pass; `subscribe::a_paused_watch_and_a_truncated_log_both_catch_up`
timed out under the parallel run and passes alone (15 s). clippy
`--all-targets -D warnings` clean.

## Built (slice 3, from the 2026-11-23 concept review)

- `relocate_waves` (storage, under `lf wave rename`) writes the Wave's
  Project `ProjectSlug`/`ProjectName` as plan edits in the same transaction
  as the Wave's rename (linked or not since slice 4). A `create`
  already queued keeps its snapshot of the old name; the rename queues a
  `project_name` field action behind it, so Linear ends on the new name.
- `retire_waves` (under `lf wave remove`) retires the queued tracker actions
  of each retiring Wave's Project in the same write (`its Wave was removed`).
  A `sent` action is left to resolve. A queued Task `create` naming that
  Project names the Task's current one (QA found it stuck otherwise).
- Tests: `tracker_actions::tests::a_removed_waves_project_is_not_created_in_the_tracker`,
  (pass), `…::a_task_moved_out_of_a_removed_wave_is_created_in_its_new_project`;
  clippy clean. QA ran rename and removal through the CLI (`scratch/qa.html`).

Compress (2026-11-23): Task tracker state no longer reads the leaving
`tracker_projects.membership_unresolved` (nothing writes it; a released `1`
held Tasks Invalid forever). `linear::` and storage `tracker` tests pass
apart from the `oauth_tests` contention failure below; clippy clean.

Then gate: Swift DTO suite for `WaveRemoval`/`WaveRenamed`, the full matrix
in CI, and the `oauth_tests` contention failure checked on the base.
Initiative names left in `rust/` are released-schema readers
(`ProjectInitiatives`, `planning_initiatives`, `tracker_wave_sync.initiative`
written empty, the ignored Wave `linear:` key) and fixtures that test them.

## Tracker decisions (built in slice 2)

- `lf wave plan connect <wave> <linear project>` links the Wave's Project to
  an existing Linear Project; `lf wave plan connect <wave>` with no Project
  makes one in the repository's default Team (LOO-980: "Teams and
  Initiatives matter only at creation"). The Initiative stops deciding which
  Projects are a Wave's: an import attributes an issue to the Wave whose
  Project it is in, and a Linear Project linked to no Wave is not imported.
  The "held until moved back" rule for a Project moved between Initiatives
  goes. With an Asana Team configured, the Wave’s Project mirrors to an
  Asana team project. Named connect here selects Linear.
- Every Wave, top-level included, is one Linear Project; no Wave links an
  Initiative now. Jack Heart, 2026-11-22, asked whether a long-running Wave
  keeps its Initiative link: "One Project only". Initiative links (Wave
  config `linear: initiative:`, `pm_wave_sync.initiative`, the connect
  flow's Initiative create) go until subwaves bring them back.
- A Wave created in a connected repository makes its Linear Project with it.
  A child Wave is its own Linear Project.
- Every Wave Task is an issue in its Wave's Linear Project, so it crosses
  Machines through the tracker as on main; the open "second Machine on a
  connected repository" question in `scratch/questions.md` is gone, and so
  is the conflict with LOO-980's "Wave membership is derived from the link,
  not stored" (true again).
- A Task with no Wave is an issue in no Project (a Linear team issue), as
  slice 2 built; `Task::goes_to_tracker` stays gone.

## Migration decisions (`remove_chapters.sql`, built in slice 1)

- A live Wave with no Project gets one, named for the Wave (36 such Waves in
  the real store, 2026-11-22). It is an ordinary local Project: in a connected
  repository the drain queues a Linear `create` on the next export, using the
  Wave's UUID so two Machines name one Linear Project. (QA run 5 confirmed this
  on the real store: `spend` and dormant `engbot` queue creates; retiring dead
  Waves before upgrading avoids creating them.)
- A Wave with several Projects keeps one: the newest started open one, else
  the newest. Its other Projects are deleted, with their `project_changes`
  and unsent `tracker_actions` retired; their Tasks move to the kept Project
  locally. The migration queues no new moves. An outstanding create or
  move keeps its place and translates its destination for retry. ("the old ones
  just dont matter".) Real store, 2026-11-22: `coverage` 8 Projects,
  `infrastructure` 7, `product` 6, `academy` 6, `experiments` 5; 1,071 of
  1,548 Tasks have a Project.
- Then `projects.wave_id` gets its unique index. If a table rebuild is
  needed for it, keep every column (additive under the staging rule).
- Released `lf` still reads `projects` and `tracker_wave_projects`; keep both
  readable, unwritten where they go.

## Later, not here

Jack Heart, 2026-11-22: "instead of having waves transition, i will introduce
subwaves that have projects and let the long running ones be initiatives
rather than projects". A Wave's Project never rolls over to a successor; new
work that would have been a next chapter is a child Wave with its own
Project.

0–1 Projects per Wave ("move to 0-1 later"), which brings back a Task's own
Wave (`tasks.wave_id`) and a rule for Waves with no Project. LOO-996's one
nestable kind replaces this again; nothing here blocks it.

## Kept from the branch

- Chapter removal, as on #1997 before 2026-11-20.
- `unavailable_tasks` gone from the wire, the printer, Swift and the DTO
  fixtures (slice 2).
- Every Task in a connected repository gets a tracker `create`, Wave or not.

## Shows it worked

```bash
lf wave create billing ; lf wave create billing/refunds
lf wave status billing/refunds                # Project: refunds (local)
lf task create --wave billing/refunds 'x'     # in refunds' Project; lists under billing/refunds only
lf task move x --wave billing                 # now in billing's Project
lf wave plan connect billing <linear project> # billing's Project is that Linear Project; x goes there
```

On a copy of the real store: every live Wave has exactly one Project, no
Task lost its Wave, `coverage` keeps one Project with all its Tasks.

Docs: `docs/work.md` "Projects" and the Linear mapping line ("a Wave maps to
an Initiative" goes); `docs/dev/reference.md` Wave Project row;
`docs/dev/reference-models.md`; `AGENTS.md` "Work model" (a Wave is one
Project).

## Checks

Storage `sqlite::plan` and `local_planning` focused tests; a migration test
from a store with a Wave of three Projects and a Wave with none; clippy.
`documented_commands::`, golden prompts, Swift DTO suites and the full
matrix for gate and CI.

## Slice 4 (human-review answers, 2026-11-23)

Jack Heart, on `scratch/concept-review.html`'s questions: "1b 2b 3b 4 - w e dont have child waves".

Built:

1. `relocate_waves` renames the Wave's Project whether or not it is linked;
   a linked one's new name goes out as a queued `project_name` field action
   (`tracker_actions::tests::renaming_a_wave_renames_its_project_in_the_tracker_too`).
2. The removal line for Projects and `WaveRemoval.projects` are gone (Rust,
   Swift, `wave_removal.json`, schema); completed Tasks keep their line.
3. Desktop's Wave header shows the Project's name, a link when its URL is
   known; the Projects button and its sheet in the Wave pane went. The URL is
   new: Linear's project `url` is read into `tracker_projects.url`
   (`remove_chapters.sql`, revision trigger includes it), served as
   `ProjectSummary.url` (required, nullable) and printed by `lf wave status`.
   Asana Projects carry none yet.
4. Child Wave Project naming: no change ("we dont have child waves").
5. Unanswered: the queued create's snapshot stays as built.

Learned: no Project URL existed anywhere before this; "link" meant the
plumbing above. A Project linked before this branch shows its name without a
link until the next tracker read sees its URL.

Checks run: storage `renaming_a_wave_renames…`,
`a_linked_projects_url_is_kept_from_the_tracker_read`,
`planning_update_triggers_compare_every_column`, migrations; CLI
`wave_lifecycle::` (pass). For gate: Swift `WaveDetailReadingTests`,
`DTOFixtureTests`, `WorkModelTests`, `RegistryQueryTests`
(`scripts/test_desktop.sh`), clippy, the full matrix in CI.
Swift `WaveDetailReadingTests|DTOFixtureTests|WorkModelTests|RegistryQueryTests|RemoveWaveSheetTests|ServerProtocolTests`:
150 pass. `ServerProtocolTests` had been failing since slice 2 added
`project` to `wave.plan.connect`; Swift's `wavePlanConnect` now carries it.
`status_tests::` passes (one fixture gained `"url": null`); clippy clean.

Compress after slice 4: the Backlog Wave menu's Project sheet shows the Wave's
one Project; its scroll-to-reference (`sourceReference`, `projectsReference`)
and truncation line went.

## Slice 5: the wire carries one Project (concept review after slice 4) — built

Chosen for review (`scratch/concept-review.html`, questions 1 and 2): the
last place the code says a Wave has several Projects is the wire.
`WaveDetailSnapshot.projects` and `WaveRoadmap.projects`
(`rust/loopflow/src/lf/commands/waves.rs`) are `Evidence<ProjectSummary>`
with `items` and `truncated`; every reader takes the first or loops over one.
Passed over: keeping the list; a bare `Option` (loses "could not read").

Built (2026-11-23): `Reading<T>` / Swift `WorkReading<Value>` (with
`present` for the Project) carry `project` on both structs; `print_project`
prints `Project   none` for a Wave without one. Desktop's planning-failure
check keeps only the unavailable case. Gone with the list: the Swift
"ended and started Projects in order" DTO test and the successor half of
the selected-Task planning test. `MockWaveFixture`'s Project gained the
`url` key slice 4 made required (it had been decoding as unavailable).

Checks run: CLI `dto_fixtures::`, `server::schema::tests::` (schema
regenerated), `status_tests::`, `wave_lifecycle::`,
`lf::commands::waves::tests::` and the three `local_planning::` tests that
read the Project; Swift `DTOFixtureTests|WaveDetailReadingTests|WorkModelTests|RegistryQueryTests|MockWaveFixtureTests|WorkNavigationTests|ServerProtocolTests`
(202 pass); clippy clean.

Compress after slice 5: storage reads a Wave's Project as one record,
`project_of_wave(wave) -> Option<Project>`; `list_projects` takes no Wave and
lists every Project (Task rows' names). Tests that asserted a Wave had one
Project (the unique index says so) went with the list. Storage `project_`,
`sqlite::children::`; execution `store::`, `ops::linear::` planning and connect;
CLI `waves::tests::`, `local_planning::`, `planning_lookup_tests::` pass;
clippy clean.

QA after slice 5 (`scratch/qa.html`, fourth run): the Wave backlog showed no
Project at all (the chapter removal dropped `currentProject` there); its title
line, Objective detail and capture heading now show the one Project
(`BacklogViewTests`, 115 Swift tests pass).

## Slice 6 — connect a new Wave to an existing Project

The regression test on the previous code linked the requested Project but
also recorded a `DeliverProjectCreation` request. Connect now calls
`add_local` to register the Wave and commit its files. With a named Project,
it links that Project and retires the queued create. With no Project, it
sends the Wave's own create.

Wave add and rename prioritize the stored Project ID in the tracker queue.
Removal runs a repository-wide pass. Desktop's Project sheet has singular
names throughout. The Work guide describes how another Machine imports a
Project-less Team issue.

### Checks on 2026-11-23

- Execution's connect and drain tests passed. The named connect sent zero
  Project creates. With 40 older Projects queued, add delivered the new
  Project and rename delivered the linked Project's new name.
- A private CLI run renamed `billing` to `payments` with the same Wave and
  Project IDs. SQLite used `projects_wave` for the Project lookup.
- `wave_lifecycle::` and `documented_commands::` passed (17 tests).
- `scripts/test_desktop.sh --filter 'WorkModelTests|BacklogViewTests|WaveDetailReadingTests'`
  passed (62 tests). Clippy passed with warnings denied.
- The KR/metric text scan finds only migrations, their fixtures and the
  schema inventory of columns marked leaving.

Compress after slice 6: 46 focused Rust tests passed for Project lookup,
Wave removal, migration, tracker delivery, connect, reteam, Task moves and
Session binding. The OS-installation adoption test is ignored locally.
A 10,000-Task SQLite fixture used the external-ID index for Project lookup
and `tracker_actions_unconfirmed` to update queued creates on Wave removal.

QA after slice 6 (`scratch/qa.html`, sixth run): a connect naming a Project
that then failed (no login, a mistyped name) had already added the Wave, whose
own Project's `create` would go out on the next send. Connect now finds the
Linear Project before adding the Wave
(`task_planning_tests::connect_naming_a_missing_project_adds_no_wave`).

## Slice 7 — remove and reconnect a Wave's tracker Project

Only live Waves admit issues and order changes from a tracker Project.
Removing a Wave leaves its completed Tasks readable. Lost-reply checks use
the observed local Project ID, including a retired Wave's, or the external
ID for a Project without a local record.

Connect checks for a live owner before adding files or registering a Wave.
The link transaction repeats that check, releases a retired owner's external
ID, and retires queued and conflicted Project edits when the link changes.
A sent action or a conflicted create needs resolution first. A retry with
the same link keeps its queued work.

The repository's lifecycle lock covers the ownership check, Wave registration
and local binding. This closes the gap in the original preflight where two
connects could both register Waves before either claimed the Project.

On an in-memory SQLite fixture with 10,000 Tasks, 10,000 action rows and
1,000 linked Projects, looking up a collision through `wave_addresses`
walked unrelated Waves. The check now follows just the holder's ancestors.
Joining accepted Projects materialized the repository's tracker inventory
for one issue. The correlated existence check uses its Project ID. Medians
over 31 warm runs were 3 µs for a collision and 7 µs for issue admission.

Checks on 2026-11-23: three focused storage tests, five execution tests
against simulated Linear, and four `documented_commands` tests passed.
Two concurrent connects after removal produced one Wave and one imported
issue under it. An offline rename followed by connect and drain issued
zero Project creates or field updates. Private CLI rename help shows one
Project. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.

Compress after slice 7: Linear refresh and its fixtures pass one
`ProjectInventory` from acquisition through SQLite. The released-store
fixture checks the fate of both historical Tasks after its two Projects
collapse. The cross-Wave move fixture delivers an older inventory after the
move and checks the Task, graph and PR. On a synthetic SQLite store with
1,000 Projects and 10,000 Tasks, the Project projection looks up the tracker
row by repository, tracker and ID and the link by external ID.

Checks on 2026-11-23: 35 focused Rust tests passed across storage, execution
and CLI, including the released schema upgrades. Formatting, Clippy with
warnings denied, and the rendered architecture page check passed.

QA after slice 7 (`scratch/qa.html`, seventh run): a completed Task left in
a removed Wave's Project the tracker does not hold kept a queued `create`
that failed every pass, and `uncreated` queued one again. Such a Task now
gets no tracker actions, on removal, on a reclaim that releases the link, and
later (`tracker_actions::tests::a_task_left_in_a_removed_waves_project_is_not_created_in_the_tracker`). The released-store seed in
`provider_columns_preserve_released_observations_and_local_execution` has its
`initiative_ids` back; an Initiative sweep had taken it out.

## Decision pass 3 — Wave–Project recovery

Slice 7 delivered live ownership on reconnect, retirement of stale Project
edits, and readable completed history. Two independent failures still break
the promise that saved Wave work can reach the configured tracker. The
current walkthrough and both review choices are in `scratch/concept-review.html`.

### Slice 8 — keep pending Task delivery usable through upgrade (built)

Built: `remove_chapters.sql` maps dropped to kept Projects (`one_project_dropped`)
and translates queued creates and `task_project` edits;
`planning_action_tests::a_released_queue_delivers_after_its_chapter_projects_collapse`
over `tests/fixtures/migrations/chapter_deliveries.sql`.

Use the released-schema fixture in
`rust/loopflow-storage/src/sqlite/children.rs`
(`the_migration_leaves_every_live_wave_one_project_with_its_tasks`). Seed a
Task create queued under a Project the migration will discard. On a private
Machine initialized with the installed app's lf, executing the branch's
`remove_chapters.sql` left the Task on the kept Project and the queued
create referencing the deleted ID. The foreign-key check was clean.
`linear/actions.rs` and `asana/actions.rs` both resolve that ID through
`ops/tracker.rs::remote_project`.

- Use the dropped-to-kept map in `remove_chapters.sql` while both records
  are known. Translate pending Task creation destinations in the serialized
  fields. Keep the owner, action ID, ordering and other fields. Check
  outstanding Task membership edits too, including an edit moving to a
  different Wave. Translate the intended destination wherever a retry can
  send it. Do not enqueue membership changes for already delivered Tasks
  solely because a local chapter disappeared.
- A sent request can already have landed. Keep its identity and the
  information needed to compare the original remote result. For an uncertain
  create, resolve by the Task's existing tracker identity, and use the
  translated destination if it must be created. For a sent move or an
  answerable conflict, distinguish an observation of the former external
  Project from the kept Project and from no Project. Never rewrite a recorded
  observation as though the tracker had moved an issue.
- Show an upgrade followed by delivery using the existing simulated tracker.
  The unsent Task appears once in the kept Project. A lost reply for an
  already-created issue produces no duplicate or unsolicited move. A pending
  move to a surviving destination still reaches it. Additional unrecognized
  fields in a queued create survive translation.

### Slice 9 — make the advertised Wave sync retry work (built)

Built: `ops/mod.rs::refresh_status` routes by `queued_tracker`; with a
tracker it runs `planning_sync::refresh_wave` for the selected Wave, else the
carrier refresh (`asana_planning_tests`, `task_planning_tests`).

A private checkout with a Linear Team and no login had a queued Project
create for `billing`. After clearing its recorded error as a probe,
`lf wave sync billing` exited 0, suggested connect, and left the error
empty. In an Asana-only repository with a seeded linked Project, the
same command exited 1 asking for a Linear Team. Both paths run through
`rust/loopflow/src/lf/commands/ops/mod.rs::refresh_status`.

- Route the shared refresh by the configured transport. Without a tracker,
  retain the repository carrier refresh. With Linear or Asana, resolve
  the selected Wave and run the existing bounded exchange, giving its
  stored Project ID priority in the drain. Reuse the delta-before-inventory
  ordering in `ops/planning_sync.rs` and `ops/tracker_events.rs`. After
  delivery, read the link again. Only linked Linear Waves have a Linear
  inventory to fetch. Use Asana's existing repository exchange for Asana.
- Show the resulting Wave status. A still-pending Project shows its action
  and available failure reason. A retry should not ask someone to connect
  a Wave already waiting for its configured tracker. Keep the singular
  Project DTO usable through the bare CLI, served mutation and Desktop.
- Exercise the shared command with the existing provider fixtures. Start
  offline, restore access, sync twice with older queued work ahead of this
  Project, and assert one remote Project with the original local identity.
  Cover an Asana link without Linear configuration and the carrier-only path.

Choices made for review: keep pending creates in their Wave on upgrade,
and use explicit sync to attempt delivery. A Project-less Team issue would
lose membership. Making connect the only retry would contradict its receipt.
Changing the snapshot semantics of all tracker creates is a separate outcome.

On the released schema with 10,000 Tasks, 10,000 actions (300 open or
conflicted) and 1,000 discarded Projects, the four queue updates took
80 ms median over seven warm runs. The create scan used
`tracker_actions_unconfirmed`, the membership scans used
`tracker_actions_task`, and all four used the temporary dropped-ID index. Naming `base_json` in
every send ran canonical identity triggers even for an unchanged payload
(319 µs versus 122 µs per single-row update). Only a resend with a former
chapter to clear updates that column.

## Remains

1. Gate: Swift DTO and desktop suites (`scripts/test_desktop.sh`), full matrix
   in CI, and check the `oauth_tests` contention failure on the base.
2. Open, not this Task: a queued create's field snapshot (`scratch/concept-review.html`
   discussion of create snapshots).

## Slice 10 (staging the one-Project index, 2026-11-23)

Built: `remove_chapters.sql` no longer adds a unique index; main's
`idx_projects_wave(wave_id, created_at)` stays. `held_wave_project_in` is the
one read of a Wave's Project (`project_of_wave`, `wave_project`, Task create,
import binding, link, rename, `retire_wave_in`): the `ProjectId::for_wave`
record first, else the oldest. LOO-1089 adds the unique index with
`-- expand: remove_chapters` after `STAGING_DAYS`. Also named the columns of
`ACCEPTED_WAVE_PROJECTS` (`SELECT p.*` failed the runtime-SQL check).
`uv run python scripts/check_migrations.py`: passes.
Test: `linear_events::tests::a_second_project_a_released_writer_adds_never_becomes_the_waves`.

## Slice 6 (human-review answer, 2026-11-23)

Jack Heart, on the plural Swift names: "1 - i dont think we should have a
wavesprojectview anymore". Delete `WaveProjectsView` (the Project sheet) and
everything that opens it, including the Backlog menu's **Project** item; the
Wave header's Project name and tracker link (slice 4) is the one place a
Wave's Project shows in Desktop. Question 2 (a queued create's snapshot)
unanswered: stays as built.

Built: `WaveProjectSheet.swift`, `WorkModel.projectWave` and the sheet in
`SessionsView` are gone. The Backlog menu appears only for a Wave with a
default Graph and holds **Workflow**. A historical Project reference selects
its Wave and opens nothing else. Kept: the inline `WaveProjectView` (name,
status, tracker sync) in the Wave pane, Backlog objective and roadmap card;
see `scratch/questions.md`. Swift README updated. `scripts/test_desktop.sh --filter 'WorkModelTests|BacklogViewTests'`: 58 pass.
