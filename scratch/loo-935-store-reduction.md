# LOO-935: remove what the product no longer uses, and keep it removed

## Demo

```bash
uv run python scripts/model_size.py              # tables, columns, storage pub fns, SQL outside storage
uv run python scripts/schema_page.py --check     # SCHEMA.md matches migrations + reference map
uv run python scripts/due_removals.py            # leaving marks, their Task and the date each may drop
```

`rust/loopflow-storage/SCHEMA.md` lists every table under its product concept;
a leaving table or column shows `leaving → LOO-xxx, drop after 2026-12-NN`. An
agent that opens a storage file gets it in context. Asked "where does a Task's
checkout live?", it answers `task_checkouts`, not `tasks.worktree`. A Desktop
pane edit no longer moves `store_revisions.planning`, and a second Mac driving
the same Machine keeps its own layout.

## The staging fact that shapes everything

Nothing in this Task can be dropped today. Installed lf is the main build
(`0.16.0+470da475a`); v0.16.0 was tagged 2026-11-15 and v0.15.3 on 2026-11-11.
Every dead item below stopped being read on or after 2026-10-09, and each was
still read by a build that may still be running in the last 30 days
(`STAGING_DAYS`). The one exception would be using an old, unrelated migration
as `-- expand:` to satisfy the checker. That games the proxy and is rejected.
`check_migrations.py` replays the schema as it was *before* the expand file,
so any migration older than 30 days "passes". It does not know when reads
stopped. Only a mark does.

So this Task's one migration is **additive**. It stops every read and write
it can, marks every leaving member, and makes the contracts start on their
own. The smaller store arrives when the marked contracts run, from about
2026-12-17. "The others start from a smaller store" is true for code and
reads, not bytes, until then.

## Inventory (callers read 2026-11-16 at 470da475a1)

Verdicts overturn `scratch/schema-review.html` (LOO-906 checkout) where marked ✗.

**Remove now: stop reads/writes, mark leaving, contract in the LOO-935 removal Task**

| Member | Why it goes | Code removed here |
|---|---|---|
| `tool_responses` | never had a caller since added 2026-07-18 | `write_tool_response`/`tool_response(_in)` (storage `sqlite/durable.rs`, execution `store/durable.rs`), `ToolResponse*` in `loopflow-model/src/durable.rs` |
| `pm_project_name_cutover` | code removed in #1898 (2026-11-14) | none left |
| `project_transition_items` | code removed in #1813 (2026-11-08) | none left |
| `task_issue_identities` | writer removed 2026-10-09, no reader | none left |
| `task_deletions` | no writer since 2026-10-09; 2 rows (LOO-335, EXP-6) guard every Task path | `deleted_task_issues_in`, `task_deleted_on`, `require_task_not_deleted` and their callers in `children.rs`, `linear_observations.rs`, `linear_acceptance.rs`, `native_session.rs`, `task/snapshot.rs`, `task.rs` |
| `task_pr_repair_incidents` | write-only; each write wakes planning | `record_task_pr_repair_incident_on` and its call in `ops/task/delivery.rs`; the read in `scripts/lifecycle_scorecard.py` |
| `project_binding_imports` | one-time import of 0.13.7 `~/.lf/waves/<id>/config.yaml` `pm.linear_project` | `import_project_binding`, `project_binding_imported`, `import_binding` in `ops/project.rs` (from `bind_project`, `ensure_first_project`) |
| `pools`, `ondemands` ✗ | since #1805 each holds only `(name, created_at)`; declarations live in `config.yaml` | `register_pool/_ondemand`, `remove_*_record`, `capacity_created_at`, the pool insert in `set_machine_ownership_prepared`, `created_at` on `Pool`/`OnDemand` (`pool list --json`, `ondemand list --json`; no reader in repo). The migration rebuilds `pool_machines` with every column and no FK to `pools` (additive) |
| `processes.viewer_hold`, `.viewer_hold_at`, `.viewed` | dead; `held_terminals` holds the live copy | none left |
| `work_pane_layouts` ✗ (review said "move") | screen state moves to the client, see below | storage `selections/panes.rs`, `desktop.layout` read and `desktop.layout.save` write, `face`/`pane_revision` on `ViewContent` (Swift decodes them, nothing uses them) |

**Already owned by a removal Task: mark it, so that Task starts itself**

| Members | Task | Note |
|---|---|---|
| `session_steers`, `steer_batches`, `steer_deliveries` | LOO-891 | no code reads or writes them since #1923 (2026-11-14) |
| `processes.native_input`, `desktop_shells.close_requested` | LOO-840 | also delete test-only `close_desktop_shell` here; keep its `jobs_settled` effect if Close needs it |
| `provider_accounts`, `provider_session_accounts`, `provider_account_limits`, `provider_account_switches`, `provider_routes`, `auth_browser_bindings` | LOO-928 | LOO-913 (#1953, open, 211 files, no migration) removes the code. Mark them here only if #1953 is on main when this Task rebases. Otherwise the mark goes into a comment-only migration in LOO-913, and LOO-928's brief gains `provider_routes` and `auth_browser_bindings` (FKs to `provider_accounts`; the review missed both) |
| LOO-874's replaced columns: `tasks.worktree`, `.branch`, `.base_commit`, `.parent_pr_id`, `.checkout_machine_id`, the `tasks.ship_*` columns, `task_ownership.written_here`, `agent_sessions.moved_to_machine_id`, `machines.pool`, `.cost_usd`, `.expires_at` | LOO-877 | mark only what `task_machine_local.sql` says it replaced (read it at implement). `relocated_task_processes` is live, so no mark |
| `tasks.regressed_in` | LOO-730 | |
| `provider_accounts.long_lived_token(_minted_at)`, `provider_session_accounts.isolated` | LOO-770 | moot once LOO-928 drops the tables. LOO-770 keeps only its config half |
| `agent_sessions.work_source` | LOO-836 | still read in about 46 places and live. No mark until LOO-836 stops the reads; this Task does not do that work |

**Keep (live, often rare)** ✗ marks a review verdict overturned

- Fleet: `rentals`, `machine_records` (rental give-back), `pool_machines`,
  `task_moves` (`lf task move --machine`, v0.16.0), `relocated_task_processes`
  (repository moved on disk), `session_moves` (`lf session move --blocked`;
  transient rows). All are empty because the feature is rare, not because they
  are dead ✗.
- `pm_issue_changes` ✗ (Linear sync reads it), `planning_peer_conflicts` ✗
  (`lf plan` status), `dashboards`/`dashboard_arrangements` ✗ (`lf desktop
  dashboard`, DashboardView), `provider_tokens` ✗ (GitHub, Linear, OpenCode Zen,
  API keys; not subscription-only), `access_profiles` (browser login for
  services; LOO-928 decides).
- `held_terminals.viewer_hold/_at/viewed`, `agent_sessions.input_published`,
  `steers`, `steer_delivered`, `steer_adapters`, `task_update_readings`,
  `wave_lifecycle_operations`, `blob_tokens`, `metric_*`.

Config-only removal Tasks (LOO-919, 560, 731, 875, 637, 640) have no schema
member and get no mark.

Not inventoried here (owned by siblings): duplicate Task facts (LOO-937),
delivery queues (LOO-938), journal tables (LOO-936).

## Screen state leaves the Machine

`work_pane_layouts` is keyed `(repository_id, kind, work_id)`, with no person
or client. Its triggers advance `planning`, and `read_stream.rs` `changed`
wakes every subscriber on any domain except `usage`, including the phone's
`view.select`. The phone never reads layouts.

**Chosen:** Desktop owns its layouts in Application Support, keyed by
`(machine_id, kind, work_id)`, with no store row and no server call.
`savePaneIntent`/`loadPaneIntent` (`swift/LoopflowMac/Views/SessionsView.swift`)
read and write the file, and `RegistryQuery.savePaneLayout`/`paneLayout` go away.
The revision and session-ownership checks the server did become: a layout
naming a Session the current reading doesn't have drops that pane on load.
Two Macs on one Machine, or a laptop on a cloud Machine, each keep their own.

Cost: `lf open` for Rex/Herdr/cmux (`open/workspace/layout.rs:77`) stops
mirroring the Desktop layout. It uses `task_layout` or the builtin, the same as
for a Task that Desktop never opened. Rejected alternative: keep the row and
drop its triggers. That fixes the wake but keeps one layout per repository for
every client. → `scratch/questions.md`.

**Dashboards** are screen state too, and they also advance `planning`. They
are not moving now, because the move needs a product decision first (see
questions). The default taken: their six triggers are dropped in this
migration (additive) and both tables join `NEVER_DISPLAYED`. Desktop shows its
own writes from the command reply. A `lf desktop dashboard` edit appears at
Desktop's next workspace load. Implement checks the reply path before
committing to this.

## Keeping it reduced

**Leaving marks.** Syntax: `-- leaving: <table>[.<column>] <Task ID>`. Mark
lines go in the migration that supersedes the member. Its first-parent merge
date on `origin/main` starts the clock. Existing leaving members get their
marks in this Task's migration; merged files are immutable.
`check_migrations.py` checks that each mark names a current member and a Task
ID. A contract's `-- expand:` names the migration holding the marks, and when
that file has marks, the contract may drop only marked members. `-- expand:`
stays. Marks add intent, Task and date; they do not replace it.

**Generated page.** `scripts/schema_page.py` replays the migrations
(reusing `check_migrations.Catalog`). It groups each table under its row in
the ownership map in `docs/dev/reference.md`, which `check_architecture.py`
already keeps covering every table, so no new concept file is needed. It
writes `rust/loopflow-storage/SCHEMA.md`: one line per table, leaving
columns and tables with Task and date. `rust/loopflow-storage/AGENTS.md` says
two sentences and links it. Implement probes whether Claude loads a nested
`AGENTS.md` when it reads a file under `rust/loopflow-storage/`; if not, it
uses `CLAUDE.md` with `@SCHEMA.md`. `--check` runs in CI beside
`check_migrations.py`. Root `AGENTS.md` (SQLite schema) and `MIGRATIONS.md`
describe marks.

**Removals start themselves.** `scripts/due_removals.py` lists marks whose
migration has been on `origin/main` for `STAGING_DAYS`, using the same date
function as the checker. With `--start`, it runs `lf task run <ID> -a claude`
for each due Task that `lf task status --json` shows `unstarted`. It never
restarts started work, and without `--start` it only prints. A weekly
LaunchAgent on the Machine that runs Tasks schedules it. This matches
`docs/dev/delivery.md`, where scheduling is the person's own scheduler line;
the LaunchAgent is needed for Keychain access (`docs/work.md`). It is
documented in `scripts/README.md`, and verify installs it on this Machine.
One removal Task, filed at implement, holds the contract for everything this
Task marks: "Drop the members LOO-935 marked leaving".

**PRs say what they added to the model.** The builtin
`rust/loopflow-definitions/src/builtins/ops/skill/pr-message.md` gains one
short paragraph. It covers what the change added and removed in stored data,
public interfaces and lines, kept general. `.lf/skills/pr-message.md` is a
stale fork of the builtin with older wording. Re-fork it from the new builtin,
keep its **Command changes** section, and add **Model changes**: the
`scripts/model_size.py` before/after for tables, columns, storage pub fns
and SQL outside storage. There is no CI job and no trend report.

## Numbers

| | Before (470da475a1) | After this Task | After contracts |
|---|---|---|---|
| tables | 88 | 88 | ≤ 74 (drops 9 here + 3 LOO-891 + 6 LOO-928 if marked; pane layouts table drop too) |
| columns | 727 | 727 | measured by `model_size.py` |
| store | 986 MB | unchanged | session_steers/steer_deliveries/steer_batches + provider_session_accounts ≈ 165k rows |
| storage `pub fn` (non-test) | 930 | measured | |
| SQL call sites outside storage (non-test) | 192 in 49 files | measured | |

`model_size.py` defines these counts, and Before is re-measured with it at
implement.

## Slices

1. **This slice: marks, page, due list.** `check_migrations.py` reads marks.
   This also covers `schema_page.py`, `due_removals.py`, `model_size.py`,
   `SCHEMA.md` + storage `AGENTS.md`, and the migration skeleton with marks
   (`scripts/new_migration.py loo935_leaving`). Focused check: a Python test
   under `python/tests/` for mark parsing, the due date and the contract
   restriction.
2. **Stop reads and writes.** Delete the code in the first inventory table:
   `tool_responses`, `task_deletions` guards, `task_pr_repair_incidents`,
   `project_binding_imports`, `pools`/`ondemands`, `close_desktop_shell`. Also
   fix the `lf pool add` → `lf machine pool add` text in `commands/rental.rs`
   and `revisions.rs`. The migration rebuilds `pool_machines` without its FK.
   Focused: `cargo test -p loopflow-storage --lib`, rentals and ownership
   tests, `store_revisions_cover_every_table`.
3. **Screen state.** Pane layouts move to Desktop files. Remove the
   `desktop.layout` read and save, the storage panes module, `face` and
   `pane_revision`, and the DTO fixture `pane_layout.json`. Drop the dashboard
   triggers. Focused: Swift `WorkSelectionTests`/`ToolTabTests`/
   `DashboardWorkspaceTests`, `served_reads`, `selections/tests.rs`.
4. **Skills and filing.** Update both pr-message skills and file the LOO-935
   removal Task. If #1953 has landed, add the account marks; otherwise leave a
   comment on LOO-928 with the missing tables.

## Done when

- `uv run python scripts/check_migrations.py` passes with an additive
  migration that carries every mark above.
- `uv run python scripts/due_removals.py` prints each marked Task with
  "drop after <merge + 30 days>".
- `schema_page.py --check` passes, and `SCHEMA.md` shows `tasks.worktree`
  as leaving → LOO-877. That mark comes from this migration, since LOO-874
  left it unmarked.
- On a private `LF_HOME` running the branch: save a Desktop pane layout,
  then `sqlite3 $LF_HOME/loopflow.db "select revision from store_revisions
  where domain='planning'"` reads the same before and after.
- `git grep -n -w -e tool_responses -e task_deletions -e task_issue_identities -e project_binding_imports -e work_pane_layouts -- rust swift ':!*/migrations/*' ':!*test*'`
  finds only `revisions.rs` `NEVER_DISPLAYED` entries.
- The PR body shows the `model_size.py` before/after.

## Forbidden

No dual write of pane layouts (server and file). No adapter keeping
`desktop.layout` for old Desktop builds: Desktop and lf ship together. No
"keep writing `pools` for the FK". No contract that uses an unrelated aged
migration as `-- expand:`.

## Docs written (write-docs, 2026-11-16)

`rust/loopflow-storage/src/MIGRATIONS.md` (Leaving marks + enforced rule),
`AGENTS.md` (SQLite schema bullet), `rust/loopflow-storage/AGENTS.md` (new;
points at generated `SCHEMA.md`), `scripts/README.md` (Store size and leaving
members), `docs/workspaces.md` + `docs/config.md` (Desktop layouts per Mac,
`lf workspace` no longer reads them), `rust/loopflow-execution/src/work/README.md`,
`rust/loopflow/src/server/README.md`, `docs/dev/reference-code.md`.
Implement makes the code match: `due_removals.py install|uninstall`, output
columns as shown, `lf desktop layout` limited to `query:`.
