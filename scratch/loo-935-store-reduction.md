# LOO-935: remove what the product no longer uses, and keep it removed

Status: implemented on this branch; gate next.

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

## The special case

Jack Heart, 2026-11-16: "You can drop everything old in this task as a special
case". So `unused_store_members.sql` is a contract that drops now, without an
aged expand step. `check_migrations.py` exempts that file alone
(`UNSTAGED_CONTRACT`, comment cites LOO-935); it still needs `-- exports:`.
The preflight (`migration_exports.rs`) writes every non-empty dropped table and
column to `<lf home>/exports/unused-store-members.json` before the SQL runs.
Released lf (0.16.0 and older) fails on the statement that names a dropped
member; the rest of the store answers.

Dropped: `tool_responses`, `pm_project_name_cutover`, `project_transition_items`,
`task_issue_identities`, `task_deletions`, `task_pr_repair_incidents`,
`project_binding_imports`, `work_pane_layouts`, `session_steers`,
`steer_batches`, `steer_deliveries`, `pools`, `ondemands`; columns
`machines.pool/cost_usd/expires_at`, `processes.viewer_hold/viewer_hold_at/viewed/native_input`,
`desktop_shells.close_requested`, `task_prs.repair_tracking_complete`,
`task_ownership.written_here`, and on `tasks`: `worktree`, `branch`,
`base_commit`, `parent_pr_id`, `checkout_machine_id`, `primary_session_id` and
the six `ship_*`. `pool_machines` is rebuilt without its FK to `pools`.
Dashboard triggers no longer wake planning.

Done here for other Tasks (not edited): LOO-891 (steer tables), LOO-840
(`native_input`, `close_requested`), LOO-877 (every column `task_machine_local`
replaced). Still marked, still read: `agent_sessions.work_source` → LOO-836,
`tasks.regressed_in` → LOO-730 (released peers exchange it through planning),
the four `provider_*` account tables → LOO-928 (#1953 not on main).
`provider_routes`/`auth_browser_bindings` are not on main; nothing to mark.

LOO-943 (filed for marked members) was abandoned once everything dropped here.
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

## Numbers (`model_size.py`)

| | Before (470da475a1) | After |
|---|---|---|
| tables | 87 | 74 |
| columns | 723 | 645 |
| storage `pub fn` (non-test) | 1050 | 1029 |
| SQL call sites outside storage | 35 in 8 files | 35 in 8 files |
| store (copy of this Machine's, after VACUUM) | 991 MB | 897 MB |

Upgrade of that copy with the branch: applied, `foreign_key_check` clean,
26 MB export.

## Checks

- `uv run pytest python/tests/test_check_migrations.py python/tests/test_due_removals.py python/tests/test_lifecycle_scorecard.py`: pass.
- `cargo nextest` library rows (storage, execution, application) for revisions, rentals, ownership, shells, planning, selections, migration_runner, query_plans, settings, schema: pass.
- Integration: `work_open_tests::`, `cli_discovery::`, `binary(served_reads)`, `local_planning::`, `binary(two_people)`, `performance_tests::`, `ops::task::tests::`, `documented_commands::`, `binary(golden_prompt)`: pass (with resolved `TMPDIR`; load average ~120 made 2 s host timeouts fail in parallel runs).
- Swift: `scripts/test_desktop.sh --skip-build --filter 'WorkSelectionTests|ToolTabTests|DashboardWorkspaceTests|MultiplexerStoreTests|DTOFixtureTests'`: 90 pass (subagent run); a wider 14-suite filter, 195 pass.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt`, `check_architecture.py`, `check_migrations.py`, `schema_page.py --check`: pass.
- Not run: the two `#[ignore]` installed-host tests in `work_open_tests.rs` (Rex, Herdr) lost their tools-only `task_layout`; gate/QA with installed hosts.
- Verify: `uv run python scripts/due_removals.py install` on the Machine that runs Tasks.
