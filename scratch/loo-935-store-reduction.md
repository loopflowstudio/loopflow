# LOO-935: remove what the product no longer uses, and keep it removed

Status (2026-11-17): `lf task view`, `lf open --diff`, the Rex follower and Task widening are gone (6fb8816a4e, 1efd501812); every view of a held terminal types (Rust, Swift, docs, tests done). Concept review's doc repairs are done; next is gate.

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
checkout). `Native::add_task_tab` (`open/workspace.rs`) builds that; `layout.rs`, `tools.rs`, `Templates`, the storage
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
- Compress (layout fold): `cargo clippy -p loopflow --all-targets -- -D warnings`, `nextest -p loopflow --lib commands::open::` (17), integration `work_open_tests::` at `-j 4` (32 of 33, the same `task_location_uses_the_named_plan_outside_a_checkout`): pass.
- QA (fold): Rex and Herdr overviews, Task/Wave widening, `--available` walked in a private Machine (`scratch/qa.html`). Fixed: overview links and receipts fall back to `lf open --workspace <url>`; an empty `lf open --available` says "No available conversations.". `work_open_tests::` touched rows at `-j 1` and `cargo clippy -p loopflow --all-targets -- -D warnings`: pass.
- Verify: `uv run python scripts/due_removals.py install` on the Machine that runs Tasks.

## lf workspace folds into lf open (2026-11-16)

Jack Heart, word for word: "i think workspace should just be for waves and repos
now", "and honesrly workspace should be combined w open fr that reason", "do it
here". Asked what `lf open --wave product` shows, the answer picked:
"Conversation; overview by flag".

Done: `lf workspace` and `WorkspaceArgs` are gone; `OpenArgs` carries
`--workspace`, `--available`, `--no-focus` and the hidden `--local-workspace`,
and `run` dispatches overviews to `run_workspace`. Remote Machines are invoked
as `lf open --workspace --local-workspace …`. `--workspace` with a Task is an
error naming `--wave`/`--repository`. `--here/--tab/--focus/--file` conflict with `--workspace`/`--available`.
PR body, User interface changes: removed `lf workspace`; added `lf open
--workspace`, `--available`, `--no-focus`.

## lf task view goes; overview tabs hold the Task's Session (2026-11-17)

Jack Heart, word for word: "i think we can kill lf task view. I think its
kinda reasonable idea but its not the right shape and other things need to
change before i can figure out the right shae." Asked what an overview tab
holds instead, the answer picked: "The Task's Session" (each tab runs the same
path as `lf open --task X --here`; read-only with Move here when another view
holds input; an ended Session resumes its provider).

- Delete `lf task view` (`commands/task_view.rs`, its Clap variant, tests,
  docs, help text and every caller). Desktop's own Task & workflow pane stays.
- Overview Task tabs and Rex follower arrivals attach the Task's Session by
  `lf open --task <ID> --here`'s rules. "Overviews start no agents" no longer
  holds; docs say an ended Session resumes when its tab opens.
- `--diff` in an overview: Changes beside each Session, as with `--task`.
- Done. `BrowserTab.task` and `waves::subscribe_task_pane` stay: Desktop's
  own Task pane uses both. The cmux/Herdr/Rex `split` renderers went with `--diff`.

## Every view of a managed Session is editable (2026-11-17)

Jack Heart, word for word: "a task tab was meant to be the task session, with
an editable input. i think our policy for managed sessions is just you get many
editable viewers into the same terminal, so you see drafts from A in B", then
"but we should change the ViewerPolicy", "in this diff."

- Every view of a held terminal sends input. There is no input claim: delete
  `control`/`replace`/`own` input semantics in the attach protocol, read-only
  views, **Move here** / **Take back** (Desktop and docs), the
  claim/`viewer_hold` paths in the holder, server and `transport/`, and the
  "already open, opened nothing" branch of `lf open` (`--here`/`--tab` attach
  another view; `--focus` still reveals an existing one).
- `lf resume <session>` opens that Session here; it no longer "moves input".
- Terminal size: the view that most recently attached, sent input or gained
  focus sets it (Jack Heart: "cna we also have re-focusing an old view re-takes
  the size").
  When that view detaches, the size goes back to the remaining view that most
  recently did one of those ("or at the very least closing the new view gives
  teh size back to the old view").
  No geometry owner, no `own: request|take`. Jack Heart, after a Rex-style
  owner was proposed instead: "actually keep that".
- Native client lf can't share: kept. A Session whose provider runs outside a
  held terminal still refuses `lf open` ("controlled elsewhere") and Desktop
  keeps **Take back** for it (`SessionActionKind::MoveHere`, `heldElsewhere`).
  Held terminals never offered Take back (`held_actions`). Say this in the PR.
- Built: `Sizing` (transport.rs) picks the live view with the latest
  attach/input/focus; relays turn on DEC 1004 and send `focus {focused}`; the
  server forwards CSI I/O only when the agent's own mode 1004 is on
  (`Terminal::focus_reporting`). Only the sizing view's query answers reach the
  agent (`input::without_reports`); another view's input ends an open paste.
  Wire: attach drops `control/replace/own/moved`, `own` request and `control`
  event gone, `attached.token` gone, `controlled.held` → `viewed`; Desktop
  status drops `input`/`moving`/`move`. `LF_RELAY_IF_FREE`, `--moved` gone;
  open intents (`--start-if-free`, `--reveal-existing`) stay for whether a
  provider starts.
  Held-terminal `registry::REVISION` is 4: a viewer from another build gets
  "Loopflow server speaks held-terminal revision", not a serde error.
- PR: hosts that report focus: any whose terminal honors DEC 1004 (Ghostty in
  Desktop, Rex, Herdr, cmux, most terminals); others fall back to attach/typing.
- `held_terminals.viewer_hold/_at/viewed` stay: they record whether anyone
  views a terminal (Task retirement's absence clock), not an input claim.
- Docs: transport/README.md, docs/sessions.md, docs/surfaces.md,
  docs/security.md (a connection is a full editor; access is per person),
  docs/server.md, docs/troubleshooting.md, docs/workspaces.md.

## --workspace on a Task, and --diff (2026-11-17)

- Jack Heart: "--workspace on task should be rejected or ignored". Chosen:
  rejected, with an error naming `--wave`/`--repository`. Task widening goes.
- Jack Heart: "lets just remove this flag for now. i am interested in bringing
  it back but not now." `lf open --diff` is removed; `lf diff` and Desktop's
  Changes pane stay.
- Jack Heart: "i think we can drop the whoel workspace follower tab concept".
  The Rex Workspace follower, automatic arrivals and the six hidden `lf open`
  flags go; an overview opens its tabs once.

## Checks (viewer policy, 2026-11-17)

- `nextest -p loopflow-execution --lib transport::` (69; two registry tests time out at load ~128 and pass alone), `-p loopflow --lib` open/lf/schema rows, integration `work_open_tests::` (33; three 2 s host deadlines fail only at `-j 2`, pass alone), `dto_fixtures::`, `cli_discovery::`, `documented_commands::`, `binary(transport_tests)` (40), `cargo clippy --all-targets -- -D warnings`, `check_architecture.py`: pass.
- Swift: `scripts/test_desktop.sh --filter 'OwnerStreamTests|ServerConnectionTests|ProviderTransitionTests|DTOFixtureTests|WorkDestinationTests'` and `test_ios.py SessionTerminalTests`: pass. `GhosttyTerminalInputTests` needs a display (gate/CI); no test covers focus reports reaching the relay.
- Compress: revision bump; `nextest -p loopflow-execution --lib transport::registry` (12): pass.
- QA (every view types, `scratch/qa.html`): two PTY views share drafts, input and size by latest attach/input/focus; Rex overview tabs hold the conversation and restart an ended provider. Fixed: `--workspace` on a Task names the Wave in any terminal; stale input-claim help and `--focus` hint. `work_open_tests::task_link_resolves_by_folder_name_without_preparing_anything`, clippy: pass.
- `lf open` keeps its "already open" report for a Session a native client holds outside a held terminal (and for a Desktop pane in Desktop mode).

## Concept review repairs (2026-11-17, done)

- `docs/dev/server-first.md`: "### Every view types" describes what is built;
  its Pieces row now names only what remains: the server's emulator answering
  every query, and Desktop's relay status handshake (`LF_SERVER_STATUS`,
  "Opening… Input paused"), which still carries Reconnecting, Session
  unavailable and Retry.
- `.lf/qa/desktop.md`: the component probe's limits no longer mention Move here.

## lf resume does one thing (2026-11-17)

Jack Heart, word for word: "i think of it as sort of open sits on top of
resume", "lets start with maknig resume do what it should do. and then we can
talk about what opens does", and on an agent running in a terminal app lf
can't share: "just always have it take over". Earlier: resume is "only about a
session", "in this eterminal"; "tasks and waves can still be turned into
sessions".

`lf session resume` / `lf resume` has one implementation (merge
`ops::human_session::open_without_replacing` and `open`):

1. Takes a Session ID, a Task, a Wave, or nothing (this checkout's); resolves
   to that scope's Session, creating it if there is none and saying so.
2. Live held terminal: add a view here (every view types).
3. Ended or crashed: resume the provider from history in a held terminal, then
   attach.
4. Provider in a native client lf can't share: always take it over (end it,
   continue here held) and say so. This applies to every caller, including
   `lf open` and Desktop's automatic opening; Desktop's **Take back** and the
   "controlled elsewhere" refusal go.
5. `--fork`, `-a <harness>`, `--message` with `-b`, `--prepare` unchanged.
6. Help (`lf/mod.rs` after_help), `resume_explanation.rs`, docs/sessions.md's
   table and every "hands it over" / "replacing another terminal" text describe
   exactly 1-4.
7. Jack Heart on `--start-if-free` and `--reveal-existing`: "kill it". Both
   flags and `SessionOpenIntent` go, with no "require existing history" mode
   for Desktop. Every caller (`lf open`, Desktop prepare, overview tabs) gets
   plain resume: resolve, create if missing, attach or restart, take over a
   native client.

`lf open` keeps calling `lf session resume`; what `lf open` itself does is the
next conversation, not this pass.
