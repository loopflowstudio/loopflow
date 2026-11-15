# LOO-696: one Rust source for each Task concept

## Problem and intended experience

A Task must mean the same thing in its header, picker, sidebar and board. An
active Task with planning state Todo shows Active wherever lifecycle appears.
A started Task remains started after unassignment. An open conversation with no
activity report earns neither Available nor Busy. Moving a Task in the shared
order moves it in every ordinary Task list.

Jack Heart, November 9, 2026: “lets lf sync and re-evaluate” and “ist his task still reelvant post loo-593”.

`lf sync --manual` completed without conflicts at `4848816f7`; checkout was clean
before and after sync. No tests run for the conflict-free sync.

Jack Heart, November 14: “I still don't think I know what this does. Do an LF sync and tell me what's going on”.

`lf sync --manual` completed without conflicts at `a442cee237`, incorporating
main `3fe914c022`. The branch had no code diff from origin/main, no Graph runs
and no PR. No tests run for the conflict-free sync.

Jack Heart, November 14: “ok lets fdo it”, following the explanation of fixing
contradictory Task status across Desktop.

The interrupted kickoff left only this note. On November 14, the resumed read
reported the feature Graph at kickoff, with no PR or implementation diff.

## Architecture and decisions

Keep the existing server, subscription and mutation paths. LOO-593 supplied
shared access; it did not remove Swift's competing interpretations. The repair
belongs in Rust projections and their existing Swift consumers, without a new
store, polling loop or UI design.

### Lifecycle and actions

`store/sqlite/durable.rs::task_state_sql` derives execution lifecycle;
`store/sqlite/task_details.rs::states_in` reads it in bulk. In
`durable.rs::TaskSection::of`, planning completion/cancellation withdraws an
unstarted Task, while started work follows runtime state. Ownership determines
Backlog versus Working/Autonomous, independently of whether work started.

`lf/commands/waves.rs::RoadmapTask` already carries runtime, section,
`run_control` and `ship_control`. Complete rows currently always carry runtime,
but its DTO is nullable. `ops/task_actions.rs::TaskActionModel` recommends a
next action; it is not a complete lifecycle menu or an authorization list.
Do not treat it as one.

Add a required `RoadmapTask.lifecycle: TaskState`, derived by one Rust helper
from runtime state and the planning terminal exception already used by
`TaskSection::of`. Make section and selection lifecycle states call that same
helper. Keep `runtime.status` as the execution reading, with its existing
meaning; lifecycle is the effective display state including withdrawal of
never-started work. This distinction justifies the derived field. No Swift
fallback between planning and runtime remains.

Derive lifecycle from the required Rust facts before serializing runtime; an
unreadable Task is unavailable, not Unstarted. A missing nullable runtime payload
does not invite a client guess. Planning text may remain in its planning editor.

Use lifecycle in `TaskHeading`, `TaskWorkflowHeader`, the picker, terminal menu
branches, `RoadmapTask.awaitsStart`, `isCompleted` and conversation intent.
Ownership remains a separate annotation. Pure enum-to-label and enum-to-intent
rendering is fine; deciding which record wins belongs to Rust. Keep command
refusals in Rust and use existing run/ship controls. Mutations still recheck
current state; a displayed status grants no new authority.

### Session membership and availability

`store/sqlite/task_work.rs::session_tasks` includes explicit binding and every
containing checkout, deduplicates, and excludes repository/Wave primary scope.
`store/sqlite/sessions.rs::summary_query` exposes those IDs. Selection's
`work_in` already groups Sessions by those IDs and ignores completed Sessions.
Use that membership everywhere. Swift's `SessionRecord.names(task:)` becomes a
lookup in server `taskIds`; delete its union with `workspace` and `work`.
`WorkProjection` indexes exactly the same IDs as `WorkModel.taskSessions`,
including Tasks without runtime payloads. Workspace location and historical Work
attribution remain available for their own purposes, not membership inference.

`store/sqlite/selections.rs::work_in` already derives Available from the shared
availability fact, Busy from every current Working report, and Blocked from
Blocked reports. Unknown process identity is tracked separately. Extract this
fold into a reusable Rust helper and expose its `selection_states` on
`RoadmapTask`; selection contents and roadmap rows must call the same helper,
not copy its branches. Supply facts in bulk within the existing reading.

Board badges render this set: Available when present, otherwise Busy when
present, otherwise no badge. A Task can have both Available and Busy across
multiple Sessions; both filters include it, and the board's single badge keeps
its existing Available-first presentation. An unknown Session alone adds
neither. Unknown evidence never authorizes removal of retained panes. Do not
fold these Session facts into Task lifecycle or NOW conditions.

### Ordering

`store/sqlite/selections.rs::work_in` orders Tasks by persisted `task_orders`,
then attention and stable rank/ID for unnamed arrivals. Its regression
`one_task_order_reorders_working_tasks_and_every_plan_and_arrivals_append`
already covers selection plans. `lf/commands/waves.rs::snapshot_tasks` instead
sorts completion/rank/identifier, and Swift sorts again in `WaveStandup`,
`WorkProjection` and `WorkModel.jumpRows`.

Extract the existing selection order into one shared Rust ordering operation.
Apply it to ordinary roadmap inventories as well as selection input, preserving
relative order when scoping or partitioning sections. Read the order once per
repository, not per Task. Remove the competing Swift rank sorts. No new order
field or client comparator is needed: arrays arrive ordered.

Keep explicitly authored Dashboard arrangements, search relevance/recent visits,
and Done's completion-date ordering: they express different concepts. Ordinary
repository and Wave lists share Task order; a saved arrangement is not rewritten
when the repository order changes.

### Shipping and NOW: confirmed distinctions

`ops/task/ship.rs::shipping_snapshot` can return Restarting or Unassigned while
the runtime lifecycle is Idle or another non-Shipping state. Its presence means
an Auto-Ship handoff exists; runtime Shipping means execution can proceed without
another interactive stop. Keep snapshot-based Stop Auto-Ship and graph stand-in
presentation. Test this distinction; do not replace every `shipping != nil`
with lifecycle Shipping.

`swift/Loopflow/Models/NowProjection.swift::nowGroup` already groups Rust
`TaskConditionState`. Waiting/Blocked/Clear/Unknown describe operating condition,
not lifecycle sections. Preserve it and its tests.

## Delete and cut over together

- `TaskWorkflowHeader` planning-state lifecycle labels, terminal OR expressions
  in `TaskActionItems` and `TaskShipControl`, and the planning-only refusal in
  `TaskWorkflowHeader.unavailable`; consume lifecycle and existing Rust controls.
- `TaskHeading.state`'s planning/runtime precedence; retain only label rendering.
- `TaskPlanningSnapshot.isTerminal` and its `isCurrent` history-filter extension;
  the history view now filters the Rust lifecycle while retaining completion-date
  windows and unresolved execution.
- `WorkPalette.jumpRows`' owner-based “Not started” suffix and rank sort.
- `WorkModel.standup`'s “any open non-available Session is Busy” fold and its
  SessionAttention dictionary input; board rows read the shared states.
- `WorkProjection`'s membership unions/primary-scope interpretation and its rank
  sorts; `SessionRecord.taskMemberships`' parallel membership derivation.
- `WaveStandup`'s ordinary rank sort and Rust roadmap's competing comparator.
- `WorkChannelChips` runtime-only display and `WaveDetailPane` history/planning
  status precedence; both now render the required lifecycle. Planning chips
  remain explicitly labeled PM.
- Fixture-only aliases between a planning Task ID and a different runtime Work
  ID; complete records name the same Task on both fields.

Replace assertions that encode those rules, including WaveStandup rank-order
expectations. Keep tests for ownership/person grouping, Done date windows,
retained conversations, historical links, unstarted Start, legal run refusal,
Dashboard arrangements, and membership across chapter/Wave moves.

Do not leave fallback decoders, old/new resolvers, duplicate folds, client-side
rank repair, or a second lifecycle precedence rule. Update required DTO keys in
all shared and inline fixtures, Rust round trips, Swift decoding, iOS consumers,
and generated server schema if affected. Required nullable keys stay strict.
Old saved payloads that cannot decode follow existing cache invalidation; no
compatibility defaults that manufacture current activity or actions.

## Risks, alternatives and failure cases

Reusing only existing fields would still require every consumer to repeat the
planning/runtime exception. Adding a Swift helper would hide that duplication
from the views while leaving a second product authority. Choose the derived
Rust lifecycle and shared state/order helpers instead.

A new presentation service or persisted display state would add synchronization
and recovery work without new information. Extend existing projections only.
No database migration is required.

The principal operational risk is loading Sessions and Task order separately
for every row. Keep bulk facts and the Work reader's pinned snapshot/savepoint
contract. No provider/network work for these derivations; subscription changes
must cover reports, membership, ownership and order. Check query plans at the
repository's 10,000-Task scale and retain the performance test's bounded query
counts. A failed or partial read stays unavailable/last-good, never empty.

Success is moving between views without mentally translating the Task's labels.
Failure is a helper reused in two views while a third keeps guessing, or a saved
snapshot restoring stale activity/actions. Tests must exercise the production
consumers, not just the helper's output.

## Remaining work

The Rust/Swift cutover is implemented as one change. Gate owns the bounded
performance/read-volume checks, full platform suites and final review; the
interactive demo below remains. Publication and landing belong to their Graph
steps, with no merge authorization implied here.

## Concept review — November 14

Outcome for loop-or-next: nothing to change and no open product choice; advance.
The implemented model matches the intended interaction. No next implement
direction is needed. `scratch/concept-review.html` shows the review beside the
conversation. This review changes no production code and starts no execution.

The important distinction survives the simplification: Active describes the
Task's lifecycle; Backlog can describe its lack of ownership; Available/Busy
describe its conversations. Auto-Ship's retained handoff supplies Stop Auto-Ship
even between autonomous runs. NOW still describes operating condition. Making
all five into one status would discard behavior the design requires.

The normal path is one Active Task with planning Todo: page, picker and heading
read `lifecycle`; board and sidebar read the shared selection states; conversation
menus read `task_ids`; ordinary arrays retain Rust's order. Unassignment changes
section without offering Start or discarding checkout, PR or conversations.
The existing mutations recheck state rather than trusting those displayed facts.

The failure path is an unavailable reading or saved startup display. Missing
required fields invalidate the payload rather than inventing Unstarted. Saved
display strips conversation activity and run/ship permissions. Recovery receives
fresh projections through the existing subscription; it must retain panes and
drafts through incomplete Session inventory. No new persisted lifecycle, migration,
provider call or refresh owner is needed.

Counterexamples retained: never-started planning completion must offer Reopen;
planning completion must not withdraw started Active work; an unknown open Session
must not become Busy; concurrent Available and Busy reports must match both
filters; explicit Dashboard arrangement and Done date order must survive ordinary
Task reordering; Restarting/Unassigned Auto-Ship must remain stoppable. The current
design and production consumers account for these cases. Earlier validation
results below are prior-run observations, not checks rerun during this review.

Next show the same Task across page, board, sidebar and picker, then unassign,
reclaim and reorder it. Gate still owns read-volume/performance and platform
checks; the interactive demo still owns visible clarity and retained native input.

## Gate acceptance and demo

### Shipping docs

Read `swift/README.md` and `docs/surfaces.md` before implementation. They describe
the intended lifecycle/ownership distinction, shared conversation membership and
availability, Task order and its exceptions, and stoppable Auto-Ship handoffs.
The Rust/Swift cutover implements these docs-first edits.
No new product question arose while writing them.

November 14: `lf open --host desktop --file swift/README.md --file docs/surfaces.md`
exited successfully and reported opening both files in Loopflow Desktop.
`git diff --check` passed. No builds/tests run for prose-only edits; implementation
and gate own the checks below.

### Acceptance scenario

One seeded scenario follows a Task from Rust through the wire to all affected
consumers. Give it a checkout/PR, runtime Active, planning Todo, an explicit
Session plus a checkout Session, and an unknown open Session. Unassign and
reclaim it, report Working and then Available, reorder it against another Task,
and refresh the projection. Keep the production Rust projection output in a
shared DTO fixture consumed by the Swift checks, so the two languages cannot
pass by constructing different meanings for the same scenario. Assert:

- Header, heading and picker show Active; unassignment may put it in Backlog
  but never produces “Not started”, Start or Reopen.
- Planning-terminal/unstarted work shows Done/Abandoned and Reopen; a started
  nonterminal runtime with conflicting planning text remains nonterminal.
- Unknown alone gives no Busy badge/filter membership; Working and Available
  agree between board and sidebar, including multiple-report/multiple-Session cases.
- Menu, projection and breadcrumb sibling membership agree on the server IDs;
  repo/Wave primaries stay out, duplicate associations yield one Session row.
- Ordinary board, picker, projection and sidebar retain server order after refresh;
  saved Dashboard arrangements and Done date ordering retain their own semantics.
- Auto-Ship Restarting/Unassigned remains stoppable even without runtime Shipping;
  NOW grouping remains driven by Task condition.

Headless commands from the checkout root, with inherited `LF_*` removed and
stdin redirected from `/dev/null` for cargo/test commands:

- `cargo test -p loopflow --lib store::sqlite::selections::tests::` — shared
  availability, membership and ordering scenarios pass.
- `cargo test -p loopflow --lib lf::commands::waves::tests::` — effective
  lifecycle and roadmap production projection cases pass.
- `cargo test -p loopflow --lib ops::task::ship::tests::` — handoff versus
  lifecycle semantics remain covered.
- `cargo test -p loopflow --test integration dto_fixtures::` — complete wire
  fixtures round-trip; missing required fields fail.
- `scripts/test_desktop.sh --filter 'WorkNavigationTests|WaveStandupTests|TaskRowMetadataTests|TaskStatusTests|DTOFixtureTests|NowProjectionTests'`
  — app/test build and production view/projection checks pass without a display.
- `cargo test -p loopflow --test integration performance_tests::` — bounded
  read work remains within the existing suite's expectations.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` — clean.
  Gate adds server schema and multiplatform checks when the final diff reaches them.

Demo: `uv run python scripts/loopflow-dev.py run`; inspect the same Task through
its ordinary page, Wave board, sidebar filters and breadcrumb picker, then drag
it and return. Its meaning and ordinary order agree. The interactive demo judges
clarity; it is not an unattended gate requirement.

Done when those consumers agree without Swift precedence or membership/order
repair. This supports Product's Work-first, shared-contract goal and dependable
Desktop sessions; headless checks do not establish three days of adoption,
two-hour sessions, native provider continuity or external project progress.

## Local validation — November 14

- `cargo test -p loopflow --lib lf::commands::waves::tests::task_concepts_share_lifecycle_activity_membership_and_order` — passed; `UPDATE_TASK_CONCEPT_FIXTURE=1` writes the shared production DTO scenario.
- Built library binary: `store::sqlite::selections::tests::` — 22 passed, including saved order, Dashboard arrangements, unknown activity and retained panes.
- Built library binary: `durable::tests::lifecycle_withdraws_only_never_started_work`, `ops::task::ship::tests::shipping_reads_what_happens_now_and_never_waits_on_a_person`, `store::query_plans::tests::session_lookups_and_steer_reads_plan_by_index` — passed.
- `cargo test -p loopflow --test integration dto_fixtures::` — 52 passed, including missing required keys and the production scenario's wire round trip.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'TaskConceptTests|WorkNavigationTests|WaveStandupTests|TaskHistoryFilterTests|TaskRowMetadataTests|TaskStatusTests|DTOFixtureTests|NowProjectionTests'` — 142 checks covered. 140 checks, including the new scenario, passed initially; two obsolete rank/identity assertions were replaced and passed with the focused `workDoesNotRequireSessions|everyHumanBoundaryRemainsReachable` filter.
- `target/debug/lf server describe --json` — regenerated the schema; Task rows require both new fields, and the final candidate matched the fixture byte for byte.
- `scripts/test_desktop.sh --skip-build --filter WorkCLISubscriptionTests` — 5 passed, including completion, abandonment, conversation initialization and retained workspace membership moves.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `uv run python scripts/check_architecture.py` — passed.
- `uv run python scripts/check_swift_multiplatform_boundaries.py` — passed.
- `lf context` — memory and scratch remain under both configured limits.

The production scenario's early seed attempts failed on `Task PR placement differs
from Task`, `InvalidColumnType(2, "project_slug", Null)`, `invalid Program Status
snapshot`, and `Task ownership entries are retained`. Its fixtures now supply
complete planning/delivery facts, distinct reporter IDs and append-only ownership.
Rust builds emitted the macOS `__eh_frame` size warning; Swift emitted warnings in
unchanged GhosttyManager and ServerClient code. Builds completed.
