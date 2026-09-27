# Cut F — every Run is a row; readers read rows

2026-09-27 · LOO-298 · One bounded implementation Run on top of Cut D
(`7714e218d`). Committed with plain git. Nothing published, installed or rebased.

## Owners now

| Fact | Owner |
| --- | --- |
| A Run exists, and its Session, invocation, Task, Wave and caller | Its `runs` row |
| A Run's provider, model and start | `runs.provider`, `runs.model`, `runs.created_at` |
| A Run's state | `runs.outcome` and `runs.ended_at`; both null until it settles |
| A Run's usage, events, final answer, repo, surface, launch request | Its record under `runs/<prefix>/<run-id>/`, unchanged |
| A step's attempts and the current one | `runs` rows at one `(invocation_id, node, iterations)`; `flow_invocations.current_run_id` |
| A saved Flow's invocation | `flow_invocations` row with no Task, written before each headless step and at each review |
| A Task is started | `tasks.started_at` or a worker claim; no reader asks `task_events` |
| Settlement receipt | `terminal.json`, still written first and exclusively. The row's end is written after it |

### Operations

| Operation | Path |
| --- | --- |
| Any launch outside a Session and a Task step | `CaptureHandle::begin_*` → `RunCapture::record_row` → `store.create_run` → `runs::insert_run_in` |
| A Task step | `bind_task_worker_run` → `insert_run_in`, in the claim's transaction; the controller then fills `provider` and `model` |
| A Session's Run | `create_session`, `replace_session_run`, `reserve_review_run` → `insert_run_in`, unchanged |
| A Run that names no Work | `create_run` copies its caller's Task and Wave, `work_source='inherited'` |
| A saved Flow's headless step | `CliFlowExecutor::run_skill` → `store.save_flow` → `run_saved(…, work)`; the Run names the invocation and becomes its current attempt |
| Settlement | `RunCapture::finish` writes `terminal.json`, then `store.end_run` |
| `lf runs`, `--wave`, `--project`, `--task`, `--parent`, `lf usage`, `lf status` Runs | `runs::collect_runs_at` → `store.runs(wave, project, task, caller, since)` → `run_record::run_snapshots` |
| `lf activity` | `store.runs(…)` in the snapshot's store, then the same `run_snapshots` |
| `lf runs --active` | each live Run's Work is `store.run(id)` |
| An Ask's Work | the asking Run's row |
| CI repair conclusions while landing | `store.runs(…, since)` filtered to the landing's worktree |
| Store cannot be written | The launch prints `warning: this Run is not recorded and will not list: <error>` once and runs. A failed end write logs and does not warn again |

`insert_run_in` is the one constructor. Every path above ends in it.

`lf ps` reads OS processes and Exec receipts. It read no Run attribution before
and reads none now.

### Schema

Two drafts, both new in this cut:

- `record_run_end`: `runs.outcome`, `runs.ended_at` and an index on
  `caller_run_id`.
- `name_tasks_on_flow_step_runs`: replaces the two `validate_run_parents`
  triggers. A Run in an invocation that names no Task may name one. An
  invocation that names a Task still refuses a different one.

No DTO changed shape. `RunSnapshot.subjects` is now built from the row's Wave,
Project and Task names; Swift and `tests/fixtures/dto` are untouched.

## Deleted (path:line at `7714e218d`)

| Item | Where |
| --- | --- |
| `scan_runs_since` | `run_record.rs:484` |
| `attributed_work` | `run_record.rs:2217` |
| `preferred_work_selector` | `run_record.rs:2262` |
| `caller_work` and its manifest path | `ops/human_session.rs:395` |
| `record_run` | `lf/commands/run.rs:916` |
| `lf/commands/run.rs` `RunWork` (now `session::RunWork`, shared) | `lf/commands/run.rs:177` |
| `collect_child_runs_at`, `collect_task_runs`, `collect_runs_started_since_at`, `collect_run_activity_since` | `lf/commands/runs.rs:73,88,125,143` |
| `collect_since_at` | `lf/commands/usage.rs:56` |
| `WorkCatalog::load`, `load_at`, `resolve_run`, `matches_run` | `lf/commands/work_catalog.rs:47,51,100,128` |
| `WorkFilter::matches` | `lf/commands/mod.rs:43` |
| `store.task_runs` (now `store.runs`) | `store/sqlite/runs.rs:316`, `store/sessions.rs:92` |
| The Run-record scan in `disposition` | `ops/chapter.rs:621–636` |
| Started-event clauses of `task_started`, `chapter_task_evidence`, `retire_chapter_backlog` | `store/sqlite/chapters.rs:85,97,125` |

## Line counts (lines before the first test module)

| File | Before | After | Delta |
| --- | --- | --- | --- |
| `store/sqlite/runs.rs` | 343 | 425 | +82 |
| `ops/session_import.rs` | 538 | 585 | +47 |
| `store/sessions.rs` | 155 | 187 | +32 |
| `run_record.rs` | 2,281 | 2,305 | +24 |
| `store/sqlite/sessions.rs` | 577 | 598 | +21 |
| `lf/commands/flow.rs` | 592 | 612 | +20 |
| `ops/pr_landing.rs` | 1,032 | 1,049 | +17 |
| `session.rs` | 67 | 83 | +16 |
| `controller/task/mod.rs` | 1,134 | 1,146 | +12 |
| `ops/flow_session.rs` | 192 | 204 | +12 |
| `controller/wave/runner.rs` | 1,143 | 1,153 | +10 |
| `lf/commands/activity.rs` | 403 | 410 | +7 |
| `run_record/active.rs` | 262 | 267 | +5 |
| `lf/commands/replay.rs` | 247 | 249 | +2 |
| `engine/agent.rs` | 2,213 | 2,214 | +1 |
| `store/sqlite.rs` | 1,922 | 1,923 | +1 |
| `store/sqlite/durable.rs` | 1,529 | 1,530 | +1 |
| `store/sqlite/chapters.rs` | 134 | 131 | −3 |
| `lf/commands/mod.rs` | 55 | 40 | −15 |
| `lf/commands/usage.rs` | 147 | 132 | −15 |
| `ops/chapter.rs` | 877 | 861 | −16 |
| `ops/human_session.rs` | 1,888 | 1,872 | −16 |
| `lf/commands/run.rs` | 1,264 | 1,224 | −40 |
| `lf/commands/runs.rs` | 400 | 357 | −43 |
| `lf/commands/work_catalog.rs` | 139 | 85 | −54 |

A file's production lines are those before its first `#[cfg(test)] mod`.
Files named `*_tests.rs` or under a `tests/` directory count as tests.

**Net production delta: +108 lines** over 25 files: +310 and −202. The store
took a listing query, the end write, inheritance and the current attempt. The
import took Runs. The readers and subject resolvers shrank. The two drafts are
41 lines of SQL and are not counted.

**Whole branch since the merge base with `origin/main` (`4cd64be3d`): +1,439.**
Cut 3 through Cut D reported their own deltas by another count of the same
files; by this count the branch stood at +1,331 before this cut.

## Could not delete — exact remaining readers

| Kept | Remaining reader |
| --- | --- |
| The `task_chapter_started` trigger and Started events | `chat/turns.rs:329` renders the event as "Task started" in a Wave's activity. No reader treats it as truth |
| `worker_generation>0` in `task_started` and `chapter_task_evidence` | A worker that has claimed and not yet launched. It is a claim, not an event |
| `resolve_manifest` | Run-id prefix selectors: `lf runs <prefix>`, `--parent`, `lf replay`, `find_session`; exact-id readers in `human_session`, `controller/task`, `start_prepared` |
| `record_dirs` | `resolve_manifest`, the import, and live-Run discovery |
| `RunSpec.subjects`, `RunManifest.subjects`, `PromptBuild.subjects`, `WorkBinding.subjects` | Written into every manifest; read by the import, by `ActiveRun.subjects` and by the prompt's Work context. No listing reads them |
| `RunFlowStep::boundary_key` | Serialized in every manifest; no reader in `src/` |
| The `prepared` marker and `run_is_prepared` | `ask_once`, `serve_ask`, `open_waiting`, `spawn_session_run`, `start_prepared` |
| `terminal.json` as state | `flow_run::recover`, `recover_task_decision`, `read_run_snapshot` for one Run, `reconcile_reserved_manifest` |
| `flows/<id>/position.json` | The saved Flow driver; see Cut G |
| `WorkCatalog` and `WorkOwner` | `lf activity` labels and filters PR, Steer and creation entries by them |

## Commands and results

Ambient `LF_*` cleared for every cargo command; `-j 4`, `nice +10`. Load 12–60.

| Command | Result |
| --- | --- |
| `cargo nextest run -p loopflow --test session_cutover_tests -E 'test(every_launch)'` before the Wave was resolved for an unbound launch | **1 failed**: the Wave-only headless Run's row had no Wave |
| Same, after | 1 passed |
| `cargo nextest run -p loopflow --no-fail-fast --test session_cutover_tests --test session_cli_tests --test dto_fixtures --test flow_tests`, first | 40 passed, **1 failed**: `flow_tests::bound_flows_keep_task_context…` expects every step Run of `lf --task INF-123 flow …` to name the Task; the step Runs named the invocation and no Task |
| `cargo nextest run -p loopflow --no-fail-fast --lib -E 'test(ops::) \| test(run_record) \| test(store::) \| test(controller::task) \| test(lf::commands)'`, first | 614 passed, **4 failed**; see Tests changed |
| The integration command, final content | **41 passed, 0 failed** |
| The lib command, final content | **618 passed, 0 failed** |
| `cargo fmt --all --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass, on the committed content |
| `scripts/check_migrations.py` | 52 shipped migrations unchanged |

Providers and tmux are shell stand-ins. No real provider or desktop ran.

## The installed Home, on a copy

Source, read only: `~/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`.
The copy held `loopflow.db` with its WAL, the Ask files, the Flow positions,
and from 414 Run records the manifest, terminal, name, provider-session,
resolution, prepared and client files. It held no event streams, so usage reads
as empty. The branch's debug `lf` ran against the copy with an empty
environment.

| Run | interactive | ask | flow review | Task review | run | unchanged | Tasks started | not imported |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `--dry-run` | 33 | 6 | 0 | 2 | 373 | 0 | 8 | 0 |
| first import | 33 | 6 | 0 | 2 | 373 | 0 | 8 | 0 |
| second import | 0 | 0 | 0 | 0 | 0 | 41 | 0 | 0 |

After: 416 Runs, 43 Sessions. 373 Runs have no Session, 360 have an end, 237
name a Task, 113 name a caller. 12 Tasks have `started_at` and no Task
disagrees with its Runs. `lf usage --days 0 --json` returned 414 Runs with no id
twice; the two rows it leaves out have no record on the copy. `lf runs --json`
returned 54.

The six Flow-step Runs Cut D reported as not imported are now Runs with no
Session.

## Tests changed

- New: `session_cutover_tests::every_launch_is_one_row_and_every_reader_lists_it_once`.
- `session_cutover_tests::launch_proceeds_when_the_store_cannot_be_written`
  gained a headless launch. `import_stores_each_old_session_once_with_its_name`
  gained two old Run records and asserts their rows.
- `store::…::headless_position_retains_attempts_and_rejects_the_replaced_run`
  gained the provider fill, the end write and the Task listing of a Task step.
- `store::…::run_constructor_infers_ancestors_and_rejects_conflicts_atomically`:
  a Run that names a Task inside an invocation that names none moved from the
  refused list to the accepted list. Two tests disagreed: this one refused it,
  and `flow_tests::bound_flows_keep_task_context…` requires it. The flow test
  describes what a person sees, so the constructor changed.
- `ops::run::tests::task_run_drill_includes_named_workers_and_opaque_helpers`
  became `task_run_drill_selects_rows_by_any_name_of_the_task`. It wrote subject
  strings and read them through `WorkCatalog`; it now stores rows and reads
  `store.runs`, with the same three selectors and the renamed Project.
- `run_record::active::tests::old_waiting_runs_are_task_exact…` registers its
  two Tasks. It named unregistered Task ids by subject string, and a row cannot
  name a Task that does not exist.
- `ops::pr_landing::tests` opens its store at the Home's `loopflow.db`.
- `run_record::tests::scanner_reduces_each_cumulative_stream_once…` became
  `reader_reduces…` and reads its one Run's record instead of scanning.
- `ops::task::tests::parked_human_boundary_reports_blockers_without_provider_preflight`
  **failed at `7714e218d` before this cut**; it was outside the earlier cuts'
  filters. It asserted `session_run_id.is_some()` for a review whose Run is
  reserved and unpublished. It now asserts the review's Session and its
  unpublished Run.
- Deleted, with the code they tested:
  `lf::commands::runs::tests::work_drill_reads_record_subjects_without_a_sql_ledger`,
  `child_drill_reads_the_exact_parent_without_time_or_count_caps`,
  `lf::commands::usage::tests::usage_reads_direct_bundle_evidence_without_a_sql_ledger`,
  `lf::commands::work_catalog::tests::shared_project_names_need_recorded_ancestry`,
  `ops::human_session::tests::task_is_the_most_specific_parent_run_subject`,
  and the `attributed_work` half of `session_work_labels_follow_stable_ancestry…`.
  The acceptance test covers the Work drill, the child drill and usage through
  the binary.

## Not proven

- A Task Flow step through the real binary. No test drives a Task worker
  through `lf`. The store test proves the step's row, provider, end and
  listing; `fill_run_provider` in the controller is exercised by no test.
- A Wave runner Run, a PR landing repair Run and an `lf ops` Run through the
  binary. Their launches pass typed Work to the same constructor; no test
  starts one.
- `lf status` Runs evidence and `lf runs --active` against rows through the
  binary. The unit test covers `--active`.
- A Run whose end write fails after its launch was recorded. It lists as
  unterminated until something rewrites the row; nothing does.
- A saved Flow resumed with `lf flow resume` after a restart, with `--task`.
  `flow_tests` covers the first drive.
- Unlimited history without the row cap: `lf runs --task` and `--parent` read
  every row and every record they name. The earlier child-drill test proved 51
  children; the acceptance test proves one.
- The draft migrations were not materialized or rehearsed beyond opening the
  Home copy.

## Decisions made here (also in `scratch/questions.md`)

Jack has reviewed none of these.

- A Flow's step Run may name a Task while its invocation names none.
- A saved Flow's review Runs still carry the Wave and no Task.
- An imported Run of a Flow step names its Task and Wave and no invocation.
- A Run that names no Work inherits its caller's Task and Wave.
- A Run whose store cannot be written is never recorded later.
- The row's end is truth for listings; `terminal.json` stays the settlement receipt.
- `RunSnapshot.subjects` comes from the row, so a bound or inherited Run lists
  its Work even when its manifest named none.
- `lf runs --wave`, `--project` and `lf usage` list only Runs that have a row.
- `worker_generation>0` still counts as started.
- The stale `parked_human_boundary` assertion was rewritten rather than left failing.
