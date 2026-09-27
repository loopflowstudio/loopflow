# Cut G — what can be deleted now that Sessions, Runs and invocations are tables

2026-09-27 · LOO-298 · Research at `f3b91950c`, before any deletion. Paths are
under `rust/loopflow/src/`. Sizes are lines before the file's first test
module. "Callers" excludes tests unless it says otherwise.

## In scope for this cut

| # | Candidate | Where | Size | Remaining callers | Why it can go |
| --- | --- | --- | --- | --- | --- |
| 1 | `observed_run_ids`, `observed_run_ids_at` | `run_record.rs:1997–2040` | 45 | None in production. Two tests: `run_record::tests::two_runs_about_one_task…` and one in `lf/commands/run.rs:1706` | A directory scan by subject string, compiled only for tests. The tests can read the two Runs they began |
| 2 | `Store::position_runs` | `store/sessions.rs:9–21` | 13 | None. Tests call `SqliteStore::position_runs` directly | An async wrapper nothing awaits |
| 3 | `RunFlowStep::boundary_key` | `run_record.rs:48,62,82` | 3 | None. It is written into every manifest and read by nothing | Older manifests that carry it still parse; unknown fields are ignored |
| 4 | `Boundary::run_dir` | `ops/flow_run.rs:27`, written at `:245`, read at `:361` | 6 | `flow_run::recover` | A Run's record directory is a function of its id and Home. `bind_run` loses a parameter |

Expected: about −65 production lines.

## Out of scope, with the exact blocker

### `flows/<id>/position.json` as the saved Flow's cursor owner

`ops/flow_run.rs` is 464 production lines. `read`, `write` and `update` are
`:106–145`; every other function in the file calls them.

Callers of `flow_run::read`: `lf/commands/flow.rs:207,232,457`,
`ops/flow_session.rs:29`, `flow_run::capture_membership`, `require_active`,
`launch_driver`. Callers of `update`: `lf/commands/flow.rs:262,275,282` and
`checkpoint`, `bind_run`, `record_decision`, `record_route`, `recover`,
`retry`, `begin_boundary`, `finish_boundary` inside the file.

Cut F made the second owner more visible: the driver now copies the cursor into
the invocation row before every headless step, not only at a review.

Making the row the owner is not reachable in this cut. What blocks it:

1. **The row lacks six facts the file holds.** `flow_invocations` has the
   capture, the cursor, `failure_json`, `state` and `current_run_id`. It has no
   column for the Flow's working directory, its message, its model, the Work
   selectors it was launched with (`wave`, `task`, `as_work`), or the active
   boundary's id and `completed` flag. Each needs a draft and a decision about
   whether it belongs on the invocation.
2. **The boundary id is the step token.** `LF_FLOW_STEP` carries
   `{invocation, boundary}` into the step's process, and a review Session's id
   is `flow:<invocation>:<boundary>`. The row's equivalent is
   `position_version`. Replacing one with the other changes stored Session ids.
3. **Three writers are other processes.** `lf flow decide`, `lf flow route` and
   `lf flow blocked` run inside the step's Run and write the verdict or route
   into the file under `position.lock`. On the row they need the same fence a
   Task's verdict uses (`record_flow_verdict`, keyed by Task). That function
   takes a Task id; a saved Flow has none.
4. **A saved Flow must run when the store cannot be written.** Cut 3 and Cut F
   both keep that rule: the Flow runs and warns. With the row as the only
   cursor owner, a Flow with no store has no cursor. Either the rule changes for
   Flows or the file stays as the fallback, which is two owners again.
5. **`driver.lock` is a kernel lock on a file in the Flow's directory.** It
   stays a file either way, so `flows/<id>/` does not disappear.
6. **The import parses `FlowRun`.** `ops/session_import.rs` reads old
   `position.json` through the type. Deleting the type moves its shape into the
   import module, about 15 lines added there.

A cut that does this needs items 1–4 decided first. Estimated result once
done: `flow_run.rs` drops to the locks, the token and `launch_driver`, about
−330 lines, against about +150 in the store.

### `task_flow_positions`

No Rust reader or writer. The table is dropped by the draft
`retain_flow_invocations`. The remaining mentions are
`store/migrations.rs:4885–4957`, a migration test that seeds the old table to
prove the draft converts it, and two drafts. Nothing to delete until the drafts
are released and that test's subject is history.

### `human-sessions/` lock files

`lock_session_launch` (`ops/human_session.rs:1731`, 17 lines) has eight
production callers: `ask_once` `:311`, `serve_ask` `:604`, the Flow review
launch `:720`, `publish_run_binding` `:785`, `open_waiting` `:973,1024,1055`
and `complete` `:1342`. It excludes two launchers of one Session from starting
two providers. A row cannot hold a kernel lock, and the architecture keeps live
exclusion with the kernel. The directory holds no other file. It stays.

### The `prepared` marker

`run_record.rs:1320` writes it, `:1336` renames it to claim a launch, `:606`
reads it. `run_is_prepared` (`ops/human_session.rs:1147`, 14 lines) has four
callers at `:317,793,1074,1394`. The rename is the atomic claim that stops two
launchers from starting one prepared Run. A column could record "reserved, not
launched", and `runs.published` already does for Task reviews. Asks and saved
Flow reviews store `published=1` at preparation, so moving them means changing
what `published` means for two Session kinds and replacing the rename with a
compare-and-set. Blocked on that decision; about −40 lines if taken.

### `resolve_manifest`

`run_record.rs:563`, 39 lines. Production callers: `lf runs <id>`, `--parent`,
`resume_run`, `lf replay`, `find_session`, `run_is_prepared`, three exact-id
reads in `human_session.rs:1120,1403,1439,1475`, `recover_task_decision`,
`start_prepared`. It resolves a Run-id prefix by listing `runs/`. A prefix
query on `runs.id` would replace the listing, but an unrecorded Run has no row
and `lf runs <id>` must still read it. Blocked on whether a prefix may miss
unrecorded Runs.

### String subjects

`RunSpec.subjects` `run_record.rs:39`, `RunManifest.subjects` `:187`,
`PromptBuild.subjects`, `WorkBinding.subjects` `ops/run.rs:23`,
`human_session::work_selector` `:871`. No listing reads them after Cut F. They
are still written into every manifest. Readers: the import;
`ActiveRun.subjects` (`run_record/active.rs:75,259`), a DTO field Swift mirrors;
the prompt's Work context. Deleting the writers empties a DTO field, so this is
a DTO and Swift change. Out of scope by the brief.

### `SessionTitleSource`

`run_record.rs:972`, 7 lines, duplicates `session::TitleSource` and adds
`Unavailable`. It is the DTO type; `swift/LoopflowMac/Views/WorkspaceBreadcrumbBar.swift:133`
and `DTOFixtureTests.swift:305` read `.unavailable`. No Rust code produces
`Unavailable`. Removing it is a DTO change. Out of scope.

### `WorkCatalog`

`lf/commands/work_catalog.rs`, 85 lines. `lf activity` labels and filters its
PR, Steer and creation entries through it. Runs no longer use it. Replacing it
means one query per entry kind with the Work filter in SQL. Reachable, but it
is a reader rewrite with no Session, Run or invocation in it.

### `SqliteStore::position_runs`

`store/sqlite/runs.rs:408`, 17 lines. Three store tests call it; no production
code does. It is the only reader of a position's attempts. Kept: the attempts
are a fact the model names, and the first production reader is the saved Flow
driver once the row owns its cursor.

### `terminal.json` as state

`flow_run::recover` `:361` and `recover_task_decision`
(`controller/task/mod.rs:584`) read a Run's outcome from its record. The row
has it after Cut F. `recover` is synchronous and holds `position.lock`; reading
the row there needs the store inside that lock. It goes with the cursor cut.

### The import-only columns

`flow_invocations.historical_session_run_id` and `historical_ready_summary`
(`own_sessions_and_runs` draft, lines 88–89). No Rust code reads them; the
import does not either. Dropping them deletes old evidence. Schema only, no
Rust lines. Needs Jack's decision.

### `ops/session_import.rs`

585 lines. Its own header says when it goes: once every Home that ran a
release older than the tables has been imported. Not yet.

### Started events and their trigger

The `task_chapter_started` trigger still writes the event. `chat/turns.rs:329`
renders it. No code treats it as truth after Cut F.

## Swift

`swift/Loopflow/Services/RegistryQuery.swift:253` reads Sessions with one call,
`lf session list --json`, and decodes `SessionRecord`. Open, rename and
complete are one call each (`:263,276,285`). Live Runs come from a separate
stream, `lf runs --active --watch --json`
(`swift/LoopflowMac/Services/RegistryQueryLocal.swift:14`), which the Session
views observe for activity, not identity. No Swift code assembles a Session
from several reads. Nothing to delete. Swift was read, not built.
