# Cut 3 — saved Flow reviews are rows; one Session list

2026-09-27 · LOO-298 · One bounded implementation Run on top of Cut 2
(`2430119a9`). Committed with plain git. Nothing published, installed or rebased.

## Owners now

| Fact | Owner |
| --- | --- |
| Saved Flow review: title + provenance, feedback, completion | `sessions` row, `kind='flow_review'`, id `flow:<invocation>:<boundary>` |
| Review attempt: provider, model, cwd, Skill, Wave, node, iterations, attempt | `runs` row with `invocation_id` set and `task_id` null |
| The invocation a review's Runs name | `flow_invocations` row with `task_id` null: capture and the cursor of the review, `pending_session_id`, `current_run_id`. Written when the Flow first waits at a review, rewritten at each later review, `completed` when the Flow finishes |
| Attempt history | `runs.session_id`; replacement is `replace_session_run`, which also moves the invocation's `current_run_id` |
| Task review provider and model | `runs.provider`, `runs.model`, written when the review Run publishes |
| Saved Flow execution: cursor, headless boundary, failure, finished | `flows/<id>/position.json`, unchanged. It no longer holds a review's Run, feedback or outcome |
| Launch exclusion | `human-sessions/.<hash>.launch.lock`, unchanged |
| Rows the store could not take | Nothing. The sidecar is gone |

### Operations

| Operation | Path |
| --- | --- |
| Flow waits at a review | `CliFlowExecutor::run_skill` → `flow_session::reserve` → `create_session(session, run, Some((invocation, cursor)))`: invocation, Session and first Run in one transaction |
| List | `store.open_sessions()`, one query over `sessions` joined to `runs`, ordered in SQL → `surface`, one function for every kind |
| Find | `store.session` or `store.session_for_run`, then `owned_target`: a Task review is `Flow`, every other Session is `Row` |
| Rename, ready, complete | `rename_session`, `ready_session`, `complete_session` for every kind but a Task review's completion |
| Open an Ask or a saved Flow review | `open_waiting`: native resume, else the prepared Run, else `replace_session_run` |
| Flow resumes after review | `reserve` returns the completed Session's `ready_summary` as the step's feedback |
| Store cannot be written | Interactive launch proceeds, prints `warning: this Session is not recorded and will not list: <error>`, stores nothing. A Flow at a review prints the same warning and waits. `lf ask` is refused |

No schema draft was added. No DTO field changed shape; Swift and
`tests/fixtures/dto` are untouched. `docs/architecture/data.md` lost its
`unrecorded-session.json` row.

## Deleted (path:line at HEAD `2430119a9`)

`rust/loopflow/src/ops/human_session.rs`:

| Item | Line |
| --- | --- |
| `flow_actions` | 202 |
| `SessionFlowMembership::of_step` | 276 |
| `run_flow_membership` (manifest membership, reads `position.json`) | 292 |
| `SessionTarget::StandaloneFlow`, `::Interactive`, `::Ask` (now `Row`) | 342–355 |
| `record_ask` | 482 |
| `prepare_ask_run` (now `prepare_run`, shared with reviews) | 516 |
| Three-source `list` and its sort | 562 |
| `parse_id` and `find_boundary_for_run` branches of `find_session` | 588–593 |
| `session_of_run` (sidecar replay on lookup) | 610 |
| `ask_attempt` (sidecar read) | 948 |
| `serve_ask_locked` (now `serve_locked`, shared with reviews) | 960 |
| `carry_session_name` | 1272 |
| `conversation_surface` | 1313 |
| `wire_title_source` | 1384 |
| `attribute_standalone_session` | 1391 |
| `session_wave_id` | 1410 |
| `session_name` (seed fallback) | 1445 |
| `recorded_provider` | 1462 |
| Sidecar tail of `rename` (`write_session_name`) | 1523–1528 |
| `boundary_id` | 1532 |
| `find_boundary_for_run` | 1543 |
| `native_session_state` | 1665 |
| `list_flow_sessions` | 1704 |
| `review_surface` | 1712 |
| `find_flow_session`, `find_flow_session_optional` | 1801, 1807 |
| Sidecar polling in `wait_for_ask` | 2157 |

`rust/loopflow/src/ops/flow_session.rs`: `parse_id` 17, `validate` 32,
`worktree` 70, `list` 76, `reviews` (directory scan) 86, `surface` 122,
`prepare_run` 127, `session_surface` 169, `mark_ready` 206,
`record_completion` 222, `recover_unpublished_run` 276, `open` 310.

`rust/loopflow/src/run_record.rs`: `RunFlowStep::occurrence` 95,
`SessionName` 972, `SessionNameRecord` 978, `read_session_name` 1005,
`write_session_name` 1028, and their two tests 2493, 2520.

Elsewhere: `ops/unrecorded_session.rs` (whole file, 63 lines);
`Boundary::ready_summary` `ops/flow_run.rs:29`; `open_conversations` and
`open_review_sessions` `store/sqlite/sessions.rs:229,257`,
`store/sessions.rs:61,74`; test
`ordinary_flow_parks_at_the_same_durable_review_after_recovery`
`lf/commands/flow.rs:687`.

`session-name.json`, `.session-name.lock` and `unrecorded-session.json` have no
reader or writer left.

## Line counts (lines before `mod tests {`)

| File | Before | After |
| --- | --- | --- |
| `ops/human_session.rs` | 2,217 | **1,866** |
| `ops/flow_session.rs` | 403 | **193** |
| `run_record.rs` | 2,384 | **2,282** |

**Net production delta: −635 lines.** Per changed file under `src/`:
`human_session.rs` −351, `flow_session.rs` −210, `run_record.rs` −102,
`unrecorded_session.rs` −63, `run.rs` −5, `flow_run.rs` −3, `ops/mod.rs` −1,
`store/sqlite/sessions.rs` +66, `lf/commands/flow.rs` +18, `store/sessions.rs`
+16, `ask.rs` 0, `session.rs` 0. `controller/task/mod.rs` (+4) and
`store/sqlite/durable.rs` (+6) changed only inside their test modules and are
not counted. Whole diff before this report: +1,088 / −1,860 over 18 files.

**Plainly:** the store grew by 82 lines to take the invocation upsert, the list
query, `waiting_flow` and `end_flow`. `run_record.rs` lost the name sidecar and
nothing else; its subject readers stay.

## Could not delete — exact remaining callers

| Kept | Remaining caller |
| --- | --- |
| `attributed_work`, `preferred_work_selector` | `human_session::caller_work` for a headless caller with no Run row (`human_session.rs:402`); `run_record/active.rs:229` |
| `session_work_path` | `human_session::surface`. It reads the Run's Task and Wave ids and looks up their names; the row stores ids, not labels |
| `flows/<id>/position.json` and `flow_run::{read,write,update}` | The saved Flow driver, `lf/commands/flow.rs`; `flow_session::waiting` reads the pinned Skill, selectors and message from it to launch and complete a review |
| Flow review reads of `position.json` at launch | `flow_session::{pinned_skill, membership, launch, complete}`. List, find, rename and ready read none of it |
| `prepared` marker and `run_is_prepared` | `ask_once`, `serve_ask`, `open_waiting`, `spawn_session_run` |
| `resolve_manifest` in `find_session` | Run-id prefix selectors |
| `NativeRun` | Interactive `open` and `complete` |
| `RunFlowStep::boundary_key` | Serialized in every Run manifest; no reader in `src/` |
| `record_task_start` (Started event) | `lf/commands/run.rs:643,705`; headless launches still have no Run row |
| `human-sessions/` directory | Launch lock files only |
| Task review position reads in `surface` and `owned_target` | `store.flow_position`: an unreadable capture must disable the review's actions and name the reason |

## Commands and results

Ambient `LF_*` cleared for every cargo command; `-j 4`, `nice +10`. Load 30–50.

| Command | Result |
| --- | --- |
| `cargo nextest run -p loopflow --test session_cutover_tests` before production edits | 3 passed, **3 failed**, 1 never finished: both review tests `the waiting review has no rows`; the launch test found no warning; the Ask test waited on an `lf ask` that did not refuse. Run killed after 8 minutes |
| Same, first run after | 5 passed, **2 failed**: replacement refused with `new Run attempt ordinal is assigned by the writer`; expected `iterations` was `[]`, actual `[[]]`. Fixed `prepare_run` and the test's expectation |
| Two review tests after the fix | 2 passed |
| `cargo nextest run -p loopflow --lib -E 'test(ops::human_session) \| test(ops::flow_session) \| test(ops::flow_run) \| test(run_record) \| test(store::sqlite) \| test(store::migrations)'` | First: 164 passed, **2 failed**. `human_provider_exit_never_completes_the_review` read a prepared Run from the boundary, which no longer holds one; rewritten to assert a review's launch binds nothing. `keyed_asks_join_recover_completion…` lost a race, see below. Final: **166 passed, 0 failed** |
| `cargo nextest run -p loopflow --test session_cutover_tests --test session_cli_tests --test dto_fixtures` | **22 passed, 0 failed** (7 + 6 + 9) |
| `cargo nextest run -p loopflow --lib --test flow_tests -E 'test(controller::task) \| test(lf::commands::flow) \| test(lf::commands::session) \| test(lf::commands::run) \| binary(flow_tests)'` | **79 passed, 0 failed**. Not in the brief; run because `surface` replaced the Task review surface |
| `cargo fmt --all --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | First: 1 error in a new test (`[run.clone()]`), fixed. Final: pass, on the committed content |
| `scripts/check_migrations.py` | 52 shipped migrations unchanged |

Providers and tmux are shell stand-ins. No installed Home, real provider or
desktop ran. The draft migrations were not materialized or rehearsed here.

### The keyed Ask test

`keyed_asks_join_recover_completion_and_keep_boundaries_independent` starts two
`ask_once` calls with one key and asserted that the first call's question is
the stored one. Both calls race for the launch lock on blocking threads. It
failed 1 of 1 in the suite and 2 of 3 alone. The test now starts the retry once
the first request's Session is stored. Whether `2430119a9` has the same race
was not established: it was not rebuilt.

## Not proven

- A saved Flow with more than one review, through the binary. The second
  review's upsert of the invocation row is exercised by no test.
- A saved Flow launched with `--wave` or `--as`. `--task` is covered by
  `flow_tests`.
- Remote Task reviews through `surface`; no test places a Task on another Home.
- A Flow at a review with an unwritable store. The warning path is not tested.

## Decisions made here (also in `scratch/questions.md`)

Jack has reviewed none of these.

- No sidecar. A Session the store could not take is never recorded.
- `lf ask` is refused when the store cannot be written, because its answer
  returns through its row. This narrows the rule in the brief.
- A saved Flow's review carries its declared Work's Wave and no Task. `work` in
  the Session DTO changed from the Task to the Wave for `lf --task X flow …`;
  `flow_tests` was changed to expect it.
- The saved Flow's invocation row is a copy taken at each review. Its cursor
  has two owners until the driver cut.
- A plain `lf ask` whose launcher fails leaves its stored Ask open.
- Readiness no longer takes the launch lock.
- Task review Sessions report `terminal_ids`; interactive and Ask Sessions
  whose Run has no provider list as `waiting` instead of failing the list.
- Old Homes: saved Flow reviews that exist only in `position.json` stop listing
  until the Flow is resumed or imported, and `session-name.json` titles are not
  read. Sessions left in `unrecorded-session.json` are never recorded.
