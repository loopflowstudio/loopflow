# Cut 1 — interactive Sessions are rows

2026-09-27 · LOO-298 · One bounded implementation Run. Nothing committed,
published, installed or rebased. Tree left dirty for the parent.

## What writes and reads the rows

| Operation | Owner now |
| --- | --- |
| Launch (`lf --tui …`, independent of any Flow step) | `lf/commands/run.rs::begin_run_capture` → `interactive_run` + `reserve_interactive_session` → `SqliteStore::create_interactive_session`: one immediate transaction inserts the `sessions` row and its first `runs` row through the existing `insert_run_in` constructor (ancestor fill + validators, `tasks.started_at` trigger). It runs as the publication callback of `begin_reserved_with_context`, after the manifest is published and before the recorder or provider exists |
| Work attribution | `PromptBuild.work` (typed `RunWork`), set in `launch_bound`: `checkout` from checkout inference, `declared` from `--task/--wave/--as`; unbound launches store null task/wave/work_source |
| Title | Written once at creation (skill name, else word pair); never recomputed on read |
| List | `store.open_interactive_sessions()` (`kind='interactive' AND completed_at IS NULL`) → `interactive_surface(session, run)` |
| Find | `find_session` → `owned_target`, dispatching on `sessions.kind`; Run id and Run-id prefix resolve through `runs.session_id` |
| Rename | `store.rename_session` (`UPDATE sessions`), human-over-generated in SQL; a Run selector fences on the current Run |
| Open | Reads provider/model/cwd from the Run row; provider history and client receipts stay Run-directory evidence (`NativeRun`) |
| Complete | `store.complete_interactive_session` sets `completed_at`, fenced on the current Run |
| Ready | Unchanged: interactive Sessions never had a ready action |

Schema: new draft `record_session_kind` adds `sessions.kind`, `runs.provider`,
`runs.model`. Existing rows convert as `flow_review`; every writer names the kind.
No DTO field changed, so Swift and `tests/fixtures/dto` are untouched.

## Deleted (path:line at HEAD `fdcfb036b`)

| Item | Location |
| --- | --- |
| `boundary_run_ids` | `ops/human_session.rs:1316` |
| `list_interactive_sessions` (Run-directory scan caller) | `ops/human_session.rs:1353` |
| Manifest-based `interactive_surface`, incl. seed-title recomputation and `attributed_work` read | `ops/human_session.rs:1370` |
| `provider_history(dir, manifest)` | `ops/human_session.rs:673` |
| Manifest branch of `find_session` (`has_interactive_history` / `provider_session_is_resolved`) | `ops/human_session.rs:616–625` |
| Interactive sidecar rename (no launch lock branch, `write_session_name` for interactive) | `ops/human_session.rs:1518–1520` |
| `resolve_provider_session` call in `stop_native_run` | `ops/human_session.rs:1690–1692` |
| `struct SessionResolution` | `run_record.rs:267` |
| `scan_unresolved_provider_runs` | `run_record.rs:537` |
| `has_interactive_history` | `run_record.rs:1016` |
| `provider_session_is_resolved` | `run_record.rs:1134` |
| `resolve_provider_session` (`session-resolution.json` writer) | `run_record.rs:1152` |
| test `resolving_a_session_keeps_its_provider_history` | `run_record.rs:3083` |
| test `unresolved_provider_scan_keeps_interactive_resumes_until_resolution` | `run_record.rs:3103` |
| `SqliteStore::session_run_ids` / `Store::session_run_ids` | `store/sqlite/sessions.rs:162`, `store/sessions.rs:45` |
| test `raw_sessions_are_named_by_a_stable_word_pair` (built an interactive Session by detaching an Ask's Run) | `tests/session_cli_tests.rs:440` |

`session-resolution.json` has no reader or writer left for any kind.

## Line counts (lines before `mod tests {`)

| File | Before | After |
| --- | --- | --- |
| `ops/human_session.rs` | 2,351 | **2,350** |
| `run_record.rs` | 2,484 | **2,384** |

Whole diff: +396 / −409 over 9 tracked files, plus the new draft and test file.
**Plainly:** `human_session.rs` is one line under the ceiling. The file lost the
scan, the manifest surface and the sidecar rename, and gained the row surface
and `NativeRun`; it did not get meaningfully smaller. The real reduction is in
`run_record.rs`. The file shrinks for real only when Ask and taskless reviews
leave it.

## Could not delete — exact remaining callers

| Kept | Remaining caller |
| --- | --- |
| `read_session_name` / `write_session_name` / `session-name.json` | Ask and taskless rename: `ops/human_session.rs::rename` tail; `session_name()`; `ops/flow_session.rs:577,600` |
| `carry_session_name` | `open_ask` (`human_session.rs`), `flow_session.rs::recover_unpublished_run` |
| `session_name` (seed fallback) | `ask_surface`, `flow_session.rs:179` |
| `session_wave_id`, `session_work_path` | `ask_surface`, `attribute_standalone_session`; `session_work_path` also labels review and interactive records |
| `attributed_work`, `preferred_work_selector` | `attribute_standalone_session`, `prepare_ask_record`, `run_record/active.rs:229` |
| Four-source `list` | Now three file/SQL sources plus the interactive query: Task reviews, Ask files, taskless Flow files |
| `record_task_start` (Started event) | Headless launches still have no Run row |

## Commands and results

Ambient `LF_*`/`LOOPFLOW_*` cleared for the final runs; `-j 4`, `nice +10`.

| Command | Result |
| --- | --- |
| `cargo test -p loopflow --test session_cutover_tests` before production edits | **2 failed**: `launch reserves one Session and its Run` left `(0, 0)`; `Run … has no row` |
| Same, after | 2 passed (18.5 s) |
| `cargo test -p loopflow --test session_cli_tests --test dto_fixtures` | 6 passed; 9 passed |
| `cargo test -p loopflow --lib -- ops::human_session store::sqlite` | First run: 57 passed, **1 failed** (`run_constructor_infers_ancestors…`, `InvalidColumnIndex(13)`: the test's own column list predated `provider`/`model`). Fixed to use `RUN_COLUMNS`. Final: 58 passed |
| `cargo test -p loopflow --lib -- ops::human_session store::sqlite store::migrations` | 127 passed |
| `cargo test -p loopflow --test flow_tests -- lf_launches_inside_a_task_checkout` | 1 passed |
| `cargo test -p loopflow --lib -- ops::flow_session` | **3 failed with this Run's ambient `LF_CONTROL_HOME`/`LF_RUN_ID` set** (`Run … was not found on this Home`); 3 passed with them cleared. Untouched code; environment leak, not this cut |
| `cargo fmt --all --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | First: 1 error (unused test import `write_provider_session`), fixed. Final: pass |
| `scripts/check_migrations.py` | 52 shipped migrations unchanged |

Providers are shell stand-ins. No installed Home, real provider or desktop ran.

## Decisions made here (also in `scratch/questions.md`)

- Interactive Session ids are `session_<uuid>`, distinct from the Run id. Run ids
  still select the Session. Swift keys by `id` and sees only a different string.
- A Session now lists from launch, before provider history exists (`waiting`).
  Previously it appeared only once `provider-session.json` was written.
- `state` is derived (completion, client receipts, provider history); no column.
- Only `tui` launches with Independent membership create a Session. IDE handoff
  and headless Runs still write no rows.
- Interactive launch now needs a writable store; a failure there fails the launch.
- Old Homes: interactive Sessions recorded only as Run directories no longer
  list, and a Run-id selector for one reports `does not belong to a Session`.
  The import cut owns them. `scratch/cutover/four-origin-import-fixture.rs.txt`
  keeps the earlier worker's four-origin fixture for that cut.
