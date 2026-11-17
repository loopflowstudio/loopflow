# LOO-935: remove what the product no longer uses, and keep it removed

Status: split implemented; `lf workspace` folded into `lf open --workspace`, which never holds a Task (2026-11-16). Next is gate.

## Demo

```bash
uv run python scripts/model_size.py              # tables, columns, storage pub fns, SQL outside storage
uv run python scripts/due_removals.py            # leaving marks, their Task and drop date
uv run python scripts/schema_page.py --check     # SCHEMA.md matches the migrations
lf open --task <ID> --host rex                   # one tab: the Task's conversation
```

`rust/loopflow-storage/SCHEMA.md` lists every table under its product concept;
`rust/loopflow-storage/AGENTS.md` points at it and loads when an agent reads a
storage file (probe 2026-11-16: a Haiku `claude -p` read of
`rust/loopflow-storage/Cargo.toml` quoted the nested file from hook output).

## Split (2026-11-16)

Jack Heart, 2026-11-16: "You can drop everything old in this task as a special
case". Then, after reading that released lf 0.16.0 still queries
`task_deletions` and the old `tasks` checkout columns: "lets split the PR".

Done: `unused_store_members.sql` is additive. It marks each member below
`-- leaving: <member> LOO-954`, rebuilds `pool_machines` without its FK to
`pools` (nothing writes `pools` now). Dashboard planning triggers stay: a
served Desktop watch needs them to see another client's Dashboard edits. The `UNSTAGED_CONTRACT` exemption, `migration_exports.rs` and the
`until_unused_store_members` test helpers are gone. `task_ownership.written_here`
is written again (1 here, 0 ingested), as base did; base already wrote none of
the `tasks` checkout, primary Session or `ship_*` columns, so nothing else
needed restoring. Indexes on leaving `tasks` columns stay (released lf queries
by them).

LOO-954 (filed 2026-11-16; LOO-943 is abandoned and `lf task reopen` refuses
abandoned Tasks) carries the contract: `-- expand: unused_store_members`,
`-- exports:`, drops only marked members. Its brief points at 8d18764018 for the
finished draft of the drops, the exporter and the upgrade test helpers. Main's
#1961 added a read of a leaving column: `FORGETTABLE`
(`rust/loopflow-storage/src/sqlite/processes.rs`) keeps a Process that
`tasks.ship_lf_process_id` names, for rows released lf wrote; LOO-954 deletes
that clause with the column (steered onto LOO-954).

Leaving → LOO-954: `tool_responses`, `pm_project_name_cutover`, `project_transition_items`,
`task_issue_identities`, `task_deletions`, `task_pr_repair_incidents`,
`project_binding_imports`, `work_pane_layouts`, `session_steers`,
`steer_batches`, `steer_deliveries`, `pools`, `ondemands`; columns
`machines.pool/cost_usd/expires_at`, `processes.viewer_hold/viewer_hold_at/viewed/native_input`,
`desktop_shells.close_requested`, `task_prs.repair_tracking_complete`,
`task_ownership.written_here`, and on `tasks`: `worktree`, `branch`,
`base_commit`, `parent_pr_id`, `checkout_machine_id`, `primary_session_id` and
the six `ship_*`.

PR body: numbers "after this PR" (tables/columns unchanged, fewer reads) and
"after the contract"; no "Upgrade limits" section.

Done here for other Tasks (not edited): LOO-891 (steer tables), LOO-840
(`native_input`, `close_requested`), LOO-877 (every column `task_machine_local`
replaced). Still marked, still read: `agent_sessions.work_source` → LOO-836,
`tasks.regressed_in` → LOO-730 (released peers exchange it through planning),
the four `provider_*` account tables → LOO-928 (#1953 not on main).
`provider_routes`/`auth_browser_bindings` are not on main; nothing to mark.

Kept, decided one by one (empty because rare, not dead): `rentals`,
`machine_records` (rental give-back), `pool_machines`, `task_moves`
(`lf task move --machine`), `relocated_task_processes` (repository moved on
disk), `session_moves` (`lf session move --blocked`), `pm_issue_changes`
(Linear sync), `planning_peer_conflicts` (`lf plan`), `dashboards` and
`dashboard_arrangements` (`lf desktop dashboard`), `provider_tokens` (GitHub,
Linear, API keys). The PR body lists these.

LOO-946 removes the retired `task_layout` catalog key after release.

## lf open

Jack Heart, 2026-11-16: "we can drop the thing where lf open does anything
other than just open the task session in other surfaces." and "we dont need to
try to copy our desktop layouts". Rex, Herdr and cmux tabs hold the
conversation (plus Changes with `--diff`, Task & workflow without a local
checkout). `layout.rs` kept only that; `tools.rs`, `Templates`, the storage
`panes` modules and `WorkPaneLayout`/`TaskLayoutTemplate` are gone. `task_layout`
stays readable and ignored (shared-file rule).

Desktop keeps pane layouts under
`$LF_HOME/desktop-cache/pane-layouts/<machine>/<kind>/<work>.json`
(`PaneLayoutStore.swift`), beside its existing `WorkCache` files.
`lf desktop layout` reads only `query:<id>`; `desktop.layout.save` is gone.

A saved layout keeps every Session it names (`loadPaneIntent`, `SessionsView.swift`,
loads the file as written). The filter it replaced tested against the first page
of interactive Sessions, emptying panes that held older Sessions and headless
Runs. A pane naming a Session that is gone shows the Session view's not-found
state; the file is not rewritten.

## Numbers (`model_size.py`)

| | Before (470da475a1) | After this PR | After LOO-954 (measured on the pre-split draft) |
|---|---|---|---|
| tables | 87 | 87 | 74 |
| columns | 723 | 723 | 645 |
| storage `pub fn` (non-test) | 1050 | 1006 | — |
| SQL call sites outside storage | 35 in 8 files | 35 in 8 files | — |
| store (copy of this Machine's, after VACUUM) | 991 MB | — | 897 MB |

## Checks

- `uv run pytest python/tests/test_check_migrations.py python/tests/test_due_removals.py python/tests/test_lifecycle_scorecard.py`: pass.
- `cargo nextest` library rows (storage, execution, application) for revisions, rentals, ownership, shells, planning, selections, migration_runner, query_plans, settings, schema: pass.
- Integration: `work_open_tests::`, `cli_discovery::`, `binary(served_reads)`, `local_planning::`, `binary(two_people)`, `performance_tests::`, `ops::task::tests::`, `documented_commands::`, `binary(golden_prompt)`: pass (with resolved `TMPDIR`; load average ~120 made 2 s host timeouts fail in parallel runs).
- Swift: `scripts/test_desktop.sh --filter DashboardWorkspaceTests` after the layout change: 14 pass.
- Swift (before it): `scripts/test_desktop.sh --skip-build --filter 'WorkSelectionTests|ToolTabTests|DashboardWorkspaceTests|MultiplexerStoreTests|DTOFixtureTests'`: 90 pass (subagent run); a wider 14-suite filter, 195 pass.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt`, `check_architecture.py`, `check_migrations.py`, `schema_page.py --check`: pass.
- Compress (after merging main): `cargo clippy -p loopflow --all-targets -- -D warnings`, `pytest test_check_migrations.py test_due_removals.py` (30), `check_migrations.py`: pass; `schema_page.py --check` was stale from main's "Provider authentication" rename, regenerated.
- QA report of the pre-split upgrade (drops, export file): `git show 8d18764018:scratch/qa.html`.
- QA: the two `#[ignore]` installed-host tests in `work_open_tests.rs` now expect one Task Session and a Task-view-only arrival; Rex and Herdr pass with `LOOPFLOW_TEST_REX`/`LOOPFLOW_TEST_HERDR` (Rex misses its 2 s answer at load ~500).
- Split: `nextest -p loopflow-storage --lib` revisions/task_ownership/migration_runner/rentals/canonical_ids/query_plans (172), `cargo clippy --all-targets -- -D warnings`, `pytest test_check_migrations.py test_due_removals.py` (29), `schema_page.py --check`: pass. After merging ffb29c6b29 (`task_summaries.sql`), `check_migrations.py` and `schema_page.py --check`: pass.
- Workspace fold: `nextest -p loopflow --lib` `lf::tests::`, `wave_resolution_matrix::`, `commands::open::`; integration `work_open_tests::`, `cli_discovery::`, `documented_commands::` at `-j 4` (72); `cargo clippy -p loopflow --all-targets -- -D warnings`: pass. `rex_agent_tool_open_uses_the_moved_viewer_instead_of_startup_context` failed once at `-j 4` (load ~38) and passed alone; gate/CI decides whether it is load-sensitive.
- Workspace widening: `nextest -p loopflow --lib` `commands::open::`, `lf::tests::`, `wave_resolution_matrix::` (57); integration `work_open_tests::` at `-j 4` (32 of 33), `cli_discovery::` + `documented_commands::` (41); `cargo clippy -p loopflow --all-targets -- -D warnings`: pass. `work_open_tests::task_location_uses_the_named_plan_outside_a_checkout` fails here with `location.state` "unavailable" ("not found"), also with this step's `src/` reverted: not this diff; gate/CI decides.
- Verify: `uv run python scripts/due_removals.py install` on the Machine that runs Tasks.

## lf workspace folds into lf open (2026-11-16)

Jack Heart, word for word: "i think workspace should just be for waves and repos
now", "and honesrly workspace should be combined w open fr that reason", "do it
here". Asked what `lf open --wave product` shows, the answer picked:
"Conversation; overview by flag".

Done: `lf workspace` and `WorkspaceArgs` are gone; `OpenArgs` carries
`--workspace`, `--available`, `--no-focus` and the hidden follower/remote flags,
and `run` dispatches overviews to `run_workspace`. Followers and remote Machines
are invoked as `lf open --workspace …`. `--workspace --task X` widens to the
Task's Wave or repository (next section). `--here/--tab/--focus/--file` conflict with `--workspace`/`--available`.
PR body, User interface changes: removed `lf workspace`; added `lf open
--workspace`, `--available`, `--no-focus`.

## --workspace never holds a Task (concept review, 2026-11-16)

Done: `overview()` (`open.rs`) widens a Task, link or Session to its Wave, else
its repository, and keeps the Task as `Overview.selected`. Native overviews route
the selected Task whatever its state, put it first so it takes focus, and send it
to peers as `--task <id>`, which they widen the same way. Deleted: `explicit`,
`Presentation`, the `native.here()` join, `place_task`, `open_conversation`, the
`Work` arms of `scope`/`remote_open`/`container_name`, the Task case of
`desktop_workspace_url`, and the conversation pane in `layout.rs` (overview tabs
are Task & workflow, plus Changes with `--diff`). `--available` opens each
conversation through `conversation::open` (`open_available`), so it keeps
`OpenArgs::conversation`. Single-conversation tests moved to `--tab`;
`workspace_from_an_unstarted_task_opens_its_repository_overview` and the
follower test's `--task` assertion cover the widening.
