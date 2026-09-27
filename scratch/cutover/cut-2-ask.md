# Cut 2 — Ask Sessions are rows

2026-09-27 · LOO-298 · One bounded implementation Run on top of Cut 1
(`d814eb617`). Nothing committed, published, installed or rebased. Tree left
dirty for the parent.

## Owners now

| Fact | Owner |
| --- | --- |
| Ask conversation: title + provenance, question, feedback, answer, completion | `sessions` row, `kind='ask'`. Question is the new `sessions.request`; the answer is `ready_summary` of a completed row |
| Ask attempt: provider, model, cwd, selected Skill, Task/Wave, caller | `runs` row. Skill is `runs.skill`; caller is the new `runs.caller_run_id`; `invocation_id` is always null; `work_source='inherited'` when the caller had Work |
| Keyed retry identity | The Session id itself: `ask_once_<sha256(key)>`. `create_session` returns the existing row for an id it already holds. No separate key column |
| Attempt history | `runs.session_id`; replacement is `replace_session_run`, fenced on the expected current Run. Title and feedback are not touched |
| Launch exclusion | `human-sessions/.<hash>.launch.lock` files. Process exclusion only, no contents |
| Rows the store could not take | `<run dir>/unrecorded-session.json`, written once, removed when recorded |

`ask_exchanges` is not an owner: it was last rewritten in `0.12.8` and has no
reader or writer in `src/` outside shipped migrations. Nothing of Ask lives there.

### Operations

| Operation | Path |
| --- | --- |
| `lf ask` | `reserve_ask` prepares the Run and writes the reservation beside it → `launch_ask` → `unrecorded_session::record` stores Session + Run in one transaction through `insert_run_in` → `wait_for_ask` polls the row |
| Keyed (`ask_once`) | Under the launch lock: existing row, else reserve + record. Completed rows return their answer without a launch |
| List | `store.open_conversations()` (`kind IN ('interactive','ask')`, open) → `conversation_surface`, one function for both kinds |
| Find | `find_session` → `owned_target` on `sessions.kind`; Run id and prefix resolve through `runs.session_id` |
| Rename | `store.rename_session`, same path as interactive |
| Ready | `store.ready_session`, fenced on the current Run; stale Runs are refused |
| Complete | `store.complete_session`; an Ask closes only with `ready_summary` present |
| Open | Native resume, else launch the prepared Run, else append a replacement Run |

Work inheritance reads the caller's `runs` row. A headless caller has no row
yet, so its manifest attribution is used until the headless cut.

Schema: draft `record_ask_sessions` adds `sessions.request` and
`runs.caller_run_id`. No DTO field changed; Swift and `tests/fixtures/dto` are
untouched. `docs/architecture/data.md` filesystem table updated.

## Reversal of Cut 1: bookkeeping never refuses a launch

`reserve_interactive_session` no longer fails the launch. If the store cannot
take the rows, the reservation is written beside the Run and the launch
proceeds. `session_of_run` records it the next time an `lf` operation resolves
that Run (rename, open, complete, ready by Run id or prefix). Until then the
Session does not list. `lf ask` follows the same rule: it opens the store
optionally, launches the conversation, and its waiting caller does not
initialize a store itself.

## Deleted (path:line at HEAD `d814eb617`)

All in `rust/loopflow/src/ops/human_session.rs` unless named.

| Item | Location |
| --- | --- |
| `AskSessionStatus` | 339–344 |
| `AskSessionRecord` file shape, incl. `work`, `work_selector`, `detail`, `retain_completed` | 346–364 |
| `SessionTarget::Ask(AskSessionRecord)` | 374 |
| `prepare_ask_record` (selector resolution through the launch resolver) | 458–506 |
| File-record `prepare_ask_run` | 508–534 |
| `session_run_id` ("predates prepared Runs") | 536–540 |
| `prepare_boundary` | 544–553 |
| Ask file source in `list` | 577 |
| `read_ask_record` branch of `find_session` | 621–626 |
| "Ask Session is not stored here yet" | 654–656 |
| File branch of `mark_ready` | 744–757 |
| `complete_ask` | 856–890 |
| File-record `serve_ask_locked`, `ask_launch_args` | 983–1043 |
| File-record `open_ask`: feedback reset (1230–1231) and `carry_session_name` call (1235–1239) | 1197–1251 |
| `ask_records` | 1343–1363 |
| `interactive_surface` (folded into `conversation_surface`) | 1365–1411 |
| Ask loop of `find_boundary_for_run` | 1584–1590 |
| `list_ask_sessions` | 1834–1859 |
| `ask_surface`: seed title via `session_name`, `session_wave_id`, manifest provider and membership | 1908–1943 |
| `ask_session_directory`, `ask_record_path` | 2254–2260 |
| `write_ask_record`, `read_ask_record` | 2262–2297 |
| File `wait_for_ask`, incl. record removal | 2299–2313 |
| Test `ordinary_and_legacy_prompt_only_asks_still_remove_completed_records` | 2689 |
| Legacy-record half of `ask_skill_and_question_survive_reopening…` | 3115 |
| `create_interactive_session`, `open_interactive_sessions`, `complete_interactive_session` (generalized, not duplicated) | `store/sqlite/sessions.rs` 160, 178, 189; `store/sessions.rs` 41, 45 |
| Test `unopened_session_has_a_run_before_any_provider_is_started` (legacy record without a Run) | `tests/session_cli_tests.rs` 156 |
| JSON seeding of `human-sessions/*.json` in five CLI tests | `tests/session_cli_tests.rs` 82, 158, 234, 357, 448 |

No reader or writer of `human-sessions/<ask>.json` remains.

## Line counts (lines before `mod tests {`)

| File | Before | After |
| --- | --- | --- |
| `ops/human_session.rs` | 2,350 | **2,217** |
| `run_record.rs` | 2,384 | **2,384** |

**Plainly:** `human_session.rs` is 133 lines (5.7%) smaller. That is real
deletion, not relocation, but it is modest. `run_record.rs` did not move: its
name sidecar and subject readers still serve taskless Flow reviews. Production
code outside tests grew by about 60 lines overall, because the store-unavailable
rule added `ops/unrecorded_session.rs` (63 lines) and the store gained
`replace_session_run`, `run` and an idempotent `create_session`. Whole diff: +1,197 / −821 over 15 tracked code and doc files, most of
the additions in tests, plus the new module and draft.

## Could not delete — exact remaining callers

| Kept | Remaining caller |
| --- | --- |
| `carry_session_name` | `ops/flow_session.rs:301` (taskless review replacement) |
| `read_session_name` / `write_session_name` / `session-name.json` | `human_session::rename` tail for `StandaloneFlow`; `session_name()`; `flow_session.rs:179`, `:577`, `:600` |
| `session_name` (seed fallback), `recorded_provider` | `flow_session.rs:179`, `:199`; `review_surface` |
| `session_wave_id` | `attribute_standalone_session`; `session_work_path` |
| `session_work_path` | Labels every kind's `work_path` DTO field |
| `attributed_work`, `preferred_work_selector` | `caller_work` for headless callers without a Run row; `attribute_standalone_session`; `run_record/active.rs:229` |
| `native_session_state`, `run_flow_membership` | `review_surface`, `flow_session.rs:181` |
| Three-source `list` | Task reviews (SQL), taskless Flow files, conversations (SQL) |
| `--as <selector>` through `resolve_work_binding` in `ask_launch_args` | The Ask prompt's Work context comes from launch resolution; omitted when it does not resolve |
| `human-sessions/` directory | Launch lock files only |

## Commands and results

Ambient `LF_*`/`LOOPFLOW_*` cleared for every cargo command; `-j 4`, `nice +10`.
The machine ran at load 30–50 from other worktrees throughout; a bare
`lf session list` cost about 5 s of CPU.

| Command | Result |
| --- | --- |
| `cargo test -p loopflow --test session_cutover_tests` before production edits | **3 failed**, 2 passed: `timed out waiting for the Ask Session row`; `bookkeeping refused the launch: … unable to open database file`; `bookkeeping refused the Ask: exit status: 1` |
| Same, first run after | **3 failed**: every launch exceeded the tests' 20 s deadline under load. Deadlines are upper bounds; raised to 180 s |
| Same, second run | **1 failed**: `ask_proceeds_when_the_store_cannot_be_written` — `no such table: task_flow_positions`. The waiting caller was opening the fresh store while `rename` initialized it. Fixed in production: a waiting caller does not open a store while its reservation is unrecorded |
| Same, final | 5 passed (76.7 s) |
| `cargo test -p loopflow --lib -- ops::human_session ops::flow_session store::sqlite store::migrations` | 130 passed (693.9 s under load) |
| `cargo test -p loopflow --test session_cutover_tests --test session_cli_tests --test dto_fixtures` | First: **1 failed**, `boundary_names…` at its 10 s provider deadline. Second: **1 failed**, `boundary_launch_and_resume…` at its 3 s metadata-open bound. Both are timing bounds in tests; raised to 180 s and 60 s. Final: 9 + 6 + 5 passed |
| `cargo test -p loopflow --test flow_tests -- lf_launches_inside_a_task_checkout` | 1 passed |
| `cargo test -p loopflow --lib -- controller::task` | 29 passed (466.3 s under load) |
| `cargo fmt --all --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass; rerun after removing an unused store method added earlier in this cut. Behavioral suites ran before that removal and were not repeated |
| `scripts/check_migrations.py` | 52 shipped migrations unchanged |

Existing Ask tests (`rg 'ask' rust/loopflow/tests`) are the five in
`session_cli_tests.rs`; they now create their Ask through the real `lf ask`.
Providers and tmux are shell stand-ins. No installed Home, real provider or
desktop ran.

## Not proven through the real binary

Keyed retry. `ask_once` is reachable only from `lf flow` inside a claimed Flow
decision, which this cut may not drive. It is proven by three unit tests over a
real SQLite store (`keyed_asks_join_recover_completion…`,
`keyed_ask_failed_launch_can_retry_the_same_session`,
`keyed_ask_does_not_replace_a_published_native_run`), with the launcher simulated.

## Decisions made here (also in `scratch/questions.md`)

- Ask Session ids stay `ask_<uuid>` and `ask_once_<hash>`; the hidden
  `session serve-ask` now takes the Ask's Run id.
- A completed Ask stays as a closed row. The file model deleted unkeyed Asks
  after delivery; the row model keeps the answer for both.
- Ask `detail` is the selected Skill, else "Request for input". It was the
  caller's skill name.
- Ask Sessions now report `terminal_ids` and always report
  `flow_membership: independent`.
- A plain `lf ask` whose launcher fails stores nothing.
- An Ask reserved with no reachable store records no Work: there was nothing
  to read the caller's row from.
- Old Homes: Asks that exist only as `human-sessions/*.json` no longer list or
  resolve. The import cut owns them.
