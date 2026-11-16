# LOO-925: the tracker mirrors the store

## Decision: any tracker — Linear and Asana bridges, Notion deleted

The store is the plan. A tracker is a bridge that carries the store's changes
out and its coworkers' changes in. Linear and Asana are the two bridges; Notion
is deleted. Git planning is not a bridge: it exchanges the store's own WorkEdit
journal between peers and keeps working as it does.

Basis, all recoverable only from here:

- LOO-691's unmerged branch `loo-691-combined` (last commit e8ffd80ca0,
  2026-11-04, "checkpoint the interrupted parity pass before splitting the
  diff") records in `scratch/design.md`: "Jack Heart, 2026-11-02: drop Notion
  in this diff; regressions on Linear, Asana and Git planning; feature and
  architectural parity between the three (one interface at one layer, no
  Linear escape hatch or Linear-named shared types)." That is the design
  doc's wording, not a transcript. The Notion delete (cd7afe0127) never
  reached main; regressions shipped instead as GitHub issues (#1780).
- Asana was rebuilt 2026-10-22 (#1607) after an earlier delete (c113ef04b4,
  2026-07-02, "asana-only: drop Linear/Notion"). The README promises "whatever
  … tracker you already use".
- The neutral queue is the Outcome under either choice, so "any tracker" costs
  only keeping Asana (~1.3k lines) on the bridge interface. Linear-only would
  delete Asana a third time.
- Run observation, 2026-11-16, `~/.lf/loopflow.db`: every `pm_items`
  (1,246) and `pm_projects` (57) row is `linear`; all 118 `.lf/config.yaml`
  planning blocks under `~/src` are Linear. No store on this Machine has Asana
  data, so making `provider` truthful needs no data repair.

Overturnable at review: `scratch/questions.md`.

## Demo

```bash
export LF_HOME=$(mktemp -d)        # branch lf, Linear stand-in (test fixture) or a sandbox Team
lf task create --title "Mirror demo" --json
lf task edit <id> --title "Mirror demo, renamed"
lf steer <id> "first comment"
lf task complete <id>
lf repo plan --json                # one queue: 4 effects, seq order, kind, phase, provider, receipt
lf task show <id> --json | jq .sync
```

Switch `.lf/config.yaml` to `provider: asana` against the Asana stand-in and
run the same five commands: the same queue rows, `provider: asana`, and
`SELECT provider FROM pm_items` answers `asana`. Pull the network: every
command saves, `lf repo plan` lists the effects as pending, and the next
connected command drains at most one bounded pass.

## Current architecture (at 470da475a1)

- **Journal already neutral.** `sqlite/planning_write.rs` is the one typed
  writer; every planning write is a `WorkEdit { target, WorkValue }` captured
  in `planning_peer_changes` (42,315 rows in the live store) with clock and
  parents. Git exchange ships these. Trackers ignore them.
- **Seven outbound receipts, four drains.** Receipts: `task_creation_intents`
  and `project_transitions` (`export_json` holds Linear GraphQL input built in
  `loopflow-storage/src/sqlite/planning_export.rs:130`, with
  `linear_records::render_project_content`), `task_changes` and
  `project_changes` (already `field` + `value_json` WorkValue +
  `base_json`/`conflict_json`/`attempted`/`acknowledged`; deletion is
  `task_changes.field='deleted'`, order is `project_changes.field='task_order'`
  + `order_effects_json`), `task_state_deliveries`, `task_comment_deliveries`.
  Drains: `ops/planning_export.rs::sync_repository_exports`,
  `ops/planning_delivery.rs::sync_repository_fields` (+ `planning_order.rs`),
  `ops/linear_observe.rs::sync_repository_deliveries` (state, comments),
  called from `planning_sync::sync_after_save` and `linear_observe::sync_scope`.
  Each joins over every pending owner with its own 5 s deadline.
- **Live store backlog, 2026-11-16:** `task_changes` 503 acknowledged, 167
  attempted, 86 pending; `task_state_deliveries` 1,064 rows; 2,656 comment
  deliveries. 167 attempted-and-never-settled field edits is the lost-reply
  rule failing quietly.
- **Execution builds Linear input too.** `planning_delivery.rs::task_input`
  emits `assigneeId`/`projectId`; Asana's `asana/mod.rs::task_fields` (664)
  translates Linear keys back. `PlanningClient` (`ops/planning_client.rs`) is
  a 3-arm enum of passthroughs returning `LinearIssue`/`LinearResult`;
  `PlanningClient::linear()` is the escape hatch (order delivery, Project
  edits).
- **Stall fix to keep** (#1829, b9339916f1): no freshness check inside a
  write; one `ListTeams` per process (`VALIDATED_TEAM_CLAIMS`); an uncertain
  creation is looked for once per `max(600 s, delta_fresh_secs())` via
  `export_observed_at`; `lf task sync ISSUE` looks now.
- **Revision triggers.** Every receipt table has
  `store_revision_<table>_{insert,update,delete}` bumping domain `planning`;
  `PlanningSyncStatus` (`loopflow-storage/src/planning_sync.rs:48`) feeds
  Task/Wave frames and Desktop.
- **Mixed versions.** `lf install` migrates while older lf processes keep
  running on the same store (`MIGRATIONS.md`, "Who applies what"); there is no
  reader floor any more. An older lf drains the old tables for the window.
- **Docs disagree with code.** `planning.md:88` "there is no generic planning
  facade"; `planning_client.rs` is one. (User docs dropped Notion in 9d71485f92.)

Prior attempt to mine before writing code: `loo-691-combined`
(f26a384079 shared records into `crate::planning`; 03b9d4311b Asana Project
status/order/assignee and `PlanningClient::linear` deleted; cd7afe0127 Notion
delete incl. OAuth provider and docs; `scratch/parity.md` capability ×
provider table). It is +4.3k/−4.3k over 66 files against an old base and was
never split; reuse its ideas and inventories, not its diff.

## Target architecture

**One queue, in the store's words.** New table `tracker_effects`:

| column | meaning |
|---|---|
| `seq` INTEGER PK AUTOINCREMENT | global send order |
| `id` TEXT UNIQUE | effect identity; the journal mutation id when there is one; the provider idempotency key where the provider accepts one (Linear create, comment) |
| `wave_id` / `project_id` / `task_id` | exactly one owner (CHECK): the tracker object the request addresses |
| `subject` | what on the owner: the field name, the comment id, `pr:<url>`; NULL for `create`/`delete`/`state` |
| `kind` | `create` · `field` · `order` · `state` · `comment` · `delete` · `link` (Initiative link or PR link) |
| `value_json` | the `WorkValue` (or, for `create`, the creation baseline `Vec<WorkValue>`); never provider input |
| `base_json`, `conflict_json` | provider baseline at enqueue; losing value after a conflict |
| `phase` | `pending` · `attempted` · `acknowledged` · `retired` |
| `provider` | the bridge the effect is for, set at enqueue (see Concept review) |
| `remote_id`, `acknowledged_revision` | what the provider answered |
| `error`, `attempted_at`, `looked_at` | delivery evidence |

Partial index `(phase, seq) WHERE phase IN ('pending','attempted')`; revision
triggers on domain `planning`, with the update trigger counting `looked_at`
only on NULL ↔ value.

**Every write has a receipt, structurally.** `planning_write::write` with
`WriteOrigin::Local` inserts the effect row in the same transaction for every
`WorkValue` a bridge carries; creation (`planning_write::create`) inserts the
`create` row. Provider calls for planning facts happen only in the drain.
Initiative rename and archive keep their `wave_lifecycle_operations`
receipts. GOAL.md name/summary → Initiative is not in this Task: no released
path sends it (the old apply branch was test-only and is deleted), so it is new
behavior, a follow-up Task (Wave `field` effects) filed with the PR.

**Bridges translate at the edge.** `ops/tracker.rs`:
`enum Tracker { Linear(LinearClient), Asana(AsanaClient) }` replaces
`PlanningClient`, built from `PlanningConfig`. Its methods take and return
store words: `send(&Effect) -> Sent`, `look(&Effect) -> Look` (readback after a
lost reply), `item(id) -> planning::Observation`, `changes(since)`. Linear
GraphQL lives in `linear/effects.rs`; Asana REST in `asana/effects.rs`. The
records a bridge answers (`Item`, `Observation`, `Comment`, `Project`,
`StateHistory`, `PlanningResponse`) move from `linear_records` to
`loopflow_storage::planning` (as f26a384079 did); `Linear*` types stay inside
`linear/`. No `linear()` escape hatch.

**Per-kind rules as functions, one lost-reply rule.**
`ops/tracker/rules.rs`: `fn settle(effect, observation) -> Settlement { Send,
Acknowledged(revision), Adopt { losing }, Hold }`, one function per kind,
carrying today's rules from `planning.md:204-272` (field baseline conflict
adopts the provider and keeps the losing value; order needs complete-list
evidence; deletion settles on trash or delete-ack, a newer active revision
retires it; state compares revision/state/completion; comment matches id or,
on Asana, visible text). Lost reply, for every kind: an `attempted` effect is
looked for before anything else on its owner is sent, at most once per
`max(600 s, delta_fresh_secs())` (`looked_at`, generalizing
`export_observed_at`); idempotent sends (Linear create and comment with our
id) re-send instead of looking.

**Bounded drain.** `ops/tracker/drain.rs::drain(store, repo, Bound)` replaces
the four drains. One pass reads unsettled effects in `seq` order; an owner
whose head is `attempted` and inside its window is skipped (head-of-line per
owner, not global); the pass stops after 20 provider requests or 5 s,
whichever first; owners run with concurrency 4. The per-object effect lock
file path (`planning_delivery::lock_fields`) is kept so an older lf and this
one never send for the same object at once. Each send keeps its own 5 s
deadline: a shared deadline was removed on purpose because a slow first export
starved every later Task (`planning_sync.rs:81`, `tracker_observe.rs:339`), so
the bound counts requests, and `sync_after_save` drains the just-saved owner
before the bounded remainder. Requests go through the existing
`linear/cooldown.rs::Budget` (creations stay `urgent`). `lf task sync <id>`
drains one owner, ignoring windows. A pass with nothing unsettled sends zero
requests.

**Truthful names.** `provider` columns get the configured bridge's name at
every writer: 14 SQL literals and 17 bound `"linear"` parameters in
`loopflow-storage` (mostly `linear_acceptance.rs`), several reached by Asana
through `planning_delivery::ingest_task` and `check_scope`. The acceptance
functions take the provider as an argument. (`ops/tracker_observe.rs` and
`rust/loopflow/tests/work/` already carry their true names.)
`planning.md`, `docs/work.md`, README say Linear or Asana.

### Concept review (2026-11-16): settled for slice 3

Implied by the design; implement builds these. No product choice in them.

- **An effect is addressed to a bridge, at enqueue.** `provider` comes from
  the owner's binding (`pm_items`/`pm_projects`/the Wave's Initiative), or
  for `create` from the configured bridge the caller passes to
  `planning_write::create`. An owner with no binding and no pending `create`
  gets no effect: a Git-only repository and a Wave on Asana (no Initiative)
  enqueue nothing, so `lf repo plan` never lists rows no bridge will drain.
  **Correction (realign):** today a Task made before a tracker was connected
  *does* export on connect: `children.rs:140` writes a `task_creation_intents`
  row for every Task, and `planning_export_owners`
  (`sqlite/planning_export.rs:45`) picks every Task with an intent and no
  `external_issue_id`. Slice 3 keeps that: a drain pass with a configured
  bridge first enqueues `create` for unbound, undeleted Tasks and unmapped
  Projects that have none (one `INSERT … SELECT`, counted against no request
  bound). Open alternative in `scratch/questions.md`. A pending effect whose
  provider is not the configured bridge retires with an error naming both.
  This removes the "Asana team fields" Wave mapping: Asana has no Wave
  object, matching `parity.md`'s n/m.
- **Comments and PR links are owned by their Task.** `comment_id` as an owner
  let a comment send before its Task's `create` settled. Head-of-line is per
  owner, so a Task's comments wait behind its creation.
- **Cross-owner waits are `Hold`.** A Task `create` or `task_project` field
  naming a Project with no remote id holds until that Project's `create`
  settles; the drain moves to the next owner. Nothing else orders owners.
- **Coalescing keeps today's rule.** Today only the newest unattempted row per
  `(owner, field)` is sent (`planning_changes.rs:62,108,411`). Enqueueing a
  `field`, `order` or `state` effect retires older *pending* effects with the
  same owner and subject in the same transaction; `attempted` ones stay for
  the lost-reply look. Comments and links never coalesce. Without this, 20
  requests a pass would spend on superseded values.
- **A conflict's losing value is shown by `lf task show`** (`sync.conflict` in
  `--json`, one line in text) from the newest settled effect with
  `conflict_json`, until a later local edit to that field settles. That is
  the surface the Demo already reads; Desktop reads the same DTO later.
  `docs/work.md:281` ("the Task shows both") then states built behavior.
- **Phases at settlement:** `Acknowledged` and `Adopt` → `acknowledged`
  (`Adopt` writes `conflict_json`); coalesced, superseded deletion, wrong
  provider → `retired`; `Hold` and `Send` leave the phase unchanged.

### Concept review (2026-11-16, after the queue commits): settled for the cut

Read against a2c956a129 and 7e3fa2ac84. No product choice in these.

- **One enqueue point, by origin.** `planning_write::local` also carries
  provider facts: the tracker-removal `TaskDeletedAt`
  (`linear_observations.rs:34`), the adopted `Disposition` after a state
  conflict (`task_state_delivery.rs:125`), the `TaskLinearId` attach
  (`planning_export.rs:312`). Enqueueing per writer, as planned, leaves
  receipts to each caller's memory; enqueueing in `local` would echo those
  facts back out. So: `WriteOrigin::{Local, Tracker, Import}`. `write_record`
  under `Local` enqueues one effect per changed value a bridge carries
  (`EffectKind::of(&WorkValue)`: `Disposition`→state, `TaskOrder`→order,
  `CommentContent`→comment, `TaskDeletedAt(Some)`→delete, bindings and
  execution-only values → none, the rest → field; base = the owner's current
  observation). `Tracker` journals for Git peers like `Local` and enqueues
  nothing; acceptance, attach, conflict adoption and removal use it. Writers
  stop calling receipt code; `create` stays in `planning_write::create`.
- **One conflict rule for both observation sources.** Inbound acceptance
  settles attempted receipts with its own SQL (`observe_attempts`,
  `planning_changes.rs:241`) while the drain uses `rules::decide`: two copies
  of the rule. `rules.rs` is pure over storage types, so it moves to
  `loopflow_storage::planning::rules`; acceptance calls `decide` for each
  attempted effect of an owner it just ingested and applies only `Settle`.
  An acquisition that saw the owner after `attempted_at` makes the drain's
  look unnecessary. Pending rebase (base ← observed, overlay the local value,
  `reconcile_edits`) stays in acceptance; it settles nothing. `observe_attempts`
  is deleted, not ported.
- **State keeps its history evidence.** Today's state rule reads
  `StateHistory` (`Unknown` holds, `Unchanged` rebases and resends,
  `Changed` adopts); `rules::field` compares only the base value and misses a
  provider A→B→A. `Observed` gains `state_history`; the state rule uses it
  when the bridge answers one (Linear), the value compare otherwise (Asana).
- **A pass with nothing to do writes nothing.** `enqueue_tracker_creations`
  opens an `Immediate` transaction at the start of every pass, on the one
  write lock every agent shares. Select first on a plain read; open the write
  only when it returns owners, with `NOT EXISTS` repeated in the insert.
  Check its plan with `EXPLAIN QUERY PLAN` against a 10k-Task store.
- **`drain` scope is one argument.** Done: `drain::Scope::{All,
  First(owner), Only(owner)}`.
- **An unknown kind fails one owner, not the pass.** `EffectKind::parse`
  errors the whole `unsettled_tracker_effects` read, so a row a later lf
  writes stops this one's drain for the repository. Read unknown kinds as a
  row the pass skips (owner held, `tracing::debug!` once), and drop
  `decide`'s `_ => Hold` arm for an exhaustive match inside the crate.

### Concept review (2026-11-16, after the cut): built

Read against 3d21fe1ccb..90f825a173; built in the commits after 81d2bd67b8.

- **Follow-up relations are `link` effects** on the source Task, subject
  `relation:<id>`, id = the relation id (`INSERT OR IGNORE`): queued where
  `recover_task_filings` recovers the filing (`retain_relation_in`) and copied
  by the draft migration when unconfirmed. The drain holds one until its target
  Task is bound (`target_pending`). Deleted: `sync_retained_relations`,
  `pending_retained_relations`, `confirm_retained_relation`, `RetainedRelation`,
  the Linear check in `drain.rs`, `locks/follow-through-relations`.
- **PR links are `link` effects**, subject `pr:<url>`, value
  `{number, state, comment_id}` (`enqueue_pr_link`). Publish drains
  `Scope::First(task)` and reads the effect into `pr.linear_link_error`
  (unsettled without an error: "waiting to be sent"). Linear: attachmentLinkURL
  (upsert by URL) + attachmentUpdate + one comment, id = an earlier lf's
  `linear_comment_id` else uuid-v5(task, url); an acknowledged earlier link on
  the subject switches create to update. Asana: one `sync_comment` per state.
  Deleted: the `Tracker::Asana` match, `link_pr_async`/`link_pr`/
  `link_pr_with_client`, `PrLinkageIds`/`PrLinkRequest`/`PrLinkageOutcome`.
  `linear_attachment_id`/`linear_comment_id` are no longer written.
- **`tracker::BridgeError { RateLimited { tracker, reset_at_ms }, Message }`**;
  `asana/` names no `Linear*` type.
- **`sync_after_save(store, repo, &EffectOwner)`** drains `Scope::First`.
- **`pm_issue_changes` stays.** `a_tracker_removal_withdraws_new_launches`
  shows removal stamps the Task; but dropping the launch read's
  `changes.removed` term failed
  `task_admission_reads_released_evidence_without_reconstructing_provider_content`
  (released stores hold removals without a stamped Task), and acceptance uses
  the table as a re-acceptance guard and revision watermark. It is inbound
  acquisition evidence, which Exclusions leave alone.

### Migration (one, additive): `tracker_effects`

`uv run python scripts/new_migration.py tracker_effects`. Creates the table,
index and triggers; copies every unsettled row from the seven receipts into
it, except creation intents never prepared (`export_json IS NULL`, which in a
Git-only repository is every Task: the connect-time enqueue above covers
those), translating `export_json.input` back to the creation baseline already
stored beside it (`export_json.baseline`); then marks the copied old rows
settled (`acknowledged=1` / settled flags) so an older lf on the same store
does not send them a second time. New code never reads or writes the old
tables. Upgrade fixture: seed a store at origin/main's schema under
`rust/loopflow-storage/tests/fixtures/migrations/` with one unsettled row of
each kind, migrate, assert seven effects and zero sendable old rows.

Window risk, accepted: an older lf process still running after install
writes its own new edits to the old tables and delivers them itself; if it
exits first they stay unsent. Its inbound acquisition, seeing no old pending
receipt, can adopt a coworker's newer value over a local edit that exists
only in `tracker_effects`; the new drain then sees the baseline moved and the
field rule adopts the provider, keeping the losing value. Same outcome as a
conflict today.

## Delete — do not maintain

Each line deletes its exclusive tests and fixtures.

Done in slice 1 (f2c37b11a0), recoverable from that commit: Notion,
`ChapterSweep` + `lf task sweep`, legacy Project adoption, the
`refresh_async` apply branch. "Chapter" names that mean the current Project
stay (`scratch/questions.md`); `~/.lf/chapter-locks/` stays until the contract
Task, since an older lf locks the same inode (`planning_sync.rs:594`).

- **The generic facade:** `ops/planning_client.rs` (→ `ops/tracker.rs`),
  `PlanningClient::linear()`, `planning_client::is_linear` and its callers,
  `asana::task_fields`.
- **Store-built GraphQL:** the `json!` inputs in
  `sqlite/planning_export.rs::prepare_planning_export`, `export_json.input`,
  `linear_records::render_project_content` in storage (moves to `linear/`).
- **Old drains and receipt writers:** `ops/planning_export.rs`,
  `ops/planning_delivery.rs`, `ops/planning_order.rs` delivery,
  `tracker_observe::{sync_repository_deliveries, deliver_task_state,
  sync_task_comments}`; storage receipt writers in `planning_changes.rs`,
  `task_state_delivery.rs`, `planning_order.rs`, the delivery half of
  `task_comments.rs`, and `task_creation_intents`/`project_transitions` export
  columns' writers. Behavior that survives: every rule above, the #1829 window,
  `lf task sync`, `--retry-updates`, `PlanningSyncStatus` fields.
- **`pm_issue_changes`:** the brief says it has no production writer; the
  sweep found one. `observe_pm_issue_change` (`sqlite/linear_observations.rs:10`)
  records a provider removal from `tracker_observe.rs`, and its readers
  (`linear/acceptance.rs` ×3, `sqlite/linear_acceptance.rs` ×2,
  `sqlite/children.rs`) use it as removal/invalidation evidence. Slice 3
  first writes `a_tracker_removal_withdraws_new_launches`; if
  `tasks.planning_deleted_at` and `task_deletions` already carry that fact the
  writer and readers go; otherwise removal becomes a column on `pm_items`
  in the migration and the table goes in the contract.
- **Receipt-skipping writers:** PR links (`link_pr_with_client`,
  `ops/linear.rs:749`, saves ids after the call;
  `ops/task.rs:2481` reposts an id-less Asana comment on every publish) become
  `link` effects. Out of scope, explicit admin commands with pre-validation:
  `connect_async::create_wave` (`linear.rs:650`) and reteam
  (`linear.rs:1378-1452`).

Table drops (`task_changes`, `project_changes`, `task_state_deliveries`,
`task_comment_deliveries`, export columns, `pm_issue_changes`) and table
renames (`task_linear_observations`, `linear_team_checks`) are a contract
migration ≥ 30 days after this merges: a follow-up Task filed with this PR.

## Forbidden outcomes

- New code reading or writing an old receipt table (dual read/write).
- A `Linear*` type, GraphQL key or `match` on provider outside `linear/`,
  `asana/` and `ops/tracker.rs`.
- A drain that loops over every unsettled owner without a request bound.
- Notion code left behind "for later"; a `PlanningClient` alias.

## Exclusions

- Wire DTO field `linear_id` (in ~10 `tests/fixtures/dto/*.json`, Swift
  mirrors) and `LinearIssueId`/`LinearProjectId` newtypes: renaming crosses
  Rust/Swift and every fixture; follow-up Task.
- Inbound acquisition protocol (deltas, watermarks, team checks) beyond
  writing the real `provider`.
- Asana feature gaps (reteam, Initiative connect) that `parity.md` marks n/m.
- Git planning exchange.

## Slices

Deepest deletions first, then the queue, then the bridges. Commit at each.

1. **Done (f2c37b11a0).** Notion, `lf task sweep` + `ChapterSweep`, legacy
   Project adoption (`legacy_current` column stays until the contract), the
   `refresh_async` apply branch and `LinearSyncOptions.plan`.
   `pm_issue_changes` moves to slice 3 with the inbound removal path.
2. **Done (85bafbe98a).** Records are
   `loopflow_storage::planning::{TrackerItem, TrackerObservation,
   TrackerComment, TrackerProject, StateHistory}`; `PlanningResponse` dies with
   the old drains.
3. **Queue, rules, drain, writers, readers: done** (a2c956a129 onward).
   Migration and queue as before (dry run on the live store: 77 effects, 0
   sendable old rows). `planning_write` has `WriteOrigin::{Local, Tracker,
   Import}`; `Local` writes queue one effect per changed value
   (`EffectKind::of`); a record created in the write queues none (its tracker
   learns it through `create`), except a new comment. Acceptance, comment
   ingestion and order observation settle attempted effects with
   `planning::rules::decide` (moved to storage, exhaustive) and overlay unsettled
   local values (`tracker_effects::reconcile_in`). Sync status, Task rows and
   threads read the queue. Deleted: storage `planning_changes.rs`,
   `planning_export.rs`, the `task_state_delivery` API.
4. **Bridges: done.** `Tracker {Linear, Asana}` in `ops/tracker.rs`;
   `linear/effects.rs`, `asana/effects.rs`. Deleted `PlanningClient`,
   `ops/planning_{client,export,delivery,order}.rs`, tracker_observe's delivery
   half, `asana::task_fields`. `drain_repository` asks nothing when the queue
   is empty, builds the Team-validated Linear client only when a `create` is
   queued, and leaves a bridge error (offline) on each effect.
5. **Truthful `provider`: done** for acceptance writes
   (`accept_linear_{project,issue}_in` take the bridge); joined readers match
   `IN ('linear','asana')`; Linear-only acceptance (teams, inventory, order)
   writes `linear`. `lf repo plan` lists the queue (`PlanStatus.effects`, schema
   regenerated). `sync.conflict` is the existing `adopted_linear` change in
   `lf task show`.
6. **Concept review fixes, Linear-fake tests, docs: done** (see the review
   above). Fake tests: lost field reply, conflict adopt, restored deletion
   (`ops/linear/planning_effect_tests.rs`). Docs: `planning.md` "One outbound
   queue", `reference.md`, `reference-code.md`, `ops/README.md`. Filed:
   contract migration (771562), wire names (9fbce4).
7. **Remaining.**
   - File with the PR: GOAL.md name/summary as Wave `field` effects
     (`EffectKind::of` queues nothing for Wave values; never sent by a
     released lf, so no regression).
   - `uncreated()`'s Project half scans `project_transitions` per unbound
     Project (small table); `task_creation_intents` and `project_transitions`
     stay live as creation record and plan transitions, only their export
     columns are dead (contract Task).

### Concept review (2026-11-16, after 7e6a2ce054): nothing left to build

Read against 79d9554d33..7e6a2ce054. Greps empty: `Tracker::Asana` outside
`ops/tracker.rs`, `sync_retained_relations`, `PlanningClient`,
`link_pr_with_client`, `Linear*` in `asana/`, `teamId|stateId|assigneeId` in
storage src. The only Linear names in `ops/tracker{,/drain}.rs` are the
`Tracker::Linear` arm, `From<LinearError>` and `linear::delta_fresh_secs`
(inbound acquisition, excluded). GOAL.md → Initiative moves out of scope (see
Target architecture). Open choices in `questions.md` keep today's behavior and
do not block publishing.

## Checks run

- `cargo nextest run -p loopflow-storage -p loopflow-execution --lib -E 'test(ops::tracker::) | test(asana_planning_tests) | test(tracker_effects) | test(task_history) | test(planning_effect_tests::) | test(planning_order_tests::) | test(task_comments_tests::) | test(linear::effects::)'` → 39 pass.
- `cargo test -p loopflow-storage --lib -- query_plans tracker_effects` → 19 pass.
- `cargo nextest run -p loopflow-execution --lib -E 'test(store::tests::) | test(linear::acceptance::) | test(planning_lookup_tests::)'` → pass.
- `cargo nextest run -p loopflow --lib -E 'test(local_planning::) | test(pr_tests::) | test(planning_lookup_tests::)' -j 4` → 62 pass.
- `cargo nextest run -p loopflow --test task_pr_authority_tests` → 12 pass.
- `cargo test -p loopflow --test integration documented_commands::` → 4 pass.
- `cargo clippy -p loopflow-storage -p loopflow-execution -p loopflow --all-targets -- -D warnings` → clean.
- `EXPLAIN QUERY PLAN`, store from branch `lf` seeded to 10,001 Tasks / 9,508 effects (84 unsettled): `uncreated()` Task half searches `tasks` by `external_issue_id`, effects by `tracker_effects_task`; drain select reads `tracker_effects_unsettled`. Old `IN_REPO` listed every repository Task (`SCAN t`, 5 ms); per-row `EXISTS` form 1 ms, now in code.
- Not run: `binary(two_people)`, `test(transport::)`, `binary(golden_prompt)`; gate/CI.

## Done when

- Creating, editing, steering, completing, reordering and deleting a Task in a
  Linear- or Asana-configured repository each leave one `tracker_effects` row
  that `lf repo plan --json` shows and the next pass settles.
- `grep -rn 'teamId\|stateId\|assigneeId' rust/loopflow-storage/src | grep -v migrations` is empty.
- `grep -rln -i notion rust/*/src --include=*.rs | grep -v migrations` is empty;
  `grep -rn -i chapter rust/*/src --include=*.rs | grep -v migrations` lists
  no live behavior.
- A pass with 100 unsettled effects sends ≤ 20 requests; a pass with none sends 0.
- `planning.md` describes one queue, two bridges and the lost-reply rule.

## Gate checks (headless)

- `uv run python scripts/check_migrations.py` → passes (additive).
- `cargo test -p loopflow-storage --lib tracker_effects` → upgrade fixture passes.
- `cargo test -p loopflow-execution --lib ops::tracker::` → per-kind and bound tests pass.
- `cargo test -p loopflow-execution --lib asana_planning_tests` → passes on the queue.
- `EXPLAIN QUERY PLAN` of the drain's select on a 10k-Task store → uses the partial index.
- CI runs the suites.

## Wild success / failure

Success: someone on Asana gets the same `lf repo plan` and conflict
story as Linear, and a third bridge (GitHub Issues, Jira) is one `effects.rs`
plus a stand-in. 167 forever-attempted edits stop existing because the
lost-reply rule is one function with one test.

Failure: the change grows like LOO-691 (66 files, never split) and stalls;
or the migration's copy double-sends through an older lf. Guards: slices land
as commits that build; old rows are marked settled in the same migration.


## User docs (written ahead of code)

`docs/work.md` ("Linear or Asana", Reference "Sync"), `docs/glossary.md`,
`docs/config.md`, `docs/dev/data.md`, `README.md`. The queue surfaces in the
existing `lf repo plan` (there is no `lf planning` command); its text output
is documented as one line per unsettled effect: owner, kind, `pending` or
"sent, waiting for <tracker> to confirm". `lf task sync <id>` drains that
owner now. Still to update with the code: `docs/dev/planning.md`,
`docs/dev/reference.md:89`, `rust/loopflow-execution/src/ops/README.md:10`.
