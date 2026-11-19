# 43bf9b — Remove chapters, KRs and metrics

Stacked on LOO-938 (PR #1971); that Task's plan is on its own branch
(`git show jack/loo-938-send-every-planning-change-through-one-queue:scratch/plan.md`).
The other files in `scratch/` are LOO-938's and are background here.

## Direction

The brief: "Remove chapters, KRs and metrics." "The chapter machinery is
unused and is why a small group of Tasks costs a Project. This is a pure
deletion; it adds no replacement."

Scope correction, steer f1ada74d, 2026-11-18, by a Session for Jack Heart:
"keep Projects as real records. A Wave may hold many Projects, Tasks keep
their Project membership, and a Task still reaches its Wave through its
Project. Remove only the chapter machinery: the single current Project and its
activation, transitions, KRs, metric targets, instruments, observations, live
metrics, and the three commands. Do not collapse Project into a link on the
Wave; the title overstates that. The direction now under discussion makes
Projects the finishing groups that map to tracker Projects, with Waves as
grouping above them."

Second steer, a1df4aca, 2026-11-18, by the same Session for Jack Heart:
"Projects that end are wanted. Keep a Project able to start, end and show its
status: `projects.work_state`, `work_terminal_at`, the status mirrored from the
tracker, and a Wave holding several Projects in order
(`tracker_wave_projects.position`). What goes is the ceremony around ending
one: the single current slot on a Wave, activation, the successor rollover in
`project_transitions`, KRs, metric targets, instruments, observations and live
metrics. If removing something would stop a Project from ending or from being
listed with its status, leave it and say so."

So `projects`, `tasks.project_id → projects.wave_id`, Project name, summary,
description, status, rank, `work_state`, `work_terminal_at`,
`tracker_wave_projects` and the tracker sync of all of them stay as they
are. What goes is everything that makes one Project of a Wave special, and
everything that hangs KRs and metrics on a Project or a Wave.

### What starts and ends a Project (checked 2026-11-18)

Nothing on the list below ends a Project, so nothing is held back for the
second steer. A Project's status changes in two places and both stay: the
tracker's status mirrored both ways (`Edit::ProjectStatus` in
`linear_events.rs`, the `status` arm of `patch_project_field`), and
`SqliteStore::abandon(WorkRef::Project)` (`sqlite/durable.rs:102`), which
writes `work_state` and `work_terminal_at`. The successor rollover is already
dead: nothing writes `project_transitions.predecessor_id` or `reset_name`;
`ensure_project` only reads them. No `lf` command completes a Project without
a tracker, on main or after; that is an addition and is on the review page as
a question, not in this plan. A Wave's Projects list in
`planning_rank, created_at, id` order (`list_projects`,
`sqlite/children.rs:1295`), unchanged.

## What people see

- `lf wave create billing` registers the Wave and commits `.lf/wave/billing/`.
  No Project row, nothing sent to Linear or Asana.
- `lf task create --wave billing 'x'` works on a Wave with no Project: the
  Task's Project is made then (see "Which Project a new Task joins").
- A Task in any Project of its Wave runs. Nothing says "configured Project is
  not In Progress", "No Project selected yet" or "Preparing Project…".
- `lf help wave` lists no `update-project`, `edit-project`; `lf wave plan`
  has `connect` and `reteam` only.
- `lf wave status`, `lf wave show`, `lf wave plan --json`: Projects with name,
  summary, status and tracker sync, in the Wave's order. No `current`, `krs`,
  `metric_targets`, `metric_portfolio`, `project_readiness`. The text line is
  `Project   <name> (<id>) · <status>`; the words "current" and "historical"
  go (`print_projects`, `lf/commands/waves.rs:440-452`).
- `lf task create --wave billing 'x'` and a move into a Wave name the Project
  the Task joined when the Wave has more than one open Project
  (`… in Project <name>`); `--json` carries the Task's `project_id` as today.
- Desktop Wave detail: no Metrics section, no KR checklist, no activation
  spinner or retry. The "Project history" button and sheet become "Projects":
  the Wave's Projects in order with name, status and tracker sync, ended ones
  included (`ProjectHistoryView` → `WaveProjectsView`). "History" named the
  succession of chapters; a Wave now holds several Projects at once. The standup board header names the Wave;
  it has no "current Project" line, metric headline or KR marks.
- Agent context for a Task or a Wave has no `## KRs`, `## Metric targets` or
  `<lf:metric-portfolio>` block.
- `lf history usage --weekly` prints its report and writes nothing to a Wave.
- A Linear Project's `content` (where KRs and targets were rendered) is never
  written again and never parsed. What is there stays.

## Source of truth

`projects` rows and `tasks.project_id`, as on main. A Wave's Projects are the
rows with its `wave_id`; none of them is selected.

### Which Project a new Task joins — made for review

`lf task create --wave`, a Task moved into a Wave, and the two planning
explanations (`planning_explain.rs:208,385`) all ask "the Wave's Project"
through `ops::project::current_project` → `selected_project` →
`waves.current_project_id` today. With no selection they ask one storage
function instead, `new_task_project(wave)` in `sqlite/children.rs` beside
`list_projects`:

> the Wave's newest `started` Project; else its newest `backlog`, `planned` or
> `paused` one; else none. Newest by `created_at`, then `id`.

When it answers none, creating or moving a Task makes a Project named for the
Wave (`started`) in the same transaction. Its tracker `create` is queued by
the drain as today (finding 1).

This is the one place a Wave still singles out a Project, and it is computed,
not stored: nobody sets it and nothing gates on it. Because nobody sets it,
the commands say it: creation and move name the Project joined whenever the
Wave has more than one open Project (see "What people see"). A Task that
landed in the wrong Project is moved in the tracker, whose membership is
mirrored back.

Measured on one store written by main (62 live Waves, 64 Projects,
2026-11-18): of the 17 Waves with a `current_project_id`, the rule answers
that Project in 17; "oldest started" answers it in 15. 22 Waves have one open
Project, 5 have two, 1 has three; 2 have two `started`. The other 45 live
Waves have no current Project and get one on their first Task.

Not guaranteed: a Project started in Linear under the Wave's Initiative after
the current one becomes the answer. That is the case the creation output
names.

*Passed over:* keeping a pointer on the Wave as a plain default (the single
current slot both steers remove); the Project whose name is the Wave's (a
rename in Linear would make a duplicate on the next Task); the first started
Project in the Wave's order (matches less of the saved data, and rank is the
tracker's, not a statement about new Tasks); `--project` on `lf task create`
(an addition; it belongs to the Projects-as-finishing-groups direction);
refusing when a Wave has several open Projects (adds a precondition);
`tasks.wave_id` so a Wave's Tasks need no Project (ruled out by the first
steer; on the review page as the larger question).

## Findings that shape the work

1. **`project_transitions` also marks "this Machine made this Project".**
   `uncreated` (`loopflow-storage/src/sqlite/tracker_actions.rs:980-992`) is
   what the drain asks on every pass (`enqueue_tracker_creations`,
   `ops/tracker/drain.rs:122`): it gives a `create` to each unbound Project
   and Task, "including those made before the tracker was connected". Its
   Project half requires a `project_transitions` row with `local_plan_json`,
   which `ensure_project` writes; the Task half uses `task_creation_intents`.
   The test exists so a Project that arrived unbound from another Machine is
   not created twice. `uncreated` stays, both halves: queuing the `create` in
   the creating transaction would miss every Project made before a tracker is
   connected. Its Project test becomes LOO-938's stamp: the Project's
   `wave_id` row in `plan_stamps` names this Machine
   (`planning_write::local_machine`), and a Project with no stamp counts as
   made here, as LOO-938 reads unstamped fields. That also settles the
   Project the released `lf` made offline and never exported: it has no
   stamp, so the first drain pass after upgrade queues it. Slice 1 checks
   both, and that a Task's `issueCreate` waits for its Project's tracker id
   when the two are queued in the same pass.
2. **Plan records carry three of the removed fields.**
   `planning_write::read` (`sqlite/planning_write.rs:101-136`) emits
   `wave_project`, `krs`, `metric_targets`; `plan_carriers.rs:125-143` orders
   a Wave after its `current_project_id` Project. The Git carrier skips a
   field it cannot decode (`ops/plan_git.rs:260-292`); the hub decodes
   `Vec<StampedRecord>` strictly (`machine/wire.rs:272,291`), so one Machine
   on LOO-938's build sending `krs` fails the whole page on this build. The
   per-field skip moves onto `StampedRecord`'s own decoding so both carriers
   have it, and `parse_record` loses its copy. `plan_stamps` rows for the
   three fields stay as rows nobody reads.
3. **Upgrades from old releases decode the removed variants.**
   `convert_released_data` and `convert_project_content`
   (`migration_runner.rs:780-835`) run after released migration
   `0.13.12.002` and parse stored JSON as `WorkValue`, including `Krs` and
   `MetricTargets`, and write `projects.krs`. Released migrations are
   immutable and their conversion has to keep working from a store older than
   that batch. The conversion keeps its own private reading of those values
   (raw JSON, validated as JSON) and no longer names `WorkValue::Krs`,
   `ProjectKR` or `MetricTarget`. `project_document.rs` parsing survives only
   as far as that conversion needs it, inside `migration_runner`.
4. **The migration is marks only, in its own file.** `new_migration.py`
   prints `plan_hub.sql` on this branch because LOO-938's migration is
   unmerged (`_unmerged`, `scripts/new_migration.py:57-75`). Unmerged slugs
   are ordered by name after merged ones (`migration_build.rs:19`), so a
   second file works: create `remove_chapters.sql` by hand; `r` sorts after
   `plan_hub`. Marks need no DDL and no trigger rebuild
   (`unused_store_members.sql` marked `tasks.worktree` and left its trigger
   and index). The idle triggers (`selected_project_wave_update`,
   `selected_project_owner_update`, `project_activation_start_from_process`,
   the `graph_run_end_from_process_*` writes) stay until the contract, which
   also rebuilds `waves` (FK column) and the `wave_addresses` view
   (`SELECT w.*`).
5. **Nothing in production deletes a `projects` row** (only a test, with
   foreign keys off), so a stale `waves.current_project_id` cannot block a
   delete during the window.
6. `ensure_wave_project` (`work/wave/bootstrap.rs:32`) has no production
   caller; about twenty tests use it to get "a Wave with a Project". The real
   path is `wave_lifecycle::ensure_first_project` (`ops/wave_lifecycle.rs:130-154`).
7. `tests/e2e/linear_oauth.py:229-319` inserts into `wave_chapters`, a table
   that no longer exists: an earlier attempt, dead.

## What goes

Behavior that must survive: Projects and their tracker sync both ways (name,
summary, description, status, rank, Initiatives, Teams, Task order), Task
identity and history, a Task reaching its Wave, `lf wave plan connect` and
`reteam`, `.lf/wave/**` directories, upgrade from any released store.

**Commands and served writes**
- `WaveCommand::EditProject`, `UpdateProject`, `WavePlanCommand::Bind`,
  `WaveProjectTarget` (`loopflow/src/lf/mod.rs:1531-1549,1922-1933,2016-2034`),
  their handlers (`bin/lf.rs:905-920,971-984,1044-1060`), entries in
  `changes_work` (`lf/mod.rs:1211-1217`), help text saying "first Project" and
  "current Project".
- `wave.plan.bind` (`lf/commands/write.rs:498-508`), `engine::wave::planning_bind`,
  `tests/fixtures/dto/server/wave_plan_bind.json`, its lines in
  `server/wire.rs`, `server/schema.rs`, `server/README.md`.

**Selection and activation**
- `loopflow-storage/src/sqlite/project_selection.rs`, whole file:
  `read_project_binding`, `write_project_binding`, `bind_project`,
  `ensure_project`, `record_project_activation`, `selected_project`,
  `project_readiness`, `ProjectReadiness*`, `ProjectActivation`.
- `require_selected_project`, `require_current_task_project`
  (`sqlite/durable.rs:412-449`) and their calls in `place_task_in`,
  `promote_task_prepared`, `create_task`, task move, `require_task_launch`.
- `WorkValue::WaveProject` and its arms (`work/edit.rs:103,147`,
  `planning_write.rs:33,69,103-116,204-215`); the `current_project_id` need in
  `plan_carriers.rs:129`.
- `RunOwner::ProjectActivation` and its arms (`graph_inventory.rs:756-761`,
  `retention.rs:250-252`, `processes.rs:613-616`,
  `loopflow-test-support/src/lib.rs:69`), `query_plans.rs:164`.
- `ops/project.rs`: `current_project`, `bind_project`, `update_project`,
  `resolve_project*`, `ProjectTarget`, `edit`, `readiness`. `WaveGraph`,
  `GraphDefault` and `wave_graph` also live in that file and stay: they move
  beside `scope_graph_default` in `ops/task.rs`, and the file goes.
- `ops/wave_lifecycle.rs` `ensure_first_project`; `WaveAdded.project`.
- `work/wave/bootstrap.rs` `ensure_wave_project`; test callers build their
  Project with `insert_project` or create a Task in the Wave.
- The `~/.lf/chapter-locks` directory name in `ops/planning_sync.rs:227-254`
  becomes `wave-locks` (the lock stays).

**KRs and metric targets**
- `ProjectKR`, `Project.krs`, `Project.metric_targets`,
  `validate_metric_targets` (`work/project.rs`), `WorkValue::Krs`,
  `MetricTargets` (`work/edit.rs:128-129,177-179`), `update_project_content`
  (`sqlite/project_content.rs`), the columns in `insert_project`,
  `PROJECT_COLUMNS`, `map_project_row` (`sqlite/children.rs`), in
  `ActionKind::of`, `field_baseline`, `creation_baseline`
  (`sqlite/tracker_actions.rs`), in `put_project` and
  `project_accepted_planning` (`sqlite/linear_events.rs`).
- `TrackerProject.krs`, `.metric_targets`, `render_project_content`,
  `render_metric_targets`, `render_krs` (`planning.rs:194-256`),
  `planning/project_document.rs` (see finding 3),
  `linear/project_document.rs`, the `content` write in
  `linear/actions.rs:160,173-181`, `project_field_input` and
  `patch_project_field` arms (`linear/mod.rs:1414-1434,2306-2331`), the parse
  in `ProjectNode::into_tracker_project` with its "older rotation archived its
  predecessor" branch (`linear/mod.rs:2334-2362`), `asana/actions.rs:387-403`,
  `ops/tracker.rs:411-425`.
- Queued `tracker_actions` whose subject is `krs` or `metric_targets`: the
  drain retires them unsent.

**Metrics**
- `loopflow-storage/src/metric.rs` (1059 lines), `sqlite/metrics.rs` (663),
  `loopflow-execution/src/ops/metrics.rs` (729), `work/wave/metrics.rs` (760),
  `store/metrics.rs` (65).
- `context_cost.rs` `observations` and `publish` (334-415); the report stays.
- `print_metric_portfolio` and helpers (`lf/commands/waves.rs:2805-2980`).
- `render_project_context` and `render_wave_context` metric and KR text
  (`ops/run.rs:71-100`).
- `World::plan`, `World::metric` and their recipe uses (`world/mod.rs:355-436`,
  `world/recipes.rs`).

**Wire types** (`tests/fixtures/dto/`, Rust and Swift fixture tests)
- `WaveDetailSnapshot` and `WaveRoadmap`: `project_readiness`,
  `metric_portfolio`. `ProjectSummary`: `current`, `krs`, `metric_targets`.
  `Project`: `krs`, `metric_targets`. `WaveAdded`: `project`.
- `metric_portfolio.json` whole; fields out of `roadmap_snapshot`,
  `wave_detail`, `work_first_entry`, `task_status`, `wave_added`,
  `task_lifecycle`, `task_history_planning`; `server_schema.json`
  regenerated (`lf server describe --json`).

**Desktop** (`swift/`)
- Whole files: `Models/MetricPortfolio.swift`,
  `LoopflowMac/Views/ProjectReadinessView.swift`,
  `LoopflowTests/ProjectActivationTests.swift`; `KeyResultProof` and
  `WaveKeyResult` in `Models/WavePlan.swift` (unused today).
- `WaveMetricPortfolioView` and helpers (`WaveDetailPane.swift:283-675`), KR
  rows in `WaveProjectView`, activation props and `activateProject`
  (`WorkModel.swift:2017-2060`, `WavesView.swift:103-107,368-370`),
  `currentProject` on `WaveWorkMap`, `WavePlan`, `WaveRoadmap`,
  `WaveDetailSnapshot`, `ProjectReadiness` in `RegistryQuery.swift`,
  `PlanningKeyResult`, board heading and header lines
  (`WaveStandupBoard.swift:97-105,219-245`).
- `waveForProject` matches any of a Wave's Projects, not the current one.
  `ProjectHistoryView` becomes `WaveProjectsView`, loses its KR list and its
  default to the current Project (it opens on the list), and its two
  "Project history" buttons (`WaveDetailPane.swift:42`,
  `WaveStandupBoard.swift:201`) read "Projects". Tests in `DTOFixtureTests`, `WaveDetailReadingTests`,
  `RegistryQueryTests`, `WorkModelTests`, `WaveStandupTests` follow.
  `MockWaveFixture.swift` loses the same fields. `swift/README.md` sections.

**Docs and skills**
- `docs/work.md` "Plan it: KRs and targets" (529-543) and the KR and metric
  mentions at 55, 358-359, 403-407, 464; `docs/dev/planning.md` intro, "Plan
  creation and selection" (400-453), "Replace a Wave's Project, keep the
  work" (455-468); `docs/dev/reference.md` Wave Project and Live metric rows
  (the architecture check requires every live table to have a row, so the
  leaving tables stay listed as leaving); `reference-models.md`,
  `reference-code.md`, `dev/index.md` (and regenerated `index.html`),
  `data.md:331`, `machines.md:69`, `agent-api.md:69,255-256`,
  `docs/index.md:121`, `glossary.md` KR row, `orchestration.md:335`,
  `AGENTS.md` Work model paragraph, `PROMPTS.md:163`.
- Builtins: `split-wave.md` metric and `update-project` lines,
  `kickoff.md:79`, `prompt.md:30,84-85`, `wave_operate.md:10`; the test at
  `skills/mod.rs:1206-1222`. `.lf/qa/tracker.md:1,6-7`.
- `tests/e2e/linear_oauth.py:104,229-319`; the `.lf/chapters` ignores in
  `scripts/check_architecture.py:75` and `python/tests/test_architecture.py:219`.

**Schema** — `remove_chapters.sql`, marks only (`-- leaving: … <Task ID>`; the
checker wants a tracker-style ID, `[A-Z]+-\d+`, and this Task's is `43bf9b`:
see questions):
`waves.current_project_id`, the seven `waves.project_activation_*`,
`project_transitions`, `projects.krs`, `projects.metric_targets`,
`projects.legacy_current`, `projects.iteration`, `tracker_projects.krs`,
`tracker_projects.metric_targets`, `pm_projects.krs`,
`pm_projects.metric_targets`, `metric_instruments`, `metric_observations`,
and `project_changes` (read only by a test since `tracker_actions.sql`
acknowledged every row). `SCHEMA.md` regenerated by `scripts/schema_page.py`.

**Tests that go with their feature:** the lists in the three inventories,
chiefly `project_selection.rs` tests, `sqlite/metrics.rs` tests,
`ops/metrics.rs` and `work/wave/metrics.rs` tests, `store/mod.rs`
`selected_project_*`, `local_planning.rs` 1024, 1399, 2706,
`global_commands.rs:2088`, `status_tests.rs:620`, `dto_fixtures.rs:129,208`,
`waves.rs` 4053, 4216, 4283. Migration tests and fixtures under
`rust/loopflow-storage/tests/fixtures/migrations/` stay: they seed released
schemas.

## Absent and error states

- Wave with no Project: every reader shows an empty Projects list; Task
  creation makes one.
- Wave whose Projects are all completed or canceled: a new Task makes a new
  Project named for the Wave.
- A record from a Machine on an older build carrying `wave_project`, `krs` or
  `metric_targets`: the field is skipped and logged, the rest applies.
- A Linear Project whose `content` holds KR markdown: read as nothing.
- `.lf/wave/<name>/metrics/*.md`: files nobody reads.
- An old `lf` on a store this build has written: Waves made here have no
  current Project, so it reports none selected. Not supported; the marks only
  delay the drop.

## Exclusions

- Dropping the marked columns and tables, rebuilding `waves`, the idle
  triggers and `wave_addresses`: the contract, after the window. Its export
  includes `projects.krs`: on the store measured above 47 of 64 Projects hold
  KR text, plus 4 instruments and 38 observations. Until then the text is in
  the leaving column and in the Linear Project's `content`, shown by no `lf`
  surface.
- A command that completes a Project without a tracker.
- `.lf/wave/**` content in this repository (`product/GOAL.md` "Chapter
  planning", `intelligence/metrics/*.md`, MEMORY notes) and `.lf/chapters/**`
  anywhere: Wave memory and other repositories' records.
- `release/v*/NOTES.md`, released migrations and their fixtures.
- Naming a Task's Project at creation, a Project per finishing group, Wave
  membership from a tracker link: the direction under discussion and LOO-980.

## Acceptance check at gate

Headless, in a fixture repository with the stand-in Linear:

```
lf wave create billing            # store: 0 projects rows for the Wave; no projectCreate sent
lf task create --wave billing x   # 1 projects row, status started; one projectCreate, then one issueCreate naming that Project
lf wave plan billing --json       # projects[0] has no current/krs/metric_targets; no metric_portfolio, project_readiness
lf help wave                      # no update-project, edit-project
lf wave plan bind billing x       # Clap: unrecognized subcommand
```

With a second `started` Project in the Wave (inserted as the stand-in Linear's
observation), `lf task create --wave billing y` prints the Project it joined,
and marking that Project completed in the stand-in sends the next Task to the
other one: a Project still ends, and is still listed with its status.

and, from the repository root,
`grep -ri 'key result\|metric_target' rust/ swift/ docs/` matches only
released migrations (`src/migrations/*.sql`, `tests/fixtures/migrations/`),
`migration_runner.rs`'s conversion of released data, and `SCHEMA.md`'s
leaving entries.

Demo: the first two commands on a fresh repository, then `lf wave status
billing` showing one Project and one Task with nothing about KRs or metrics;
on main the first command alone has already created a Linear Project.

## Slices

1. **A Wave with no selected Project, end to end.** `lf wave create` makes no
   Project; `new_task_project` answers Task creation and moves, and their
   output names the Project when the Wave has several open;
   `require_selected_project` and `project_selection.rs` go; `uncreated`
   tests the Project's stamp instead of `project_transitions`;
   `wave_project` leaves the plan record and the hub decodes per field;
   `remove_chapters.sql` exists with the selection marks. This is the slice
   that can prove the plan wrong: it shows whether anything still needs "the"
   Project (launch, Session membership, tracker export, the plan carriers'
   ordering), and whether the stamp test exports a Project made before a
   tracker was connected and one left unexported by the released `lf`
   (finding 1). Shown by the first two acceptance commands against the
   stand-in Linear and by `cargo test -p loopflow --lib task_graph_launch_tests::`
   on a Task whose Project is not the Wave's newest.
2. **Commands and activation.** The three commands, `wave.plan.bind`,
   `RunOwner::ProjectActivation`, `project_readiness` off the wire, Desktop's
   activation and readiness views, `ensure_wave_project` and its test callers.
3. **KRs and metric targets.** The `WorkValue` variants, Project fields,
   Linear `content`, tracker action subjects, agent context text, wire fields,
   Desktop KR rows, finding 3's private conversion.
4. **Metrics.** The five metric modules, `context_cost` publishing, the
   portfolio on the wire and in Desktop, the demo world, then docs, builtins
   and the remaining marks in one pass, with the grep.

Each slice regenerates `server_schema.json` and `SCHEMA.md` if it touched
them and deletes the tests of what it removed.

### Status (2026-11-18)

Slices 1–4 are built. What they settled:

- Slice 2: the Project a new Task joins, and the Project made for a Wave
  with none, are written in the Task's own save or move transaction
  (`join_project_in`), so a failed save leaves no Project.
- Slice 3: a Linear Project's `content` is still read, only to keep the
  released `## KRs` / `## Metric targets` sections out of the description
  and in place when it is edited; the reader's KRs and raw target JSON serve
  released-store conversion only. Saved values carrying removed fields decode
  without them (`work::edit::known_values`); queued KR/target actions are
  retired by `remove_chapters.sql`.
- Slice 4: `project_changes` is not marked here; it leaves with
  `task_changes` under the tracker-queue contract.
  `tests/e2e/linear_oauth.py` lost its chapter seed, but it still seeds
  `pm_snapshots` and runs `lf status`, so it was stale before this change.

Left for gate: the acceptance commands above against the stand-in Linear,
and the Desktop suites named in scripts/testing.md.

## Docs written ahead of the code (2026-11-18)

They describe the behavior above as intended; the implement steps read them.

- `docs/work.md`: "Plan it: KRs and targets" is now "Projects" (example, which
  Project a new Task joins, where Projects start and end); KR and metric
  mentions gone.
- `docs/dev/planning.md`: intro and Words row; "Plan creation and selection"
  and "Replace a Wave's Project, keep the work" are one section, "Projects of
  a Wave"; the KR/target field paragraphs gone.
- `docs/glossary.md` (KR row → Project row), `docs/agent-api.md`,
  `docs/index.md`, `docs/orchestration.md`, `docs/dev/machines.md`,
  `docs/dev/data.md`, `docs/dev/reference-models.md`, `docs/dev/index.md` and
  regenerated `index.html`, `AGENTS.md` Work model, `PROMPTS.md`,
  `swift/README.md`, `.lf/qa/tracker.md`.

Left for slice 4, because they follow the code: `docs/dev/reference.md` rows
84-85, 199, 217 and `reference-code.md` 57, 70-75, 82 (the architecture check
ties them to live tables), the builtin skills, `SCHEMA.md`.

Settled while writing: Task creation prints `<title> · <id> in Project <name>`
when the Wave has more than one open Project.

Open, from writing `docs/work.md` "Projects": with `edit-project` gone and no
tracker, no `lf` command renames, adds or ends a Project, so such a Wave keeps
the one Project its first Task made, named for the Wave. The docs say so. Is
that the wanted no-tracker behavior, or does a rename stay?
