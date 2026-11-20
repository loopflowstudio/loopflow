# LOO-995 — Remove chapters, KRs and metrics

Stacked on LOO-938 (PR #1971); LOO-938's plan and notes live on its own branch.

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

`projects`, `tasks.project_id → projects.wave_id`, Project name, summary,
description, status, rank, `work_state`, `work_terminal_at`,
`tracker_wave_projects` and their tracker sync both ways stay. What goes is
everything that selects one Project of a Wave, activates or rolls it over, or
hangs KRs and metrics on a Project or a Wave.

## Built (Slices 1–4 + merge with LOO-938)

- **Waves select no Project (`rust/loopflow-storage/src/sqlite/children.rs`,
  `rust/loopflow-execution/src/ops/wave_lifecycle.rs`, `ops/task.rs`):**
  - `lf wave create <name>` registers the Wave and commits `.lf/wave/<name>/`
    with zero `projects` rows and no tracker `projectCreate`.
  - A new or moved Task in a Wave asks `new_task_project`: the Wave's newest
    `started` Project, else its newest `backlog`, `planned` or `paused` one
    (by `created_at DESC, id DESC`), else a `started` Project named for the
    Wave created inside the same save or move transaction (`join_project_in`
    in `promote_task_prepared` and `move_task_to_wave`), so a failed save
    leaves no orphan Project.
  - Measured on one store written by main (62 live Waves, 64 Projects,
    2026-11-18): of the 17 Waves with a `current_project_id`, this rule
    answers that Project in 17 ("oldest started" in 15); 22 Waves have one
    open Project, 5 have two, 1 has three, 2 have two `started`, and 45 live
    Waves have none and get one on their first Task.
  - `lf task create` and `lf task move` append ` in Project <name>`
    (`joined_project_suffix`) when the Wave holds more than one open Project.
  - A Task in any Project of its Wave launches without a selected-Project or
    In-Progress Project gate (`project_selection.rs`,
    `require_selected_project`, `require_current_task_project` and
    `RunOwner::ProjectActivation` are gone).
- **Tracker creation and cross-build plan records
  (`rust/loopflow-storage/src/sqlite/tracker_actions.rs`, `work/edit.rs`):**
  - `uncreated` tests `plan_stamps` (`project_wave`) instead of
    `project_transitions`: a Project is exported to Linear/Asana unless
    another Machine stamped its Wave, so unstamped Projects from released
    stores and Projects made before connecting a tracker still queue a
    `create`.
  - `WorkValue::WaveProject`, `Krs` and `MetricTargets` are gone.
    `StampedRecord` (`known_fields`) and `work::edit::known_values` skip and
    log unknown fields per entry so records or saved `create` baselines from
    older builds still apply across both Git and hub carriers.
- **Commands, wire types and Desktop (`rust/loopflow/src/lf/`, `swift/`,
  `tests/fixtures/dto/`):**
  - `lf wave update-project`, `lf wave edit-project`, `lf wave plan bind` and
    served `wave.plan.bind` are gone (`lf wave plan` keeps `connect` and
    `reteam`).
  - `WaveDetailSnapshot`, `WaveRoadmap`, `ProjectSummary`, `Project` and
    `WaveAdded` drop `project_readiness`, `metric_portfolio`, `current`, `krs`,
    `metric_targets` and `WaveAdded.project`. `print_projects` prints
    `Project   <name> (<id>) · <status>` without "current" or "historical".
  - Desktop drops `MetricPortfolio.swift`, `ProjectReadinessView.swift`,
    `WaveMetricPortfolioView`, activation actions and KR rows;
    `ProjectHistoryView` is now `WaveProjectsView` ("Projects" button in
    `WaveDetailPane` and `WaveStandupBoard`), and `waveForProject` matches any
    of a Wave's Projects.
- **Linear `content`, upgrades, live metrics and agent context
  (`rust/loopflow-storage/src/planning/project_document.rs`,
  `migration_runner.rs`, `rust/loopflow-execution/src/linear/mod.rs`,
  `ops/run.rs`, `rust/loopflow/src/lf/commands/context_cost.rs`):**
  - Nothing writes KRs or metric targets to Linear `content`.
    `parse_project_document` still parses `content` so released `## KRs`,
    `## Metric targets` and `workflow:` sections stay out of
    `Project.description` (and agent context) and survive description edits;
    `ReleasedKr` and `released_targets` serve `migration_runner`'s upgrade
    conversion of released stores only.
  - All five metric modules (`loopflow-storage/src/metric.rs`,
    `sqlite/metrics.rs`, `ops/metrics.rs`, `work/wave/metrics.rs`,
    `store/metrics.rs`), `World::metric`, `World::plan` and
    `context_cost::publish` are gone (`lf history usage --weekly` still prints
    its report). Agent context for Tasks and Waves carries no KRs, metric
    targets or `<lf:metric-portfolio>`.
- **Schema and docs (`rust/loopflow-storage/src/migrations/remove_chapters.sql`,
  `SCHEMA.md`, `docs/`, builtins):**
  - `remove_chapters.sql` retires queued `krs` / `metric_targets` rows in
    `tracker_actions` and marks `waves.current_project_id`, the seven
    `waves.project_activation_*`, `project_transitions`,
    `projects.{krs,metric_targets,legacy_current,iteration}`,
    `tracker_projects.{krs,metric_targets}`, `pm_projects.{krs,metric_targets}`,
    `metric_instruments` and `metric_observations` leaving under `LOO-995`.
    (`project_changes` stays unmarked here; it leaves with `task_changes`
    under the tracker-queue contract.)
  - User docs, architecture docs (`docs/dev/reference.md`,
    `reference-models.md`, `reference-code.md`, `planning.md`) and builtin
    skills match the built code.

## Exclusions

- Dropping the marked columns and tables, rebuilding `waves`, the idle triggers
  and `wave_addresses`: the contract migration after `STAGING_DAYS`, exporting
  `projects.krs` (47 of 64 Projects on the measured store hold KR text, plus 4
  instruments and 38 observations).
- Completing, adding or renaming a Project without a tracker (see
  `scratch/questions.md`).
- Authored `.lf/wave/**` content in this repository, `.lf/chapters/**` in other
  repositories, `release/v*/NOTES.md`, and released migrations and their
  fixtures.
- Naming a Task's Project at creation (`--project`), Projects as finishing
  groups, and Wave membership from a tracker link (LOO-980).

## Remaining work (later Graph steps)

All implementation slices are complete.

### For `compress`

- `SqliteStore::join_project` (`rust/loopflow-storage/src/sqlite/children.rs:1288`)
  and the `name: Option<&str>` parameter on `join_project_in`
  (`children.rs:2472`) were added in slice 1 before slice 2 moved Task creation
  and moves onto `join_project_in(&tx, wave, None)`. `join_project` now has
  only test callers (`test_fixture.rs`, `test_support::seed_wave_project` and
  unit/integration tests): move the named seeding helper behind test support /
  `#[cfg(test)]` so `join_project_in` takes `(conn, wave)` only.
- Note: `SqliteStore::edit_project`
  (`rust/loopflow-storage/src/sqlite/project_content.rs:21`) also has only test
  callers since `lf wave edit-project` went; `scratch/questions.md` keeps it
  pending the open local-rename question unless `compress` folds its test uses
  into `planning_write` directly.

### Checks recorded and deferred

- `cargo check --all-targets`: passed (all 7 workspace crates, post-merge with
  LOO-938 `c2beea658a`).
- `cargo test -p loopflow --test integration dto_fixtures`: passed (47/47).
- `uv run python scripts/schema_page.py --check`: passed.
- `check_migrations` `_check_staging` and `_check_runtime_sql` against
  `origin/main` merge-base: passed (`['plan_hub.sql', 'remove_chapters.sql']`);
  full `uv run python scripts/check_migrations.py` waits on LOO-938 rebasing
  onto `origin/main` (`task_auto.sql`).
- `grep -ri 'key result\|metric_target' rust/ swift/ docs/`: matches only
  released migrations (`src/migrations/*.sql`, `tests/fixtures/migrations/`),
  `remove_chapters.sql`, `migration_runner.rs`'s conversion of released data,
  `linear_migration_tests.rs`'s released-schema upgrade test, and
  `SCHEMA.md`'s leaving marks.
- Deferred to `gate` / CI:
  - Acceptance flow in a fixture repository with stand-in Linear:
    `lf wave create billing` (0 `projects` rows, no `projectCreate`),
    `lf task create --wave billing x` (1 `projects` row `started`,
    `projectCreate` then `issueCreate`), `lf wave plan billing --json` (no
    `current`/`krs`/`metric_targets`/`metric_portfolio`/`project_readiness`),
    `lf help wave` (no `update-project`/`edit-project`),
    `lf wave plan bind billing x` (unrecognized subcommand), and second
    `started` Project join/completion check.
  - `cargo test -p loopflow --lib task_graph_launch_tests::`
  - `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  - Swift / Desktop suites (`scripts/testing.md`) in CI / macOS test runner.
