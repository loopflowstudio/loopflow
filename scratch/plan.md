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

So `projects`, `tasks.project_id → projects.wave_id`, Project name, summary,
description, status, rank, and the tracker sync of all of them stay as they
are. What goes is everything that makes one Project of a Wave special, and
everything that hangs KRs and metrics on a Project or a Wave.

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
  summary, status and tracker sync. No `current`, `krs`, `metric_targets`,
  `metric_portfolio`, `project_readiness`.
- Desktop Wave detail: no Metrics section, no KR checklist, no activation
  spinner or retry. The "Project history" sheet stays as the Wave's Projects
  list (name, status, tracker sync). The standup board header names the Wave;
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
function instead, in `sqlite/children.rs` beside `list_projects`:

> the Wave's newest `started` Project; else its newest `backlog`, `planned` or
> `paused` one; else none. Newest by `created_at`, then `id`.

When it answers none, creating or moving a Task makes a Project named for the
Wave (`started`), in the same transaction, and queues its tracker `create`.
On stores written by main the answer equals `current_project_id` whenever that
Project is In Progress, which the old rule required.

*Passed over:* `--project` on `lf task create` (an addition; it belongs to the
Projects-as-finishing-groups direction still under discussion); refusing when
a Wave has several open Projects (adds a precondition); `tasks.wave_id` so a
Wave's Tasks need no Project (ruled out by the steer).

## Findings that shape the work

1. **`project_transitions` also gates tracker export.** `uncreated`
   (`loopflow-storage/src/sqlite/tracker_actions.rs:980-992`) exports a local
   Project only when a `project_transitions` row with `local_plan_json` names
   it; `ensure_project` (`sqlite/project_selection.rs:123-229`) writes that
   row. Without the table a Project made here would never reach Linear. The
   Project half of `uncreated` goes; creating a Project queues its `create`
   action in the creating transaction. That also keeps a Project that arrived
   from another Machine by Git or hub from being created twice. A Project made
   offline by the released `lf` and still unexported at upgrade has only its
   transition row: the migration cannot build the action's baseline
   (`creation_baseline` is Rust), so the first open after upgrade is where it
   is queued, or it is left and said so. Slice 1 settles which by running it.
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
  `resolve_project*`, `ProjectTarget`, `edit`, `readiness`. What is left of the
  file is the one call to the new storage function, so the file goes too.
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
  `ProjectHistoryView` loses its KR list and its default to the current
  Project. Tests in `DTOFixtureTests`, `WaveDetailReadingTests`,
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
  triggers and `wave_addresses`: the contract, after the window.
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
lf task create --wave billing x   # 1 projects row, status started; one projectCreate, one issueCreate
lf wave plan billing --json       # projects[0] has no current/krs/metric_targets; no metric_portfolio, project_readiness
lf help wave                      # no update-project, edit-project
lf wave plan bind billing x       # Clap: unrecognized subcommand
```

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
   Project; the storage function above answers Task creation and moves;
   `require_selected_project` and `project_selection.rs` go;
   Project creation queues its tracker `create`; `uncreated` loses its Project
   half; `wave_project` leaves the plan record and the hub decodes per field;
   `remove_chapters.sql` exists with the selection marks. This is the slice
   that can prove the plan wrong: it shows whether anything still needs "the"
   Project (launch, Session membership, tracker export, the plan carriers'
   ordering), and what happens to a Project left unexported by the released
   `lf` (finding 1). Shown by the first two acceptance commands against the
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
