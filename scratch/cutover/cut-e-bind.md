# Cut E — started, bind, Task Runs

2026-09-27 · LOO-298 · One bounded implementation Run on top of Cut 3
(`ae6d5031f`). Committed with plain git. Nothing published, installed or rebased.

## Owners now

| Fact | Owner |
| --- | --- |
| A Task is started | A `runs` row names it |
| When it started | `tasks.started_at`, written once by the Run insert and bind triggers of the `record_task_first_run` draft, at the time of that write |
| Started agrees with Runs | The same draft's `validate_task_started_insert` and `validate_task_started_update` triggers; `retain_task_first_run` keeps the last Run |
| A Session's Task | Its Runs' `task_id`; bind fills it, `work_source='bound'` |
| A Task's Runs | `SELECT … FROM runs WHERE task_id=? AND published=1` |
| A Run's usage, outcome, subjects | Its record under `runs/<prefix>/<run-id>/`, unchanged |
| A direct launch that names Work and has no Session | A `runs` row with `session_id` null, written when its record is published |

### Operations

| Operation | Path |
| --- | --- |
| `lf session bind <session> --task <issue>` | `human_session::bind` → `store.bind_session` → `runs::bind_session_runs_in`, one immediate transaction |
| Refusals | The Session already has a Task (the message names it); a Run's Wave is not the Task's Wave; the Session is a Flow review |
| Wave filled upward | `runs::task_wave_in`, shared by the Run constructor and bind |
| `lf runs --task <issue> --json` | `runs::collect_task_runs` → `store.task_runs`, then each Run's own record for its evidence |
| Headless or IDE launch with Work | `run::begin_run_capture` → `record_run` → `store.create_run` |

No schema draft was added or changed: the column and its triggers were already
in `record_task_first_run`. No DTO changed shape; Swift and
`tests/fixtures/dto` are untouched.

## Deleted

| Item | Where (at `ae6d5031f`) |
| --- | --- |
| `record_task_start` and its two callers | `lf/commands/run.rs:929`, `:643`, `:705` |
| `begin_chapter_task` (the launch's Started event writer) | `store/sqlite/chapters.rs:80` |
| `interactive_run` (now `launch_run`, shared with headless launches) | `lf/commands/run.rs:876` |
| The catalog and seven-day scan behind `lf runs --task` | `lf/commands/runs.rs` `list` |

`begin_chapter_task` also refused a launch whose Task was not ready or whose
Chapter was not current. Those two refusals went with it.

## Line counts (lines before the test module)

| File | Before | After |
| --- | --- | --- |
| `store/sqlite/runs.rs` | 268 | 343 |
| `lf/commands/runs.rs` | 367 | 400 |
| `ops/human_session.rs` | 1,865 | 1,888 |
| `lf/commands/run.rs` | 1,243 | 1,264 |
| `lf/commands/session.rs` | 206 | 220 |
| `store/sessions.rs` | 129 | 140 |
| `store/sqlite/sessions.rs` | 548 | 559 |
| `lf/mod.rs` | 1,853 | 1,862 |
| `store/sqlite/chapters.rs` | 148 | 134 |

**Net production delta: +183 lines.** This cut adds a command, a query and a
writer; it deletes one writer.

## Could not delete — exact remaining readers

| Kept | Remaining reader |
| --- | --- |
| Started events in `task_events` | `task_started`, `chapter_task_evidence` and `retire_chapter_backlog` (`store/sqlite/chapters.rs`) still read them beside `started_at`. The `task_chapter_started` trigger still writes one when a Task worker's generation rises. Old Homes hold starts that have no Run row |
| `WorkCatalog::matches_run` and `scan_runs_since` | `lf runs --wave`, `--project`, `lf usage`, `lf activity`, `lf status` |

## Commands and results

Ambient `LF_*` cleared for every cargo command; `-j 4`, `nice +10`.

| Command | Result |
| --- | --- |
| `cargo nextest run -p loopflow --test session_cutover_tests -E 'test(binding_an_orphan)'` | 1 passed. It was written after the production edits and never seen failing |
| `cargo nextest run -p loopflow --no-fail-fast --test session_cutover_tests --test session_cli_tests --test dto_fixtures --test flow_tests` | **39 passed, 0 failed** |
| `cargo nextest run -p loopflow --no-fail-fast --lib -E 'test(ops::human_session) \| test(ops::flow_session) \| test(ops::flow_run) \| test(run_record) \| test(store::sqlite) \| test(store::migrations) \| test(controller::task) \| test(lf::commands::run) \| test(lf::commands::session) \| test(store::tests) \| test(ops::chapter)'` | **266 passed, 0 failed**, before the store bind test was added |
| `cargo nextest run -p loopflow --lib -E 'test(bind_fills_every_run)'` | 1 passed |
| `cargo fmt --all --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `scripts/check_migrations.py` | 52 shipped migrations unchanged |

Providers and tmux are shell stand-ins. No installed Home, real provider or
desktop ran.

## Tests changed

- `flow_tests::observing_and_preparing_a_task_are_not_execution` and
  `lf_launches_inside_a_task_checkout_bind_to_that_task` counted Started events
  in `task_events`. A direct launch no longer writes that event, so they now
  read `tasks.started_at`. The first also asserts a second launch leaves the
  value unchanged. Expected counts are the same.
- New: `session_cutover_tests::binding_an_orphan_session_starts_its_task_once`
  and `durable_store_tests::bind_fills_every_run_of_a_session_and_refuses_another_wave`.

## Not proven

- Bind of a closed Session, of a Wave-only Session in the Task's Wave, and the
  Flow review refusal, through the binary. The store test covers a replaced
  Run and the other-Wave refusal only.
- Two binds racing for one Session. The store's existing
  `competing_run_assignments_start_only_the_winning_task` covers the triggers,
  not `bind_session`.
- `lf runs --task` for a Task whose Runs were launched on another Home.
- A headless launch with an unwritable store. It prints a warning and
  proceeds; no test forces it.

## Decisions made here (also in `scratch/questions.md`)

Jack has reviewed none of these.

- Bind asks for no confirmation. The spec said to confirm once.
- Bind refuses when any Run of the Session has a Task, including the same Task.
- Bind refuses a Flow review Session.
- `lf runs --task` lists only Runs that have a row, with no seven-day window
  and no cap. Runs begun by the Wave runner, PR landing and child agents have
  no row and no longer list under `--task`.
- A direct launch gets a row only when it names Work.
- A saved Flow's headless step launched with `--task` stores a Run naming the
  Task and no invocation.
- A launch is no longer refused for a Task that is not ready or whose Chapter
  is not current.
- `lf session list --task` was not added.
