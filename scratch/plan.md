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
  exactly one Wave. `projects.wave_id` is unique and not null.
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
- Migration collapses and fills as below, then `projects_wave` (unique)
  replaces `idx_projects_wave`; `tracker_wave_projects.position` leaving.
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

## Remaining

Nothing to implement. Gate: Swift DTO suite for `WaveRemoval`/`WaveRenamed`,
the full matrix in CI, and the `oauth_tests` contention failure checked on the
base. Initiative names left in `rust/` are released-schema readers
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
  goes. Asana: the same with an Asana project.
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

- A live Wave with no Project gets one, named for the Wave, local until
  connected (36 such Waves in the real store, 2026-11-22).
- A Wave with several Projects keeps one: the newest started open one, else
  the newest. Its other Projects are deleted, with their `project_changes`
  and unsent `tracker_actions` retired; their Tasks move to the kept Project
  locally, and nothing is written to the tracker for them. ("the old ones
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
